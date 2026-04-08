use reqwest::Client;
use std::time::Duration;

/// Initiates standard asynchronous JSON push to Google Gemini 3.1 Pro endpoint.
/// Enforces strong 60 second timeouts blocking hung LLM servers gracefully.
pub async fn invoke_gemini(api_key: &str, system_prompt: &str, payload_json: &str) -> Option<String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap();

    let list_url = format!("https://generativelanguage.googleapis.com/v1beta/models?key={}", api_key);
    let mut chosen_model_path = "models/gemini-1.5-pro".to_string(); // Ultimate fallback
    
    // Natively fetch dynamic Deepmind model matrix
    if let Ok(res) = client.get(&list_url).send().await {
        if let Ok(json) = res.json::<serde_json::Value>().await {
            if let Some(models) = json["models"].as_array() {
                let mut highest_tier = 0;
                for m in models {
                    if let Some(name) = m["name"].as_str() {
                        let tier = if name.contains("gemini-3.1-pro") { 4 }
                            else if name.contains("gemini-3.1") { 3 }
                            else if name.contains("gemini-1.5-pro") { 2 }
                            else if name.contains("gemini-pro") { 1 }
                            else { 0 };
                            
                        if tier > highest_tier {
                            highest_tier = tier;
                            chosen_model_path = name.to_string();
                        }
                    }
                }
            }
        }
    }

    let url = format!("https://generativelanguage.googleapis.com/v1beta/{}:generateContent?key={}", chosen_model_path, api_key);
    
    // Natively structure Google's strict JSON POST syntax mapping Context Schema correctly.
    let request_body = serde_json::json!({
        "system_instruction": {
            "parts": [{ "text": system_prompt }]
        },
        "contents": [{
            "parts": [{ "text": payload_json }],
            "role": "user"
        }]
    });

    let resp = match client.post(&url).json(&request_body).send().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("⚠️ CRITICAL ERROR: Network mapping failed trying to reach Deepmind. Ensure internet connectivity natively. {}", e);
            return None;
        }
    };

    if !resp.status().is_success() {
        let status = resp.status();
        let err_text = resp.text().await.unwrap_or_else(|_| "Unknown Google Endpoint Error".to_string());
        eprintln!("❌ Gemini API strongly rejected the request! HTTP {}\nDetails: {}", status, err_text);
        return None;
    }

    let json_resp: serde_json::Value = resp.json().await.ok()?;
    
    // Safely extract the raw Markdown string navigating safely through Deepmind's JSON topology!
    json_resp["candidates"][0]["content"]["parts"][0]["text"].as_str().map(|s| s.to_string())
}