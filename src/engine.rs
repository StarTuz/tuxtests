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
        findings: Vec::new(),
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
            let smart_outcome = bench::smart::check_health(&drive.name);
            drive.health_ok = smart_outcome.health_ok;
            drive.smartctl_exit_code = smart_outcome.exit_code;
            drive.smart = smart_outcome.report;
            kernel_anomalies.extend(smart_outcome.anomalies);

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
    let findings = build_findings(&storage_drives, full_bench, &kernel_anomalies);

    models::TuxPayload {
        summary_header,
        system: sys_specs,
        drives: storage_drives,
        benchmarks,
        findings,
        kernel_anomalies,
        fstab: hardware::storage::extract_fstab(),
    }
}

fn build_findings(
    drives: &[models::DriveInfo],
    smart_requested: bool,
    kernel_anomalies: &[String],
) -> Vec<models::DiagnosticFinding> {
    let mut findings = Vec::new();

    if smart_requested {
        for drive in drives {
            if let Some(report) = &drive.smart {
                if !report.available {
                    findings.push(models::DiagnosticFinding {
                        category: models::FindingCategory::Privilege,
                        severity: models::FindingSeverity::Notice,
                        title: format!("SMART data unavailable for {}", drive.name),
                        evidence: report.limitations.join("; "),
                        explanation: "TuxTests could not collect structured SMART data for this drive, so health interpretation is limited to the available kernel and topology data.".to_string(),
                        recommended_action: Some(
                            "Run the deeper scan from a session where polkit or sudo can grant smartctl access, then compare the structured SMART report.".to_string(),
                        ),
                        confidence: "high".to_string(),
                        drive: Some(drive.name.clone()),
                    });
                    continue;
                }

                if report.passed == Some(false) {
                    findings.push(models::DiagnosticFinding {
                        category: models::FindingCategory::Smart,
                        severity: models::FindingSeverity::Critical,
                        title: format!("SMART overall-health check failed for {}", drive.name),
                        evidence: report
                            .exit_status_description
                            .join("; ")
                            .if_empty("smart_status.passed=false"),
                        explanation: "The drive reported a failing SMART health state. This is a direct device health signal, not an AI inference.".to_string(),
                        recommended_action: Some(
                            "Back up important data immediately, then inspect the full smartctl report and plan replacement if the failure is confirmed.".to_string(),
                        ),
                        confidence: "high".to_string(),
                        drive: Some(drive.name.clone()),
                    });
                }

                for (label, value, severity) in smart_counter_findings(report) {
                    findings.push(models::DiagnosticFinding {
                        category: models::FindingCategory::Smart,
                        severity,
                        title: format!("{label} reported on {}", drive.name),
                        evidence: format!("{label}={value}"),
                        explanation: "SMART counters can reveal degradation before the top-level health flag changes. Non-zero values should be interpreted with drive type, age, and trend history in mind.".to_string(),
                        recommended_action: Some(
                            "Review the full SMART details, rerun after workload, and treat rising counts as a stronger replacement signal than a single static reading.".to_string(),
                        ),
                        confidence: "medium".to_string(),
                        drive: Some(drive.name.clone()),
                    });
                }
            }
        }
    }

    for anomaly in kernel_anomalies {
        if anomaly.contains("DPC: error containment")
            || anomaly.contains("PoisonedTLP")
            || anomaly.contains("DL_ActiveErr")
        {
            findings.push(models::DiagnosticFinding {
                category: models::FindingCategory::Pcie,
                severity: models::FindingSeverity::Warning,
                title: "PCIe error-containment messages detected".to_string(),
                evidence: anomaly.clone(),
                explanation: "Kernel PCIe DPC/AER messages can indicate link instability, firmware quirks, or physical signal-integrity issues. TuxTests should present this as a diagnostic lead rather than a confirmed root cause.".to_string(),
                recommended_action: Some(
                    "Inspect the affected PCIe path, compare privileged lspci output, and check BIOS/firmware before making power-management changes.".to_string(),
                ),
                confidence: "medium".to_string(),
                drive: None,
            });
        }
    }

    findings
}

trait EmptyStringFallback {
    fn if_empty(self, fallback: &str) -> String;
}

impl EmptyStringFallback for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

fn smart_counter_findings(
    report: &models::SmartReport,
) -> Vec<(&'static str, i64, models::FindingSeverity)> {
    let counters = [
        (
            "reallocated sectors",
            report.reallocated_sectors,
            models::FindingSeverity::Warning,
        ),
        (
            "current pending sectors",
            report.current_pending_sectors,
            models::FindingSeverity::Critical,
        ),
        (
            "offline uncorrectable sectors",
            report.offline_uncorrectable,
            models::FindingSeverity::Critical,
        ),
        (
            "NVMe media errors",
            report.media_errors,
            models::FindingSeverity::Warning,
        ),
        (
            "NVMe error-log entries",
            report.num_err_log_entries,
            models::FindingSeverity::Notice,
        ),
    ];

    counters
        .into_iter()
        .filter_map(|(label, value, severity)| {
            value
                .filter(|count| *count > 0)
                .map(|count| (label, count, severity))
        })
        .collect()
}

pub fn payload_json(payload: &models::TuxPayload) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(payload)
}

pub async fn analyze_payload(payload: &models::TuxPayload) -> Result<String, String> {
    ai::analyzer::get_analysis(payload).await
}

pub async fn analyze_payload_quiet(payload: &models::TuxPayload) -> Result<String, String> {
    ai::analyzer::get_analysis_quiet(payload).await
}
