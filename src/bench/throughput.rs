use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::time::Instant;
use std::process::Command;

/// Safely evaluates active mount bounds using basic native `df -m`.
/// Fails closed if space is aggressively limited.
pub fn ensure_capacity_safety(mount_point: &str) -> bool {
    let output = Command::new("df")
        .args(["-m", mount_point]) // megabytes natively
        .output()
        .ok();
        
    if let Some(out) = output {
         let stdout = String::from_utf8_lossy(&out.stdout);
         let lines: Vec<&str> = stdout.lines().collect();
         if lines.len() > 1 {
             let cols: Vec<&str> = lines[1].split_whitespace().collect();
             if cols.len() > 3 {
                 // Column 4 is "Available" in MB
                 if let Ok(available_mb) = cols[3].parse::<u64>() {
                     return available_mb > 5000; // Strictly ensure > 5GB native free space!
                 }
             }
         }
    }
    false
}

/// Synthetically generates a volatile 1GB footprint sequentially mapping native IO throughput.
pub fn run_buffered_bench(mount_point: &str) -> Option<u32> {
    if !ensure_capacity_safety(mount_point) {
        println!("⚠️ Skipping throughput diagnostics on {} -> Sub-5GB Capacity Bounds Triggered!", mount_point);
        return None;
    }
    
    let bench_file = format!("{}/.tuxtests_bench.tmp", mount_point);
    let path = Path::new(&bench_file);
    
    // Ensure write safely natively over user permission bounds without triggering Polkit again natively!
    let file_result = File::create(path);
    if file_result.is_err() {
        // Silently skip if writing to a restricted host partition
        return None; 
    }
    let mut file = file_result.unwrap();

    let buffer = vec![0u8; 1_048_576]; // Strictly isolated 1MB sequential chunks
    let start = Instant::now();
    
    for _ in 0..1024 { // Iterate exactly exactly 1024 MB
        if file.write_all(&buffer).is_err() { break; }
    }
    let _ = file.sync_all(); 
    
    let duration = start.elapsed().as_secs_f64();
    let write_mb_s = (1024.0 / duration) as u32;
    
    let _ = std::fs::remove_file(path); // Aggressive systemic cleanup!
    
    Some(write_mb_s)
}
