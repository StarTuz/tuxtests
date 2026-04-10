mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::update_config,
            commands::get_payload,
            commands::analyze_payload
        ])
        .run(tauri::generate_context!())
        .expect("error while running TuxTests Tauri application");
}
