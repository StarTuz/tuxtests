use tuxtests::ai::config::AppConfig;
use tuxtests::models::TuxPayload;

#[tauri::command]
pub fn get_config() -> AppConfig {
    tuxtests::engine::load_config()
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
