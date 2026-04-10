use tuxtests::ai::config::AppConfig;
use tuxtests::engine::ConfigUpdate;
use tuxtests::models::TuxPayload;

#[tauri::command]
pub fn get_config() -> AppConfig {
    tuxtests::engine::load_config()
}

#[tauri::command]
pub fn update_config(
    provider: String,
    ollama_model: String,
    ollama_url: String,
) -> Result<AppConfig, String> {
    tuxtests::engine::apply_config_update(ConfigUpdate {
        provider: Some(provider),
        ollama_model: Some(ollama_model),
        ollama_url: Some(ollama_url),
    })
}

#[tauri::command]
pub async fn get_payload(full_bench: bool) -> Result<TuxPayload, String> {
    tauri::async_runtime::spawn_blocking(move || tuxtests::engine::collect_payload(full_bench))
        .await
        .map_err(|err| format!("payload collection task failed: {err}"))
}

#[tauri::command]
pub async fn analyze_payload(payload: TuxPayload) -> Result<String, String> {
    tuxtests::engine::analyze_payload_quiet(&payload).await
}
