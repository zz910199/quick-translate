pub mod domain;
pub mod platform;
pub mod providers;
pub mod services;
pub mod state;
pub mod storage;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            use tauri::Manager;
            app.manage(state::persistent_services()?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::translation::translate_text,
            commands::translation::list_languages,
            commands::translation::list_providers,
            commands::translation::get_translation_selection,
            commands::translation::save_translation_selection,
            commands::clipboard::write_clipboard
        ])
        .run(tauri::generate_context!())
        .expect("error while running Quick Translate");
}
pub mod commands;
