use crate::{
    domain::LanguageCode,
    platform::screen_capture::{CapturedDisplay, ImageRegion, ScreenCapture, ScreenshotRect},
};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
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
    Cancelled,
    Engine,
}

pub trait OcrEngine: Send + Sync {
    fn recognize(
        &self,
        image: ImageRegion,
        source_language: Option<&LanguageCode>,
    ) -> Result<OcrResult, OcrError>;

    fn recognize_with_cancel(
        &self,
        image: ImageRegion,
        source_language: Option<&LanguageCode>,
        _cancelled: &AtomicBool,
    ) -> Result<OcrResult, OcrError> {
        self.recognize(image, source_language)
    }
}

struct CaptureSession {
    generation: u64,
    frames: HashMap<String, CapturedDisplay>,
    cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
struct CaptureState {
    generation: u64,
    session: Option<CaptureSession>,
    finished_generation: Option<u64>,
}

pub struct OcrService {
    capture: Arc<dyn ScreenCapture>,
    engine: Arc<dyn OcrEngine>,
    state: Mutex<CaptureState>,
}

impl OcrService {
    pub fn new(capture: Arc<dyn ScreenCapture>, engine: Arc<dyn OcrEngine>) -> Self {
        Self {
            capture,
            engine,
            state: Mutex::new(CaptureState::default()),
        }
    }

    pub fn begin(&self) -> Result<Vec<CapturedDisplay>, OcrError> {
        self.begin_with_preparation(|| Ok(()))
    }

    pub fn begin_with_preparation(
        &self,
        prepare: impl FnOnce() -> Result<(), OcrError>,
    ) -> Result<Vec<CapturedDisplay>, OcrError> {
        let generation = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| OcrError::CaptureUnavailable)?;
            if let Some(old) = state.session.take() {
                old.cancelled.store(true, Ordering::Release);
            }
            state.generation = state.generation.wrapping_add(1);
            state.finished_generation = None;
            let generation = state.generation;
            state.session = Some(CaptureSession {
                generation,
                frames: HashMap::new(),
                cancelled: Arc::new(AtomicBool::new(false)),
            });
            generation
        };
        if let Err(error) = prepare() {
            self.cancel_generation(generation);
            return Err(error);
        }
        let displays = match self.capture.capture_all_displays() {
            Ok(displays) => displays,
            Err(_) => {
                self.cancel_generation(generation);
                return Err(OcrError::CaptureUnavailable);
            }
        };
        if displays.is_empty() {
            self.cancel_generation(generation);
            return Err(OcrError::CaptureUnavailable);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| OcrError::CaptureUnavailable)?;
        let session = state
            .session
            .as_mut()
            .filter(|s| s.generation == generation)
            .ok_or(OcrError::Cancelled)?;
        let displays: Vec<_> = displays
            .into_iter()
            .map(|mut d| {
                d.id = format!("s{generation}--{}", d.id);
                d
            })
            .collect();
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
        session.frames = displays.into_iter().map(|d| (d.id.clone(), d)).collect();
        Ok(previews)
    }

    pub fn display(&self, id: &str) -> Result<CapturedDisplay, OcrError> {
        let state = self
            .state
            .lock()
            .map_err(|_| OcrError::CaptureUnavailable)?;
        let d = state
            .session
            .as_ref()
            .and_then(|s| s.frames.get(id))
            .ok_or(OcrError::CaptureUnavailable)?;
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
        let (frame, cancelled) = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| OcrError::CaptureUnavailable)?;
            let session = state.session.as_mut().ok_or(OcrError::CaptureUnavailable)?;
            let selected = session
                .frames
                .remove(display_id)
                .ok_or(OcrError::CaptureUnavailable)?;
            session.frames.clear();
            (selected, Arc::clone(&session.cancelled))
        };
        let image = crop(frame, rect)?;
        if cancelled.load(Ordering::Acquire) {
            return Err(OcrError::Cancelled);
        }
        let result = self
            .engine
            .recognize_with_cancel(image, source_language, &cancelled);
        if cancelled.load(Ordering::Acquire) {
            return Err(OcrError::Cancelled);
        }
        let mut result = result?;
        if result.text.trim().is_empty() {
            return Err(OcrError::EmptyResult);
        }
        result.text = result.text.trim().to_owned();
        Ok(result)
    }

    pub fn cancel(&self) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(session) = state.session.take() {
                session.cancelled.store(true, Ordering::Release);
            }
        }
    }

    fn cancel_generation(&self, generation: u64) {
        if let Ok(mut state) = self.state.lock() {
            if state
                .session
                .as_ref()
                .is_some_and(|s| s.generation == generation)
            {
                if let Some(session) = state.session.take() {
                    session.cancelled.store(true, Ordering::Release);
                }
            }
        }
    }

    pub fn cancel_session(&self, display_id: &str) -> bool {
        if let Ok(mut state) = self.state.lock() {
            if state
                .session
                .as_ref()
                .is_some_and(|s| display_id.starts_with(&format!("s{}--", s.generation)))
            {
                if let Some(session) = state.session.take() {
                    session.cancelled.store(true, Ordering::Release);
                }
                return true;
            }
        }
        false
    }

    pub fn finish(&self, display_id: &str) -> bool {
        if let Ok(mut state) = self.state.lock() {
            if state
                .session
                .as_ref()
                .is_some_and(|s| display_id.starts_with(&format!("s{}--", s.generation)))
            {
                if let Some(session) = state.session.take() {
                    session.cancelled.store(true, Ordering::Release);
                    state.finished_generation = Some(session.generation);
                }
                return true;
            }
        }
        false
    }

    /// Startup may return its display IDs after OCR already completed on an early overlay.
    pub fn is_finished(&self, display_id: &str) -> bool {
        self.state.lock().is_ok_and(|state| {
            state
                .finished_generation
                .is_some_and(|generation| display_id.starts_with(&format!("s{generation}--")))
        })
    }

    pub fn is_current(&self, display_id: &str) -> bool {
        self.state.lock().is_ok_and(|state| {
            state
                .session
                .as_ref()
                .is_some_and(|s| display_id.starts_with(&format!("s{}--", s.generation)))
        })
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
