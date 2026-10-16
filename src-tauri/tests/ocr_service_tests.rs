use quick_translate_lib::{
    domain::LanguageCode,
    platform::screen_capture::{
        CaptureError, CapturedDisplay, ImageRegion, ScreenCapture, ScreenshotRect,
    },
    services::ocr::{OcrBlock, OcrEngine, OcrError, OcrResult, OcrService},
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::Duration;

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
    let id = &displays[0].id;
    let result = service.recognize(
        id,
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
                id,
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
    let id = service.begin().unwrap().remove(0).id;
    assert_eq!(
        service
            .recognize(
                &id,
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
                &id,
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

struct PausedEngine {
    started: mpsc::Sender<()>,
    release: Mutex<mpsc::Receiver<()>>,
}
impl OcrEngine for PausedEngine {
    fn recognize(
        &self,
        _image: ImageRegion,
        _language: Option<&LanguageCode>,
    ) -> Result<OcrResult, OcrError> {
        self.started.send(()).unwrap();
        self.release.lock().unwrap().recv().unwrap();
        Ok(OcrResult {
            text: "stale text".into(),
            confidence: None,
            blocks: vec![],
        })
    }
}

#[test]
fn ocr_service_tests_discards_old_completion_without_clearing_new_session() {
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let service = Arc::new(OcrService::new(
        Arc::new(FakeCapture),
        Arc::new(PausedEngine {
            started: started_tx,
            release: Mutex::new(release_rx),
        }),
    ));
    let old_id = service.begin().unwrap().remove(0).id;
    let worker_service = service.clone();
    let worker_id = old_id.clone();
    let worker = std::thread::spawn(move || {
        worker_service.recognize(
            &worker_id,
            ScreenshotRect {
                x: 1,
                y: 1,
                width: 1,
                height: 1,
            },
            None,
        )
    });
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(service.cancel_session(&old_id));
    let new_id = service.begin().unwrap().remove(0).id;
    assert_ne!(old_id, new_id);
    assert!(!service.cancel_session(&old_id));
    release_tx.send(()).unwrap();
    assert_eq!(worker.join().unwrap().unwrap_err(), OcrError::Cancelled);
    assert!(!service.finish(&old_id));
    assert!(service.display(&new_id).is_ok());
    assert!(service.cancel_session(&new_id));
    assert_eq!(
        service.display(&new_id).unwrap_err(),
        OcrError::CaptureUnavailable
    );
}

struct CooperativeEngine(mpsc::Sender<()>, AtomicBool);
impl OcrEngine for CooperativeEngine {
    fn recognize(
        &self,
        _image: ImageRegion,
        _language: Option<&LanguageCode>,
    ) -> Result<OcrResult, OcrError> {
        unreachable!()
    }
    fn recognize_with_cancel(
        &self,
        image: ImageRegion,
        _language: Option<&LanguageCode>,
        cancelled: &AtomicBool,
    ) -> Result<OcrResult, OcrError> {
        self.0.send(()).unwrap();
        drop(image);
        while !cancelled.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
        self.1.store(true, Ordering::Release);
        Err(OcrError::Cancelled)
    }
}

#[test]
fn ocr_service_tests_cancellation_reaches_running_engine() {
    let (started_tx, started_rx) = mpsc::channel();
    let engine = Arc::new(CooperativeEngine(started_tx, AtomicBool::new(false)));
    let service = Arc::new(OcrService::new(Arc::new(FakeCapture), engine.clone()));
    let id = service.begin().unwrap().remove(0).id;
    let worker_service = service.clone();
    let worker_id = id.clone();
    let worker = std::thread::spawn(move || {
        worker_service.recognize(
            &worker_id,
            ScreenshotRect {
                x: 1,
                y: 1,
                width: 1,
                height: 1,
            },
            None,
        )
    });
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(service.cancel_session(&id));
    assert_eq!(worker.join().unwrap().unwrap_err(), OcrError::Cancelled);
    assert!(engine.1.load(Ordering::Acquire));
}

struct PausedCapture {
    entered: mpsc::Sender<()>,
    release: Mutex<mpsc::Receiver<()>>,
    calls: std::sync::atomic::AtomicUsize,
}
impl ScreenCapture for PausedCapture {
    fn capture_all_displays(&self) -> Result<Vec<CapturedDisplay>, CaptureError> {
        if self.calls.fetch_add(1, Ordering::AcqRel) == 0 {
            self.entered.send(()).unwrap();
            self.release.lock().unwrap().recv().unwrap();
        }
        FakeCapture.capture_all_displays()
    }
}

#[test]
fn ocr_service_tests_older_capture_cannot_replace_newer_session() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let capture = Arc::new(PausedCapture {
        entered: entered_tx,
        release: Mutex::new(release_rx),
        calls: std::sync::atomic::AtomicUsize::new(0),
    });
    let service = Arc::new(OcrService::new(
        capture,
        Arc::new(FakeOcr(Err(OcrError::Engine))),
    ));
    let old_service = service.clone();
    let old = std::thread::spawn(move || old_service.begin());
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let new_id = service.begin().unwrap().remove(0).id;
    release_tx.send(()).unwrap();
    assert_eq!(old.join().unwrap().unwrap_err(), OcrError::Cancelled);
    assert!(service.display(&new_id).is_ok());
}

struct FailedCapture;

#[test]
fn ocr_service_tests_completed_ocr_is_distinct_from_cancelled_startup() {
    let service = service(Ok(OcrResult {
        text: "early result".into(),
        confidence: None,
        blocks: vec![],
    }));
    let id = service.begin().unwrap().remove(0).id;
    service
        .recognize(
            &id,
            ScreenshotRect {
                x: 1,
                y: 1,
                width: 1,
                height: 1,
            },
            Some(&"en".parse().unwrap()),
        )
        .unwrap();
    assert!(service.finish(&id));
    assert!(!service.is_current(&id));
    assert!(service.is_finished(&id));
    let next_id = service.begin().unwrap().remove(0).id;
    assert!(!service.is_finished(&id));
    assert!(service.cancel_session(&next_id));
    assert!(!service.is_finished(&next_id));
}

impl ScreenCapture for FailedCapture {
    fn capture_all_displays(&self) -> Result<Vec<CapturedDisplay>, CaptureError> {
        Err(CaptureError::Unavailable)
    }
}

#[test]
fn ocr_service_tests_failed_capture_does_not_leave_active_session() {
    let service = OcrService::new(
        Arc::new(FailedCapture),
        Arc::new(FakeOcr(Err(OcrError::Engine))),
    );
    assert_eq!(service.begin().unwrap_err(), OcrError::CaptureUnavailable);
    assert!(!service.is_current("s1--display-0"));
}
