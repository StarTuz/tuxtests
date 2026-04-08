use serde::{Serialize, Deserialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TuxPayload {
    pub system: SystemInfo,
    pub drives: Vec<DriveInfo>,
    
    /// BTreeMap ensures benchmark ordering natively resolves identically for the LLM
    pub benchmarks: BTreeMap<String, BenchmarkResult>,
    pub kernel_anomalies: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SystemInfo {
    pub cpu: String,
    pub ram_gb: u64,
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
    
    // Optional properties depending on disk topology topology edge cases
    pub serial: Option<String>,
    pub smartctl_exit_code: Option<i32>,
    pub parent: Option<String>,
    pub is_luks: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BenchmarkResult {
    pub write_mb_s: u32,
}
