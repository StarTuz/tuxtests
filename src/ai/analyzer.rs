use crate::ai::config::AppConfig;
use crate::ai::{config, gemini, ollama};
use crate::models::TuxPayload;

/// Inherited verbatim from GEMINI.md Prompt Schema!
const SYSTEM_PROMPT: &str = "You are an expert Linux diagnostics agent. Analyze the provided JSON representing a Linux machine's hardware layout. Identify specific bottlenecks (e.g., drives at >90% capacity, high-speed SSDs bottlenecked by physical USB 2.0 connections) and provide 3 concrete, actionable upgrade or mitigation suggestions. Format output strictly in Markdown. When PCIe or ASPM issues are discussed, ground your advice in the payload's pcie_aspm_policy and per-device pcie_path facts. Use both `aspm_capability` and `aspm` when present. Treat local payload facts as highest priority. Distinguish clearly between observed facts, reasonable inferences, and open diagnostic questions. Do not recommend disabling ASPM if the relevant PCIe link already reports `ASPM Disabled`; instead say that ASPM is already disabled on that link. If a link reports `ASPM not supported`, do not present ASPM toggling as confirmed remediation for that link, but you may still discuss capability mismatches or firmware behavior as diagnostic possibilities. If `aspm_probe_error` is present, treat that as an inspection limitation and explicitly say the per-link ASPM state could not be confirmed from the automated probe.";

#[derive(Debug, Clone, PartialEq, Eq)]
enum AnalysisTarget {
    Gemini,
    Ollama { model: String, url: String },
}

/// Main AI routing module handling data serialization.
pub async fn run_analysis(payload: &TuxPayload) {
    match get_analysis(payload).await {
        Ok(markdown) => println!(
            "\n============= AI BOTTLENECK ANALYSIS =============\n\n{}\n\n==================================================",
            markdown
        ),
        Err(err) => eprintln!("{}", err),
    }
}

pub async fn get_analysis(payload: &TuxPayload) -> Result<String, String> {
    let config = config::AppConfig::load();
    let system_prompt = build_system_prompt(payload);
    let payload_str = serde_json::to_string(payload)
        .expect("Critically failed to mathematically stringify TuxPayload models");

    let analysis_target =
        match resolve_analysis_target(&config, config::AppConfig::get_gemini_key().is_some()) {
            Ok(target) => target,
            Err(err) => return Err(err),
        };

    if payload.drives.is_empty() {
        eprintln!(
            "⚠️ No drives were discovered in the current scan payload. AI analysis may be incomplete."
        );
    }

    let output = match &analysis_target {
        AnalysisTarget::Gemini => {
            let key = config::AppConfig::get_gemini_key().expect("Gemini key should exist");
            gemini::invoke_gemini(&key, &system_prompt, &payload_str).await
        }
        AnalysisTarget::Ollama { model, url } => {
            eprintln!(
                "ℹ️ Using Ollama provider with model '{}' at {}.",
                model, url
            );
            ollama::invoke_ollama(url, model, &system_prompt, &payload_str).await
        }
    };

    match output {
        Some(markdown) => Ok(markdown),
        None => Err(format!(
            "❌ Failed to route inference through the '{}' provider. Check provider-specific diagnostics above.",
            provider_name(&analysis_target)
        )),
    }
}

fn build_system_prompt(payload: &TuxPayload) -> String {
    let mut prompt = String::from(SYSTEM_PROMPT);

    let already_disabled = collect_links(payload, |path| {
        matches!(path.aspm.as_deref(), Some("ASPM Disabled"))
            || matches!(path.aspm_capability.as_deref(), Some("ASPM not supported"))
    });
    if !already_disabled.is_empty() {
        prompt.push_str("\n\nObserved PCIe links where ASPM is already disabled. Do not recommend turning ASPM off again on these links, but you may still discuss other PCIe or firmware causes if supported by the payload: ");
        prompt.push_str(&already_disabled.join(", "));
        prompt.push('.');
    }

    let unsupported_links = collect_links(payload, |path| {
        matches!(path.aspm_capability.as_deref(), Some("ASPM not supported"))
    });
    if !unsupported_links.is_empty() {
        prompt.push_str(
            "\nObserved PCIe links where the local payload reports `ASPM not supported`: ",
        );
        prompt.push_str(&unsupported_links.join(", "));
        prompt.push_str(". Do not present ASPM toggling as a confirmed fix there. You may still mention capability mismatches, firmware quirks, or further investigation as possibilities, but label them as inference rather than established cause.");
    }

    let probe_limited = collect_links(payload, |path| path.aspm_probe_error.is_some());
    if !probe_limited.is_empty() {
        prompt.push_str("\nInspection-only rule: `aspm_probe_error` means the automated probe lacked visibility. Probe errors are not root causes by themselves, but they do mean ASPM or link-management behavior remains unconfirmed and may require elevated inspection for: ");
        prompt.push_str(&probe_limited.join(", "));
        prompt.push('.');
    }

    let downgraded_links = collect_links(payload, link_is_downgraded);
    if !downgraded_links.is_empty() {
        prompt.push_str("\nReal PCIe bandwidth constraints are present on these downgraded links and may be treated as actual bottlenecks when relevant: ");
        prompt.push_str(&downgraded_links.join(", "));
        prompt.push('.');
    }

    prompt
}

fn collect_links<F>(payload: &TuxPayload, predicate: F) -> Vec<String>
where
    F: Fn(&crate::models::PcieDeviceInfo) -> bool,
{
    let mut labels = Vec::new();
    for drive in &payload.drives {
        for path in &drive.pcie_path {
            if predicate(path) {
                let label = format!("{} on {}", path.bdf, drive.name);
                if !labels.iter().any(|existing| existing == &label) {
                    labels.push(label);
                }
            }
        }
    }
    labels
}

fn link_is_downgraded(path: &crate::models::PcieDeviceInfo) -> bool {
    let speed_downgraded = match (
        parse_numeric_prefix(path.current_link_speed.as_deref()),
        parse_numeric_prefix(path.max_link_speed.as_deref()),
    ) {
        (Some(current), Some(max)) => current + f32::EPSILON < max,
        _ => false,
    };

    let width_downgraded = match (
        parse_u32(path.current_link_width.as_deref()),
        parse_u32(path.max_link_width.as_deref()),
    ) {
        (Some(current), Some(max)) => current < max,
        _ => false,
    };

    speed_downgraded || width_downgraded
}

fn parse_numeric_prefix(value: Option<&str>) -> Option<f32> {
    let token = value?.split_whitespace().next()?;
    token.parse().ok()
}

fn parse_u32(value: Option<&str>) -> Option<u32> {
    value?.trim().parse().ok()
}

fn resolve_analysis_target(
    config: &AppConfig,
    gemini_key_available: bool,
) -> Result<AnalysisTarget, String> {
    let provider = config::normalize_provider(&config.provider).map_err(|err| {
        format!(
            "❌ Invalid provider configuration '{}': {}. Use `tuxtests --set-llm-provider <gemini|ollama>` to repair it.",
            config.provider, err
        )
    })?;

    match provider.as_str() {
        "gemini" => {
            if gemini_key_available {
                Ok(AnalysisTarget::Gemini)
            } else {
                Err("❌ Gemini API key is missing from the system keyring. Run `tuxtests --set-gemini-key \"YOUR_KEY_HERE\"` first.".to_string())
            }
        }
        "ollama" => Ok(AnalysisTarget::Ollama {
            model: config.ollama_model.clone(),
            url: config.ollama_url.clone(),
        }),
        _ => unreachable!(),
    }
}

fn provider_name(target: &AnalysisTarget) -> &'static str {
    match target {
        AnalysisTarget::Gemini => "gemini",
        AnalysisTarget::Ollama { .. } => "ollama",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_system_prompt, link_is_downgraded, provider_name, resolve_analysis_target,
        AnalysisTarget,
    };
    use crate::ai::config::AppConfig;
    use crate::models::{DriveInfo, PcieDeviceInfo, SystemInfo, TuxPayload};
    use std::collections::BTreeMap;

    fn config(provider: &str) -> AppConfig {
        AppConfig {
            provider: provider.to_string(),
            ollama_model: "mistral".to_string(),
            ollama_url: "http://127.0.0.1:11434".to_string(),
        }
    }

    fn payload_with_pcie_path(pcie_path: Vec<PcieDeviceInfo>) -> TuxPayload {
        TuxPayload {
            summary_header: "summary".to_string(),
            system: SystemInfo {
                os_release: BTreeMap::new(),
                hostname: "host".to_string(),
                kernel_version: "6.x".to_string(),
                cpu: "cpu".to_string(),
                ram_gb: 32,
                motherboard: None,
                pcie_aspm_policy: Some("default".to_string()),
            },
            drives: vec![DriveInfo {
                name: "nvme0n1".to_string(),
                drive_type: "disk".to_string(),
                connection: "Internal (NVME)".to_string(),
                capacity_gb: 1000,
                usage_percent: 10,
                health_ok: true,
                physical_path: "/dev/mock".to_string(),
                fstype: None,
                uuid: None,
                label: None,
                active_mountpoints: Vec::new(),
                topology: Vec::new(),
                pcie_path,
                serial: None,
                smartctl_exit_code: None,
                parent: None,
                is_luks: None,
            }],
            benchmarks: BTreeMap::new(),
            kernel_anomalies: Vec::new(),
            fstab: Vec::new(),
        }
    }

    #[test]
    fn selects_gemini_when_key_exists() {
        let target = resolve_analysis_target(&config("gemini"), true).unwrap();
        assert_eq!(target, AnalysisTarget::Gemini);
        assert_eq!(provider_name(&target), "gemini");
    }

    #[test]
    fn rejects_gemini_when_key_is_missing() {
        let err = resolve_analysis_target(&config("gemini"), false).unwrap_err();
        assert!(err.contains("Gemini API key is missing"));
    }

    #[test]
    fn selects_ollama_with_model_and_url() {
        let target = resolve_analysis_target(&config("ollama"), false).unwrap();
        assert_eq!(
            target,
            AnalysisTarget::Ollama {
                model: "mistral".to_string(),
                url: "http://127.0.0.1:11434".to_string(),
            }
        );
        assert_eq!(provider_name(&target), "ollama");
    }

    #[test]
    fn rejects_invalid_provider_configuration() {
        let err = resolve_analysis_target(&config("bad-provider"), true).unwrap_err();
        assert!(err.contains("Invalid provider configuration"));
    }

    #[test]
    fn marks_aspm_disabled_links_as_no_toggle_targets_in_prompt() {
        let payload = payload_with_pcie_path(vec![PcieDeviceInfo {
            bdf: "0000:03:00.0".to_string(),
            driver: Some("nvme".to_string()),
            current_link_speed: Some("8.0 GT/s PCIe".to_string()),
            current_link_width: Some("4".to_string()),
            max_link_speed: Some("16.0 GT/s PCIe".to_string()),
            max_link_width: Some("4".to_string()),
            aspm_capability: Some("ASPM L1".to_string()),
            aspm: Some("ASPM Disabled".to_string()),
            aspm_source: Some("sudo_lspci".to_string()),
            aspm_probe_error: None,
        }]);

        let prompt = build_system_prompt(&payload);
        assert!(prompt.contains("Do not recommend turning ASPM off again"));
        assert!(prompt.contains("0000:03:00.0 on nvme0n1"));
        assert!(prompt.contains("Real PCIe bandwidth constraints"));
    }

    #[test]
    fn marks_probe_errors_as_unconfirmed_not_suppressed_in_prompt() {
        let payload = payload_with_pcie_path(vec![PcieDeviceInfo {
            bdf: "0000:00:14.0".to_string(),
            driver: Some("xhci_hcd".to_string()),
            current_link_speed: None,
            current_link_width: None,
            max_link_speed: None,
            max_link_width: None,
            aspm_capability: None,
            aspm: None,
            aspm_source: None,
            aspm_probe_error: Some("probe failed".to_string()),
        }]);

        let prompt = build_system_prompt(&payload);
        assert!(prompt.contains("remains unconfirmed and may require elevated inspection"));
        assert!(prompt.contains("0000:00:14.0 on nvme0n1"));
    }

    #[test]
    fn marks_aspm_not_supported_links_as_inference_only() {
        let payload = payload_with_pcie_path(vec![PcieDeviceInfo {
            bdf: "0000:00:1b.4".to_string(),
            driver: Some("pcieport".to_string()),
            current_link_speed: Some("8.0 GT/s PCIe".to_string()),
            current_link_width: Some("4".to_string()),
            max_link_speed: Some("8.0 GT/s PCIe".to_string()),
            max_link_width: Some("4".to_string()),
            aspm_capability: Some("ASPM not supported".to_string()),
            aspm: Some("ASPM Disabled".to_string()),
            aspm_source: Some("sudo_lspci".to_string()),
            aspm_probe_error: None,
        }]);

        let prompt = build_system_prompt(&payload);
        assert!(prompt.contains("ASPM not supported"));
        assert!(prompt.contains("inference rather than established cause"));
    }

    #[test]
    fn detects_downgraded_links() {
        assert!(link_is_downgraded(&PcieDeviceInfo {
            bdf: "0000:03:00.0".to_string(),
            driver: None,
            current_link_speed: Some("8.0 GT/s PCIe".to_string()),
            current_link_width: Some("4".to_string()),
            max_link_speed: Some("16.0 GT/s PCIe".to_string()),
            max_link_width: Some("4".to_string()),
            aspm_capability: None,
            aspm: None,
            aspm_source: None,
            aspm_probe_error: None,
        }));
    }
}
