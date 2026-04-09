use crate::ai::config::AppConfig;
use crate::ai::{config, gemini, ollama};
use crate::models::TuxPayload;

/// Inherited verbatim from GEMINI.md Prompt Schema!
const SYSTEM_PROMPT: &str = "You are an expert Linux diagnostics agent. Analyze the provided JSON representing a Linux machine's hardware layout. Identify specific bottlenecks (e.g., drives at >90% capacity, high-speed SSDs bottlenecked by physical USB 2.0 connections) and provide 3 concrete, actionable upgrade or mitigation suggestions. Format output strictly in Markdown.";

#[derive(Debug, Clone, PartialEq, Eq)]
enum AnalysisTarget {
    Gemini,
    Ollama { model: String, url: String },
}

/// Main AI routing module handling data serialization.
pub async fn run_analysis(payload: &TuxPayload) {
    let config = config::AppConfig::load();
    let payload_str = serde_json::to_string(payload)
        .expect("Critically failed to mathematically stringify TuxPayload models");

    let analysis_target =
        match resolve_analysis_target(&config, config::AppConfig::get_gemini_key().is_some()) {
            Ok(target) => target,
            Err(err) => {
                eprintln!("{}", err);
                return;
            }
        };

    if payload.drives.is_empty() {
        eprintln!(
            "⚠️ No drives were discovered in the current scan payload. AI analysis may be incomplete."
        );
    }

    let output = match &analysis_target {
        AnalysisTarget::Gemini => {
            let key = config::AppConfig::get_gemini_key().expect("Gemini key should exist");
            gemini::invoke_gemini(&key, SYSTEM_PROMPT, &payload_str).await
        }
        AnalysisTarget::Ollama { model, url } => {
            eprintln!(
                "ℹ️ Using Ollama provider with model '{}' at {}.",
                model, url
            );
            ollama::invoke_ollama(url, model, SYSTEM_PROMPT, &payload_str).await
        }
    };

    match output {
        Some(markdown) => println!("\n============= AI BOTTLENECK ANALYSIS =============\n\n{}\n\n==================================================", markdown),
        None => eprintln!(
            "❌ Failed to route inference through the '{}' provider. Check provider-specific diagnostics above.",
            provider_name(&analysis_target)
        ),
    }
}

fn resolve_analysis_target(
    config: &AppConfig,
    gemini_key_available: bool,
) -> Result<AnalysisTarget, String> {
    let provider = config::normalize_provider(&config.provider).map_err(|err| {
        format!(
            "❌ Invalid provider configuration '{}': {}. Use `tuxtests --set-llm-provider <gemini|ollama>` to repair it.",
            config.provider, err
        )
    })?;

    match provider.as_str() {
        "gemini" => {
            if gemini_key_available {
                Ok(AnalysisTarget::Gemini)
            } else {
                Err("❌ Gemini API key is missing from the system keyring. Run `tuxtests --set-gemini-key \"YOUR_KEY_HERE\"` first.".to_string())
            }
        }
        "ollama" => Ok(AnalysisTarget::Ollama {
            model: config.ollama_model.clone(),
            url: config.ollama_url.clone(),
        }),
        _ => unreachable!(),
    }
}

fn provider_name(target: &AnalysisTarget) -> &'static str {
    match target {
        AnalysisTarget::Gemini => "gemini",
        AnalysisTarget::Ollama { .. } => "ollama",
    }
}

#[cfg(test)]
mod tests {
    use super::{provider_name, resolve_analysis_target, AnalysisTarget};
    use crate::ai::config::AppConfig;

    fn config(provider: &str) -> AppConfig {
        AppConfig {
            provider: provider.to_string(),
            ollama_model: "mistral".to_string(),
            ollama_url: "http://127.0.0.1:11434".to_string(),
        }
    }

    #[test]
    fn selects_gemini_when_key_exists() {
        let target = resolve_analysis_target(&config("gemini"), true).unwrap();
        assert_eq!(target, AnalysisTarget::Gemini);
        assert_eq!(provider_name(&target), "gemini");
    }

    #[test]
    fn rejects_gemini_when_key_is_missing() {
        let err = resolve_analysis_target(&config("gemini"), false).unwrap_err();
        assert!(err.contains("Gemini API key is missing"));
    }

    #[test]
    fn selects_ollama_with_model_and_url() {
        let target = resolve_analysis_target(&config("ollama"), false).unwrap();
        assert_eq!(
            target,
            AnalysisTarget::Ollama {
                model: "mistral".to_string(),
                url: "http://127.0.0.1:11434".to_string(),
            }
        );
        assert_eq!(provider_name(&target), "ollama");
    }

    #[test]
    fn rejects_invalid_provider_configuration() {
        let err = resolve_analysis_target(&config("bad-provider"), true).unwrap_err();
        assert!(err.contains("Invalid provider configuration"));
    }
}
