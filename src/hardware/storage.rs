use crate::hardware::connection;
use crate::models::DriveInfo;
use std::process::Command;

// Ephemeral struct exclusively intercepting the raw JSON tree from the local Linux CLI.
#[derive(serde::Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<LsblkDevice>,
}

#[derive(serde::Deserialize)]
pub struct LsblkDevice {
    pub name: String,
    #[serde(rename = "type")]
    pub device_type: String,
    pub size: serde_json::Value,
    pub pkname: Option<String>,

    // Natively parses array of possible physical mounts cleanly.
    pub mountpoints: Option<Vec<Option<String>>>,

    // Natively fetch physical transport type from `lsblk`
    pub tran: Option<String>,

    pub fstype: Option<String>,
    pub uuid: Option<String>,
    pub label: Option<String>,
}

/// Executes an unprivileged subprocess polling the kernel block topology.
/// Wraps it into `models::DriveInfo`, fetching legacy USB connection speeds natively during creation.
/// Returns a tuple of strongly-typed DriveInfo AND its active mount route.
pub fn scan_drives() -> Vec<(DriveInfo, Option<String>)> {
    let output_result = Command::new("lsblk")
        .args([
            "-J",
            "-b",
            "-o",
            "NAME,TYPE,SIZE,PKNAME,MOUNTPOINTS,TRAN,FSTYPE,UUID,LABEL",
        ])
        .output();

    let output = match output_result {
        Ok(out) if out.status.success() => out,
        _ => {
            eprintln!("⚠️ Warning: lsblk failed to poll hardware (missing /sys/dev/block?). Returning empty drive list.");
            return Vec::new();
        }
    };

    let parsed: LsblkOutput = match serde_json::from_slice(&output.stdout) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("⚠️ Warning: Failed to parse lsblk output: {}", e);
            let raw_str = String::from_utf8_lossy(&output.stdout);
            eprintln!(
                "Debug Dump (first 256 chars):\n{}",
                raw_str.chars().take(256).collect::<String>()
            );
            eprintln!("To fix this JSON mapping issue natively, we will analyze this dump!");
            return Vec::new();
        }
    };

    let mut drives = Vec::new();

    for dev in parsed.blockdevices {
        // Natively trap legacy stringified ints vs modern raw integers elegantly.
        let size_num: u64 = if dev.size.is_number() {
            dev.size.as_u64().unwrap_or(0)
        } else if dev.size.is_string() {
            dev.size.as_str().unwrap().parse().unwrap_or(0)
        } else {
            0
        };

        let capacity_gb = size_num / 1_073_741_824;

        let raw_transport = dev.tran.unwrap_or_default().to_uppercase();
        let safe_fallback = if raw_transport == "USB" {
            "USB (External / Unknown Speed)".to_string()
        } else if !raw_transport.is_empty() {
            format!("Internal ({})", raw_transport)
        } else {
            "Internal/PCIe/SATA (Unknown)".to_string()
        };

        // Let udev explicitly quantify the math natively, otherwise fall back to pure `lsblk` connection type!
        let (udev_conn, syspath, topology) = connection::get_device_topology(&dev.name);
        let connection = udev_conn.unwrap_or(safe_fallback);
        let physical_path = if !syspath.is_empty() {
            syspath
        } else {
            "Unmapped in Phase 4".to_string()
        };

        // Map the structured mount points safely.
        let mountpoints_vec: Vec<String> = dev
            .mountpoints
            .unwrap_or_default()
            .into_iter()
            .flatten()
            .collect();
        
        let mount_target = mountpoints_vec.first().cloned();

        let mapped_drive = DriveInfo {
            name: dev.name,
            drive_type: dev.device_type,
            connection,
            capacity_gb,

            usage_percent: 0,
            health_ok: true,
            physical_path,
            
            fstype: dev.fstype,
            uuid: dev.uuid,
            label: dev.label,
            active_mountpoints: mountpoints_vec,

            topology,
            serial: None,
            smartctl_exit_code: None,
            parent: dev.pkname,
            is_luks: None,
        };

        drives.push((mapped_drive, mount_target));
    }

    drives
}

/// Parses the local /etc/fstab safely into strong typing
pub fn extract_fstab() -> Vec<crate::models::FstabEntry> {
    let mut entries = Vec::new();
    if let Ok(content) = std::fs::read_to_string("/etc/fstab") {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                entries.push(crate::models::FstabEntry {
                    file_system: parts[0].to_string(),
                    mount_point: parts[1].to_string(),
                    type_: parts[2].to_string(),
                    options: parts[3].to_string(),
                    dump: parts.get(4).unwrap_or(&"0").to_string(),
                    pass: parts.get(5).unwrap_or(&"0").to_string(),
                });
            }
        }
    }
    entries
}
