use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Serialize;

use crate::error::{AppError, AppResult};

const BASE_DPI: f64 = 96.0;
const MAX_CAPTURE_PIXELS: u64 = 40_000_000;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PixelRect {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LogicalRect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProcessMetadata {
    pub id: u32,
    pub name: Option<String>,
    pub executable_path: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MonitorMetadata {
    pub device_name: String,
    pub bounds_physical: PixelRect,
    pub work_area_physical: PixelRect,
    pub is_primary: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScreenshotData {
    pub mime_type: String,
    pub data_url: String,
    pub width_px: u32,
    pub height_px: u32,
    pub byte_size: usize,
    pub capture_method: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct WindowContextSnapshot {
    #[serde(skip)]
    pub(crate) native_window_handle: usize,
    pub captured_at_unix_ms: u64,
    pub window_handle: String,
    pub title: String,
    pub class_name: String,
    pub process: ProcessMetadata,
    pub bounds_physical: PixelRect,
    pub bounds_logical: LogicalRect,
    pub dpi: u32,
    pub scale_factor: f64,
    pub monitor: MonitorMetadata,
    pub screenshot: ScreenshotData,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ActiveTargetSummary {
    pub title: String,
    pub process_name: Option<String>,
    pub process_id: u32,
}

impl From<&WindowContextSnapshot> for ActiveTargetSummary {
    fn from(snapshot: &WindowContextSnapshot) -> Self {
        Self {
            title: snapshot.title.clone(),
            process_name: snapshot.process.name.clone(),
            process_id: snapshot.process.id,
        }
    }
}

#[derive(Clone, Debug)]
struct ObservedWindow {
    handle: usize,
    title: String,
    class_name: String,
    process: ProcessMetadata,
    bounds_physical: PixelRect,
    dpi: u32,
    monitor: MonitorMetadata,
    minimized: bool,
    warnings: Vec<String>,
}

#[derive(Debug)]
struct EncodedCapture {
    png: Vec<u8>,
    width: u32,
    height: u32,
    method: &'static str,
}

trait ContextAdapter {
    fn inspect_foreground(&self) -> Result<ObservedWindow, String>;
    fn capture(&self, window: &ObservedWindow) -> Result<EncodedCapture, String>;
}

fn logical_rect(rect: &PixelRect, dpi: u32) -> LogicalRect {
    let scale = f64::from(dpi.max(1)) / BASE_DPI;
    LogicalRect {
        left: f64::from(rect.left) / scale,
        top: f64::from(rect.top) / scale,
        width: f64::from(rect.width) / scale,
        height: f64::from(rect.height) / scale,
    }
}

fn validate_capture_candidate(window: &ObservedWindow) -> Result<(), String> {
    if window.process.id == std::process::id() {
        return Err(
            "DeskFlow is still the foreground window. Focus a target application and try again."
                .to_string(),
        );
    }
    if window.minimized {
        return Err("The foreground window is minimized and cannot be captured.".to_string());
    }
    if window.bounds_physical.width == 0 || window.bounds_physical.height == 0 {
        return Err("The foreground window has invalid bounds.".to_string());
    }

    let pixels = u64::from(window.bounds_physical.width)
        .checked_mul(u64::from(window.bounds_physical.height))
        .ok_or_else(|| "The foreground window is too large to capture safely.".to_string())?;
    if pixels > MAX_CAPTURE_PIXELS {
        return Err("The foreground window is too large to capture safely.".to_string());
    }
    Ok(())
}

fn capture_with_adapter(adapter: &impl ContextAdapter) -> Result<WindowContextSnapshot, String> {
    let window = adapter.inspect_foreground()?;
    validate_capture_candidate(&window)?;
    let capture = adapter.capture(&window)?;
    let byte_size = capture.png.len();
    let data_url = format!("data:image/png;base64,{}", STANDARD.encode(capture.png));
    let dpi = window.dpi.max(1);

    Ok(WindowContextSnapshot {
        native_window_handle: window.handle,
        captured_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX),
        window_handle: format!("0x{:X}", window.handle),
        title: window.title,
        class_name: window.class_name,
        process: window.process,
        bounds_logical: logical_rect(&window.bounds_physical, dpi),
        bounds_physical: window.bounds_physical,
        dpi,
        scale_factor: f64::from(dpi) / BASE_DPI,
        monitor: window.monitor,
        screenshot: ScreenshotData {
            mime_type: "image/png".to_string(),
            data_url,
            width_px: capture.width,
            height_px: capture.height,
            byte_size,
            capture_method: capture.method.to_string(),
        },
        warnings: window.warnings,
    })
}

pub fn capture_foreground_context() -> AppResult<WindowContextSnapshot> {
    #[cfg(windows)]
    {
        capture_with_adapter(&platform::WindowsContextAdapter).map_err(AppError::Context)
    }

    #[cfg(not(windows))]
    {
        Err(AppError::Context(
            "Active-window capture is supported only on Windows.".to_string(),
        ))
    }
}

/// Captures a specific live HWND for developer probes without changing the production
/// foreground-only command boundary. This function is not exposed through Tauri IPC.
pub fn capture_window_context(native_window_handle: usize) -> AppResult<WindowContextSnapshot> {
    #[cfg(windows)]
    {
        capture_with_adapter(&platform::WindowsHandleContextAdapter::new(
            native_window_handle,
        ))
        .map_err(AppError::Context)
    }

    #[cfg(not(windows))]
    {
        let _ = native_window_handle;
        Err(AppError::Context(
            "Window capture is supported only on Windows.".to_string(),
        ))
    }
}

#[cfg(windows)]
mod platform {
    use std::{mem::size_of, path::Path};

    use windows::{
        Win32::{
            Foundation::{CloseHandle, HWND, RECT},
            Graphics::{
                Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute},
                Gdi::{
                    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT,
                    CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
                    DeleteObject, GetDC, GetDIBits, GetMonitorInfoW, HBITMAP, HDC, HGDIOBJ,
                    MONITOR_DEFAULTTONEAREST, MONITORINFO, MONITORINFOEXW, MonitorFromWindow,
                    ROP_CODE, ReleaseDC, SRCCOPY, SelectObject,
                },
            },
            System::Threading::{
                OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                QueryFullProcessImageNameW,
            },
            UI::{
                HiDpi::GetDpiForWindow,
                WindowsAndMessaging::{
                    GetClassNameW, GetForegroundWindow, GetWindowRect, GetWindowTextLengthW,
                    GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow,
                },
            },
        },
        core::PWSTR,
    };

    use super::{
        ContextAdapter, EncodedCapture, MonitorMetadata, ObservedWindow, PixelRect, ProcessMetadata,
    };

    const MONITORINFOF_PRIMARY: u32 = 1;

    pub struct WindowsContextAdapter;

    pub struct WindowsHandleContextAdapter {
        hwnd: HWND,
    }

    impl WindowsHandleContextAdapter {
        pub fn new(native_window_handle: usize) -> Self {
            Self {
                hwnd: HWND(native_window_handle as *mut std::ffi::c_void),
            }
        }
    }

    fn rect_to_pixel_rect(rect: RECT) -> Result<PixelRect, String> {
        let width = rect.right.saturating_sub(rect.left);
        let height = rect.bottom.saturating_sub(rect.top);
        Ok(PixelRect {
            left: rect.left,
            top: rect.top,
            width: width
                .try_into()
                .map_err(|_| "The window width reported by Windows is invalid.".to_string())?,
            height: height
                .try_into()
                .map_err(|_| "The window height reported by Windows is invalid.".to_string())?,
        })
    }

    fn window_text(hwnd: HWND) -> String {
        // SAFETY: hwnd was returned by GetForegroundWindow and the UTF-16 buffer is writable.
        let length = unsafe { GetWindowTextLengthW(hwnd) };
        if length <= 0 {
            return String::new();
        }
        let mut buffer = vec![0_u16; length as usize + 1];
        // SAFETY: the buffer includes space for the terminating NUL.
        let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
        String::from_utf16_lossy(&buffer[..copied.max(0) as usize])
    }

    fn window_class(hwnd: HWND) -> String {
        let mut buffer = [0_u16; 256];
        // SAFETY: hwnd is valid for the duration of this synchronous inspection.
        let copied = unsafe { GetClassNameW(hwnd, &mut buffer) };
        String::from_utf16_lossy(&buffer[..copied.max(0) as usize])
    }

    fn process_metadata(process_id: u32) -> (ProcessMetadata, Option<String>) {
        // SAFETY: the requested access is read-only and process_id came from Windows.
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) };
        let Ok(process) = process else {
            return (
                ProcessMetadata {
                    id: process_id,
                    name: None,
                    executable_path: None,
                },
                Some("Windows did not allow access to the target process path.".to_string()),
            );
        };

        let mut buffer = vec![0_u16; 32_768];
        let mut length = buffer.len() as u32;
        // SAFETY: process is an open read-only handle and length describes the output buffer.
        let query = unsafe {
            QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &mut length,
            )
        };
        // SAFETY: process was opened above and must be closed exactly once.
        let _ = unsafe { CloseHandle(process) };

        match query {
            Ok(()) => {
                let path = String::from_utf16_lossy(&buffer[..length as usize]);
                let name = Path::new(&path)
                    .file_name()
                    .map(|value| value.to_string_lossy().into_owned());
                (
                    ProcessMetadata {
                        id: process_id,
                        name,
                        executable_path: Some(path),
                    },
                    None,
                )
            }
            Err(_) => (
                ProcessMetadata {
                    id: process_id,
                    name: None,
                    executable_path: None,
                },
                Some("Windows did not allow access to the target process path.".to_string()),
            ),
        }
    }

    fn frame_bounds(hwnd: HWND) -> Result<PixelRect, String> {
        let mut rect = RECT::default();
        // SAFETY: rect points to a correctly sized RECT output value.
        let dwm_result = unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                (&mut rect as *mut RECT).cast(),
                size_of::<RECT>() as u32,
            )
        };
        if dwm_result.is_err() {
            // SAFETY: hwnd was returned by GetForegroundWindow and rect is writable.
            unsafe { GetWindowRect(hwnd, &mut rect) }
                .map_err(|error| format!("Windows could not read the target bounds: {error}"))?;
        }
        rect_to_pixel_rect(rect)
    }

    fn monitor_metadata(hwnd: HWND) -> Result<MonitorMetadata, String> {
        // SAFETY: hwnd is a live foreground window; nearest always returns a monitor.
        let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
        if monitor.0.is_null() {
            return Err("Windows could not identify the target monitor.".to_string());
        }

        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        // SAFETY: MONITORINFOEXW begins with MONITORINFO and cbSize advertises the full buffer.
        let success = unsafe {
            GetMonitorInfoW(
                monitor,
                (&mut info as *mut MONITORINFOEXW).cast::<MONITORINFO>(),
            )
        };
        if !success.as_bool() {
            return Err("Windows could not read the target monitor bounds.".to_string());
        }

        let device_length = info
            .szDevice
            .iter()
            .position(|character| *character == 0)
            .unwrap_or(info.szDevice.len());
        Ok(MonitorMetadata {
            device_name: String::from_utf16_lossy(&info.szDevice[..device_length]),
            bounds_physical: rect_to_pixel_rect(info.monitorInfo.rcMonitor)?,
            work_area_physical: rect_to_pixel_rect(info.monitorInfo.rcWork)?,
            is_primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
        })
    }

    struct CaptureResources {
        screen_dc: HDC,
        memory_dc: HDC,
        bitmap: HBITMAP,
        previous: HGDIOBJ,
    }

    impl Drop for CaptureResources {
        fn drop(&mut self) {
            // SAFETY: every handle was created and selected by capture_screen_rect.
            unsafe {
                if !self.previous.0.is_null() {
                    SelectObject(self.memory_dc, self.previous);
                }
                if !self.bitmap.0.is_null() {
                    let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
                }
                if !self.memory_dc.0.is_null() {
                    let _ = DeleteDC(self.memory_dc);
                }
                if !self.screen_dc.0.is_null() {
                    let _ = ReleaseDC(None, self.screen_dc);
                }
            }
        }
    }

    fn encode_png(mut bgra: Vec<u8>, width: u32, height: u32) -> Result<Vec<u8>, String> {
        let (pixels, remainder) = bgra.as_chunks_mut::<4>();
        debug_assert!(remainder.is_empty());
        for pixel in pixels {
            pixel.swap(0, 2);
            pixel[3] = 255;
        }

        let mut output = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut output, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder
                .write_header()
                .map_err(|error| format!("DeskFlow could not initialize PNG encoding: {error}"))?;
            writer
                .write_image_data(&bgra)
                .map_err(|error| format!("DeskFlow could not encode the screenshot: {error}"))?;
        }
        Ok(output)
    }

    fn capture_screen_rect(rect: &PixelRect) -> Result<EncodedCapture, String> {
        let width: i32 = rect
            .width
            .try_into()
            .map_err(|_| "The target width is too large to capture.".to_string())?;
        let height: i32 = rect
            .height
            .try_into()
            .map_err(|_| "The target height is too large to capture.".to_string())?;

        // SAFETY: requesting the desktop DC with no owning window is a documented Win32 path.
        let screen_dc = unsafe { GetDC(None) };
        if screen_dc.0.is_null() {
            return Err("Windows did not provide a screen capture context.".to_string());
        }
        // SAFETY: screen_dc is valid until ReleaseDC in CaptureResources::drop.
        let memory_dc = unsafe { CreateCompatibleDC(Some(screen_dc)) };
        if memory_dc.0.is_null() {
            // SAFETY: screen_dc was acquired immediately above.
            unsafe { ReleaseDC(None, screen_dc) };
            return Err("Windows could not create a screenshot buffer.".to_string());
        }
        // SAFETY: screen_dc is valid and width/height were validated before this call.
        let bitmap = unsafe { CreateCompatibleBitmap(screen_dc, width, height) };
        if bitmap.0.is_null() {
            // SAFETY: both handles were acquired above and are not owned elsewhere.
            unsafe {
                let _ = DeleteDC(memory_dc);
                let _ = ReleaseDC(None, screen_dc);
            }
            return Err("Windows could not allocate the screenshot bitmap.".to_string());
        }
        // SAFETY: memory_dc and bitmap are valid compatible GDI objects.
        let previous = unsafe { SelectObject(memory_dc, HGDIOBJ(bitmap.0)) };
        if previous.0.is_null() {
            // SAFETY: the bitmap was not selected, so all handles can be released directly.
            unsafe {
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
                let _ = DeleteDC(memory_dc);
                let _ = ReleaseDC(None, screen_dc);
            }
            return Err("Windows could not prepare the screenshot bitmap.".to_string());
        }
        let resources = CaptureResources {
            screen_dc,
            memory_dc,
            bitmap,
            previous,
        };

        // SAFETY: all DCs are valid and the destination bitmap is at least width x height.
        unsafe {
            BitBlt(
                resources.memory_dc,
                0,
                0,
                width,
                height,
                Some(resources.screen_dc),
                rect.left,
                rect.top,
                ROP_CODE(SRCCOPY.0 | CAPTUREBLT.0),
            )
        }
        .map_err(|error| {
            format!("Windows blocked the screenshot or the target is protected: {error}")
        })?;

        let byte_count = (u64::from(rect.width) * u64::from(rect.height) * 4)
            .try_into()
            .map_err(|_| "The screenshot buffer is too large.".to_string())?;
        let mut pixels = vec![0_u8; byte_count];
        let mut bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        // SAFETY: pixels has width * height * 4 writable bytes and bitmap_info requests 32-bit BGRA.
        let scanlines = unsafe {
            GetDIBits(
                resources.memory_dc,
                resources.bitmap,
                0,
                rect.height,
                Some(pixels.as_mut_ptr().cast()),
                &mut bitmap_info,
                DIB_RGB_COLORS,
            )
        };
        if scanlines != height {
            return Err(
                "Windows blocked the screenshot or returned incomplete image data.".to_string(),
            );
        }

        Ok(EncodedCapture {
            png: encode_png(pixels, rect.width, rect.height)?,
            width: rect.width,
            height: rect.height,
            method: "screen_bitblt",
        })
    }

    fn inspect_window(hwnd: HWND) -> Result<ObservedWindow, String> {
        // SAFETY: Windows validates the opaque HWND value.
        if hwnd.0.is_null() || !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            return Err("Windows did not report a live target window.".to_string());
        }

        let mut process_id = 0_u32;
        // SAFETY: process_id is a writable output location and hwnd is live.
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
        if process_id == 0 {
            return Err("Windows did not report the target process identity.".to_string());
        }

        let (process, process_warning) = process_metadata(process_id);
        let mut warnings = Vec::new();
        if let Some(warning) = process_warning {
            warnings.push(warning);
        }

        // SAFETY: hwnd remains valid during this synchronous inspection.
        let reported_dpi = unsafe { GetDpiForWindow(hwnd) };
        if reported_dpi == 0 {
            warnings.push("Windows did not report a DPI; DeskFlow used 96 DPI.".to_string());
        }
        let dpi = if reported_dpi == 0 { 96 } else { reported_dpi };

        Ok(ObservedWindow {
            handle: hwnd.0 as usize,
            title: window_text(hwnd),
            class_name: window_class(hwnd),
            process,
            bounds_physical: frame_bounds(hwnd)?,
            dpi,
            monitor: monitor_metadata(hwnd)?,
            // SAFETY: hwnd is a live window.
            minimized: unsafe { IsIconic(hwnd) }.as_bool(),
            warnings,
        })
    }

    impl ContextAdapter for WindowsContextAdapter {
        fn inspect_foreground(&self) -> Result<ObservedWindow, String> {
            // SAFETY: GetForegroundWindow has no preconditions.
            inspect_window(unsafe { GetForegroundWindow() })
        }

        fn capture(&self, window: &ObservedWindow) -> Result<EncodedCapture, String> {
            capture_screen_rect(&window.bounds_physical)
        }
    }

    impl ContextAdapter for WindowsHandleContextAdapter {
        fn inspect_foreground(&self) -> Result<ObservedWindow, String> {
            inspect_window(self.hwnd)
        }

        fn capture(&self, window: &ObservedWindow) -> Result<EncodedCapture, String> {
            capture_screen_rect(&window.bounds_physical)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    struct FakeAdapter {
        window: ObservedWindow,
        capture_result: Result<EncodedCapture, String>,
        capture_called: Cell<bool>,
    }

    impl ContextAdapter for FakeAdapter {
        fn inspect_foreground(&self) -> Result<ObservedWindow, String> {
            Ok(self.window.clone())
        }

        fn capture(&self, _window: &ObservedWindow) -> Result<EncodedCapture, String> {
            self.capture_called.set(true);
            self.capture_result
                .as_ref()
                .map(|capture| EncodedCapture {
                    png: capture.png.clone(),
                    width: capture.width,
                    height: capture.height,
                    method: capture.method,
                })
                .map_err(Clone::clone)
        }
    }

    fn sample_window() -> ObservedWindow {
        ObservedWindow {
            handle: 0x1234,
            title: "Notes".to_string(),
            class_name: "Notepad".to_string(),
            process: ProcessMetadata {
                id: std::process::id().saturating_add(1),
                name: Some("notepad.exe".to_string()),
                executable_path: Some("C:\\Windows\\System32\\notepad.exe".to_string()),
            },
            bounds_physical: PixelRect {
                left: -1920,
                top: 120,
                width: 1920,
                height: 1080,
            },
            dpi: 144,
            monitor: MonitorMetadata {
                device_name: "\\\\.\\DISPLAY2".to_string(),
                bounds_physical: PixelRect {
                    left: -1920,
                    top: 0,
                    width: 1920,
                    height: 1080,
                },
                work_area_physical: PixelRect {
                    left: -1920,
                    top: 0,
                    width: 1920,
                    height: 1040,
                },
                is_primary: false,
            },
            minimized: false,
            warnings: Vec::new(),
        }
    }

    fn successful_adapter() -> FakeAdapter {
        FakeAdapter {
            window: sample_window(),
            capture_result: Ok(EncodedCapture {
                png: vec![137, 80, 78, 71],
                width: 1920,
                height: 1080,
                method: "test_capture",
            }),
            capture_called: Cell::new(false),
        }
    }

    #[test]
    fn converts_negative_multi_monitor_bounds_using_window_dpi() {
        let adapter = successful_adapter();
        let snapshot = capture_with_adapter(&adapter).unwrap();

        assert_eq!(snapshot.bounds_logical.left, -1280.0);
        assert_eq!(snapshot.bounds_logical.top, 80.0);
        assert_eq!(snapshot.bounds_logical.width, 1280.0);
        assert_eq!(snapshot.bounds_logical.height, 720.0);
        assert_eq!(snapshot.scale_factor, 1.5);
        assert_eq!(snapshot.monitor.device_name, "\\\\.\\DISPLAY2");
        assert!(
            snapshot
                .screenshot
                .data_url
                .starts_with("data:image/png;base64,")
        );
    }

    #[test]
    fn rejects_minimized_windows_before_reading_pixels() {
        let mut adapter = successful_adapter();
        adapter.window.minimized = true;

        let error = capture_with_adapter(&adapter).unwrap_err();
        assert!(error.contains("minimized"));
        assert!(!adapter.capture_called.get());
    }

    #[test]
    fn surfaces_protected_window_capture_failures() {
        let mut adapter = successful_adapter();
        adapter.capture_result =
            Err("Windows blocked the screenshot or the target is protected.".to_string());

        let error = capture_with_adapter(&adapter).unwrap_err();
        assert!(error.contains("protected"));
        assert!(adapter.capture_called.get());
    }

    #[test]
    fn refuses_unbounded_capture_allocations() {
        let mut adapter = successful_adapter();
        adapter.window.bounds_physical.width = 16_384;
        adapter.window.bounds_physical.height = 16_384;

        let error = capture_with_adapter(&adapter).unwrap_err();
        assert!(error.contains("too large"));
        assert!(!adapter.capture_called.get());
    }
}
