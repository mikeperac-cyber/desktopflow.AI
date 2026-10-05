#[cfg(windows)]
mod windows_target {
    use std::{
        ffi::c_void,
        sync::atomic::{AtomicBool, AtomicIsize, Ordering},
    };

    use windows::{
        Win32::{
            Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
            Graphics::Gdi::{COLOR_WINDOW, GetSysColorBrush, UpdateWindow},
            System::LibraryLoader::GetModuleHandleW,
            UI::WindowsAndMessaging::{
                BS_DEFPUSHBUTTON, CreateWindowExW, DefWindowProcW, DispatchMessageW,
                ES_AUTOHSCROLL, GetMessageW, GetWindowTextLengthW, GetWindowTextW, HMENU,
                IDC_ARROW, LoadCursorW, MSG, MoveWindow, PostQuitMessage, RegisterClassW, SW_SHOW,
                SetForegroundWindow, SetWindowTextW, ShowWindow, TranslateMessage, WINDOW_EX_STYLE,
                WINDOW_STYLE, WM_COMMAND, WM_CREATE, WM_DESTROY, WNDCLASSW, WS_BORDER, WS_CHILD,
                WS_OVERLAPPEDWINDOW, WS_TABSTOP, WS_VISIBLE,
            },
        },
        core::{PCWSTR, w},
    };

    const EDIT_ID: isize = 1001;
    const CONTINUE_ID: usize = 1002;
    static EDIT_HANDLE: AtomicIsize = AtomicIsize::new(0);
    static CONTINUE_HANDLE: AtomicIsize = AtomicIsize::new(0);
    static STATUS_HANDLE: AtomicIsize = AtomicIsize::new(0);
    static INTERFACE_CHANGED: AtomicBool = AtomicBool::new(false);

    fn menu_id(value: isize) -> HMENU {
        HMENU(value as *mut c_void)
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_CREATE => {
                // SAFETY: all controls are created as children of the live test window.
                let _ = unsafe {
                    CreateWindowExW(
                        WINDOW_EX_STYLE::default(),
                        w!("STATIC"),
                        w!("Name"),
                        WS_CHILD | WS_VISIBLE,
                        24,
                        25,
                        80,
                        24,
                        Some(hwnd),
                        None,
                        None,
                        None,
                    )
                };
                // SAFETY: the built-in EDIT class and bounded geometry are valid.
                if let Ok(edit) = unsafe {
                    CreateWindowExW(
                        WINDOW_EX_STYLE::default(),
                        w!("EDIT"),
                        PCWSTR::null(),
                        WS_CHILD
                            | WS_VISIBLE
                            | WS_TABSTOP
                            | WS_BORDER
                            | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                        24,
                        52,
                        260,
                        30,
                        Some(hwnd),
                        Some(menu_id(EDIT_ID)),
                        None,
                        None,
                    )
                } {
                    EDIT_HANDLE.store(edit.0 as isize, Ordering::SeqCst);
                }
                // SAFETY: the built-in BUTTON class and bounded geometry are valid.
                if let Ok(button) = unsafe {
                    CreateWindowExW(
                        WINDOW_EX_STYLE::default(),
                        w!("BUTTON"),
                        w!("Continue"),
                        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                        24,
                        96,
                        120,
                        34,
                        Some(hwnd),
                        Some(menu_id(CONTINUE_ID as isize)),
                        None,
                        None,
                    )
                } {
                    CONTINUE_HANDLE.store(button.0 as isize, Ordering::SeqCst);
                }
                // SAFETY: the built-in STATIC class and bounded geometry are valid.
                if let Ok(status) = unsafe {
                    CreateWindowExW(
                        WINDOW_EX_STYLE::default(),
                        w!("STATIC"),
                        w!("Waiting for input"),
                        WS_CHILD | WS_VISIBLE,
                        24,
                        148,
                        260,
                        24,
                        Some(hwnd),
                        None,
                        None,
                        None,
                    )
                } {
                    STATUS_HANDLE.store(status.0 as isize, Ordering::SeqCst);
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let command_id = wparam.0 & 0xffff;
                if command_id == EDIT_ID as usize {
                    let edit = HWND(EDIT_HANDLE.load(Ordering::SeqCst) as *mut c_void);
                    let button = HWND(CONTINUE_HANDLE.load(Ordering::SeqCst) as *mut c_void);
                    let status = HWND(STATUS_HANDLE.load(Ordering::SeqCst) as *mut c_void);
                    // SAFETY: edit is a live child window and the buffer is sized from Windows.
                    let length = unsafe { GetWindowTextLengthW(edit) }.max(0) as usize;
                    let mut buffer = vec![0_u16; length + 1];
                    let copied = unsafe { GetWindowTextW(edit, &mut buffer) }.max(0) as usize;
                    let name = String::from_utf16_lossy(&buffer[..copied]);
                    if name == "Mike" && !INTERFACE_CHANGED.swap(true, Ordering::SeqCst) {
                        // SAFETY: these are live child windows owned by this process.
                        let _ = unsafe { SetWindowTextW(button, w!("Continue after refresh")) };
                        let _ = unsafe { MoveWindow(button, 124, 96, 190, 34, true) };
                        let _ = unsafe {
                            SetWindowTextW(status, w!("Layout changed — continue again"))
                        };
                    }
                    return LRESULT(0);
                }
                if command_id != CONTINUE_ID {
                    // SAFETY: unrelated control notifications use the default window behavior.
                    return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
                }
                let edit = HWND(EDIT_HANDLE.load(Ordering::SeqCst) as *mut c_void);
                let button = HWND(CONTINUE_HANDLE.load(Ordering::SeqCst) as *mut c_void);
                let status = HWND(STATUS_HANDLE.load(Ordering::SeqCst) as *mut c_void);
                // SAFETY: handles were stored only after successful child creation.
                let length = unsafe { GetWindowTextLengthW(edit) }.max(0) as usize;
                let mut buffer = vec![0_u16; length + 1];
                // SAFETY: buffer is writable and has room for the terminator.
                let copied = unsafe { GetWindowTextW(edit, &mut buffer) }.max(0) as usize;
                let name = String::from_utf16_lossy(&buffer[..copied]);
                if name == "Mike" {
                    // SAFETY: button is a live child window and the buffer is bounded.
                    let button_length = unsafe { GetWindowTextLengthW(button) }.max(0) as usize;
                    let mut button_buffer = vec![0_u16; button_length + 1];
                    let button_copied =
                        unsafe { GetWindowTextW(button, &mut button_buffer) }.max(0) as usize;
                    let button_name = String::from_utf16_lossy(&button_buffer[..button_copied]);
                    if button_name != "Continue after refresh" {
                        INTERFACE_CHANGED.store(true, Ordering::SeqCst);
                        // SAFETY: these are live child windows owned by this process.
                        let _ = unsafe { SetWindowTextW(button, w!("Continue after refresh")) };
                        // SAFETY: button is a live child window and geometry is bounded.
                        let _ = unsafe { MoveWindow(button, 124, 96, 190, 34, true) };
                        let _ = unsafe {
                            SetWindowTextW(status, w!("Layout changed — continue again"))
                        };
                    } else {
                        // SAFETY: these are live windows owned by this process.
                        let _ = unsafe { SetWindowTextW(status, w!("Welcome, Mike")) };
                        let _ = unsafe {
                            SetWindowTextW(hwnd, w!("DeskFlow Executor Test — Complete"))
                        };
                    }
                } else {
                    // SAFETY: status is a live child window.
                    let _ = unsafe { SetWindowTextW(status, w!("Enter Mike to continue")) };
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                // SAFETY: posts termination to this thread's message queue.
                unsafe { PostQuitMessage(0) };
                LRESULT(0)
            }
            _ => {
                // SAFETY: unhandled messages are delegated to the default window procedure.
                unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
            }
        }
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        // SAFETY: None requests the module containing this executable.
        let instance: HINSTANCE = unsafe { GetModuleHandleW(None) }?.into();
        // SAFETY: loading the system arrow cursor needs no owning instance.
        let cursor = unsafe { LoadCursorW(None, IDC_ARROW) }?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: cursor,
            // SAFETY: this returns a shared system brush that must not be deleted.
            hbrBackground: unsafe { GetSysColorBrush(COLOR_WINDOW) },
            lpszClassName: w!("DeskFlowExecutorTarget"),
            ..Default::default()
        };
        // SAFETY: class points to static strings and a process-lifetime window procedure.
        if unsafe { RegisterClassW(&class) } == 0 {
            return Err("could not register the deterministic test window".into());
        }
        // SAFETY: the registered class and module instance are valid.
        let window = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("DeskFlowExecutorTarget"),
                w!("DeskFlow Executor Test"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                180,
                160,
                360,
                250,
                None,
                None,
                Some(instance),
                None,
            )
        }?;
        // SAFETY: window was created successfully.
        let _ = unsafe { ShowWindow(window, SW_SHOW) };
        let _ = unsafe { UpdateWindow(window) };
        let _ = unsafe { SetForegroundWindow(window) };

        let mut message = MSG::default();
        // SAFETY: message points to valid storage for the duration of the loop.
        while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
            // SAFETY: message was produced by GetMessageW.
            let _ = unsafe { TranslateMessage(&message) };
            unsafe { DispatchMessageW(&message) };
        }
        Ok(())
    }
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    windows_target::run()
}

#[cfg(not(windows))]
fn main() {
    eprintln!("The DeskFlow executor target is available only on Windows.");
}
