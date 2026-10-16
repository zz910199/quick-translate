use crate::{
    commands::translation::{AppErrorDto, AppServices},
    platform::screen_capture::{CapturedDisplay, ScreenshotRect},
    services::ocr::{OcrError, OcrResult, OcrService},
};
use serde::Serialize;
use std::sync::Arc;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, State, WebviewUrl,
    WebviewWindowBuilder,
};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturedDisplayDto {
    pub id: String,
    pub origin_x: i32,
    pub origin_y: i32,
    pub scale_factor: f64,
    pub preview_data_url: String,
    pub width: u32,
    pub height: u32,
}
impl From<CapturedDisplay> for CapturedDisplayDto {
    fn from(d: CapturedDisplay) -> Self {
        Self {
            id: d.id,
            origin_x: d.origin_x,
            origin_y: d.origin_y,
            scale_factor: d.scale_factor,
            preview_data_url: d.preview_data_url,
            width: d.width,
            height: d.height,
        }
    }
}
pub type OcrResultDto = OcrResult;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OcrResultEvent {
    display_id: String,
    #[serde(flatten)]
    result: OcrResult,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OcrErrorEvent {
    display_id: String,
    #[serde(flatten)]
    error: AppErrorDto,
}

fn app_error(error: OcrError) -> AppErrorDto {
    match error {
        OcrError::EmptyResult => {
            AppErrorDto::new("empty_result", "未识别到文字，请重新截图。", true)
        }
        OcrError::LanguageUnavailable => AppErrorDto::new(
            "language_unavailable",
            "未安装所选语言的 Windows OCR 语言包。",
            false,
        ),
        OcrError::Timeout => AppErrorDto::new("ocr_timeout", "文字识别超时，请重试。", true),
        OcrError::InvalidRegion => {
            AppErrorDto::new("invalid_region", "请选择有效的截图区域。", true)
        }
        OcrError::CaptureUnavailable => {
            AppErrorDto::new("capture_unavailable", "无法获取屏幕截图，请重试。", true)
        }
        OcrError::Cancelled => AppErrorDto::new("cancelled", "截图已取消。", true),
        OcrError::Engine => AppErrorDto::new("ocr_error", "文字识别失败，请重试。", true),
    }
}

fn close_session_overlays(app: &AppHandle, display_id: &str) {
    let Some((session, _)) = display_id.split_once("--") else {
        return;
    };
    let prefix = format!("screenshot-{session}--");
    for (label, window) in app.webview_windows() {
        if label.starts_with(&prefix) {
            let _ = window.close();
        }
    }
}

fn close_overlays(app: &AppHandle) {
    for (label, window) in app.webview_windows() {
        if label.starts_with("screenshot-") {
            let _ = window.close();
        }
    }
}

#[tauri::command]
pub async fn begin_screenshot_translation(
    app: AppHandle,
    state: State<'_, Arc<OcrService>>,
) -> Result<Vec<CapturedDisplayDto>, AppErrorDto> {
    close_overlays(&app);
    let service = Arc::clone(state.inner());
    let displays = tauri::async_runtime::spawn_blocking(move || service.begin())
        .await
        .map_err(|_| app_error(OcrError::CaptureUnavailable))?
        .map_err(app_error)?;
    let dtos: Vec<_> = displays.into_iter().map(CapturedDisplayDto::from).collect();
    for display in &dtos {
        if !state.is_current(&display.id) {
            close_session_overlays(&app, &display.id);
            return Err(app_error(OcrError::Cancelled));
        }
        let label = format!("screenshot-{}", display.id);
        let url = WebviewUrl::App(format!("index.html?screenshot={}", display.id).into());
        let window = WebviewWindowBuilder::new(&app, label, url)
            .title("选择截图区域")
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .visible(false)
            .build()
            .map_err(|_| app_error(OcrError::CaptureUnavailable));
        match window {
            Ok(window) => {
                if window
                    .set_position(Position::Physical(PhysicalPosition::new(
                        display.origin_x,
                        display.origin_y,
                    )))
                    .is_err()
                    || window
                        .set_size(Size::Physical(PhysicalSize::new(
                            display.width,
                            display.height,
                        )))
                        .is_err()
                    || window.show().is_err()
                {
                    close_session_overlays(&app, &display.id);
                    state.cancel_session(&display.id);
                    return Err(app_error(OcrError::CaptureUnavailable));
                }
                if !state.is_current(&display.id) {
                    close_session_overlays(&app, &display.id);
                    return Err(app_error(OcrError::Cancelled));
                }
            }
            Err(error) => {
                close_session_overlays(&app, &display.id);
                state.cancel_session(&display.id);
                return Err(error);
            }
        }
    }
    Ok(dtos)
}

#[tauri::command]
pub fn get_screenshot_display(
    state: State<'_, Arc<OcrService>>,
    display_id: String,
) -> Result<CapturedDisplayDto, AppErrorDto> {
    state
        .display(&display_id)
        .map(CapturedDisplayDto::from)
        .map_err(app_error)
}

#[tauri::command]
pub async fn recognize_screenshot_region(
    app: AppHandle,
    state: State<'_, Arc<OcrService>>,
    services: State<'_, AppServices>,
    display_id: String,
    rect: ScreenshotRect,
) -> Result<OcrResultDto, AppErrorDto> {
    let event_display_id = display_id.clone();
    let result = async {
        let language = services
            .get_translation_selection()?
            .source
            .parse()
            .map_err(|_| app_error(OcrError::LanguageUnavailable))?;
        let service = Arc::clone(state.inner());
        tauri::async_runtime::spawn_blocking(move || {
            service.recognize(&display_id, rect, Some(&language))
        })
        .await
        .map_err(|_| app_error(OcrError::Engine))?
        .map_err(app_error)
    }
    .await;
    if state.finish(&event_display_id) {
        match &result {
            Ok(value) => {
                let _ = app.emit_to(
                    "main",
                    "screenshot-ocr-result",
                    OcrResultEvent {
                        display_id: event_display_id.clone(),
                        result: value.clone(),
                    },
                );
            }
            Err(error) => {
                let _ = app.emit_to(
                    "main",
                    "screenshot-ocr-error",
                    OcrErrorEvent {
                        display_id: event_display_id.clone(),
                        error: error.clone(),
                    },
                );
            }
        }
    }
    close_session_overlays(&app, &event_display_id);
    result
}

#[tauri::command]
pub fn cancel_screenshot_translation(
    app: AppHandle,
    state: State<'_, Arc<OcrService>>,
    display_id: String,
) {
    state.cancel_session(&display_id);
    close_session_overlays(&app, &display_id);
}
