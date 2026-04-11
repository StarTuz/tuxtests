use clap::Parser;
use tuxtests::{ai, engine};

/// TuxTests: Linux Hardware & Drive Intelligence Tool
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Launch the Ratatui terminal dashboard
    #[arg(long)]
    tui: bool,

    /// Perform full LLM analysis on hardware
    #[arg(short, long)]
    analyze: bool,

    /// Trigger deeper SMART tracking and non-destructive benchmarking
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

    /// Print the normalized runtime configuration as JSON and exit.
    #[arg(long)]
    print_config: bool,

    /// Emit the collected hardware payload as JSON instead of running AI analysis.
    #[arg(long)]
    dump_payload: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if args.print_config {
        match engine::config_json() {
            Ok(config_json) => println!("{}", config_json),
            Err(err) => eprintln!("❌ Failed to serialize active configuration: {}", err),
        }
        return;
    }

    // AI Persistence Configuration Setup
    if args.set_llm_provider.is_some()
        || args.set_ollama_model.is_some()
        || args.set_ollama_url.is_some()
    {
        let update = engine::ConfigUpdate {
            provider: args.set_llm_provider,
            ollama_model: args.set_ollama_model,
            ollama_url: args.set_ollama_url,
        };

        match engine::apply_config_update(update) {
            Ok(config) => println!(
                "⚙️ TuxTests AI Configuration updated: provider={}, ollama_model={}, ollama_url={}",
                config.provider, config.ollama_model, config.ollama_url
            ),
            Err(err) => eprintln!("❌ {}", err),
        }
        return;
    }

    // Keyring Ingestion
    if let Some(key) = args.set_gemini_key {
        eprintln!("🔑 Attempting to secure Gemini API key inside native credential vault...");
        match keyring::Entry::new("tuxtests", "gemini_api") {
            Ok(entry) => {
                if entry.set_password(&key).is_ok() {
                    // Provide automatic frictionless switching!
                    let mut config = engine::load_config();
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
        eprintln!(
            "🛠️ Injecting Mock Regression Fixture directly into AI Analyzer: {}",
            mock_file
        );
        let payload = match engine::build_mock_payload(&mock_file) {
            Ok(payload) => payload,
            Err(err) => {
                eprintln!("❌ {}", err);
                return;
            }
        };

        if args.dump_payload {
            print_payload_json(&payload);
        } else {
            // Fire safely to models directly without Polkit
            ai::analyzer::run_analysis(&payload).await;
        }
        return;
    }

    if args.tui {
        if let Err(err) = tuxtests::ui::tui::run().await {
            eprintln!("❌ Failed to launch terminal UI: {}", err);
        }
    } else if args.analyze || args.full_bench || args.dump_payload {
        eprintln!("🚀 Initiating TuxTests Hardware Analysis...");
        let payload = engine::collect_payload(args.full_bench);

        if args.dump_payload {
            print_payload_json(&payload);
        } else {
            ai::analyzer::run_analysis(&payload).await;
        }
    } else {
        println!("TuxTests MVP Scaffolding Initialized. Run with `--analyze` or `--full-bench`.");
    }
}

fn print_payload_json(payload: &tuxtests::models::TuxPayload) {
    match engine::payload_json(payload) {
        Ok(json) => println!("{}", json),
        Err(err) => eprintln!("❌ Failed to serialize scan payload: {}", err),
    }
}
