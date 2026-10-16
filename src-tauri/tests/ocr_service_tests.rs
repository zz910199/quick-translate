use quick_translate_lib::{
    domain::LanguageCode,
    platform::screen_capture::{
        CaptureError, CapturedDisplay, ImageRegion, ScreenCapture, ScreenshotRect,
    },
    services::ocr::{OcrBlock, OcrEngine, OcrError, OcrResult, OcrService},
};
use std::sync::Arc;

struct FakeCapture;
impl ScreenCapture for FakeCapture {
    fn capture_all_displays(&self) -> Result<Vec<CapturedDisplay>, CaptureError> {
        Ok(vec![CapturedDisplay {
            id: "monitor-1".into(),
            origin_x: -100,
            origin_y: 20,
            scale_factor: 1.5,
            preview_data_url: "data:image/jpeg;base64,AA==".into(),
            width: 3,
            height: 2,
            rgba: vec![
                10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255, 100, 110, 120, 255, 130, 140,
                150, 255, 160, 170, 180, 255,
            ],
        }])
    }
}
struct FakeOcr(Result<OcrResult, OcrError>);
impl OcrEngine for FakeOcr {
    fn recognize(
        &self,
        image: ImageRegion,
        language: Option<&LanguageCode>,
    ) -> Result<OcrResult, OcrError> {
        assert_eq!((image.width, image.height), (1, 1));
        assert_eq!(image.rgba, [130, 140, 150, 255]);
        assert_eq!(language.map(LanguageCode::as_str), Some("en"));
        self.0.clone()
    }
}
fn service(result: Result<OcrResult, OcrError>) -> OcrService {
    OcrService::new(Arc::new(FakeCapture), Arc::new(FakeOcr(result)))
}
fn run(result: Result<OcrResult, OcrError>) -> Result<OcrResult, OcrError> {
    let service = service(result);
    let displays = service.begin().unwrap();
    assert_eq!(displays.len(), 1);
    assert_eq!(displays[0].origin_x, -100);
    let result = service.recognize(
        "monitor-1",
        ScreenshotRect {
            x: 1,
            y: 1,
            width: 1,
            height: 1,
        },
        Some(&"en".parse().unwrap()),
    );
    assert_eq!(
        service
            .recognize(
                "monitor-1",
                ScreenshotRect {
                    x: 1,
                    y: 1,
                    width: 1,
                    height: 1
                },
                None
            )
            .unwrap_err(),
        OcrError::CaptureUnavailable
    );
    result
}
#[test]
fn ocr_service_tests_returns_text_and_block_bounds() {
    let expected = OcrResult {
        text: "hello".into(),
        confidence: Some(0.8),
        blocks: vec![OcrBlock {
            text: "hello".into(),
            confidence: Some(0.8),
            bounds: ScreenshotRect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
        }],
    };
    let actual = run(Ok(expected)).unwrap();
    assert_eq!(actual.text, "hello");
    assert_eq!(actual.blocks[0].bounds.width, 1);
}
#[test]
fn ocr_service_tests_maps_whitespace_to_empty_result() {
    assert_eq!(
        run(Ok(OcrResult {
            text: " \n ".into(),
            confidence: None,
            blocks: vec![]
        }))
        .unwrap_err(),
        OcrError::EmptyResult
    );
}
#[test]
fn ocr_service_tests_preserves_language_unavailable() {
    assert_eq!(
        run(Err(OcrError::LanguageUnavailable)).unwrap_err(),
        OcrError::LanguageUnavailable
    );
}
#[test]
fn ocr_service_tests_preserves_timeout() {
    assert_eq!(run(Err(OcrError::Timeout)).unwrap_err(), OcrError::Timeout);
}
#[test]
fn ocr_service_tests_rejects_invalid_rect_and_releases_capture() {
    let service = service(Err(OcrError::Engine));
    service.begin().unwrap();
    assert_eq!(
        service
            .recognize(
                "monitor-1",
                ScreenshotRect {
                    x: 2,
                    y: 1,
                    width: 2,
                    height: 1
                },
                None
            )
            .unwrap_err(),
        OcrError::InvalidRegion
    );
    assert_eq!(
        service
            .recognize(
                "monitor-1",
                ScreenshotRect {
                    x: 1,
                    y: 1,
                    width: 1,
                    height: 1
                },
                None
            )
            .unwrap_err(),
        OcrError::CaptureUnavailable
    );
}
