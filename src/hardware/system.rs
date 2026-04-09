use crate::models::SystemInfo;
use sysinfo::System;
use std::fs;

/// Extracts static hardware topologies from the host (CPU Type and aggregate RAM).
pub fn get_system_specs() -> SystemInfo {
    let mut sys = System::new_all();
    sys.refresh_all();

    // Safely extract RAM and divide safely to gigabytes.
    let total_ram_gb = sys.total_memory() / 1_073_741_824;

    let cpu_brand = sys
        .cpus()
        .first()
        .map(|c| c.brand())
        .unwrap_or("Unknown CPU")
        .to_string();

    let mut os_release = "Unknown Linux".to_string();
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if line.starts_with("PRETTY_NAME=") {
                os_release = line.replace("PRETTY_NAME=", "").replace("\"", "");
                break;
            }
        }
    }

    let kernel_version = System::kernel_version().unwrap_or_else(|| "Unknown Kernel".to_string());

    SystemInfo {
        os_release,
        kernel_version,
        cpu: cpu_brand,
        ram_gb: total_ram_gb,
    }
}
