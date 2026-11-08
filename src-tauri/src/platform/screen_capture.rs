use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug)]
pub struct ImageRegion {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub struct CapturedDisplay {
    pub id: String,
    pub origin_x: i32,
    pub origin_y: i32,
    pub scale_factor: f64,
    pub preview_data_url: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureError {
    Unavailable,
}

pub trait ScreenCapture: Send + Sync {
    fn capture_all_displays(&self) -> Result<Vec<CapturedDisplay>, CaptureError>;
}

pub struct SystemScreenCapture;

/// Wait for the desktop compositor to present window visibility changes before BitBlt.
/// Called on the capture worker, never the UI thread.
#[cfg(windows)]
pub fn wait_for_compositor() -> Result<(), CaptureError> {
    #[link(name = "dwmapi")]
    unsafe extern "system" {
        fn DwmFlush() -> i32;
    }
    if unsafe { DwmFlush() } < 0 {
        Err(CaptureError::Unavailable)
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
pub fn wait_for_compositor() -> Result<(), CaptureError> {
    Err(CaptureError::Unavailable)
}

/// The overlay stretches the preview to the window, so a preview far beyond the display resolution
/// only adds encode time, IPC payload and WebView decode work.
#[cfg(windows)]
const MAX_PREVIEW_EDGE: u32 = 2560;

#[cfg(windows)]
fn preview_size(width: u32, height: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= MAX_PREVIEW_EDGE {
        return (width, height);
    }
    let scale = MAX_PREVIEW_EDGE as f64 / f64::from(longest);
    (
        ((f64::from(width) * scale).round() as u32).max(1),
        ((f64::from(height) * scale).round() as u32).max(1),
    )
}

#[cfg(windows)]
fn encode_preview(rgb: &[u8], width: u32, height: u32) -> Result<String, CaptureError> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use image::{codecs::jpeg::JpegEncoder, ColorType};
    if rgb.len() != width as usize * height as usize * 3 {
        return Err(CaptureError::Unavailable);
    }
    let (preview_width, preview_height) = preview_size(width, height);
    let scaled;
    let (bytes, encode_width, encode_height) = if (preview_width, preview_height) == (width, height)
    {
        (rgb, width, height)
    } else {
        let source = image::RgbImage::from_raw(width, height, rgb.to_vec())
            .ok_or(CaptureError::Unavailable)?;
        scaled = image::imageops::resize(
            &source,
            preview_width,
            preview_height,
            image::imageops::FilterType::Triangle,
        );
        (scaled.as_raw().as_slice(), preview_width, preview_height)
    };
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, 75)
        .encode(bytes, encode_width, encode_height, ColorType::Rgb8.into())
        .map_err(|_| CaptureError::Unavailable)?;
    Ok(format!("data:image/jpeg;base64,{}", STANDARD.encode(jpeg)))
}

#[cfg(test)]
mod preview_tests {
    use super::*;
    #[test]
    fn rgba_capture_encodes_as_readable_jpeg_preview() {
        let url = encode_preview(&[255, 0, 0], 1, 1).unwrap();
        let encoded = url.strip_prefix("data:image/jpeg;base64,").unwrap();
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (1, 1));
    }

    #[test]
    fn preview_size_caps_the_longest_edge_and_keeps_aspect() {
        assert_eq!(preview_size(3840, 2160), (2560, 1440));
        assert_eq!(preview_size(1920, 1080), (1920, 1080));
        assert_eq!(preview_size(1080, 3840), (720, 2560));
    }

    #[test]
    fn oversized_capture_is_downscaled_before_encoding() {
        let width = 3000u32;
        let height = 2u32;
        let rgb = vec![128u8; width as usize * height as usize * 3];
        let url = encode_preview(&rgb, width, height).unwrap();
        let encoded = url.strip_prefix("data:image/jpeg;base64,").unwrap();
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!(
            (decoded.width(), decoded.height()),
            preview_size(width, height)
        );
    }
}

#[cfg(windows)]
mod windows_capture {
    use super::*;
    use std::{
        ffi::c_void,
        mem::{size_of, zeroed},
        ptr::{null, null_mut},
    };

    type Handle = *mut c_void;
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[repr(C)]
    struct MonitorInfo {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
    }
    #[repr(C)]
    struct BitmapInfoHeader {
        size: u32,
        width: i32,
        height: i32,
        planes: u16,
        bit_count: u16,
        compression: u32,
        size_image: u32,
        x_pels_per_meter: i32,
        y_pels_per_meter: i32,
        clr_used: u32,
        clr_important: u32,
    }
    #[repr(C)]
    struct BitmapInfo {
        header: BitmapInfoHeader,
        colors: [u32; 3],
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumDisplayMonitors(
            hdc: Handle,
            clip: *const Rect,
            callback: unsafe extern "system" fn(Handle, Handle, *const Rect, isize) -> i32,
            data: isize,
        ) -> i32;
        fn GetMonitorInfoW(monitor: Handle, info: *mut MonitorInfo) -> i32;
        fn GetDC(window: Handle) -> Handle;
        fn ReleaseDC(window: Handle, hdc: Handle) -> i32;
    }
    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn CreateCompatibleDC(hdc: Handle) -> Handle;
        fn DeleteDC(hdc: Handle) -> i32;
        fn CreateDIBSection(
            hdc: Handle,
            info: *const BitmapInfo,
            usage: u32,
            bits: *mut *mut c_void,
            section: Handle,
            offset: u32,
        ) -> Handle;
        fn SelectObject(hdc: Handle, object: Handle) -> Handle;
        fn DeleteObject(object: Handle) -> i32;
        fn BitBlt(
            dest: Handle,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            source: Handle,
            sx: i32,
            sy: i32,
            rop: u32,
        ) -> i32;
    }
    #[link(name = "shcore")]
    unsafe extern "system" {
        fn GetDpiForMonitor(monitor: Handle, kind: i32, x: *mut u32, y: *mut u32) -> i32;
    }

    unsafe extern "system" fn collect(
        monitor: Handle,
        _: Handle,
        _: *const Rect,
        data: isize,
    ) -> i32 {
        let monitors = &mut *(data as *mut Vec<(Handle, Rect, f64)>);
        let mut info: MonitorInfo = zeroed();
        info.size = size_of::<MonitorInfo>() as u32;
        if GetMonitorInfoW(monitor, &mut info) != 0 {
            let mut dpi_x = 96;
            let mut dpi_y = 96;
            let scale = if GetDpiForMonitor(monitor, 0, &mut dpi_x, &mut dpi_y) == 0 {
                dpi_x as f64 / 96.0
            } else {
                1.0
            };
            monitors.push((monitor, info.monitor, scale));
        }
        1
    }

    struct ScreenDc(Handle);
    impl Drop for ScreenDc {
        fn drop(&mut self) {
            unsafe {
                ReleaseDC(null_mut(), self.0);
            }
        }
    }
    struct MemoryDc(Handle);
    impl Drop for MemoryDc {
        fn drop(&mut self) {
            unsafe {
                DeleteDC(self.0);
            }
        }
    }
    struct Bitmap(Handle);
    impl Drop for Bitmap {
        fn drop(&mut self) {
            unsafe {
                DeleteObject(self.0);
            }
        }
    }

    fn capture(
        index: usize,
        rect: Rect,
        scale_factor: f64,
    ) -> Result<CapturedDisplay, CaptureError> {
        let width = (rect.right - rect.left) as u32;
        let height = (rect.bottom - rect.top) as u32;
        if width == 0 || height == 0 || width > 16384 || height > 16384 {
            return Err(CaptureError::Unavailable);
        }
        let screen = ScreenDc(unsafe { GetDC(null_mut()) });
        if screen.0.is_null() {
            return Err(CaptureError::Unavailable);
        }
        let memory = MemoryDc(unsafe { CreateCompatibleDC(screen.0) });
        if memory.0.is_null() {
            return Err(CaptureError::Unavailable);
        }
        let info = BitmapInfo {
            header: BitmapInfoHeader {
                size: size_of::<BitmapInfoHeader>() as u32,
                width: width as i32,
                height: -(height as i32),
                planes: 1,
                bit_count: 32,
                compression: 0,
                size_image: 0,
                x_pels_per_meter: 0,
                y_pels_per_meter: 0,
                clr_used: 0,
                clr_important: 0,
            },
            colors: [0; 3],
        };
        let mut bits = null_mut();
        let bitmap =
            Bitmap(unsafe { CreateDIBSection(screen.0, &info, 0, &mut bits, null_mut(), 0) });
        if bitmap.0.is_null() || bits.is_null() {
            return Err(CaptureError::Unavailable);
        }
        let old = unsafe { SelectObject(memory.0, bitmap.0) };
        if old.is_null() {
            return Err(CaptureError::Unavailable);
        }
        let copied = unsafe {
            BitBlt(
                memory.0,
                0,
                0,
                width as i32,
                height as i32,
                screen.0,
                rect.left,
                rect.top,
                0x00CC0020 | 0x40000000,
            )
        };
        unsafe {
            SelectObject(memory.0, old);
        }
        if copied == 0 {
            return Err(CaptureError::Unavailable);
        }
        let mut rgba = unsafe {
            std::slice::from_raw_parts(bits as *const u8, width as usize * height as usize * 4)
        }
        .to_vec();
        // One pass: BGRA -> RGBA for OCR, and the packed RGB bytes the preview encoder needs.
        let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
        for pixel in rgba.chunks_exact_mut(4) {
            pixel.swap(0, 2);
            pixel[3] = 255;
            rgb.extend_from_slice(&pixel[..3]);
        }
        let preview_data_url = encode_preview(&rgb, width, height)?;
        Ok(CapturedDisplay {
            id: format!("display-{index}"),
            origin_x: rect.left,
            origin_y: rect.top,
            scale_factor,
            preview_data_url,
            width,
            height,
            rgba,
        })
    }

    impl ScreenCapture for SystemScreenCapture {
        fn capture_all_displays(&self) -> Result<Vec<CapturedDisplay>, CaptureError> {
            let mut monitors: Vec<(Handle, Rect, f64)> = Vec::new();
            if unsafe {
                EnumDisplayMonitors(
                    null_mut(),
                    null(),
                    collect,
                    &mut monitors as *mut _ as isize,
                )
            } == 0
            {
                return Err(CaptureError::Unavailable);
            }
            monitors
                .into_iter()
                .enumerate()
                .map(|(i, (_, rect, scale))| capture(i, rect, scale))
                .collect()
        }
    }
}

#[cfg(not(windows))]
impl ScreenCapture for SystemScreenCapture {
    fn capture_all_displays(&self) -> Result<Vec<CapturedDisplay>, CaptureError> {
        Err(CaptureError::Unavailable)
    }
}
