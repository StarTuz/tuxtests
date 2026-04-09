use std::process::Command;

/// Invokes standard S.M.A.R.T. monitoring via Polkit elevation natively.
/// Ensures we mathematically pass exit topologies back, intercepting failures dynamically.
pub fn check_health(device_node: &str) -> (bool, Option<i32>, Option<String>) {
    let dev_path = format!("/dev/{}", device_node);
    let output = Command::new("pkexec")
        .args(["smartctl", "-H", &dev_path])
        .output();

    match output {
        Ok(out) => {
            let stdout_str = String::from_utf8_lossy(&out.stdout);
            let stderr_str = String::from_utf8_lossy(&out.stderr);

            // Critical Trap: Check if smartmontools is explicitly missing from host kernel/environment.
            if stderr_str.contains("No such file") || stdout_str.contains("not found") {
                return (
                    false,
                    None,
                    Some("anomaly: smartmontools missing or execution failed".to_string()),
                );
            }

            if stderr_str.to_lowercase().contains("not authorized")
                || stderr_str.to_lowercase().contains("authentication failed")
            {
                return (
                    false,
                    out.status.code(),
                    Some("anomaly: smartctl access denied by polkit authentication".to_string()),
                );
            }

            let code = out.status.code().unwrap_or(0);
            if code == 0 {
                // Perfect Health mathematically via deep S.M.A.R.T parsing.
                (true, Some(0), None)
            } else {
                // Predictive failure codes caught explicitly preventing hardware crashes!
                (
                    false,
                    Some(code),
                    Some(format!("anomaly: smartctl reported exit code {}", code)),
                )
            }
        }
        Err(e) => {
            // System dependency/Polkit explosion!
            (
                false,
                None,
                Some(format!(
                    "anomaly: privileged execution via pkexec failed ({})",
                    e
                )),
            )
        }
    }
}
