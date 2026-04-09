use ollama_rs::generation::completion::request::GenerationRequest;
use ollama_rs::Ollama;

/// Fires an isolated, strictly-offline completion payload dynamically resolving against localhost endpoints natively.
/// Returns identical Option<String> structural payload like Gemini API driver.
pub async fn invoke_ollama(model: &str, system_prompt: &str, payload_json: &str) -> Option<String> {
    println!(
        "🔌 Dispatching payload natively to local Ollama Engine [{}]...",
        model
    );

    let ollama = Ollama::default();

    use ollama_rs::generation::options::GenerationOptions;

    let req = GenerationRequest::new(model.to_string(), payload_json.to_string())
        .system(system_prompt.to_string())
        .options(GenerationOptions::default().num_ctx(8192));

    match ollama.generate(req).await {
        Ok(res) => Some(res.response),
        Err(e) => {
            eprintln!("❌ CRITICAL ERROR: Ollama Offline Engine failed natively: {}. Ensure your active model '{}' is dynamically running correctly.", e, model);
            None
        }
    }
}
