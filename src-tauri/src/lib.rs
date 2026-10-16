pub mod domain;
pub mod platform;
pub mod providers;
pub mod services;
pub mod state;
pub mod storage;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            use tauri::Manager;
            let services = state::persistent_services()?;
            let accelerator = services
                .load_settings()
                .map_err(|_| "无法读取设置")?
                .shortcut;
            app.manage(services);
            app.manage(std::sync::Arc::new(services::ocr::OcrService::new(
                std::sync::Arc::new(platform::screen_capture::SystemScreenCapture),
                std::sync::Arc::new(platform::windows_ocr::WindowsOcrEngine),
            )));
            let handle = app.handle().clone();
            let shortcuts = platform::shortcut::ShortcutController::new(
                std::sync::Arc::new(platform::shortcut::SystemGlobalShortcut(handle.clone())),
                std::sync::Arc::new(move || platform::tray::request_screenshot(&handle)),
            );
            // Keep the app usable when another application owns the shortcut; settings displays the error.
            let _ = shortcuts.apply(&accelerator, || Ok(()));
            app.manage(shortcuts);
            platform::tray::install(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::translation::translate_text,
            commands::translation::list_languages,
            commands::translation::list_providers,
            commands::translation::get_translation_selection,
            commands::translation::save_translation_selection,
            commands::clipboard::write_clipboard,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::save_provider_config,
            commands::settings::set_provider_secret,
            commands::settings::delete_provider_secret,
            commands::history::list_history,
            commands::history::delete_history_entry,
            commands::history::clear_history,
            commands::screenshot::begin_screenshot_translation,
            commands::screenshot::get_screenshot_display,
            commands::screenshot::recognize_screenshot_region,
            commands::screenshot::cancel_screenshot_translation
        ])
        .run(tauri::generate_context!())
        .expect("error while running Quick Translate");
}
pub mod commands;
