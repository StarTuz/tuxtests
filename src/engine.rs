use crate::{ai, bench, hardware, models};

#[derive(Debug, Default, Clone)]
pub struct ConfigUpdate {
    pub provider: Option<String>,
    pub ollama_model: Option<String>,
    pub ollama_url: Option<String>,
}

pub fn load_config() -> ai::config::AppConfig {
    ai::config::AppConfig::load()
}

pub fn config_json() -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&load_config())
}

pub fn apply_config_update(update: ConfigUpdate) -> Result<ai::config::AppConfig, String> {
    let mut config = load_config();

    if let Some(provider) = update.provider {
        config.provider = ai::config::normalize_provider(&provider)?;
    }

    if let Some(model) = update.ollama_model {
        config.ollama_model = ai::config::normalize_ollama_model(&model)?;
    }

    if let Some(url) = update.ollama_url {
        config.ollama_url = ai::config::normalize_ollama_url(&url)?;
    }

    if config.save() {
        Ok(config)
    } else {
        Err("failed to persist TuxTests AI configuration".to_string())
    }
}

pub fn build_mock_payload(mock_file: &str) -> Result<models::TuxPayload, String> {
    let content = std::fs::read_to_string(mock_file)
        .map_err(|err| format!("failed to read mock file '{}': {}", mock_file, err))?;

    let mocked_drive: models::DriveInfo = serde_json::from_str(&content).map_err(|err| {
        format!(
            "mock fixture '{}' is not valid DriveInfo JSON: {}",
            mock_file, err
        )
    })?;

    Ok(models::TuxPayload {
        summary_header: "System has 1 drives, 0 are USB. Maximum topology depth detected: 3."
            .to_string(),
        system: models::SystemInfo {
            os_release: {
                let mut map = std::collections::BTreeMap::new();
                map.insert("PRETTY_NAME".to_string(), "Mock GNU/Linux".to_string());
                map
            },
            hostname: "mock-host".to_string(),
            kernel_version: "6.x-mock".to_string(),
            cpu: "Mock Sandbox CPU (Threadripper Stub)".to_string(),
            ram_gb: 128,
            motherboard: Some("MockBoard 9000".to_string()),
            pcie_aspm_policy: Some("default".to_string()),
        },
        drives: vec![mocked_drive],
        benchmarks: std::collections::BTreeMap::new(),
        kernel_anomalies: vec![
            "mock anomaly: High predictive failure counts on dummy payload".to_string(),
        ],
        fstab: Vec::new(),
    })
}

pub fn collect_payload(full_bench: bool) -> models::TuxPayload {
    let sys_specs = hardware::system::get_system_specs();
    let mut storage_drives = Vec::new();
    let mut benchmarks = std::collections::BTreeMap::new();
    let mut kernel_anomalies = Vec::new();

    let global_log_output = ai::rag::fetch_kernel_logs();

    for (mut drive, mount_opt) in hardware::storage::scan_drives() {
        let mut drive_anomalies = ai::rag::retrieve_kernel_anomalies(&drive, &global_log_output);
        kernel_anomalies.append(&mut drive_anomalies);

        if full_bench {
            eprintln!(
                "🔒 Triggering Privileged Polkit S.M.A.R.T diagnostic on {}...",
                drive.name
            );
            let (ok, exit, anomaly) = bench::smart::check_health(&drive.name);
            drive.health_ok = ok;
            drive.smartctl_exit_code = exit;
            if let Some(err) = anomaly {
                kernel_anomalies.push(err);
            }

            if let Some(mount) = mount_opt {
                if let Some(mb_s) = bench::throughput::run_buffered_bench(&mount) {
                    benchmarks.insert(
                        drive.name.clone(),
                        models::BenchmarkResult { write_mb_s: mb_s },
                    );
                }
            }
        }

        storage_drives.push(drive);
    }

    for drive in &mut storage_drives {
        hardware::pci::enrich_anomaly_link_aspm(&mut drive.pcie_path, &kernel_anomalies);
    }

    let total_drives = storage_drives.len();
    let usb_count = storage_drives
        .iter()
        .filter(|d| d.connection.to_lowercase().contains("usb"))
        .count();
    let max_depth = storage_drives
        .iter()
        .flat_map(|d| d.topology.iter().map(|t| t.level))
        .max()
        .unwrap_or(0);

    let summary_header = format!(
        "System has {} drives, {} are USB. Maximum topology depth detected: {}.",
        total_drives, usb_count, max_depth
    );

    models::TuxPayload {
        summary_header,
        system: sys_specs,
        drives: storage_drives,
        benchmarks,
        kernel_anomalies,
        fstab: hardware::storage::extract_fstab(),
    }
}

pub fn payload_json(payload: &models::TuxPayload) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(payload)
}

pub async fn analyze_payload(payload: &models::TuxPayload) -> Result<String, String> {
    ai::analyzer::get_analysis(payload).await
}
