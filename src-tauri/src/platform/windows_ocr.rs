use crate::{
    domain::LanguageCode,
    platform::screen_capture::ImageRegion,
    services::ocr::{OcrBlock, OcrEngine, OcrError, OcrResult},
};

pub struct WindowsOcrEngine;

#[cfg(windows)]
impl OcrEngine for WindowsOcrEngine {
    fn recognize(
        &self,
        image: ImageRegion,
        source_language: Option<&LanguageCode>,
    ) -> Result<OcrResult, OcrError> {
        use std::time::{Duration, Instant};
        use windows::{
            core::{Interface, HSTRING},
            Globalization::Language,
            Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap},
            Media::Ocr::OcrEngine as NativeOcrEngine,
            Storage::Streams::DataWriter,
        };
        let engine = match source_language {
            Some(code) => {
                let language = Language::CreateLanguage(&HSTRING::from(code.as_str()))
                    .map_err(|_| OcrError::LanguageUnavailable)?;
                NativeOcrEngine::TryCreateFromLanguage(&language)
                    .map_err(|_| OcrError::LanguageUnavailable)?
            }
            None => NativeOcrEngine::TryCreateFromUserProfileLanguages()
                .map_err(|_| OcrError::LanguageUnavailable)?,
        };
        if engine.as_raw().is_null() {
            return Err(OcrError::LanguageUnavailable);
        }
        let writer = DataWriter::new().map_err(|_| OcrError::Engine)?;
        writer
            .WriteBytes(&image.rgba)
            .map_err(|_| OcrError::Engine)?;
        let buffer = writer.DetachBuffer().map_err(|_| OcrError::Engine)?;
        let bitmap = SoftwareBitmap::CreateCopyFromBuffer(
            &buffer,
            BitmapPixelFormat::Rgba8,
            image.width as i32,
            image.height as i32,
        )
        .map_err(|_| OcrError::Engine)?;
        let operation = engine
            .RecognizeAsync(&bitmap)
            .map_err(|_| OcrError::Engine)?;
        let deadline = Instant::now() + Duration::from_secs(10);
        while operation.Status().map_err(|_| OcrError::Engine)?.0 == 0 {
            if Instant::now() >= deadline {
                let _ = operation.Cancel();
                return Err(OcrError::Timeout);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let recognized = operation.GetResults().map_err(|_| OcrError::Engine)?;
        let mut blocks = Vec::new();
        let lines = recognized.Lines().map_err(|_| OcrError::Engine)?;
        for i in 0..lines.Size().map_err(|_| OcrError::Engine)? {
            let line = lines.GetAt(i).map_err(|_| OcrError::Engine)?;
            let words = line.Words().map_err(|_| OcrError::Engine)?;
            for j in 0..words.Size().map_err(|_| OcrError::Engine)? {
                let word = words.GetAt(j).map_err(|_| OcrError::Engine)?;
                let bounds = word.BoundingRect().map_err(|_| OcrError::Engine)?;
                blocks.push(OcrBlock {
                    text: word.Text().map_err(|_| OcrError::Engine)?.to_string(),
                    confidence: None,
                    bounds: crate::platform::screen_capture::ScreenshotRect {
                        x: bounds.X.max(0.0) as u32,
                        y: bounds.Y.max(0.0) as u32,
                        width: bounds.Width.max(0.0).ceil() as u32,
                        height: bounds.Height.max(0.0).ceil() as u32,
                    },
                });
            }
        }
        Ok(OcrResult {
            text: recognized.Text().map_err(|_| OcrError::Engine)?.to_string(),
            confidence: None,
            blocks,
        })
    }
}

#[cfg(not(windows))]
impl OcrEngine for WindowsOcrEngine {
    fn recognize(
        &self,
        _image: ImageRegion,
        _source_language: Option<&LanguageCode>,
    ) -> Result<OcrResult, OcrError> {
        Err(OcrError::Engine)
    }
}
