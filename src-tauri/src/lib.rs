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
            app.manage(std::sync::Arc::new(services::ocr::OcrService::new(
                std::sync::Arc::new(platform::screen_capture::SystemScreenCapture),
                std::sync::Arc::new(platform::windows_ocr::WindowsOcrEngine),
            )));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::translation::translate_text,
            commands::translation::list_languages,
            commands::translation::list_providers,
            commands::translation::get_translation_selection,
            commands::translation::save_translation_selection,
            commands::clipboard::write_clipboard,
            commands::screenshot::begin_screenshot_translation,
            commands::screenshot::get_screenshot_display,
            commands::screenshot::recognize_screenshot_region,
            commands::screenshot::cancel_screenshot_translation
        ])
        .run(tauri::generate_context!())
        .expect("error while running Quick Translate");
}
pub mod commands;
