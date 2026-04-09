use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TuxPayload {
    pub summary_header: String,
    pub system: SystemInfo,
    pub drives: Vec<DriveInfo>,

    /// BTreeMap ensures benchmark ordering natively resolves identically for the LLM
    pub benchmarks: BTreeMap<String, BenchmarkResult>,
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

    // Optional properties depending on disk topology topology edge cases
    pub serial: Option<String>,
    pub smartctl_exit_code: Option<i32>,
    pub parent: Option<String>,
    pub is_luks: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TopologyNode {
    pub level: usize,
    pub subsystem: String,
    pub sysname: String,
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
