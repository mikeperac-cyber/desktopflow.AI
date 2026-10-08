//! One-click action recorder (Phase 14).
//!
//! The recorder watches the user operate the already-captured target and turns
//! observed input into the same typed [`crate::ai::ActionPlan`] every other
//! provider produces. Recorded plans flow through the identical gates: Rust
//! semantic validation, two-stage confirmation, live target revalidation,
//! typed verification, and bounded recovery.
//!
//! Boundary notes:
//! - Only input delivered to the captured HWND is recorded; input anywhere
//!   else is ignored and counted, never stored.
//! - Keystrokes into password or otherwise sensitive controls are dropped at
//!   the recorder boundary and counted, never stored.
//! - Recorded text is sanitized with the same redaction used for diagnostics.
//! - The recorder never synthesizes shell, script, or binary commands: the
//!   output vocabulary is exactly the executor allowlist.

use std::{
    collections::HashSet,
    ffi::c_void,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
    System::Threading::GetCurrentThreadId,
    UI::{
        Input::KeyboardAndMouse::{
            GetKeyboardLayout, GetKeyboardState, HKL, ToUnicodeEx, VIRTUAL_KEY, VK_BACK,
            VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_F1, VK_F24, VK_HOME, VK_LEFT,
            VK_LWIN, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_RWIN, VK_SHIFT, VK_SPACE,
            VK_TAB, VK_UP,
        },
        WindowsAndMessaging::{
            CallNextHookEx, DispatchMessageW, GetCursorPos, GetForegroundWindow, GetMessageW,
            GetWindowThreadProcessId, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, PostThreadMessageW,
            SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL,
            WM_LBUTTONDOWN, WM_MOUSEWHEEL, WM_QUIT,
        },
    },
};

use crate::{
    ai::{
        self, ActionKind, ActionPlan, PlanStatus, PlannedAction, PlanningResult, RiskLevel,
        ScrollDirection, VerificationKind, VerificationSpec,
    },
    context::WindowContextSnapshot,
    error::{AppError, AppResult},
    security,
    uia::{NormalizedUiElement, UiAutomationSnapshot},
};

pub const RECORDER_PROVIDER_ID: &str = "recorder";
pub const RECORDER_MODEL_ID: &str = "observed-actions";
/// Mirrors the executor's 12-step ceiling so a recording can never outgrow a
/// single validated plan.
pub const MAX_RECORDED_STEPS: usize = 12;
const MAX_RECORDED_TEXT_CHARS: usize = 4_000;
const HOOK_THREAD_READY_TIMEOUT: Duration = Duration::from_secs(2);

static LOCAL_REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn new_request_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or(0);
    format!(
        "recorder-{timestamp}-{}",
        LOCAL_REQUEST_COUNTER.fetch_add(1, Ordering::SeqCst)
    )
}

/// Raw input observed by the low-level hooks before it becomes plan steps.
#[derive(Clone, Debug)]
pub enum RecordedEvent {
    LeftClick { x: i32, y: i32 },
    TypedChar { value: char },
    NamedKey { name: &'static str },
    Hotkey { keys: Vec<String> },
    Wheel { delta: i16 },
}

/// Outcome counters surfaced in the plan summary and recording status.
#[derive(Clone, Copy, Debug, Default)]
pub struct RecordingOutcome {
    pub steps: usize,
    pub skipped: usize,
    pub redacted: usize,
    pub truncated: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecordingStatus {
    pub recording: bool,
    pub event_count: usize,
    pub skipped_count: usize,
    pub elapsed_ms: u64,
    pub target_title: String,
}

/// Live session owned by [`ActiveRecording`]. The hook thread pushes events;
/// `stop` drains them into a validated plan.
pub struct RecordingSession {
    started_at_ms: u64,
    events: Mutex<Vec<RecordedEvent>>,
    external_ignored: AtomicUsize,
}

impl RecordingSession {
    fn push(&self, event: RecordedEvent) {
        recover_lock(&self.events).push(event);
    }

    fn drain(&self) -> Vec<RecordedEvent> {
        std::mem::take(&mut *recover_lock(&self.events))
    }

    fn status(&self, title: &str) -> RecordingStatus {
        RecordingStatus {
            recording: true,
            event_count: recover_lock(&self.events).len(),
            skipped_count: self.external_ignored.load(Ordering::SeqCst),
            elapsed_ms: timestamp_ms().saturating_sub(self.started_at_ms),
            target_title: title.to_string(),
        }
    }
}

fn recover_lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or(0)
}

/// Global hook sink. Low-level hook procedures must be free functions, so the
/// active session is published here while recording and cleared on stop.
struct HookTarget {
    hwnd: usize,
    session: std::sync::Arc<RecordingSession>,
    pressed: Mutex<HashSet<u32>>,
}

static HOOK_TARGET: Mutex<Option<HookTarget>> = Mutex::new(None);

fn with_target<R>(action: impl FnOnce(&HookTarget) -> R) -> Option<R> {
    recover_lock(&HOOK_TARGET).as_ref().map(action)
}

fn foreground_matches(hwnd: usize) -> bool {
    // SAFETY: reads the current foreground HWND without dereferencing it.
    let foreground = unsafe { GetForegroundWindow() };
    foreground.0 as usize == hwnd
}

unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let message = wparam.0 as u32;
        if message == WM_LBUTTONDOWN || message == WM_MOUSEWHEEL {
            // SAFETY: Windows passes a valid MSLLHOOKSTRUCT for low-level mouse events.
            let info = unsafe { *(lparam.0 as *const MSLLHOOKSTRUCT) };
            if let Some(target) =
                with_target(|target| (target.hwnd, std::sync::Arc::clone(&target.session)))
            {
                let (hwnd, session) = target;
                if foreground_matches(hwnd) {
                    if message == WM_LBUTTONDOWN {
                        session.push(RecordedEvent::LeftClick {
                            x: info.pt.x,
                            y: info.pt.y,
                        });
                    } else {
                        session.push(RecordedEvent::Wheel {
                            delta: (info.mouseData >> 16) as u16 as i16,
                        });
                    }
                } else {
                    session.external_ignored.fetch_add(1, Ordering::SeqCst);
                }
            }
        }
    }
    // SAFETY: forwarding unhandled input preserves the system input chain.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn vk(code: VIRTUAL_KEY) -> u32 {
    u32::from(code.0)
}

fn is_modifier(vk_code: u32) -> bool {
    matches!(
        vk_code,
        code if code == vk(VK_SHIFT)
            || code == vk(VK_CONTROL)
            || code == vk(VK_MENU)
            || code == 0xA0
            || code == 0xA1
            || code == 0xA4
            || code == 0xA5
            || code == vk(VK_LWIN)
            || code == vk(VK_RWIN)
    )
}

fn named_key(vk_code: u32) -> Option<&'static str> {
    Some(match vk_code {
        code if code == vk(VK_RETURN) => "enter",
        code if code == vk(VK_TAB) => "tab",
        code if code == vk(VK_ESCAPE) => "escape",
        code if code == vk(VK_SPACE) => "space",
        code if code == vk(VK_BACK) => "backspace",
        code if code == vk(VK_DELETE) => "delete",
        code if code == vk(VK_HOME) => "home",
        code if code == vk(VK_END) => "end",
        code if code == vk(VK_PRIOR) => "pageup",
        code if code == vk(VK_NEXT) => "pagedown",
        code if code == vk(VK_UP) => "arrowup",
        code if code == vk(VK_DOWN) => "arrowdown",
        code if code == vk(VK_LEFT) => "arrowleft",
        code if code == vk(VK_RIGHT) => "arrowright",
        _ => return None,
    })
}

fn function_key(vk_code: u32) -> Option<&'static str> {
    if vk_code < vk(VK_F1) || vk_code > vk(VK_F24) {
        return None;
    }
    Some(match vk_code - vk(VK_F1) {
        0 => "f1",
        1 => "f2",
        2 => "f3",
        3 => "f4",
        4 => "f5",
        5 => "f6",
        6 => "f7",
        7 => "f8",
        8 => "f9",
        9 => "f10",
        10 => "f11",
        11 => "f12",
        _ => return None,
    })
}

/// Translates a virtual key to the printable character the foreground layout
/// would produce, or `None` for non-printable keys.
fn translate_char(vk_code: u32, scan_code: u32, hwnd: usize) -> Option<char> {
    let mut key_state = [0_u8; 256];
    // SAFETY: the keystate buffer is owned for the duration of the call.
    if unsafe { GetKeyboardState(&mut key_state) }.is_err() {
        return None;
    }
    // Ctrl/Alt combinations are hotkeys, not text; only Shift may shape text.
    key_state[vk(VK_CONTROL) as usize & 0xFF] = 0;
    key_state[vk(VK_MENU) as usize & 0xFF] = 0;
    // SAFETY: reads the foreground thread identity without touching the window.
    let thread_id = unsafe { GetWindowThreadProcessId(HWND(hwnd as *mut c_void), None) };
    let layout: HKL = unsafe { GetKeyboardLayout(thread_id) };
    let mut buffer = [0_u16; 8];
    // SAFETY: the translation buffer is owned for the duration of the call.
    let produced =
        unsafe { ToUnicodeEx(vk_code, scan_code, &key_state, &mut buffer, 0, Some(layout)) };
    if produced != 1 {
        return None;
    }
    let value = char::decode_utf16([buffer[0]]).next()?.ok()?;
    if value.is_control() || (value.is_whitespace() && value != ' ') {
        return None;
    }
    Some(value)
}

unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: Windows passes a valid KBDLLHOOKSTRUCT for low-level key events.
    let info = unsafe { *(lparam.0 as *const KBDLLHOOKSTRUCT) };
    // Skip input DeskFlow itself synthesized so replay can never feed a recording.
    const LLKHF_INJECTED: u32 = 0x10;
    const LLKHF_UP: u32 = 0x80;
    let released = info.flags.0 & LLKHF_UP != 0;
    if code >= 0 && info.flags.0 & LLKHF_INJECTED == 0 {
        with_target(|target| {
            if released {
                recover_lock(&target.pressed).remove(&info.vkCode);
                return;
            }
            if !recover_lock(&target.pressed).insert(info.vkCode) {
                return; // Auto-repeat: the held key is already recorded once.
            }
            if is_modifier(info.vkCode) {
                return;
            }
            if !foreground_matches(target.hwnd) {
                target
                    .session
                    .external_ignored
                    .fetch_add(1, Ordering::SeqCst);
                return;
            }
            let pressed = recover_lock(&target.pressed);
            let ctrl = pressed
                .iter()
                .any(|key| *key == vk(VK_CONTROL) || *key == 0xA2 || *key == 0xA3);
            let alt = pressed
                .iter()
                .any(|key| *key == vk(VK_MENU) || *key == 0xA4 || *key == 0xA5);
            let shift = pressed
                .iter()
                .any(|key| *key == vk(VK_SHIFT) || *key == 0xA0 || *key == 0xA1);
            let win = pressed
                .iter()
                .any(|key| *key == vk(VK_LWIN) || *key == vk(VK_RWIN));
            drop(pressed);
            if ctrl || alt || win {
                let mut keys = Vec::with_capacity(4);
                if ctrl {
                    keys.push("ctrl".to_string());
                }
                if alt {
                    keys.push("alt".to_string());
                }
                if shift {
                    keys.push("shift".to_string());
                }
                if win {
                    keys.push("win".to_string());
                }
                let main = named_key(info.vkCode)
                    .or_else(|| function_key(info.vkCode))
                    .map(str::to_string)
                    .or_else(|| {
                        char::from_u32(info.vkCode)
                            .filter(|value| value.is_ascii_alphanumeric())
                            .map(|value| value.to_ascii_lowercase().to_string())
                    });
                if let Some(main) = main
                    && keys.len() < 4
                {
                    keys.push(main);
                    if keys.len() >= 2 {
                        target.session.push(RecordedEvent::Hotkey { keys });
                    }
                }
                return;
            }
            if let Some(name) = named_key(info.vkCode).or_else(|| function_key(info.vkCode)) {
                target.session.push(RecordedEvent::NamedKey { name });
                return;
            }
            if let Some(value) = translate_char(info.vkCode, info.scanCode, target.hwnd) {
                target.session.push(RecordedEvent::TypedChar { value });
            }
        });
    }
    // SAFETY: forwarding unhandled input preserves the system input chain.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

struct HookThread {
    thread_id: Mutex<Option<u32>>,
    ready: Mutex<bool>,
}

static HOOK_THREAD: Mutex<Option<HookThread>> = Mutex::new(None);

fn hook_thread_main() {
    // SAFETY: low-level hooks observe input process-wide; handles are released
    // in the matching shutdown path below.
    let mouse = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), None, 0) };
    let keyboard = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), None, 0) };
    let installed = mouse.is_ok() && keyboard.is_ok();
    if let Some(thread) = recover_lock(&HOOK_THREAD).as_ref() {
        // SAFETY: the installing thread is alive and pumping messages here.
        *recover_lock(&thread.thread_id) = Some(unsafe { GetCurrentThreadId() });
        *recover_lock(&thread.ready) = installed;
    }
    if installed {
        let mut message = MSG::default();
        // SAFETY: message loop required for low-level hook delivery.
        while unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {
            // SAFETY: standard message pump for the hook thread.
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            if message.message == WM_QUIT {
                break;
            }
        }
    }
    if let Ok(hook) = mouse {
        // SAFETY: every installed hook is released exactly once here.
        let _ = unsafe { UnhookWindowsHookEx(hook) };
    }
    if let Ok(hook) = keyboard {
        // SAFETY: every installed hook is released exactly once here.
        let _ = unsafe { UnhookWindowsHookEx(hook) };
    }
}

/// Handle for one recording. Dropping without [`ActiveRecording::shutdown`]
/// still releases the hooks; shutdown additionally returns drained events.
pub struct ActiveRecording {
    hwnd: usize,
    title: String,
    snapshot: UiAutomationSnapshot,
    session: std::sync::Arc<RecordingSession>,
    worker: Option<JoinHandle<()>>,
    finished: AtomicBool,
}

impl ActiveRecording {
    pub fn begin(
        context: &WindowContextSnapshot,
        snapshot: &UiAutomationSnapshot,
    ) -> AppResult<Self> {
        if with_target(|_| {}).is_some() {
            return Err(AppError::ExecutionPolicy(
                "a recording is already in progress.".to_string(),
            ));
        }
        let session = std::sync::Arc::new(RecordingSession {
            started_at_ms: timestamp_ms(),
            events: Mutex::new(Vec::new()),
            external_ignored: AtomicUsize::new(0),
        });
        *recover_lock(&HOOK_TARGET) = Some(HookTarget {
            hwnd: context.native_window_handle,
            session: std::sync::Arc::clone(&session),
            pressed: Mutex::new(HashSet::new()),
        });
        *recover_lock(&HOOK_THREAD) = Some(HookThread {
            thread_id: Mutex::new(None),
            ready: Mutex::new(false),
        });
        let worker = thread::Builder::new()
            .name("deskflow-recorder".to_string())
            .spawn(hook_thread_main)
            .map_err(|_| AppError::Execution("the recorder worker could not start.".to_string()))?;
        let deadline = std::time::Instant::now() + HOOK_THREAD_READY_TIMEOUT;
        loop {
            let ready = recover_lock(&HOOK_THREAD)
                .as_ref()
                .map(|thread| *recover_lock(&thread.ready))
                .unwrap_or(false);
            if ready {
                break;
            }
            if std::time::Instant::now() >= deadline {
                *recover_lock(&HOOK_TARGET) = None;
                return Err(AppError::Execution(
                    "the recorder could not observe desktop input.".to_string(),
                ));
            }
            thread::sleep(Duration::from_millis(10));
        }
        Ok(Self {
            hwnd: context.native_window_handle,
            title: context.title.clone(),
            snapshot: snapshot.clone(),
            session,
            worker: Some(worker),
            finished: AtomicBool::new(false),
        })
    }

    pub fn hwnd(&self) -> usize {
        self.hwnd
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn snapshot(&self) -> &UiAutomationSnapshot {
        &self.snapshot
    }

    pub fn status(&self) -> RecordingStatus {
        self.session.status(&self.title)
    }

    /// Stops the hooks, joins the worker, and returns every recorded event.
    pub fn shutdown(&mut self) -> AppResult<Vec<RecordedEvent>> {
        if self.finished.swap(true, Ordering::SeqCst) {
            return Ok(Vec::new());
        }
        *recover_lock(&HOOK_TARGET) = None;
        if let Some(thread) = recover_lock(&HOOK_THREAD).as_ref()
            && let Some(id) = *recover_lock(&thread.thread_id)
        {
            // SAFETY: posts quit to the known hook thread only.
            let _ = unsafe { PostThreadMessageW(id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        }
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| {
                AppError::Execution("the recorder worker stopped unexpectedly.".to_string())
            })?;
        }
        *recover_lock(&HOOK_THREAD) = None;
        Ok(self.session.drain())
    }
}

impl Drop for ActiveRecording {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

/// Resolves a recorded click to the deepest, smallest on-screen element under
/// the cursor. Window chrome (depth 0), disabled, and offscreen elements can
/// never become steps.
pub fn hit_test(elements: &[NormalizedUiElement], x: i32, y: i32) -> Option<&NormalizedUiElement> {
    elements
        .iter()
        .filter(|element| element.is_enabled && !element.is_offscreen && element.depth > 0)
        .filter_map(|element| {
            let bounds = element.bounds_physical.as_ref()?;
            if bounds.width == 0 || bounds.height == 0 {
                return None;
            }
            let inside_x = (x as i64) >= (bounds.left as i64)
                && (x as i64) < (bounds.left as i64 + bounds.width as i64);
            let inside_y = (y as i64) >= (bounds.top as i64)
                && (y as i64) < (bounds.top as i64 + bounds.height as i64);
            if inside_x && inside_y {
                let area = bounds.width as u64 * bounds.height as u64;
                Some((element, area))
            } else {
                None
            }
        })
        .min_by(|left, right| {
            right
                .0
                .depth
                .cmp(&left.0.depth)
                .then_with(|| left.1.cmp(&right.1))
        })
        .map(|(element, _)| element)
}

fn is_sensitive_target(element: &NormalizedUiElement) -> bool {
    element.is_password
        || security::is_sensitive_control_indicator(
            &element.name,
            &element.automation_id,
            &element.class_name,
            &element.role,
        )
}

fn short_label(value: &str, maximum: usize) -> String {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= maximum {
        return collapsed;
    }
    collapsed
        .chars()
        .take(maximum.saturating_sub(1))
        .collect::<String>()
        + "…"
}

/// Pure event-to-step compiler. Every emitted step satisfies
/// [`ai::validate_action_plan`]; anything else is counted, never stored.
pub struct SessionBuilder<'a> {
    elements: &'a [NormalizedUiElement],
    focus_element_id: Option<String>,
    pending_text: Option<(String, String)>,
    steps: Vec<PlannedAction>,
    skipped: usize,
    redacted: usize,
    truncated: usize,
}

impl<'a> SessionBuilder<'a> {
    pub fn new(snapshot: &'a UiAutomationSnapshot) -> Self {
        let focus_element_id = snapshot
            .elements
            .iter()
            .find(|element| {
                element.has_keyboard_focus && element.is_enabled && !element.is_offscreen
            })
            .map(|element| element.id.clone());
        Self {
            elements: &snapshot.elements,
            focus_element_id,
            pending_text: None,
            steps: Vec::new(),
            skipped: 0,
            redacted: 0,
            truncated: 0,
        }
    }

    fn element(&self, id: &str) -> Option<&'a NormalizedUiElement> {
        self.elements.iter().find(|element| element.id == id)
    }

    fn push_step(&mut self, step: PlannedAction) {
        if self.steps.len() >= MAX_RECORDED_STEPS {
            self.truncated += 1;
            return;
        }
        self.steps.push(step);
    }

    fn verification_for(&self, target_id: Option<&str>) -> VerificationSpec {
        const VERIFY_TIMEOUT_MS: u64 = 2_000;
        match target_id.and_then(|id| self.element(id)) {
            Some(element) if element.is_keyboard_focusable => VerificationSpec {
                kind: VerificationKind::HasKeyboardFocus,
                target_id: Some(element.id.clone()),
                expected_text: None,
                expected_bool: None,
                timeout_ms: VERIFY_TIMEOUT_MS,
            },
            Some(element) => VerificationSpec {
                kind: VerificationKind::ElementExists,
                target_id: Some(element.id.clone()),
                expected_text: None,
                expected_bool: None,
                timeout_ms: VERIFY_TIMEOUT_MS,
            },
            None => VerificationSpec {
                kind: VerificationKind::WindowExists,
                target_id: None,
                expected_text: None,
                expected_bool: None,
                timeout_ms: VERIFY_TIMEOUT_MS,
            },
        }
    }

    fn flush_text(&mut self) {
        let Some((target_id, text)) = self.pending_text.take() else {
            return;
        };
        let Some(element) = self.element(&target_id) else {
            self.skipped += 1;
            return;
        };
        let sanitized = security::sanitize_sensitive_text(&text);
        if sanitized.trim().is_empty() {
            self.skipped += 1;
            return;
        }
        let name = short_label(&element.name, 40);
        let id = format!("rec-{:02}", self.steps.len() + 1);
        self.push_step(PlannedAction {
            id,
            kind: ActionKind::TypeText,
            target_id: Some(element.id.clone()),
            text: Some(sanitized.clone()),
            keys: Vec::new(),
            scroll_direction: None,
            amount: None,
            duration_ms: None,
            description: format!(
                "Type '{}' into '{}'",
                short_label(&sanitized, 60),
                if name.is_empty() {
                    element.role.clone()
                } else {
                    name
                }
            ),
            expected_result: "The text appears in the control.".to_string(),
            verification: self.verification_for(Some(&element.id)),
            risk: RiskLevel::Low,
            requires_user_approval: false,
        });
    }

    pub fn push_event(&mut self, event: RecordedEvent) {
        match event {
            RecordedEvent::LeftClick { x, y } => {
                self.flush_text();
                let Some(element) = hit_test(self.elements, x, y) else {
                    self.skipped += 1;
                    return;
                };
                let name = short_label(&element.name, 40);
                let label = if name.is_empty() {
                    element.role.clone()
                } else {
                    name
                };
                self.focus_element_id = Some(element.id.clone());
                let id = format!("rec-{:02}", self.steps.len() + 1);
                self.push_step(PlannedAction {
                    id,
                    kind: ActionKind::Click,
                    target_id: Some(element.id.clone()),
                    text: None,
                    keys: Vec::new(),
                    scroll_direction: None,
                    amount: None,
                    duration_ms: None,
                    description: format!("Click '{label}'"),
                    expected_result: format!("The '{label}' control activates."),
                    verification: self.verification_for(Some(&element.id)),
                    risk: RiskLevel::Low,
                    requires_user_approval: false,
                });
            }
            RecordedEvent::TypedChar { value } => {
                let target_id = match self.focus_element_id.clone() {
                    Some(id) => id,
                    None => {
                        self.skipped += 1;
                        return;
                    }
                };
                let Some(element) = self.element(&target_id) else {
                    self.skipped += 1;
                    return;
                };
                if is_sensitive_target(element) {
                    self.redacted += 1;
                    return;
                }
                match self.pending_text.as_mut() {
                    Some((id, text)) if *id == target_id => {
                        if text.chars().count() >= MAX_RECORDED_TEXT_CHARS {
                            self.flush_text();
                            self.pending_text = Some((target_id, value.to_string()));
                        } else {
                            text.push(value);
                        }
                    }
                    _ => {
                        self.flush_text();
                        self.pending_text = Some((target_id, value.to_string()));
                    }
                }
            }
            RecordedEvent::NamedKey { name } => {
                self.flush_text();
                let id = format!("rec-{:02}", self.steps.len() + 1);
                self.push_step(PlannedAction {
                    id,
                    kind: ActionKind::KeyPress,
                    target_id: None,
                    text: None,
                    keys: vec![name.to_string()],
                    scroll_direction: None,
                    amount: None,
                    duration_ms: None,
                    description: format!("Press '{name}'"),
                    expected_result: format!("The interface responds to '{name}'."),
                    verification: self.verification_for(self.focus_element_id.as_deref()),
                    risk: RiskLevel::Low,
                    requires_user_approval: false,
                });
            }
            RecordedEvent::Hotkey { keys } => {
                self.flush_text();
                if keys.len() < 2 || keys.len() > 4 {
                    self.skipped += 1;
                    return;
                }
                let id = format!("rec-{:02}", self.steps.len() + 1);
                self.push_step(PlannedAction {
                    id,
                    kind: ActionKind::Hotkey,
                    target_id: None,
                    text: None,
                    keys,
                    scroll_direction: None,
                    amount: None,
                    duration_ms: None,
                    description: "Press the recorded shortcut".to_string(),
                    expected_result: "The interface responds to the shortcut.".to_string(),
                    verification: self.verification_for(self.focus_element_id.as_deref()),
                    risk: RiskLevel::Low,
                    requires_user_approval: false,
                });
            }
            RecordedEvent::Wheel { delta } => {
                self.flush_text();
                let id = format!("rec-{:02}", self.steps.len() + 1);
                self.push_step(PlannedAction {
                    id,
                    kind: ActionKind::Scroll,
                    target_id: self.focus_element_id.clone(),
                    text: None,
                    keys: Vec::new(),
                    scroll_direction: Some(if delta > 0 {
                        ScrollDirection::Up
                    } else {
                        ScrollDirection::Down
                    }),
                    amount: Some(3),
                    duration_ms: None,
                    description: "Scroll the recorded area".to_string(),
                    expected_result: "The area scrolls.".to_string(),
                    verification: self.verification_for(self.focus_element_id.as_deref()),
                    risk: RiskLevel::Low,
                    requires_user_approval: false,
                });
            }
        }
    }

    pub fn finish(mut self) -> AppResult<(Vec<PlannedAction>, RecordingOutcome)> {
        self.flush_text();
        if self.steps.is_empty() {
            return Err(AppError::InvalidPlan(
                "no recordable actions were observed. Interact with the captured target — clicks, typing, shortcuts, and scrolling — then stop the recording."
                    .to_string(),
            ));
        }
        let outcome = RecordingOutcome {
            steps: self.steps.len(),
            skipped: self.skipped,
            redacted: self.redacted,
            truncated: self.truncated,
        };
        Ok((self.steps, outcome))
    }
}

/// Compiles drained events into a validated [`ActionPlan`].
pub fn build_plan(
    snapshot: &UiAutomationSnapshot,
    events: Vec<RecordedEvent>,
) -> AppResult<(ActionPlan, RecordingOutcome)> {
    let mut builder = SessionBuilder::new(snapshot);
    for event in events {
        builder.push_event(event);
    }
    let (steps, outcome) = builder.finish()?;
    let mut summary = format!(
        "Observed {} recorded action{} on '{}'.",
        outcome.steps,
        if outcome.steps == 1 { "" } else { "s" },
        snapshot.target.title,
    );
    if outcome.skipped > 0 {
        summary.push_str(&format!(
            " {} input{} ignored.",
            outcome.skipped,
            if outcome.skipped == 1 { "" } else { "s" }
        ));
    }
    if outcome.redacted > 0 {
        summary.push_str(&format!(
            " {} sensitive keystroke{} dropped and never stored.",
            outcome.redacted,
            if outcome.redacted == 1 { "" } else { "s" }
        ));
    }
    if outcome.truncated > 0 {
        summary.push_str(&format!(
            " {} extra action{} omitted at the 12-step safety limit.",
            outcome.truncated,
            if outcome.truncated == 1 { "" } else { "s" }
        ));
    }
    let plan = ActionPlan {
        status: PlanStatus::Ready,
        title: format!(
            "Recorded workflow on '{}'",
            short_label(&snapshot.target.title, 80)
        ),
        summary,
        overall_risk: RiskLevel::Low,
        steps,
    };
    ai::validate_action_plan(&plan, snapshot)?;
    Ok((plan, outcome))
}

pub fn build_result(snapshot: &UiAutomationSnapshot, plan: ActionPlan) -> PlanningResult {
    PlanningResult {
        created_at_unix_ms: timestamp_ms(),
        provider: RECORDER_PROVIDER_ID.to_string(),
        model: RECORDER_MODEL_ID.to_string(),
        provider_request_id: new_request_id(),
        screenshot_included: false,
        observed_element_count: snapshot.elements.len(),
        usage: Default::default(),
        plan,
    }
}

/// Captures the cursor position for diagnostics without recording movement.
#[allow(dead_code)]
pub fn cursor_position() -> Option<(i32, i32)> {
    let mut point = POINT { x: 0, y: 0 };
    // SAFETY: point is a valid writable POINT for the duration of the call.
    unsafe { GetCursorPos(&mut point).ok()? };
    Some((point.x, point.y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ai::{PlanRequest, PlanningModel},
        context::{ActiveTargetSummary, ProcessMetadata},
        uia::UiAutomationLimits,
    };

    pub(crate) fn element_fixture(
        id: &str,
        name: &str,
        role: &str,
        bounds: Option<(i32, i32, u32, u32)>,
        focusable: bool,
        password: bool,
    ) -> NormalizedUiElement {
        NormalizedUiElement {
            id: id.to_string(),
            parent_id: Some("uia-0001".to_string()),
            depth: 2,
            name: name.to_string(),
            role: role.to_string(),
            automation_id: format!("auto_{id}"),
            class_name: "Button".to_string(),
            framework_id: "Win32".to_string(),
            bounds_physical: bounds.map(|(left, top, width, height)| crate::context::PixelRect {
                left,
                top,
                width,
                height,
            }),
            is_enabled: true,
            is_offscreen: false,
            is_keyboard_focusable: focusable,
            has_keyboard_focus: false,
            is_password: password,
            supported_patterns: vec!["invoke".to_string()],
        }
    }

    pub(crate) fn snapshot_fixture() -> UiAutomationSnapshot {
        UiAutomationSnapshot {
            captured_at_unix_ms: 1,
            target: ActiveTargetSummary {
                title: "Test App".to_string(),
                process_name: Some("test.exe".to_string()),
                process_id: 42,
            },
            root_id: Some("uia-0001".to_string()),
            elements: vec![
                NormalizedUiElement {
                    id: "uia-0001".to_string(),
                    parent_id: None,
                    depth: 0,
                    name: "Test App".to_string(),
                    role: "window".to_string(),
                    automation_id: String::new(),
                    class_name: "Window".to_string(),
                    framework_id: "Win32".to_string(),
                    bounds_physical: Some(crate::context::PixelRect {
                        left: 0,
                        top: 0,
                        width: 800,
                        height: 600,
                    }),
                    is_enabled: true,
                    is_offscreen: false,
                    is_keyboard_focusable: false,
                    has_keyboard_focus: false,
                    is_password: false,
                    supported_patterns: Vec::new(),
                },
                element_fixture(
                    "uia-0002",
                    "Save",
                    "button",
                    Some((10, 10, 80, 30)),
                    true,
                    false,
                ),
                element_fixture(
                    "uia-0003",
                    "Name",
                    "edit",
                    Some((10, 50, 200, 30)),
                    true,
                    false,
                )
                .with_value_pattern(),
                element_fixture(
                    "uia-0004",
                    "Password",
                    "edit",
                    Some((10, 90, 200, 30)),
                    true,
                    true,
                ),
            ],
            visited_count: 4,
            filtered_count: 4,
            truncated: false,
            duration_ms: 5,
            limits: UiAutomationLimits::default(),
            warnings: Vec::new(),
        }
    }

    trait FixtureElement {
        fn with_value_pattern(self) -> NormalizedUiElement;
    }

    impl FixtureElement for NormalizedUiElement {
        fn with_value_pattern(mut self) -> NormalizedUiElement {
            self.supported_patterns.push("value".to_string());
            self
        }
    }

    #[test]
    fn hit_test_prefers_deepest_smallest_enabled_element() {
        let snapshot = snapshot_fixture();
        let hit = hit_test(&snapshot.elements, 20, 20).expect("button hit");
        assert_eq!(hit.id, "uia-0002");
        // Window chrome (depth 0) never becomes a step even when it contains the point.
        assert!(hit_test(&snapshot.elements, 700, 500).is_none());
    }

    #[test]
    fn click_and_typing_compile_to_validated_steps() {
        let snapshot = snapshot_fixture();
        let events = vec![
            RecordedEvent::LeftClick { x: 20, y: 60 },
            RecordedEvent::TypedChar { value: 'h' },
            RecordedEvent::TypedChar { value: 'i' },
            RecordedEvent::NamedKey { name: "enter" },
        ];
        let (plan, outcome) = build_plan(&snapshot, events).expect("recorded plan");
        assert_eq!(outcome.steps, 3);
        assert_eq!(plan.steps[0].kind, ActionKind::Click);
        assert_eq!(plan.steps[0].target_id.as_deref(), Some("uia-0003"));
        assert_eq!(plan.steps[1].kind, ActionKind::TypeText);
        assert_eq!(plan.steps[1].text.as_deref(), Some("hi"));
        assert_eq!(plan.steps[2].kind, ActionKind::KeyPress);
        assert_eq!(plan.steps[2].keys, vec!["enter".to_string()]);
    }

    #[test]
    fn password_keystrokes_are_dropped_and_counted() {
        let snapshot = snapshot_fixture();
        let events = vec![
            RecordedEvent::LeftClick { x: 20, y: 100 },
            RecordedEvent::TypedChar { value: 's' },
            RecordedEvent::TypedChar { value: '3' },
            RecordedEvent::TypedChar { value: 'c' },
        ];
        let (plan, outcome) = build_plan(&snapshot, events).expect("recorded plan");
        assert_eq!(outcome.redacted, 3);
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].kind, ActionKind::Click);
        assert!(plan.summary.contains("never stored"));
    }

    #[test]
    fn hotkey_and_scroll_steps_validate() {
        let snapshot = snapshot_fixture();
        let events = vec![
            RecordedEvent::LeftClick { x: 20, y: 20 },
            RecordedEvent::Hotkey {
                keys: vec!["ctrl".to_string(), "s".to_string()],
            },
            RecordedEvent::Wheel { delta: -120 },
        ];
        let (plan, _) = build_plan(&snapshot, events).expect("recorded plan");
        assert_eq!(plan.steps[1].kind, ActionKind::Hotkey);
        assert_eq!(plan.steps[2].kind, ActionKind::Scroll);
        assert_eq!(plan.steps[2].scroll_direction, Some(ScrollDirection::Down));
    }

    #[test]
    fn empty_recordings_are_rejected_with_guidance() {
        let snapshot = snapshot_fixture();
        let error = build_plan(&snapshot, Vec::new()).expect_err("empty recording");
        assert!(matches!(error, AppError::InvalidPlan(_)));
    }

    #[test]
    fn recordings_truncate_at_the_step_ceiling() {
        let snapshot = snapshot_fixture();
        let events = (0..30)
            .map(|index| RecordedEvent::LeftClick {
                x: 20 + (index % 5),
                y: 20,
            })
            .collect::<Vec<_>>();
        let (plan, outcome) = build_plan(&snapshot, events).expect("recorded plan");
        assert_eq!(plan.steps.len(), MAX_RECORDED_STEPS);
        assert!(outcome.truncated > 0);
        assert!(plan.summary.contains("12-step safety limit"));
    }

    #[test]
    fn recorder_request_ids_are_bound_to_the_recorder() {
        let id = new_request_id();
        assert!(id.starts_with("recorder-"));
    }

    #[test]
    fn unsupported_plan_request_shape_is_unused() {
        let request = PlanRequest {
            instruction: "Recorded workflow".to_string(),
            model: PlanningModel::Fast,
            include_screenshot: false,
        };
        assert!(request.validate().is_ok());
    }

    #[test]
    fn hook_lifecycle_installs_and_releases_cleanly() {
        let context = WindowContextSnapshot {
            captured_at_unix_ms: 1,
            native_window_handle: 0,
            window_handle: "0x0".to_string(),
            title: "Test App".to_string(),
            class_name: "Test".to_string(),
            process: ProcessMetadata {
                id: 42,
                name: Some("test.exe".to_string()),
                executable_path: None,
            },
            bounds_physical: crate::context::PixelRect {
                left: 0,
                top: 0,
                width: 800,
                height: 600,
            },
            bounds_logical: crate::context::LogicalRect {
                left: 0.0,
                top: 0.0,
                width: 800.0,
                height: 600.0,
            },
            dpi: 96,
            scale_factor: 1.0,
            monitor: crate::context::MonitorMetadata {
                device_name: "test".to_string(),
                bounds_physical: crate::context::PixelRect {
                    left: 0,
                    top: 0,
                    width: 800,
                    height: 600,
                },
                work_area_physical: crate::context::PixelRect {
                    left: 0,
                    top: 0,
                    width: 800,
                    height: 600,
                },
                is_primary: true,
            },
            screenshot: crate::context::ScreenshotData {
                mime_type: "image/png".to_string(),
                data_url: "data:image/png;base64,".to_string(),
                width_px: 0,
                height_px: 0,
                byte_size: 0,
                capture_method: "test".to_string(),
            },
            warnings: Vec::new(),
        };
        let snapshot = snapshot_fixture();
        let mut recording = ActiveRecording::begin(&context, &snapshot).expect("hooks install");
        assert!(recording.status().recording);
        let events = recording.shutdown().expect("hooks release");
        assert!(events.is_empty());
        assert!(build_plan(&snapshot, events).is_err());
    }
}
