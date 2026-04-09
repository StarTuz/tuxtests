use clap::Parser;
use tuxtests::{ai, bench, hardware, models};

/// TuxTests: Linux Hardware & Drive Intelligence Tool
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Perform full LLM analysis on hardware
    #[arg(short, long)]
    analyze: bool,

    /// Trigger root Polkit privileges for deep SMART tracking & destructive benchmarking
    #[arg(long)]
    full_bench: bool,

    /// Set the LLM provider (gemini or ollama)
    #[arg(long)]
    set_llm_provider: Option<String>,

    /// Securely set the Gemini API Key
    #[arg(long)]
    set_gemini_key: Option<String>,

    /// Set the local Ollama API url
    #[arg(long)]
    set_ollama_url: Option<String>,

    /// Specifically target the physical offline model executing natively (defaults to `mistral`)
    #[arg(long)]
    set_ollama_model: Option<String>,

    /// Supply a mock JSON fixture for pure logic AI processing isolated from hardware.
    #[arg(long)]
    mock: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // AI Persistence Configuration Setup
    if args.set_llm_provider.is_some() || args.set_ollama_model.is_some() {
        let mut config = ai::config::AppConfig::load();
        if let Some(prov) = args.set_llm_provider {
            config.provider = prov;
        }
        if let Some(model) = args.set_ollama_model {
            config.ollama_model = model;
        }
        config.save();
        println!(
            "⚙️ TuxTests AI Configuration natively flushed into ~/.config/tuxtests/config.toml!"
        );
        return;
    }

    // Keyring Ingestion
    if let Some(key) = args.set_gemini_key {
        println!("🔑 Attempting to secure Gemini API key inside native credential vault...");
        match keyring::Entry::new("tuxtests", "gemini_api") {
            Ok(entry) => {
                if entry.set_password(&key).is_ok() {
                    // Provide automatic frictionless switching!
                    let mut config = ai::config::AppConfig::load();
                    if config.provider != "gemini" {
                        config.provider = "gemini".to_string();
                        config.save();
                    }
                    println!("✅ Key securely vaulted & Provider switched to Cloud Gemini! You can now run `--analyze`.");
                } else {
                    println!("❌ Failed to write to Secret Service. You may not have a dbus agent running.");
                }
            }
            Err(e) => println!("❌ Keyring failed to initialize: {}", e),
        }
        return;
    }

    // Mock Offline Mode
    if let Some(mock_file) = args.mock {
        println!(
            "🛠️ Injecting Mock Regression Fixture directly into AI Analyzer: {}",
            mock_file
        );
        let content = std::fs::read_to_string(&mock_file)
            .unwrap_or_else(|_| panic!("Failed to read mock file natively at: {}", mock_file));

        let mocked_drive: models::DriveInfo = serde_json::from_str(&content)
            .expect("Mock fixture physically deviated from the strict DriveInfo map!");

        let payload = models::TuxPayload {
            system: models::SystemInfo {
                cpu: "Mock Sandbox CPU (Threadripper Stub)".to_string(),
                ram_gb: 128,
            },
            drives: vec![mocked_drive],
            benchmarks: std::collections::BTreeMap::new(),
            kernel_anomalies: vec![
                "mock anomaly: High predictive failure counts on dummy payload".to_string(),
            ],
        };

        // Fire safely to models directly without Polkit
        ai::analyzer::run_analysis(&payload).await;
        return;
    }

    if args.analyze || args.full_bench {
        println!("🚀 Initiating TuxTests Hardware Analysis...");

        let sys_specs = hardware::system::get_system_specs();
        let mut storage_drives = Vec::new();
        let mut benchmarks = std::collections::BTreeMap::new();
        let mut kernel_anomalies = Vec::new();

        for (mut drive, mount_opt) in hardware::storage::scan_drives() {
            if args.full_bench {
                println!(
                    "🔒 Triggering Privileged Polkit S.M.A.R.T diagnostic on {}...",
                    drive.name
                );
                let (ok, exit, anomaly) = bench::smart::check_health(&drive.name);
                drive.health_ok = ok;
                drive.smartctl_exit_code = exit;
                if let Some(err) = anomaly {
                    kernel_anomalies.push(err);
                }

                if let Some(mount) = mount_opt {
                    if let Some(mb_s) = bench::throughput::run_buffered_bench(&mount) {
                        benchmarks.insert(
                            drive.name.clone(),
                            models::BenchmarkResult { write_mb_s: mb_s },
                        );
                    }
                }
            }

            storage_drives.push(drive);
        }

        let payload = models::TuxPayload {
            system: sys_specs,
            drives: storage_drives,
            benchmarks,
            kernel_anomalies,
        };

        ai::analyzer::run_analysis(&payload).await;
    } else {
        println!("TuxTests MVP Scaffolding Initialized. Run with `--analyze` or `--full-bench`.");
    }
}
