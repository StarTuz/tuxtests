use ollama_rs::generation::completion::request::GenerationRequest;
use ollama_rs::generation::options::GenerationOptions;
use ollama_rs::Ollama;

/// Fires an isolated, strictly-offline completion payload dynamically resolving against localhost endpoints natively.
/// Returns identical Option<String> structural payload like Gemini API driver.
pub async fn invoke_ollama(
    base_url: &str,
    model: &str,
    system_prompt: &str,
    payload_json: &str,
    emit_diagnostics: bool,
) -> Option<String> {
    if emit_diagnostics {
        eprintln!(
            "🔌 Dispatching payload natively to local Ollama Engine [{}] via {}...",
            model, base_url
        );
    }

    let ollama = match Ollama::try_new(base_url) {
        Ok(client) => client,
        Err(e) => {
            if emit_diagnostics {
                eprintln!(
                    "❌ CRITICAL ERROR: Ollama URL '{}' is invalid: {}.",
                    base_url, e
                );
            }
            return None;
        }
    };

    // Explicitly merge instructions and payload as a single user prompt! Model instruct templates (especially Gemma) often fail dynamically handling separate .system() flags due to lacking a native 'system' role.
    let merged_prompt = format!(
        "{}\n\n### HARDWARE PAYLOAD (JSON) ###\n{}",
        system_prompt, payload_json
    );

    let req = GenerationRequest::new(model.to_string(), merged_prompt)
        .options(GenerationOptions::default().num_ctx(16384));

    match ollama.generate(req).await {
        Ok(res) => Some(res.response),
        Err(e) => {
            if emit_diagnostics {
                eprintln!("❌ CRITICAL ERROR: Ollama Offline Engine failed natively: {}. Ensure your active model '{}' is dynamically running correctly.", e, model);
            }
            None
        }
    }
}
