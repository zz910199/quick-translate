use crate::{
    domain::LanguageCode,
    platform::screen_capture::{CapturedDisplay, ImageRegion, ScreenCapture, ScreenshotRect},
};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrBlock {
    pub text: String,
    pub confidence: Option<f32>,
    pub bounds: ScreenshotRect,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrResult {
    pub text: String,
    pub confidence: Option<f32>,
    pub blocks: Vec<OcrBlock>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OcrError {
    EmptyResult,
    LanguageUnavailable,
    Timeout,
    InvalidRegion,
    CaptureUnavailable,
    Engine,
}

pub trait OcrEngine: Send + Sync {
    fn recognize(
        &self,
        image: ImageRegion,
        source_language: Option<&LanguageCode>,
    ) -> Result<OcrResult, OcrError>;
}

pub struct OcrService {
    capture: Arc<dyn ScreenCapture>,
    engine: Arc<dyn OcrEngine>,
    frames: Mutex<HashMap<String, CapturedDisplay>>,
}

impl OcrService {
    pub fn new(capture: Arc<dyn ScreenCapture>, engine: Arc<dyn OcrEngine>) -> Self {
        Self {
            capture,
            engine,
            frames: Mutex::new(HashMap::new()),
        }
    }

    pub fn begin(&self) -> Result<Vec<CapturedDisplay>, OcrError> {
        self.cancel();
        let displays = self
            .capture
            .capture_all_displays()
            .map_err(|_| OcrError::CaptureUnavailable)?;
        if displays.is_empty() {
            return Err(OcrError::CaptureUnavailable);
        }
        let mut frames = self
            .frames
            .lock()
            .map_err(|_| OcrError::CaptureUnavailable)?;
        let previews = displays
            .iter()
            .map(|d| CapturedDisplay {
                id: d.id.clone(),
                origin_x: d.origin_x,
                origin_y: d.origin_y,
                scale_factor: d.scale_factor,
                preview_data_url: d.preview_data_url.clone(),
                width: d.width,
                height: d.height,
                rgba: Vec::new(),
            })
            .collect();
        *frames = displays.into_iter().map(|d| (d.id.clone(), d)).collect();
        Ok(previews)
    }

    pub fn display(&self, id: &str) -> Result<CapturedDisplay, OcrError> {
        let frames = self
            .frames
            .lock()
            .map_err(|_| OcrError::CaptureUnavailable)?;
        let d = frames.get(id).ok_or(OcrError::CaptureUnavailable)?;
        Ok(CapturedDisplay {
            id: d.id.clone(),
            origin_x: d.origin_x,
            origin_y: d.origin_y,
            scale_factor: d.scale_factor,
            preview_data_url: d.preview_data_url.clone(),
            width: d.width,
            height: d.height,
            rgba: Vec::new(),
        })
    }

    pub fn recognize(
        &self,
        display_id: &str,
        rect: ScreenshotRect,
        source_language: Option<&LanguageCode>,
    ) -> Result<OcrResult, OcrError> {
        let frame = {
            let mut frames = self
                .frames
                .lock()
                .map_err(|_| OcrError::CaptureUnavailable)?;
            let selected = frames.remove(display_id);
            frames.clear();
            selected.ok_or(OcrError::CaptureUnavailable)?
        };
        let image = crop(frame, rect)?;
        let mut result = self.engine.recognize(image, source_language)?;
        if result.text.trim().is_empty() {
            return Err(OcrError::EmptyResult);
        }
        result.text = result.text.trim().to_owned();
        Ok(result)
    }

    pub fn cancel(&self) {
        if let Ok(mut frames) = self.frames.lock() {
            frames.clear();
        }
    }
}

fn crop(frame: CapturedDisplay, rect: ScreenshotRect) -> Result<ImageRegion, OcrError> {
    if rect.width == 0
        || rect.height == 0
        || rect
            .x
            .checked_add(rect.width)
            .is_none_or(|end| end > frame.width)
        || rect
            .y
            .checked_add(rect.height)
            .is_none_or(|end| end > frame.height)
    {
        return Err(OcrError::InvalidRegion);
    }
    let row_bytes = usize::try_from(rect.width).map_err(|_| OcrError::InvalidRegion)? * 4;
    let mut rgba = Vec::with_capacity(row_bytes * rect.height as usize);
    for y in rect.y..rect.y + rect.height {
        let start = (y as usize * frame.width as usize + rect.x as usize) * 4;
        let end = start + row_bytes;
        rgba.extend_from_slice(frame.rgba.get(start..end).ok_or(OcrError::InvalidRegion)?);
    }
    Ok(ImageRegion {
        rgba,
        width: rect.width,
        height: rect.height,
    })
}
