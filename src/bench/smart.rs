use crate::models::{SmartProbeStatus, SmartReport, SmartTransport};
use serde_json::Value;
use std::fs;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct SmartOutcome {
    pub health_ok: bool,
    pub exit_code: Option<i32>,
    pub report: Option<SmartReport>,
    pub anomalies: Vec<String>,
}

/// Invokes S.M.A.R.T. monitoring and captures structured smartctl JSON.
pub fn check_health(device_node: &str) -> SmartOutcome {
    let dev_path = format!("/dev/{}", device_node);
    let use_direct_smartctl = current_euid().is_some_and(|euid| euid == 0);
    let mut command = if use_direct_smartctl {
        Command::new("smartctl")
    } else {
        let mut command = Command::new("pkexec");
        command.arg("smartctl");
        command
    };
    let output = command.args(["-x", "-j", &dev_path]).output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let code = out.status.code().unwrap_or(0);
            let descriptions = describe_exit_status(code);

            if smartmontools_missing(&stdout, &stderr) {
                return unavailable_outcome(
                    Some(code),
                    SmartProbeStatus::ToolMissing,
                    format!("anomaly: smartmontools missing or execution failed on {device_node}"),
                );
            }

            if access_denied(&stderr) {
                return unavailable_outcome(
                    Some(code),
                    SmartProbeStatus::AccessDenied,
                    access_denied_anomaly(device_node, use_direct_smartctl),
                );
            }

            match parse_smart_json(&stdout, Some(code), descriptions.clone()) {
                Ok(mut report) => {
                    if !stderr.trim().is_empty() {
                        report.limitations.push(stderr.trim().to_string());
                    }

                    let health_ok = infer_health_ok(&report, code);
                    let mut anomalies = Vec::new();
                    if !descriptions.is_empty() {
                        anomalies.push(format!(
                            "anomaly: smartctl reported exit code {code} on {device_node}: {}",
                            descriptions.join(", ")
                        ));
                    }

                    SmartOutcome {
                        health_ok,
                        exit_code: Some(code),
                        report: Some(report),
                        anomalies,
                    }
                }
                Err(err) => {
                    let mut anomaly =
                        format!("anomaly: smartctl JSON parsing failed on {device_node}: {err}");
                    if !descriptions.is_empty() {
                        anomaly.push_str(&format!("; exit status: {}", descriptions.join(", ")));
                    }

                    SmartOutcome {
                        health_ok: code == 0,
                        exit_code: Some(code),
                        report: Some(unavailable_report(
                            Some(code),
                            SmartProbeStatus::ParseFailed,
                            descriptions,
                            vec![err.to_string()],
                        )),
                        anomalies: vec![anomaly],
                    }
                }
            }
        }
        Err(err) => unavailable_outcome(
            None,
            SmartProbeStatus::ExecutionFailed,
            execution_failed_anomaly(use_direct_smartctl, err),
        ),
    }
}

pub fn skipped(reason: impl Into<String>) -> SmartOutcome {
    SmartOutcome {
        health_ok: true,
        exit_code: None,
        report: Some(unavailable_report(
            None,
            SmartProbeStatus::NotApplicable,
            Vec::new(),
            vec![format!("SMART not applicable: {}", reason.into())],
        )),
        anomalies: Vec::new(),
    }
}

fn unavailable_outcome(
    exit_code: Option<i32>,
    status: SmartProbeStatus,
    anomaly: String,
) -> SmartOutcome {
    SmartOutcome {
        health_ok: false,
        exit_code,
        report: Some(unavailable_report(
            exit_code,
            status,
            exit_code.map(describe_exit_status).unwrap_or_default(),
            vec![anomaly.clone()],
        )),
        anomalies: vec![anomaly],
    }
}

fn unavailable_report(
    exit_code: Option<i32>,
    status: SmartProbeStatus,
    exit_status_description: Vec<String>,
    limitations: Vec<String>,
) -> SmartReport {
    SmartReport {
        status,
        available: false,
        passed: None,
        transport: SmartTransport::Unknown,
        model: None,
        serial: None,
        temperature_celsius: None,
        power_on_hours: None,
        power_cycles: None,
        unsafe_shutdowns: None,
        percentage_used: None,
        reallocated_sectors: None,
        current_pending_sectors: None,
        offline_uncorrectable: None,
        media_errors: None,
        num_err_log_entries: None,
        self_test_status: None,
        smartctl_exit_code: exit_code,
        exit_status_description,
        limitations,
    }
}

fn parse_smart_json(
    stdout: &str,
    exit_code: Option<i32>,
    exit_status_description: Vec<String>,
) -> Result<SmartReport, serde_json::Error> {
    let json: Value = serde_json::from_str(stdout)?;
    let nvme_log = json.get("nvme_smart_health_information_log");

    let mut report = SmartReport {
        status: SmartProbeStatus::Available,
        available: true,
        passed: json
            .pointer("/smart_status/passed")
            .and_then(Value::as_bool),
        transport: transport_from_json(&json),
        model: string_at(
            &json,
            &["/model_name", "/model_family", "/device/model_name"],
        ),
        serial: string_at(&json, &["/serial_number"]),
        temperature_celsius: integer_at(
            &json,
            &[
                "/temperature/current",
                "/nvme_smart_health_information_log/temperature",
            ],
        )
        .or_else(|| ata_raw_attribute_any(&json, &["Temperature_Celsius"], &[194]))
        .or_else(|| ata_raw_attribute_any(&json, &["Airflow_Temperature_Cel"], &[190])),
        power_on_hours: integer_at(&json, &["/power_on_time/hours"])
            .or_else(|| ata_raw_attribute_any(&json, &["Power_On_Hours"], &[9])),
        power_cycles: integer_at(&json, &["/power_cycle_count"])
            .or_else(|| ata_raw_attribute_any(&json, &["Power_Cycle_Count"], &[12])),
        unsafe_shutdowns: nvme_log
            .and_then(|log| log.get("unsafe_shutdowns"))
            .and_then(value_to_i64),
        percentage_used: nvme_log
            .and_then(|log| log.get("percentage_used"))
            .and_then(value_to_i64),
        reallocated_sectors: ata_raw_attribute_any(&json, &["Reallocated_Sector_Ct"], &[5])
            .or_else(|| integer_at(&json, &["/scsi_grown_defect_list"])),
        current_pending_sectors: ata_raw_attribute_any(&json, &["Current_Pending_Sector"], &[197]),
        offline_uncorrectable: ata_raw_attribute_any(&json, &["Offline_Uncorrectable"], &[198]),
        media_errors: nvme_log
            .and_then(|log| log.get("media_errors"))
            .and_then(value_to_i64),
        num_err_log_entries: nvme_log
            .and_then(|log| log.get("num_err_log_entries"))
            .and_then(value_to_i64),
        self_test_status: json
            .pointer("/ata_smart_self_test_log/standard/table/0/status/string")
            .and_then(Value::as_str)
            .map(str::to_string),
        smartctl_exit_code: exit_code,
        exit_status_description,
        limitations: smartctl_messages(&json),
    };

    if !has_usable_smart_facts(&report) {
        report.available = false;
        report.status =
            unavailable_status_from_json(exit_code.unwrap_or_default(), &report.limitations);
    }

    Ok(report)
}

fn infer_health_ok(report: &SmartReport, code: i32) -> bool {
    if code & smart_access_bits() != 0 {
        return false;
    }

    if let Some(passed) = report.passed {
        return passed && code & smart_failure_bits() == 0;
    }

    code == 0 || code & smart_failure_bits() == 0
}

fn smart_access_bits() -> i32 {
    1 | 2
}

fn smart_failure_bits() -> i32 {
    4 | 8 | 16 | 32 | 64 | 128
}

fn describe_exit_status(code: i32) -> Vec<String> {
    [
        (1, "command line did not parse"),
        (2, "device open failed or SMART access failed"),
        (4, "SMART status indicates disk failing"),
        (8, "prefail attribute is at or below threshold"),
        (16, "old-age or prefail attribute is at or below threshold"),
        (32, "SMART error log contains errors"),
        (64, "SMART self-test log contains errors"),
        (128, "device error log contains errors"),
    ]
    .iter()
    .filter_map(|(bit, description)| {
        if code & bit != 0 {
            Some((*description).to_string())
        } else {
            None
        }
    })
    .collect()
}

fn transport_from_json(json: &Value) -> SmartTransport {
    let transport = [
        string_at(json, &["/device/type"]),
        string_at(json, &["/device/protocol"]),
        string_at(json, &["/device/name"]),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ")
    .to_lowercase();

    if transport.contains("nvme") {
        SmartTransport::Nvme
    } else if transport.contains("snt")
        || transport.contains("usb")
        || transport.contains("jmicron")
        || transport.contains("asmedia")
    {
        SmartTransport::UsbBridge
    } else if transport.contains("scsi") {
        SmartTransport::Scsi
    } else if transport.contains("ata") || transport.contains("sat") {
        SmartTransport::Ata
    } else {
        SmartTransport::Unknown
    }
}

fn has_usable_smart_facts(report: &SmartReport) -> bool {
    report.passed.is_some()
        || report.model.is_some()
        || report.serial.is_some()
        || report.temperature_celsius.is_some()
        || report.power_on_hours.is_some()
        || report.power_cycles.is_some()
        || report.unsafe_shutdowns.is_some()
        || report.percentage_used.is_some()
        || report.reallocated_sectors.is_some()
        || report.current_pending_sectors.is_some()
        || report.offline_uncorrectable.is_some()
        || report.media_errors.is_some()
        || report.num_err_log_entries.is_some()
        || report.self_test_status.is_some()
}

fn unavailable_status_from_json(code: i32, limitations: &[String]) -> SmartProbeStatus {
    let limitation_text = limitations.join("\n").to_lowercase();

    if limitation_text.contains("unknown usb bridge")
        || limitation_text.contains("unsupported")
        || limitation_text.contains("please specify device type")
    {
        SmartProbeStatus::Unsupported
    } else if code & 2 != 0 {
        SmartProbeStatus::AccessDenied
    } else if code & 1 != 0 {
        SmartProbeStatus::ParseFailed
    } else {
        SmartProbeStatus::Unknown
    }
}

fn smartctl_messages(json: &Value) -> Vec<String> {
    json.pointer("/smartctl/messages")
        .and_then(Value::as_array)
        .map(|messages| {
            messages
                .iter()
                .filter_map(|message| message.get("string").and_then(Value::as_str))
                .map(str::trim)
                .filter(|message| !message.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn ata_raw_attribute_any(json: &Value, names: &[&str], ids: &[i64]) -> Option<i64> {
    json.pointer("/ata_smart_attributes/table")
        .and_then(Value::as_array)?
        .iter()
        .find(|attribute| ata_attribute_matches(attribute, names, ids))
        .and_then(|attribute| {
            attribute
                .pointer("/raw/value")
                .and_then(value_to_i64)
                .or_else(|| attribute.pointer("/raw/string").and_then(value_to_i64))
                .or_else(|| attribute.get("value").and_then(value_to_i64))
        })
}

fn ata_attribute_matches(attribute: &Value, names: &[&str], ids: &[i64]) -> bool {
    let name_matches = attribute
        .get("name")
        .and_then(Value::as_str)
        .is_some_and(|name| names.contains(&name));
    let id_matches = attribute
        .get("id")
        .and_then(value_to_i64)
        .is_some_and(|id| ids.contains(&id));

    name_matches || id_matches
}

fn string_at(json: &Value, paths: &[&str]) -> Option<String> {
    paths.iter().find_map(|path| {
        json.pointer(path)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn integer_at(json: &Value, paths: &[&str]) -> Option<i64> {
    paths
        .iter()
        .find_map(|path| json.pointer(path).and_then(value_to_i64))
}

fn value_to_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|number| i64::try_from(number).ok()))
        .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
}

fn smartmontools_missing(stdout: &str, stderr: &str) -> bool {
    let combined = format!("{stdout}\n{stderr}").to_lowercase();
    combined.contains("no such file") || combined.contains("not found")
}

fn access_denied(stderr: &str) -> bool {
    let stderr = stderr.to_lowercase();
    stderr.contains("not authorized")
        || stderr.contains("authentication failed")
        || stderr.contains("operation not permitted")
}

fn access_denied_anomaly(device_node: &str, direct: bool) -> String {
    if direct {
        format!("anomaly: smartctl access denied on {device_node}")
    } else {
        format!("anomaly: smartctl access denied by polkit authentication on {device_node}")
    }
}

fn execution_failed_anomaly(direct: bool, err: std::io::Error) -> String {
    if direct {
        format!("anomaly: smartctl execution failed ({err})")
    } else {
        format!("anomaly: privileged execution via pkexec failed ({err})")
    }
}

fn current_euid() -> Option<u32> {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| euid_from_proc_status(&status))
}

fn euid_from_proc_status(status: &str) -> Option<u32> {
    status.lines().find_map(|line| {
        let fields = line.strip_prefix("Uid:")?;
        fields.split_whitespace().nth(1)?.parse().ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nvme_smart_json() {
        let json = r#"{
            "device": {"type": "sntasmedia", "protocol": "NVMe"},
            "model_name": "Lexar NM790",
            "serial_number": "ABC123",
            "smart_status": {"passed": true},
            "temperature": {"current": 42},
            "power_on_time": {"hours": 1234},
            "power_cycle_count": 56,
            "nvme_smart_health_information_log": {
                "unsafe_shutdowns": 7,
                "percentage_used": 2,
                "media_errors": 0,
                "num_err_log_entries": 1
            }
        }"#;

        let report = parse_smart_json(json, Some(0), Vec::new()).unwrap();
        assert_eq!(report.transport, SmartTransport::Nvme);
        assert_eq!(report.status, SmartProbeStatus::Available);
        assert!(report.available);
        assert_eq!(report.model.as_deref(), Some("Lexar NM790"));
        assert_eq!(report.serial.as_deref(), Some("ABC123"));
        assert_eq!(report.passed, Some(true));
        assert_eq!(report.temperature_celsius, Some(42));
        assert_eq!(report.power_on_hours, Some(1234));
        assert_eq!(report.power_cycles, Some(56));
        assert_eq!(report.unsafe_shutdowns, Some(7));
        assert_eq!(report.percentage_used, Some(2));
        assert_eq!(report.media_errors, Some(0));
        assert_eq!(report.num_err_log_entries, Some(1));
    }

    #[test]
    fn parses_ata_smart_json() {
        let json = r#"{
            "device": {"type": "sat"},
            "model_name": "WD Blue",
            "smart_status": {"passed": true},
            "ata_smart_attributes": {
                "table": [
                    {"name": "Reallocated_Sector_Ct", "raw": {"value": 1}},
                    {"name": "Current_Pending_Sector", "raw": {"value": 2}},
                    {"name": "Offline_Uncorrectable", "raw": {"value": 3}}
                ]
            },
            "ata_smart_self_test_log": {
                "standard": {
                    "table": [{"status": {"string": "Completed without error"}}]
                }
            }
        }"#;

        let report = parse_smart_json(json, Some(0), Vec::new()).unwrap();
        assert_eq!(report.transport, SmartTransport::Ata);
        assert_eq!(report.reallocated_sectors, Some(1));
        assert_eq!(report.current_pending_sectors, Some(2));
        assert_eq!(report.offline_uncorrectable, Some(3));
        assert_eq!(
            report.self_test_status.as_deref(),
            Some("Completed without error")
        );
    }

    #[test]
    fn parses_ata_smart_attributes_by_id_and_raw_string() {
        let json = r#"{
            "device": {"type": "ata"},
            "smart_status": {"passed": true},
            "ata_smart_attributes": {
                "table": [
                    {"id": 5, "name": "Vendor_Reallocated", "raw": {"string": "4"}},
                    {"id": 9, "name": "Vendor_Power_On", "raw": {"string": "5000"}},
                    {"id": 12, "name": "Vendor_Power_Cycle", "raw": {"value": 22}},
                    {"id": 190, "name": "Airflow_Temperature_Cel", "raw": {"value": 44}},
                    {"id": 197, "name": "Vendor_Pending", "raw": {"value": 1}},
                    {"id": 198, "name": "Vendor_Uncorrectable", "raw": {"value": 2}}
                ]
            }
        }"#;

        let report = parse_smart_json(json, Some(0), Vec::new()).unwrap();
        assert_eq!(report.transport, SmartTransport::Ata);
        assert_eq!(report.reallocated_sectors, Some(4));
        assert_eq!(report.power_on_hours, Some(5000));
        assert_eq!(report.power_cycles, Some(22));
        assert_eq!(report.temperature_celsius, Some(44));
        assert_eq!(report.current_pending_sectors, Some(1));
        assert_eq!(report.offline_uncorrectable, Some(2));
    }

    #[test]
    fn parses_scsi_grown_defect_count() {
        let json = r#"{
            "device": {"type": "scsi"},
            "smart_status": {"passed": true},
            "vendor": "SEAGATE",
            "product": "ST1000",
            "scsi_grown_defect_list": 3
        }"#;

        let report = parse_smart_json(json, Some(0), Vec::new()).unwrap();
        assert_eq!(report.transport, SmartTransport::Scsi);
        assert_eq!(report.reallocated_sectors, Some(3));
    }

    #[test]
    fn marks_json_error_payload_without_smart_facts_unavailable() {
        let json = r#"{
            "smartctl": {
                "messages": [
                    {
                        "severity": "error",
                        "string": "Unknown USB bridge. Please specify device type with the -d option."
                    }
                ]
            },
            "device": {"type": "unknown"}
        }"#;

        let report = parse_smart_json(
            json,
            Some(1),
            vec!["command line did not parse".to_string()],
        )
        .unwrap();
        assert!(!report.available);
        assert_eq!(report.status, SmartProbeStatus::Unsupported);
        assert_eq!(report.limitations.len(), 1);
    }

    #[test]
    fn interprets_smartctl_exit_status_bits() {
        let descriptions = describe_exit_status(4 | 32 | 64);
        assert!(descriptions.contains(&"SMART status indicates disk failing".to_string()));
        assert!(descriptions.contains(&"SMART error log contains errors".to_string()));
        assert!(descriptions.contains(&"SMART self-test log contains errors".to_string()));
    }

    #[test]
    fn parses_effective_uid_from_proc_status() {
        let status = "Name:\ttuxtests\nUid:\t1000\t0\t0\t0\n";
        assert_eq!(euid_from_proc_status(status), Some(0));
    }
}
