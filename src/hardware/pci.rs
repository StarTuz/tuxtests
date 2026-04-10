use crate::models::{PcieDeviceInfo, TopologyNode};
use std::process::Command;

pub fn collect_pcie_path(topology: &[TopologyNode]) -> Vec<PcieDeviceInfo> {
    topology
        .iter()
        .filter(|node| node.subsystem == "pci" && is_pci_bdf(&node.sysname))
        .map(|node| collect_pcie_device(&node.sysname))
        .collect()
}

pub fn read_aspm_policy() -> Option<String> {
    std::fs::read_to_string("/sys/module/pcie_aspm/parameters/policy")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn enrich_anomaly_link_aspm(devices: &mut [PcieDeviceInfo], kernel_anomalies: &[String]) {
    let anomaly_bdfs = extract_bdfs_from_anomalies(kernel_anomalies);
    let path_has_anomaly = devices
        .iter()
        .any(|device| anomaly_bdfs.iter().any(|bdf| bdf == &device.bdf));

    for device in devices.iter_mut() {
        if path_has_anomaly && (device.aspm.is_none() || device.aspm_capability.is_none()) {
            match read_lspci_link_details(&device.bdf, true) {
                ProbeOutcome::Success(details) => {
                    if device.aspm.is_none() {
                        device.aspm = details.aspm;
                    }
                    if device.aspm_capability.is_none() {
                        device.aspm_capability = details.aspm_capability;
                    }
                    device.aspm_source = Some(details.source);
                    device.aspm_probe_error = None;
                }
                ProbeOutcome::Failed(err) => {
                    if device.aspm.is_none() || device.aspm_capability.is_none() {
                        device.aspm_probe_error = Some(err);
                    }
                }
            }
        }
    }
}

fn collect_pcie_device(bdf: &str) -> PcieDeviceInfo {
    let sysfs_dir = format!("/sys/bus/pci/devices/{}", bdf);
    let probe = read_lspci_link_details(bdf, false);

    let (aspm_capability, aspm, aspm_source, aspm_probe_error) = match probe {
        ProbeOutcome::Success(details) => (
            details.aspm_capability,
            details.aspm,
            Some(details.source),
            None,
        ),
        ProbeOutcome::Failed(err) => (None, None, None, Some(err)),
    };

    PcieDeviceInfo {
        bdf: bdf.to_string(),
        driver: read_driver_name(&sysfs_dir),
        current_link_speed: read_trimmed(format!("{}/current_link_speed", sysfs_dir)),
        current_link_width: read_trimmed(format!("{}/current_link_width", sysfs_dir)),
        max_link_speed: read_trimmed(format!("{}/max_link_speed", sysfs_dir)),
        max_link_width: read_trimmed(format!("{}/max_link_width", sysfs_dir)),
        aspm_capability,
        aspm,
        aspm_source,
        aspm_probe_error,
    }
}

fn read_driver_name(sysfs_dir: &str) -> Option<String> {
    let path = format!("{}/driver", sysfs_dir);
    std::fs::read_link(path)
        .ok()
        .and_then(|link| {
            link.file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .filter(|value| !value.is_empty())
}

fn read_trimmed(path: String) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LspciLinkDetails {
    aspm_capability: Option<String>,
    aspm: Option<String>,
    source: String,
}

enum ProbeOutcome {
    Success(LspciLinkDetails),
    Failed(String),
}

fn read_lspci_link_details(bdf: &str, privileged: bool) -> ProbeOutcome {
    if privileged {
        match run_lspci_command("sudo", &["-n", "lspci", "-vv", "-s", bdf]) {
            Ok(stdout) => parse_link_details(&stdout, "sudo_lspci").map_or_else(
                || {
                    ProbeOutcome::Failed(format!(
                        "sudo lspci returned output without PCIe link details for {}",
                        bdf
                    ))
                },
                ProbeOutcome::Success,
            ),
            Err(sudo_error) => ProbeOutcome::Failed(format!(
                "elevated PCIe inspection was unavailable for {} ({}). Re-run `sudo lspci -vv -s {}` or run TuxTests under sudo for fuller PCIe/ASPM results.",
                bdf, sudo_error, bdf
            )),
        }
    } else {
        match run_lspci_command("lspci", &["-vv", "-s", bdf]) {
            Ok(stdout) => parse_link_details(&stdout, "lspci").map_or_else(
                || {
                    ProbeOutcome::Failed(format!(
                        "unprivileged lspci output did not expose PCIe link details for {}",
                        bdf
                    ))
                },
                ProbeOutcome::Success,
            ),
            Err(err) => ProbeOutcome::Failed(err),
        }
    }
}

fn run_lspci_command(program: &str, args: &[&str]) -> Result<String, String> {
    match Command::new(program).args(args).output() {
        Ok(output) if output.status.success() => {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        }
        Ok(output) => Err(command_error_message(program, output)),
        Err(err) => Err(format!("{} could not be executed ({})", program, err)),
    }
}

fn command_error_message(program: &str, output: std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    if stderr.is_empty() {
        format!("{} exited with status {}", program, output.status)
    } else {
        format!(
            "{} exited with status {}: {}",
            program, output.status, stderr
        )
    }
}

fn parse_link_details(stdout: &str, source: &str) -> Option<LspciLinkDetails> {
    let aspm = parse_aspm_from_lspci(stdout);
    let aspm_capability = parse_aspm_capability_from_lspci(stdout);
    if aspm.is_none() && aspm_capability.is_none() {
        None
    } else {
        Some(LspciLinkDetails {
            aspm_capability,
            aspm,
            source: source.to_string(),
        })
    }
}

fn parse_aspm_from_lspci(stdout: &str) -> Option<String> {
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("LnkCtl:") {
            let aspm_segment = rest
                .split(';')
                .find(|segment| segment.trim_start().starts_with("ASPM"))?;
            return Some(aspm_segment.trim().to_string());
        }
    }
    None
}

fn parse_aspm_capability_from_lspci(stdout: &str) -> Option<String> {
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("LnkCap:") {
            let aspm_start = rest.find("ASPM ")?;
            let aspm_segment = &rest[aspm_start..];
            let end = aspm_segment.find(',').unwrap_or(aspm_segment.len());
            return Some(aspm_segment[..end].trim().to_string());
        }
    }
    None
}

fn is_pci_bdf(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 12
        && bytes[4] == b':'
        && bytes[7] == b':'
        && bytes[10] == b'.'
        && bytes
            .iter()
            .enumerate()
            .all(|(idx, byte)| matches!(idx, 4 | 7 | 10) || byte.is_ascii_hexdigit())
}

fn extract_bdfs_from_anomalies(kernel_anomalies: &[String]) -> Vec<String> {
    let mut bdfs = Vec::new();
    for anomaly in kernel_anomalies {
        for token in anomaly.split_whitespace() {
            let cleaned =
                token.trim_matches(|c: char| matches!(c, ':' | ',' | '(' | ')' | '[' | ']'));
            if is_pci_bdf(cleaned) && !bdfs.iter().any(|existing| existing == cleaned) {
                bdfs.push(cleaned.to_string());
            }
        }
    }
    bdfs
}

#[cfg(test)]
mod tests {
    use super::{
        extract_bdfs_from_anomalies, is_pci_bdf, parse_aspm_capability_from_lspci,
        parse_aspm_from_lspci, parse_link_details,
    };

    #[test]
    fn parses_aspm_segment_from_lspci_output() {
        let stdout = "LnkCap: Port #0, Speed 16GT/s, Width x4, ASPM L1, Exit Latency L1 <64us\nLnkCtl: ASPM Disabled; RCB 64 bytes, LnkDisable- CommClk+\n";
        assert_eq!(
            parse_aspm_from_lspci(stdout),
            Some("ASPM Disabled".to_string())
        );
    }

    #[test]
    fn parses_aspm_capability_from_lspci_output() {
        let stdout = "LnkCap: Port #21, Speed 8GT/s, Width x4, ASPM not supported\nLnkCtl: ASPM Disabled; RCB 64 bytes, LnkDisable- CommClk+\n";
        assert_eq!(
            parse_aspm_capability_from_lspci(stdout),
            Some("ASPM not supported".to_string())
        );
    }

    #[test]
    fn parses_combined_link_details_from_lspci_output() {
        let stdout = "LnkCap: Port #0, Speed 5GT/s, Width x1, ASPM L0s L1, Exit Latency L1 unlimited\nLnkCtl: ASPM Disabled; RCB 64 bytes, LnkDisable- CommClk+\n";
        let details = parse_link_details(stdout, "lspci").expect("link details");
        assert_eq!(details.aspm_capability, Some("ASPM L0s L1".to_string()));
        assert_eq!(details.aspm, Some("ASPM Disabled".to_string()));
        assert_eq!(details.source, "lspci");
    }

    #[test]
    fn validates_pci_bdf_strings() {
        assert!(is_pci_bdf("0000:03:00.0"));
        assert!(!is_pci_bdf("03:00.0"));
        assert!(!is_pci_bdf("not-a-bdf"));
    }

    #[test]
    fn extracts_unique_bdfs_from_anomalies() {
        let anomalies = vec![
            "[    0.518532] pcieport 0000:00:1b.0: DPC error".to_string(),
            "[    0.518906] pcieport 0000:00:1b.4: DPC error".to_string(),
            "[    0.518907] pcieport 0000:00:1b.0: repeated".to_string(),
        ];

        assert_eq!(
            extract_bdfs_from_anomalies(&anomalies),
            vec!["0000:00:1b.0".to_string(), "0000:00:1b.4".to_string()]
        );
    }
}
