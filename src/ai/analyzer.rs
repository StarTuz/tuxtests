use crate::ai::{config, gemini, ollama};
use crate::models::TuxPayload;

/// Inherited verbatim from GEMINI.md Prompt Schema!
const SYSTEM_PROMPT: &str = "You are an expert Linux diagnostics agent. Analyze the provided JSON representing a Linux machine's hardware layout. Identify specific bottlenecks (e.g., drives at >90% capacity, high-speed SSDs bottlenecked by physical USB 2.0 connections) and provide 3 concrete, actionable upgrade or mitigation suggestions. Format output strictly in Markdown.";

/// Main AI routing module handling data serialization.
pub async fn run_analysis(payload: &TuxPayload) {
    let config = config::AppConfig::load();
    let payload_str = serde_json::to_string(payload)
        .expect("Critically failed to mathematically stringify TuxPayload models");

    if payload.drives.is_empty() {
        eprintln!(
            "⚠️ No drives were discovered in the current scan payload. AI analysis may be incomplete."
        );
    }

    let provider = match config::normalize_provider(&config.provider) {
        Ok(provider) => provider,
        Err(err) => {
            eprintln!(
                "❌ Invalid provider configuration '{}': {}. Use `tuxtests --set-llm-provider <gemini|ollama>` to repair it.",
                config.provider, err
            );
            return;
        }
    };

    let output = match provider.as_str() {
        "gemini" => {
            if let Some(key) = config::AppConfig::get_gemini_key() {
                gemini::invoke_gemini(&key, SYSTEM_PROMPT, &payload_str).await
            } else {
                eprintln!(
                    "❌ Gemini API key is missing from the system keyring. Run `tuxtests --set-gemini-key \"YOUR_KEY_HERE\"` first."
                );
                return;
            }
        }
        "ollama" => {
            eprintln!(
                "ℹ️ Using Ollama provider with model '{}' at {}.",
                config.ollama_model, config.ollama_url
            );
            ollama::invoke_ollama(
                &config.ollama_url,
                &config.ollama_model,
                SYSTEM_PROMPT,
                &payload_str,
            )
            .await
        }
        _ => unreachable!(),
    };

    match output {
        Some(markdown) => println!("\n============= AI BOTTLENECK ANALYSIS =============\n\n{}\n\n==================================================", markdown),
        None => eprintln!(
            "❌ Failed to route inference through the '{}' provider. Check provider-specific diagnostics above.",
            provider
        ),
    }
}
