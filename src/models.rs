use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TuxPayload {
    pub summary_header: String,
    pub system: SystemInfo,
    pub drives: Vec<DriveInfo>,

    /// BTreeMap ensures benchmark ordering natively resolves identically for the LLM
    pub benchmarks: BTreeMap<String, BenchmarkResult>,
    #[serde(default)]
    pub findings: Vec<DiagnosticFinding>,
    pub kernel_anomalies: Vec<String>,
    pub fstab: Vec<FstabEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SystemInfo {
    pub os_release: BTreeMap<String, String>,
    pub hostname: String,
    pub kernel_version: String,
    pub cpu: String,
    pub ram_gb: u64,
    pub motherboard: Option<String>,
    pub pcie_aspm_policy: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DriveInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub drive_type: String, // E.g., NVMe, HDD, LVM
    pub connection: String,
    pub capacity_gb: u64,
    pub usage_percent: u8,
    pub health_ok: bool,
    pub physical_path: String,

    pub fstype: Option<String>,
    pub uuid: Option<String>,
    pub label: Option<String>,
    pub active_mountpoints: Vec<String>,

    /// New granular lineages for UI tree visualization
    pub topology: Vec<TopologyNode>,
    #[serde(default)]
    pub pcie_path: Vec<PcieDeviceInfo>,

    // Optional properties depending on disk topology topology edge cases
    pub serial: Option<String>,
    pub smartctl_exit_code: Option<i32>,
    #[serde(default)]
    pub smart: Option<SmartReport>,
    pub parent: Option<String>,
    pub is_luks: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FindingCategory {
    Smart,
    Pcie,
    Topology,
    Capacity,
    Benchmark,
    Privilege,
    Logs,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Info,
    Notice,
    Warning,
    Critical,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticFinding {
    pub category: FindingCategory,
    pub severity: FindingSeverity,
    pub title: String,
    pub evidence: String,
    pub explanation: String,
    pub recommended_action: Option<String>,
    pub confidence: String,
    pub drive: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SmartTransport {
    Ata,
    Nvme,
    Scsi,
    UsbBridge,
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SmartProbeStatus {
    Available,
    NotApplicable,
    AccessDenied,
    ToolMissing,
    ExecutionFailed,
    ParseFailed,
    #[default]
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SmartReport {
    #[serde(default)]
    pub status: SmartProbeStatus,
    pub available: bool,
    pub passed: Option<bool>,
    pub transport: SmartTransport,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub temperature_celsius: Option<i64>,
    pub power_on_hours: Option<i64>,
    pub power_cycles: Option<i64>,
    pub unsafe_shutdowns: Option<i64>,
    pub percentage_used: Option<i64>,
    pub reallocated_sectors: Option<i64>,
    pub current_pending_sectors: Option<i64>,
    pub offline_uncorrectable: Option<i64>,
    pub media_errors: Option<i64>,
    pub num_err_log_entries: Option<i64>,
    pub self_test_status: Option<String>,
    pub smartctl_exit_code: Option<i32>,
    pub exit_status_description: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TopologyNode {
    pub level: usize,
    pub subsystem: String,
    pub sysname: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PcieDeviceInfo {
    pub bdf: String,
    pub driver: Option<String>,
    pub current_link_speed: Option<String>,
    pub current_link_width: Option<String>,
    pub max_link_speed: Option<String>,
    pub max_link_width: Option<String>,
    pub aspm_capability: Option<String>,
    pub aspm: Option<String>,
    pub aspm_source: Option<String>,
    pub aspm_probe_error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BenchmarkResult {
    pub write_mb_s: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FstabEntry {
    pub file_system: String,
    pub mount_point: String,
    pub type_: String,
    pub options: String,
    pub dump: String,
    pub pass: String,
}
