use crate::models::SystemInfo;
use sysinfo::System;

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

    SystemInfo {
        cpu: cpu_brand,
        ram_gb: total_ram_gb,
    }
}
