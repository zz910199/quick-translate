use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};

pub fn open_translator(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
/// The main window owns capture IDs and early-event buffering for every entry point.
pub fn request_screenshot(app: &AppHandle) {
    open_translator(app);
    let _ = app.emit_to("main", "screenshot-requested", ());
}
pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "打开翻译器", true, None::<&str>)?;
    let screenshot = MenuItem::with_id(app, "screenshot", "截图翻译", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &screenshot, &quit])?;
    let mut builder = TrayIconBuilder::with_id("quick-translate")
        .tooltip("Quick Translate")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => open_translator(app),
            "screenshot" => request_screenshot(app),
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
