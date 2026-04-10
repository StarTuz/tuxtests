use crate::hardware::pci;
use crate::models::SystemInfo;
use std::fs;
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

    let mut os_release = std::collections::BTreeMap::new();
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if let Some((k, v)) = line.split_once('=') {
                os_release.insert(k.to_string(), v.replace("\"", "").to_string());
            }
        }
    }

    let hostname = System::host_name()
        .or_else(|| {
            fs::read_to_string("/etc/hostname")
                .ok()
                .map(|value| value.trim().to_string())
        })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Unknown Host".to_string());

    let kernel_version = System::kernel_version().unwrap_or_else(|| "Unknown Kernel".to_string());
    let motherboard = read_motherboard();

    SystemInfo {
        os_release,
        hostname,
        kernel_version,
        cpu: cpu_brand,
        ram_gb: total_ram_gb,
        motherboard,
        pcie_aspm_policy: pci::read_aspm_policy(),
    }
}

fn read_motherboard() -> Option<String> {
    let vendor = fs::read_to_string("/sys/devices/virtual/dmi/id/board_vendor")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let board = fs::read_to_string("/sys/devices/virtual/dmi/id/board_name")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    match (vendor, board) {
        (Some(vendor), Some(board)) if vendor == board => Some(board),
        (Some(vendor), Some(board)) => Some(format!("{} {}", vendor, board)),
        (Some(vendor), None) => Some(vendor),
        (None, Some(board)) => Some(board),
        (None, None) => None,
    }
}
