use crate::ai::{config, gemini, ollama};
use crate::models::TuxPayload;

/// Inherited verbatim from GEMINI.md Prompt Schema!
const SYSTEM_PROMPT: &str = "You are an expert Linux diagnostics agent. Analyze the provided JSON representing a Linux machine's hardware layout. Identify specific bottlenecks (e.g., drives at >90% capacity, high-speed SSDs bottlenecked by physical USB 2.0 connections) and provide 3 concrete, actionable upgrade or mitigation suggestions. Format output strictly in Markdown.";

/// Main AI routing module handling data serialization.
pub async fn run_analysis(payload: &TuxPayload) {
    let config = config::AppConfig::load();
    let payload_str = serde_json::to_string_pretty(payload)
        .expect("Critically failed to mathematically stringify TuxPayload models");


    let output = if config.provider == "gemini" {
        if let Some(key) = config::AppConfig::get_gemini_key() {
            gemini::invoke_gemini(&key, SYSTEM_PROMPT, &payload_str).await
        } else {
            eprintln!("CRITICAL ERROR: Gemini API key natively blocked or missing from Security/Secret Service ring. Run `--set-gemini-key`.");
            return;
        }
    } else if config.provider == "ollama" {
        ollama::invoke_ollama(
            &config.ollama_model,
            &format!("{}\n\n{}", SYSTEM_PROMPT, payload_str),
        )
        .await
    } else {
        eprintln!(
            "❌ Erroneous Provider configuration natively trapped. Switch provider logic via CLI."
        );
        return;
    };

    match output {
        Some(markdown) => println!("\n============= AI BOTTLENECK ANALYSIS =============\n\n{}\n\n==================================================", markdown),
        None => eprintln!("Failed to securely route network inference via {} engine!", config.provider),
    }
}
