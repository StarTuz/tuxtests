//! Hybrid Log-Scraping RAG Engine
//! Instead of semantic vector search, uses Identifier-Based Filtering on local kernel logs.

use crate::models::DriveInfo;
use std::process::Command;

/// Fetches the raw kernel log buffer natively using the 3-Tier Privilege Bridge.
pub fn fetch_kernel_logs() -> String {
    let mut log_output = String::new();

    // The 3-Tier Privilege Bridge

    // Attempt 1: Native dmesg (Standard & Quiet)
    if let Ok(output) = Command::new("dmesg").output() {
        if output.status.success() {
            log_output = String::from_utf8_lossy(&output.stdout).to_string();
        }
    }

    // Attempt 2: Silent Fallback to journalctl (often readable if user is in wheel/adm group)
    if log_output.is_empty() {
        if let Ok(output) = Command::new("journalctl")
            .args(["-k", "--no-pager", "-n", "500"])
            .output()
        {
            if output.status.success() {
                log_output = String::from_utf8_lossy(&output.stdout).to_string();
            }
        }
    }

    // Attempt 3: The Escalator (pkexec dmesg)
    if log_output.is_empty() {
        if let Ok(output) = Command::new("pkexec").arg("dmesg").output() {
            if output.status.success() {
                log_output = String::from_utf8_lossy(&output.stdout).to_string();
            }
        }
    }

    log_output
}

/// Parses a local log buffer (dmesg or syslog) for occurrences of specific block identifiers.
/// Captures critical warnings like "I/O errors" or "reset high-speed USB device" globally tied to the block or its serial.
pub fn retrieve_kernel_anomalies(drive: &DriveInfo, log_output: &str) -> Vec<String> {
    let mut anomalies = Vec::new();

    // Build a list of matchable target strings for this device
    let mut targets = vec![drive.name.clone()];

    if let Some(serial) = &drive.serial {
        if !serial.trim().is_empty() {
            targets.push(serial.clone());
        }
    }

    for node in &drive.topology {
        if !node.sysname.trim().is_empty() {
            targets.push(node.sysname.clone());
        }
    }

    // Filter logs for critical errors related to these targets
    for line in log_output.lines() {
        let l_lower = line.to_lowercase();
        // Standard kernel anomaly keywords: I/O errors, resets, aborts
        if l_lower.contains("error")
            || l_lower.contains("reset")
            || l_lower.contains("abort")
            || l_lower.contains("fail")
        {
            for target in &targets {
                if line.contains(target) {
                    anomalies.push(line.to_string());
                    break; // Move to next line
                }
            }
        }
    }

    anomalies.truncate(20);
    anomalies
}
