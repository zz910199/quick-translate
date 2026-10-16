use super::translation::{AppErrorDto, AppServices};
use tauri::State;
#[tauri::command]
pub fn write_clipboard(state: State<'_, AppServices>, text: String) -> Result<(), AppErrorDto> {
    state
        .clipboard
        .write_text(&text)
        .map_err(|_| AppErrorDto::new("clipboard", "无法写入剪贴板，请重试。", true))
}
