use ollama_rs::Ollama;
use ollama_rs::generation::completion::request::GenerationRequest;

/// Fires an isolated, strictly-offline completion payload dynamically resolving against localhost endpoints natively.
/// Returns identical Option<String> structural payload like Gemini API driver.
pub async fn invoke_ollama(model: &str, prompt: &str) -> Option<String> {
    println!("🔌 Dispatching payload natively to local Ollama Engine [{}]...", model);
    
    // Natively bind to localhost port 11434 bypassing external domains
    let ollama = Ollama::default();
    
    let req = GenerationRequest::new(model.to_string(), prompt.to_string());
    
    match ollama.generate(req).await {
        Ok(res) => Some(res.response),
        Err(e) => {
             eprintln!("❌ CRITICAL ERROR: Ollama Offline Engine failed natively: {}. Ensure your active model '{}' is dynamically running correctly.", e, model);
             None
        }
    }
}