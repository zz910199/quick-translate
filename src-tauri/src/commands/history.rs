use super::translation::{AppErrorDto, AppServices};
use serde::Serialize;
use tauri::State;
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntryDto {
    pub id: i64,
    pub source_text: String,
    pub translated_text: String,
    pub source: String,
    pub target: String,
    pub provider_id: String,
    pub timestamp: String,
}
fn storage_error() -> AppErrorDto {
    AppErrorDto::new("storage", "无法读写历史记录，请重试。", true)
}
impl AppServices {
    pub fn list_history(&self) -> Result<Vec<HistoryEntryDto>, AppErrorDto> {
        Ok(self
            .history
            .list(100)
            .map_err(|_| storage_error())?
            .into_iter()
            .filter_map(|e| {
                Some(HistoryEntryDto {
                    id: e.id?,
                    source_text: e.source_text,
                    translated_text: e.translated_text,
                    source: e.source_language.as_str().into(),
                    target: e.target_language.as_str().into(),
                    provider_id: e.provider_id.as_str().into(),
                    timestamp: e.timestamp.to_rfc3339(),
                })
            })
            .collect())
    }
    pub fn delete_history_entry(&self, id: i64) -> Result<(), AppErrorDto> {
        self.history.delete(id).map_err(|_| storage_error())
    }
    pub fn clear_history(&self) -> Result<(), AppErrorDto> {
        self.history.clear().map_err(|_| storage_error())
    }
}
#[tauri::command]
pub fn list_history(state: State<'_, AppServices>) -> Result<Vec<HistoryEntryDto>, AppErrorDto> {
    state.list_history()
}
#[tauri::command]
pub fn delete_history_entry(state: State<'_, AppServices>, id: i64) -> Result<(), AppErrorDto> {
    state.delete_history_entry(id)
}
#[tauri::command]
pub fn clear_history(state: State<'_, AppServices>) -> Result<(), AppErrorDto> {
    state.clear_history()
}
