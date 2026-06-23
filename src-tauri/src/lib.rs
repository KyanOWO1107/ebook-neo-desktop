mod manifest;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            manifest::cancel_download,
            manifest::check_rclone_remote,
            manifest::download_selected,
            manifest::load_manifest,
            manifest::load_settings,
            manifest::open_download_root,
            manifest::save_settings,
            manifest::scan_sync_plan,
            manifest::start_download,
            manifest::update_manifest_from_git
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
