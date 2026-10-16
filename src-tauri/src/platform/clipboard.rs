#[derive(Debug)]
pub struct ClipboardError;
pub trait Clipboard: Send + Sync {
    fn write_text(&self, text: &str) -> Result<(), ClipboardError>;
}
#[derive(Default)]
pub struct SystemClipboard;
impl Clipboard for SystemClipboard {
    fn write_text(&self, text: &str) -> Result<(), ClipboardError> {
        arboard::Clipboard::new()
            .map_err(|_| ClipboardError)?
            .set_text(text)
            .map_err(|_| ClipboardError)
    }
}
