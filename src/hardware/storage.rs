use crate::hardware::{connection, pci};
use crate::models::DriveInfo;
use std::process::Command;

// Ephemeral struct exclusively intercepting the raw JSON tree from the local Linux CLI.
#[derive(serde::Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<LsblkDevice>,
}

#[derive(serde::Deserialize)]
pub struct LsblkDevice {
    pub name: String,
    #[serde(rename = "type")]
    pub device_type: String,
    pub size: serde_json::Value,
    pub pkname: Option<String>,
    pub serial: Option<String>,

    // Natively parses array of possible physical mounts cleanly.
    pub mountpoints: Option<Vec<Option<String>>>,

    // Natively fetch physical transport type from `lsblk`
    pub tran: Option<String>,

    pub fstype: Option<String>,
    pub uuid: Option<String>,
    pub label: Option<String>,

    #[serde(rename = "fsuse%")]
    pub fsuse_percent: Option<String>,
}

/// Executes an unprivileged subprocess polling the kernel block topology.
/// Wraps it into `models::DriveInfo`, fetching legacy USB connection speeds natively during creation.
/// Returns a tuple of strongly-typed DriveInfo AND its active mount route.
pub fn scan_drives() -> Vec<(DriveInfo, Option<String>)> {
    let output_result = Command::new("lsblk")
        .args([
            "-J",
            "-b",
            "-o",
            "NAME,TYPE,SIZE,PKNAME,SERIAL,MOUNTPOINTS,TRAN,FSTYPE,UUID,LABEL,FSUSE%",
        ])
        .output();

    let output = match output_result {
        Ok(out) if out.status.success() => out,
        _ => {
            eprintln!("⚠️ Warning: lsblk failed to poll hardware (missing /sys/dev/block?). Returning empty drive list.");
            return Vec::new();
        }
    };

    let parsed: LsblkOutput = match serde_json::from_slice(&output.stdout) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("⚠️ Warning: Failed to parse lsblk output: {}", e);
            let raw_str = String::from_utf8_lossy(&output.stdout);
            eprintln!(
                "Debug Dump (first 256 chars):\n{}",
                raw_str.chars().take(256).collect::<String>()
            );
            eprintln!("To fix this JSON mapping issue natively, we will analyze this dump!");
            return Vec::new();
        }
    };

    let mut drives = Vec::new();

    for dev in parsed.blockdevices {
        // Natively trap legacy stringified ints vs modern raw integers elegantly.
        let size_num = parse_size_bytes(&dev.size);

        let capacity_gb = size_num / 1_073_741_824;

        let raw_transport = dev.tran.unwrap_or_default().to_uppercase();
        let safe_fallback = if raw_transport == "USB" {
            "USB (External / Unknown Speed)".to_string()
        } else if !raw_transport.is_empty() {
            format!("Internal ({})", raw_transport)
        } else {
            "Internal/PCIe/SATA (Unknown)".to_string()
        };

        // Let udev explicitly quantify the math natively, otherwise fall back to pure `lsblk` connection type!
        let (udev_conn, syspath, topology) = connection::get_device_topology(&dev.name);
        let connection = udev_conn.unwrap_or(safe_fallback);
        let physical_path = if !syspath.is_empty() {
            syspath
        } else {
            "Unmapped in Phase 4".to_string()
        };

        // Map the structured mount points safely.
        let mountpoints_vec = normalize_mountpoints(dev.mountpoints);

        let mount_target = mountpoints_vec.first().cloned();

        let usage_percent = parse_usage_percent(dev.fsuse_percent.as_deref());
        let is_luks = infer_is_luks(&dev.device_type, dev.fstype.as_deref());
        let pcie_path = pci::collect_pcie_path(&topology);

        let mapped_drive = DriveInfo {
            name: dev.name,
            drive_type: dev.device_type,
            connection,
            capacity_gb,

            usage_percent,
            health_ok: true,
            physical_path,

            fstype: dev.fstype,
            uuid: dev.uuid,
            label: dev.label,
            active_mountpoints: mountpoints_vec,

            topology,
            pcie_path,
            serial: dev.serial.filter(|value| !value.trim().is_empty()),
            smartctl_exit_code: None,
            smart: None,
            parent: dev.pkname,
            is_luks,
        };

        drives.push((mapped_drive, mount_target));
    }

    drives
}

fn parse_size_bytes(size: &serde_json::Value) -> u64 {
    if size.is_number() {
        size.as_u64().unwrap_or(0)
    } else if size.is_string() {
        size.as_str()
            .unwrap_or_default()
            .trim()
            .parse()
            .unwrap_or(0)
    } else {
        0
    }
}

fn normalize_mountpoints(mountpoints: Option<Vec<Option<String>>>) -> Vec<String> {
    mountpoints
        .unwrap_or_default()
        .into_iter()
        .flatten()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn parse_usage_percent(value: Option<&str>) -> u8 {
    value
        .unwrap_or_default()
        .trim()
        .trim_end_matches('%')
        .parse::<u8>()
        .unwrap_or(0)
}

fn infer_is_luks(device_type: &str, fstype: Option<&str>) -> Option<bool> {
    let type_hint = device_type.eq_ignore_ascii_case("crypt");
    let fs_hint = fstype
        .map(|value| value.eq_ignore_ascii_case("crypto_luks"))
        .unwrap_or(false);

    if type_hint || fs_hint {
        Some(true)
    } else {
        None
    }
}

/// Parses the local /etc/fstab safely into strong typing
pub fn extract_fstab() -> Vec<crate::models::FstabEntry> {
    let mut entries = Vec::new();
    if let Ok(content) = std::fs::read_to_string("/etc/fstab") {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                entries.push(crate::models::FstabEntry {
                    file_system: parts[0].to_string(),
                    mount_point: parts[1].to_string(),
                    type_: parts[2].to_string(),
                    options: parts[3].to_string(),
                    dump: parts.get(4).unwrap_or(&"0").to_string(),
                    pass: parts.get(5).unwrap_or(&"0").to_string(),
                });
            }
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::{infer_is_luks, normalize_mountpoints, parse_size_bytes, parse_usage_percent};

    #[test]
    fn parses_size_from_number_or_string() {
        assert_eq!(parse_size_bytes(&serde_json::json!(1024)), 1024);
        assert_eq!(parse_size_bytes(&serde_json::json!("2048")), 2048);
        assert_eq!(parse_size_bytes(&serde_json::json!(null)), 0);
    }

    #[test]
    fn normalizes_mountpoints() {
        let mountpoints = normalize_mountpoints(Some(vec![
            Some("/".to_string()),
            None,
            Some(" /home ".to_string()),
            Some("".to_string()),
        ]));

        assert_eq!(mountpoints, vec!["/".to_string(), "/home".to_string()]);
    }

    #[test]
    fn parses_usage_percent_safely() {
        assert_eq!(parse_usage_percent(Some("84%")), 84);
        assert_eq!(parse_usage_percent(Some(" 9 ")), 9);
        assert_eq!(parse_usage_percent(None), 0);
    }

    #[test]
    fn infers_luks_from_type_or_fstype() {
        assert_eq!(infer_is_luks("crypt", None), Some(true));
        assert_eq!(infer_is_luks("part", Some("crypto_LUKS")), Some(true));
        assert_eq!(infer_is_luks("disk", Some("ext4")), None);
    }
}
