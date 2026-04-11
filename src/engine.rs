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
            let smart_outcome = if let Some(reason) = smart_skip_reason(&drive) {
                eprintln!(
                    "ℹ Skipping S.M.A.R.T diagnostic on {}: {}",
                    drive.name, reason
                );
                bench::smart::skipped(reason)
            } else {
                eprintln!(
                    "🔒 Triggering privileged S.M.A.R.T diagnostic on {}...",
                    drive.name
                );
                bench::smart::check_health(&drive.name)
            };
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
                    let not_applicable = report
                        .limitations
                        .iter()
                        .any(|limitation| limitation.contains("SMART not applicable"));
                    findings.push(models::DiagnosticFinding {
                        category: if not_applicable {
                            models::FindingCategory::Smart
                        } else {
                            models::FindingCategory::Privilege
                        },
                        severity: if not_applicable {
                            models::FindingSeverity::Info
                        } else {
                            models::FindingSeverity::Notice
                        },
                        title: if not_applicable {
                            format!("SMART skipped for {}", drive.name)
                        } else {
                            format!("SMART data unavailable for {}", drive.name)
                        },
                        evidence: report.limitations.join("; "),
                        explanation: if not_applicable {
                            "This block device is not a physical SMART-capable target, so skipping it avoids noisy privileged probes.".to_string()
                        } else {
                            "TuxTests could not collect structured SMART data for this drive, so health interpretation is limited to the available kernel and topology data.".to_string()
                        },
                        recommended_action: if not_applicable {
                            None
                        } else {
                            Some(
                                "Run the deeper scan from a session where polkit or sudo can grant smartctl access, then compare the structured SMART report.".to_string(),
                            )
                        },
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

                findings.extend(smart_advisory_findings(drive, report));
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

fn smart_advisory_findings(
    drive: &models::DriveInfo,
    report: &models::SmartReport,
) -> Vec<models::DiagnosticFinding> {
    let mut findings = Vec::new();

    if let Some(temperature) = report.temperature_celsius {
        let severity = if temperature >= 70 {
            Some(models::FindingSeverity::Critical)
        } else if temperature >= 60 {
            Some(models::FindingSeverity::Warning)
        } else {
            None
        };

        if let Some(severity) = severity {
            findings.push(models::DiagnosticFinding {
                category: models::FindingCategory::Smart,
                severity,
                title: format!("High SMART temperature on {}", drive.name),
                evidence: format!("temperature_celsius={temperature}"),
                explanation: "Sustained high drive temperature can shorten device lifespan and may throttle performance. This finding is threshold-based and should be interpreted with workload and sensor accuracy in mind.".to_string(),
                recommended_action: Some(
                    "Improve airflow or drive placement, rerun the scan after the system idles, and compare with vendor temperature guidance.".to_string(),
                ),
                confidence: "medium".to_string(),
                drive: Some(drive.name.clone()),
            });
        }
    }

    if let Some(percentage_used) = report.percentage_used {
        let severity = if percentage_used >= 100 {
            Some(models::FindingSeverity::Warning)
        } else if percentage_used >= 80 {
            Some(models::FindingSeverity::Notice)
        } else {
            None
        };

        if let Some(severity) = severity {
            findings.push(models::DiagnosticFinding {
                category: models::FindingCategory::Smart,
                severity,
                title: format!("NVMe endurance usage is elevated on {}", drive.name),
                evidence: format!("percentage_used={percentage_used}"),
                explanation: "NVMe percentage-used estimates consumed endurance. It is not the same as immediate failure, but high values are useful for replacement planning.".to_string(),
                recommended_action: Some(
                    "Check whether the value is stable over time, verify backups, and plan replacement if endurance usage keeps rising toward or beyond 100%.".to_string(),
                ),
                confidence: "medium".to_string(),
                drive: Some(drive.name.clone()),
            });
        }
    }

    if report.unsafe_shutdowns.is_some_and(|count| count >= 10) {
        findings.push(models::DiagnosticFinding {
            category: models::FindingCategory::Smart,
            severity: models::FindingSeverity::Notice,
            title: format!("NVMe unsafe shutdown count is notable on {}", drive.name),
            evidence: format!("unsafe_shutdowns={}", report.unsafe_shutdowns.unwrap_or_default()),
            explanation: "Unsafe shutdown counts can indicate power loss, forced resets, or enclosure disconnects. A static historical count is less concerning than a count that continues to rise.".to_string(),
            recommended_action: Some(
                "Recheck after normal use and investigate power, cabling, or enclosure stability if the count increases.".to_string(),
            ),
            confidence: "medium".to_string(),
            drive: Some(drive.name.clone()),
        });
    }

    findings
}

fn smart_skip_reason(drive: &models::DriveInfo) -> Option<String> {
    if drive.physical_path.contains("/virtual/") {
        return Some("virtual block device".to_string());
    }

    let name = drive.name.as_str();
    if name.starts_with("zram")
        || name.starts_with("ram")
        || name.starts_with("loop")
        || name.starts_with("dm-")
    {
        return Some("virtual or mapped block device".to_string());
    }

    None
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_drive(name: &str, physical_path: &str) -> models::DriveInfo {
        models::DriveInfo {
            name: name.to_string(),
            drive_type: "disk".to_string(),
            connection: "Internal".to_string(),
            capacity_gb: 1,
            usage_percent: 0,
            health_ok: true,
            physical_path: physical_path.to_string(),
            fstype: None,
            uuid: None,
            label: None,
            active_mountpoints: Vec::new(),
            topology: Vec::new(),
            pcie_path: Vec::new(),
            serial: None,
            smartctl_exit_code: None,
            smart: None,
            parent: None,
            is_luks: None,
        }
    }

    #[test]
    fn skips_virtual_block_devices_for_smart() {
        let drive = test_drive("zram0", "/sys/devices/virtual/block/zram0");
        assert_eq!(
            smart_skip_reason(&drive).as_deref(),
            Some("virtual block device")
        );
    }

    #[test]
    fn classifies_not_applicable_smart_as_info() {
        let mut drive = test_drive("zram0", "/sys/devices/virtual/block/zram0");
        drive.smart = bench::smart::skipped("virtual block device").report;

        let findings = build_findings(&[drive], true, &[]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, models::FindingCategory::Smart);
        assert_eq!(findings[0].severity, models::FindingSeverity::Info);
        assert_eq!(findings[0].recommended_action, None);
    }

    #[test]
    fn creates_temperature_and_endurance_smart_advisories() {
        let drive = test_drive("nvme0n1", "/sys/devices/pci0000:00/0000:00:1b.4/nvme/nvme0");
        let report = models::SmartReport {
            available: true,
            passed: Some(true),
            transport: models::SmartTransport::Nvme,
            model: Some("Mock NVMe".to_string()),
            serial: None,
            temperature_celsius: Some(72),
            power_on_hours: Some(1000),
            power_cycles: Some(10),
            unsafe_shutdowns: Some(12),
            percentage_used: Some(85),
            reallocated_sectors: None,
            current_pending_sectors: None,
            offline_uncorrectable: None,
            media_errors: Some(0),
            num_err_log_entries: Some(0),
            self_test_status: None,
            smartctl_exit_code: Some(0),
            exit_status_description: Vec::new(),
            limitations: Vec::new(),
        };

        let findings = smart_advisory_findings(&drive, &report);
        assert_eq!(findings.len(), 3);
        assert!(findings
            .iter()
            .any(|finding| finding.title.contains("High SMART temperature")));
        assert!(findings
            .iter()
            .any(|finding| finding.title.contains("endurance usage")));
        assert!(findings
            .iter()
            .any(|finding| finding.title.contains("unsafe shutdown")));
    }
}
