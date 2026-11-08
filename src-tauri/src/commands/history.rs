use super::translation::{AppErrorDto, AppServices};
use crate::storage::HistoryEntry;
use serde::Serialize;
use tauri::State;
#[derive(Debug, Serialize)]
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
/// Entries are always read from storage with an id, so a missing one means the row is gone.
pub fn history_dto(entry: HistoryEntry) -> HistoryEntryDto {
    HistoryEntryDto {
        id: entry.id.unwrap_or_default(),
        source_text: entry.source_text,
        translated_text: entry.translated_text,
        source: entry.source_language.as_str().into(),
        target: entry.target_language.as_str().into(),
        provider_id: entry.provider_id.as_str().into(),
        timestamp: entry.timestamp.to_rfc3339(),
    }
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
            .map(history_dto)
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
#[tauri::command]
pub async fn retranslate_history_entry(
    state: State<'_, AppServices>,
    id: i64,
    source_text: Option<String>,
) -> Result<HistoryEntryDto, AppErrorDto> {
    state.retranslate_history_entry(id, source_text).await
}
