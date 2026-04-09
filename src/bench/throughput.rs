use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

const MIN_FREE_MB: u64 = 5000;
const MIN_FREE_PERCENT: u8 = 10;
const BENCH_SIZE_MB: u32 = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CapacitySnapshot {
    total_mb: u64,
    available_mb: u64,
    used_percent: u8,
}

/// Safely evaluates active mount bounds using basic native `df -Pm`.
/// Fails closed if space is aggressively limited.
pub fn ensure_capacity_safety(mount_point: &str) -> bool {
    capacity_check_message(mount_point).is_ok()
}

pub fn capacity_check_message(mount_point: &str) -> Result<String, String> {
    let output = Command::new("df")
        .args(["-Pm", mount_point])
        .output()
        .map_err(|e| format!("failed to invoke df for {}: {}", mount_point, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "df failed for {}{}",
            mount_point,
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {}", stderr)
            }
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let snapshot = parse_df_output(&stdout)
        .ok_or_else(|| format!("unable to parse df output for {}", mount_point))?;

    validate_capacity(&snapshot)
}

fn validate_capacity(snapshot: &CapacitySnapshot) -> Result<String, String> {
    let free_percent = free_percent(snapshot);

    if snapshot.available_mb <= MIN_FREE_MB {
        return Err(format!(
            "{}MB free is not enough; need more than {}MB free",
            snapshot.available_mb, MIN_FREE_MB
        ));
    }

    if free_percent < MIN_FREE_PERCENT {
        return Err(format!(
            "{}% free is below the {}% minimum",
            free_percent, MIN_FREE_PERCENT
        ));
    }

    Ok(format!(
        "{}MB free out of {}MB total ({}% free)",
        snapshot.available_mb, snapshot.total_mb, free_percent
    ))
}

fn free_percent(snapshot: &CapacitySnapshot) -> u8 {
    100u8.saturating_sub(snapshot.used_percent)
}

fn parse_df_output(stdout: &str) -> Option<CapacitySnapshot> {
    let lines: Vec<&str> = stdout.lines().collect();
    let data_line = lines
        .iter()
        .skip(1)
        .find(|line| !line.trim().is_empty())
        .copied()?;
    let cols: Vec<&str> = data_line.split_whitespace().collect();
    if cols.len() < 5 {
        return None;
    }

    let total_mb = cols.get(1)?.parse::<u64>().ok()?;
    let available_mb = cols.get(3)?.parse::<u64>().ok()?;
    let used_percent = cols.get(4)?.trim_end_matches('%').parse::<u8>().ok()?;

    Some(CapacitySnapshot {
        total_mb,
        available_mb,
        used_percent,
    })
}

/// Synthetically generates a volatile 1GB footprint sequentially mapping native IO throughput.
pub fn run_buffered_bench(mount_point: &str) -> Option<u32> {
    match capacity_check_message(mount_point) {
        Ok(message) => {
            eprintln!(
                "📊 Throughput safety check passed on {} -> {}.",
                mount_point, message
            );
        }
        Err(reason) => {
            eprintln!(
                "⚠️ Skipping throughput diagnostics on {} -> {}.",
                mount_point, reason
            );
            return None;
        }
    }

    let bench_file = format!("{}/.tuxtests_bench.tmp", mount_point);
    let path = Path::new(&bench_file);

    // Ensure write safely natively over user permission bounds without triggering Polkit again natively.
    let file_result = File::create(path);
    if file_result.is_err() {
        eprintln!(
            "⚠️ Skipping throughput diagnostics on {} -> unable to create temporary benchmark file.",
            mount_point
        );
        return None;
    }
    let mut file = file_result.unwrap();

    let buffer = vec![0u8; 1_048_576];
    let start = Instant::now();

    for _ in 0..BENCH_SIZE_MB {
        if file.write_all(&buffer).is_err() {
            eprintln!(
                "⚠️ Throughput benchmark on {} ended early because the write stream failed.",
                mount_point
            );
            break;
        }
    }
    let _ = file.sync_all();

    let duration = start.elapsed().as_secs_f64();
    let write_mb_s = (f64::from(BENCH_SIZE_MB) / duration) as u32;

    let _ = std::fs::remove_file(path);

    Some(write_mb_s)
}

#[cfg(test)]
mod tests {
    use super::{free_percent, parse_df_output, validate_capacity, CapacitySnapshot};

    #[test]
    fn parses_df_output() {
        let stdout = "Filesystem 1048576-blocks Used Available Capacity Mounted on\n/dev/sda1 10000 2000 8000 20% /\n";
        let snapshot = parse_df_output(stdout).unwrap();

        assert_eq!(
            snapshot,
            CapacitySnapshot {
                total_mb: 10000,
                available_mb: 8000,
                used_percent: 20,
            }
        );
    }

    #[test]
    fn free_percent_is_inverse_of_used_percent() {
        let snapshot = CapacitySnapshot {
            total_mb: 10000,
            available_mb: 8000,
            used_percent: 20,
        };

        assert_eq!(free_percent(&snapshot), 80);
    }

    #[test]
    fn rejects_low_available_space() {
        let snapshot = CapacitySnapshot {
            total_mb: 10000,
            available_mb: 5000,
            used_percent: 20,
        };

        let message = validate_capacity(&snapshot).unwrap_err();
        assert!(message.contains("need more than 5000MB free"));
    }

    #[test]
    fn rejects_low_free_percent() {
        let snapshot = CapacitySnapshot {
            total_mb: 100000,
            available_mb: 9000,
            used_percent: 95,
        };

        let message = validate_capacity(&snapshot).unwrap_err();
        assert!(message.contains("below the 10% minimum"));
    }

    #[test]
    fn accepts_safe_capacity() {
        let snapshot = CapacitySnapshot {
            total_mb: 100000,
            available_mb: 25000,
            used_percent: 75,
        };

        let message = validate_capacity(&snapshot).unwrap();
        assert!(message.contains("25% free"));
    }
}
