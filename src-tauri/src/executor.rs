use std::{
    collections::HashSet,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;

use crate::{
    ai::{
        self, ActionKind, ActionPlan, PlanStatus, PlannedAction, RiskLevel, VerificationKind,
        VerificationSpec,
    },
    context::WindowContextSnapshot,
    error::{AppError, AppResult},
    uia::UiAutomationSnapshot,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Completed,
    Failed,
    VerificationFailed,
    Cancelled,
    EmergencyStopped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStepStatus {
    Completed,
    Failed,
    VerificationFailed,
    Cancelled,
}

#[derive(Clone, Debug, Serialize)]
pub struct VerificationEvidence {
    pub kind: VerificationKind,
    pub method: String,
    pub attempts: u32,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutionStepResult {
    pub plan_attempt: u8,
    pub id: String,
    pub kind: ActionKind,
    pub target_id: Option<String>,
    pub status: ExecutionStepStatus,
    pub method: Option<String>,
    pub duration_ms: u64,
    pub verification: Option<VerificationEvidence>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutionReport {
    pub started_at_unix_ms: u64,
    pub finished_at_unix_ms: u64,
    pub status: ExecutionStatus,
    pub total_steps: usize,
    pub completed_steps: usize,
    pub plan_attempts: u8,
    pub replan_attempts: u8,
    pub recovered: bool,
    pub step_results: Vec<ExecutionStepResult>,
    pub failure_message: Option<String>,
    pub recovery_failure_message: Option<String>,
}

pub fn release_stuck_inputs() {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP,
            MOUSEEVENTF_LEFTUP, MOUSEINPUT, SendInput, VK_CONTROL, VK_LWIN, VK_MENU, VK_SHIFT,
        };
        let inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_CONTROL,
                        dwFlags: KEYEVENTF_KEYUP,
                        ..Default::default()
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_SHIFT,
                        dwFlags: KEYEVENTF_KEYUP,
                        ..Default::default()
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_MENU,
                        dwFlags: KEYEVENTF_KEYUP,
                        ..Default::default()
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_LWIN,
                        dwFlags: KEYEVENTF_KEYUP,
                        ..Default::default()
                    },
                },
            },
            INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dwFlags: MOUSEEVENTF_LEFTUP,
                        ..Default::default()
                    },
                },
            },
        ];
        // SAFETY: inputs is a valid contiguous slice and cbSize matches INPUT exactly.
        unsafe {
            let _ = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }
}

trait ActionDriver {
    fn perform(&mut self, step: &PlannedAction) -> Result<String, String>;
    fn verify(
        &mut self,
        verification: &VerificationSpec,
        cancellation: Option<&std::sync::Arc<std::sync::atomic::AtomicBool>>,
    ) -> Result<VerificationEvidence, String>;
}

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn validate_execution_policy(
    context: &WindowContextSnapshot,
    snapshot: &UiAutomationSnapshot,
    plan: &ActionPlan,
    maximum_steps: usize,
    approval_policy: &crate::settings::ApprovalPolicy,
    approved_step_ids: &[String],
) -> AppResult<()> {
    ai::validate_action_plan(plan, snapshot)?;
    if plan.status != PlanStatus::Ready {
        return Err(AppError::ExecutionPolicy(
            "only a ready, locally validated plan can run.".to_string(),
        ));
    }
    if plan.steps.len() > maximum_steps {
        return Err(AppError::ExecutionPolicy(format!(
            "the plan has {} steps but the current limit is {maximum_steps}.",
            plan.steps.len()
        )));
    }
    for step in &plan.steps {
        let requires_approval = step.risk == RiskLevel::High
            || step.requires_user_approval
            || *approval_policy == crate::settings::ApprovalPolicy::AlwaysAsk;
        if requires_approval && !approved_step_ids.iter().any(|id| id == &step.id) {
            return Err(AppError::ExecutionPolicy(format!(
                "step '{}' requires explicit user approval before execution.",
                step.id
            )));
        }
    }
    if context.process.id != snapshot.target.process_id {
        return Err(AppError::ExecutionPolicy(
            "the plan and captured target no longer identify the same process.".to_string(),
        ));
    }
    for step in &plan.steps {
        let mut unique_keys = HashSet::new();
        if !step
            .keys
            .iter()
            .all(|key| unique_keys.insert(key.trim().to_ascii_lowercase()))
        {
            return Err(AppError::ExecutionPolicy(format!(
                "step '{}' repeats a key and was rejected to prevent stuck input.",
                step.id
            )));
        }
    }
    Ok(())
}

/// Reports whether the shared emergency-stop / cancellation flag has been set.
fn is_cancelled(cancellation: Option<&std::sync::Arc<std::sync::atomic::AtomicBool>>) -> bool {
    cancellation.is_some_and(|token| token.load(std::sync::atomic::Ordering::SeqCst))
}

fn run_steps(
    driver: &mut impl ActionDriver,
    plan: &ActionPlan,
    delay: Duration,
    cancellation: Option<&std::sync::Arc<std::sync::atomic::AtomicBool>>,
) -> ExecutionReport {
    let started_at_unix_ms = timestamp_ms();
    let mut results = Vec::with_capacity(plan.steps.len());
    let mut completed_steps = 0;
    let mut failure_message = None;
    let mut was_cancelled = false;

    for (index, step) in plan.steps.iter().enumerate() {
        if is_cancelled(cancellation) {
            release_stuck_inputs();
            was_cancelled = true;
            failure_message =
                Some("Execution stopped by emergency stop / cancellation.".to_string());
            break;
        }

        let started = Instant::now();
        match driver.perform(step) {
            Ok(method) => match driver.verify(&step.verification, cancellation) {
                Ok(verification) => {
                    completed_steps += 1;
                    results.push(ExecutionStepResult {
                        plan_attempt: 1,
                        id: step.id.clone(),
                        kind: step.kind,
                        target_id: step.target_id.clone(),
                        status: ExecutionStepStatus::Completed,
                        method: Some(method),
                        duration_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
                        verification: Some(verification),
                        error: None,
                    });
                    if index + 1 < plan.steps.len() {
                        let step_delay_start = Instant::now();
                        while step_delay_start.elapsed() < delay {
                            if is_cancelled(cancellation) {
                                release_stuck_inputs();
                                was_cancelled = true;
                                failure_message = Some(
                                    "Execution stopped by emergency stop / cancellation."
                                        .to_string(),
                                );
                                break;
                            }
                            thread::sleep(
                                Duration::from_millis(20)
                                    .min(delay.saturating_sub(step_delay_start.elapsed())),
                            );
                        }
                        if was_cancelled {
                            break;
                        }
                    }
                }
                Err(error) => {
                    if is_cancelled(cancellation) {
                        release_stuck_inputs();
                        was_cancelled = true;
                        failure_message =
                            Some("Execution stopped by emergency stop / cancellation.".to_string());
                        results.push(ExecutionStepResult {
                            plan_attempt: 1,
                            id: step.id.clone(),
                            kind: step.kind,
                            target_id: step.target_id.clone(),
                            status: ExecutionStepStatus::Cancelled,
                            method: Some(method),
                            duration_ms: started
                                .elapsed()
                                .as_millis()
                                .try_into()
                                .unwrap_or(u64::MAX),
                            verification: None,
                            error: Some(error),
                        });
                        break;
                    }
                    failure_message = Some(format!(
                        "Step '{}' changed the UI, but verification failed: {error}",
                        step.id
                    ));
                    results.push(ExecutionStepResult {
                        plan_attempt: 1,
                        id: step.id.clone(),
                        kind: step.kind,
                        target_id: step.target_id.clone(),
                        status: ExecutionStepStatus::VerificationFailed,
                        method: Some(method),
                        duration_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
                        verification: None,
                        error: Some(error),
                    });
                    break;
                }
            },
            Err(error) => {
                if is_cancelled(cancellation) {
                    release_stuck_inputs();
                    was_cancelled = true;
                    failure_message =
                        Some("Execution stopped by emergency stop / cancellation.".to_string());
                    results.push(ExecutionStepResult {
                        plan_attempt: 1,
                        id: step.id.clone(),
                        kind: step.kind,
                        target_id: step.target_id.clone(),
                        status: ExecutionStepStatus::Cancelled,
                        method: None,
                        duration_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
                        verification: None,
                        error: Some(error),
                    });
                    break;
                }
                failure_message = Some(format!("Step '{}' stopped: {error}", step.id));
                results.push(ExecutionStepResult {
                    plan_attempt: 1,
                    id: step.id.clone(),
                    kind: step.kind,
                    target_id: step.target_id.clone(),
                    status: ExecutionStepStatus::Failed,
                    method: None,
                    duration_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
                    verification: None,
                    error: Some(error),
                });
                break;
            }
        }
    }

    let status = if was_cancelled {
        ExecutionStatus::Cancelled
    } else if results
        .last()
        .is_some_and(|result| result.status == ExecutionStepStatus::VerificationFailed)
    {
        ExecutionStatus::VerificationFailed
    } else if failure_message.is_some() {
        ExecutionStatus::Failed
    } else {
        ExecutionStatus::Completed
    };

    ExecutionReport {
        started_at_unix_ms,
        finished_at_unix_ms: timestamp_ms(),
        status,
        total_steps: plan.steps.len(),
        completed_steps,
        plan_attempts: 1,
        replan_attempts: 0,
        recovered: false,
        step_results: results,
        failure_message,
        recovery_failure_message: None,
    }
}

// Eight explicit arguments are kept so every Tauri command, workflow recovery
// step, and developer probe passes policy inputs positionally without hiding
// them inside an opaque struct at the IPC boundary.
#[allow(clippy::too_many_arguments)]
pub fn execute_plan(
    context: &WindowContextSnapshot,
    snapshot: &UiAutomationSnapshot,
    plan: &ActionPlan,
    maximum_steps: usize,
    execution_delay_ms: u32,
    cancellation: Option<&std::sync::Arc<std::sync::atomic::AtomicBool>>,
    approval_policy: &crate::settings::ApprovalPolicy,
    approved_step_ids: &[String],
) -> AppResult<ExecutionReport> {
    validate_execution_policy(
        context,
        snapshot,
        plan,
        maximum_steps,
        approval_policy,
        approved_step_ids,
    )?;

    #[cfg(windows)]
    {
        let mut driver =
            platform::WindowsActionDriver::new(context, snapshot).map_err(AppError::Execution)?;
        Ok(run_steps(
            &mut driver,
            plan,
            Duration::from_millis(u64::from(execution_delay_ms)),
            cancellation,
        ))
    }

    #[cfg(not(windows))]
    {
        let _ = (
            context,
            snapshot,
            plan,
            maximum_steps,
            execution_delay_ms,
            cancellation,
            approval_policy,
            approved_step_ids,
        );
        Err(AppError::Execution(
            "the action executor is available only on Windows.".to_string(),
        ))
    }
}

#[cfg(windows)]
mod platform {
    use std::{
        ffi::c_void,
        mem::size_of,
        thread,
        time::{Duration, Instant},
    };

    use windows::{
        Win32::{
            Foundation::{HWND, RECT},
            System::Com::{
                CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
                CoUninitialize,
            },
            UI::{
                Accessibility::{
                    CUIAutomation8, IUIAutomation, IUIAutomationElement,
                    IUIAutomationInvokePattern, IUIAutomationScrollItemPattern,
                    IUIAutomationScrollPattern, IUIAutomationSelectionItemPattern,
                    IUIAutomationTogglePattern, IUIAutomationTreeWalker, IUIAutomationValuePattern,
                    ScrollAmount_NoAmount, ScrollAmount_SmallDecrement,
                    ScrollAmount_SmallIncrement, ToggleState_On, UIA_AppBarControlTypeId,
                    UIA_ButtonControlTypeId, UIA_CONTROLTYPE_ID, UIA_CalendarControlTypeId,
                    UIA_CheckBoxControlTypeId, UIA_ComboBoxControlTypeId, UIA_CustomControlTypeId,
                    UIA_DataGridControlTypeId, UIA_DataItemControlTypeId,
                    UIA_DocumentControlTypeId, UIA_EditControlTypeId, UIA_GroupControlTypeId,
                    UIA_HeaderControlTypeId, UIA_HeaderItemControlTypeId,
                    UIA_HyperlinkControlTypeId, UIA_ImageControlTypeId, UIA_InvokePatternId,
                    UIA_ListControlTypeId, UIA_ListItemControlTypeId, UIA_MenuBarControlTypeId,
                    UIA_MenuControlTypeId, UIA_MenuItemControlTypeId, UIA_PaneControlTypeId,
                    UIA_ProgressBarControlTypeId, UIA_RadioButtonControlTypeId,
                    UIA_ScrollBarControlTypeId, UIA_ScrollItemPatternId, UIA_ScrollPatternId,
                    UIA_SelectionItemPatternId, UIA_SeparatorControlTypeId,
                    UIA_SliderControlTypeId, UIA_SpinnerControlTypeId,
                    UIA_SplitButtonControlTypeId, UIA_StatusBarControlTypeId, UIA_TabControlTypeId,
                    UIA_TabItemControlTypeId, UIA_TableControlTypeId, UIA_TextControlTypeId,
                    UIA_ThumbControlTypeId, UIA_TitleBarControlTypeId, UIA_TogglePatternId,
                    UIA_ToolBarControlTypeId, UIA_ToolTipControlTypeId, UIA_TreeControlTypeId,
                    UIA_TreeItemControlTypeId, UIA_ValuePatternId, UIA_WindowControlTypeId,
                },
                Input::KeyboardAndMouse::{
                    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
                    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MOUSEEVENTF_HWHEEL,
                    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT,
                    SendInput, VIRTUAL_KEY, VK_BACK, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END,
                    VK_ESCAPE, VK_HOME, VK_LEFT, VK_LWIN, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN,
                    VK_RIGHT, VK_SHIFT, VK_SPACE, VK_TAB, VK_UP,
                },
                WindowsAndMessaging::{
                    GetForegroundWindow, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
                    GetWindowThreadProcessId, IsIconic, IsWindow, SetCursorPos,
                    SetForegroundWindow,
                },
            },
        },
        core::BSTR,
    };

    use crate::{
        ai::{ActionKind, PlannedAction, ScrollDirection, VerificationKind, VerificationSpec},
        context::{PixelRect, WindowContextSnapshot},
        uia::{NormalizedUiElement, UiAutomationSnapshot},
    };

    use super::{ActionDriver, VerificationEvidence, is_cancelled};

    const MAX_REVALIDATION_VISITS: usize = 1_500;
    const MAX_REVALIDATION_DURATION: Duration = Duration::from_millis(1_500);

    struct ComApartment;

    impl ComApartment {
        fn initialize() -> Result<Self, String> {
            // SAFETY: this execution worker owns the COM apartment for the guard lifetime.
            unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
                .ok()
                .map_err(|error| format!("Windows could not initialize UI Automation: {error}"))?;
            Ok(Self)
        }
    }

    impl Drop for ComApartment {
        fn drop(&mut self) {
            // SAFETY: initialization succeeded on this thread and is balanced once.
            unsafe { CoUninitialize() };
        }
    }

    #[derive(Clone, Debug)]
    struct Candidate {
        name: String,
        role: String,
        automation_id: String,
        class_name: String,
        framework_id: String,
        bounds: Option<PixelRect>,
        is_enabled: bool,
        is_offscreen: bool,
        is_keyboard_focusable: bool,
        has_keyboard_focus: bool,
        is_password: bool,
    }

    struct PendingElement {
        element: IUIAutomationElement,
        raw_depth: usize,
        normalized_parent: Option<Candidate>,
        is_root: bool,
    }

    struct ResolvedElement {
        element: IUIAutomationElement,
        candidate: Candidate,
    }

    pub struct WindowsActionDriver<'a> {
        _apartment: ComApartment,
        automation: IUIAutomation,
        walker: IUIAutomationTreeWalker,
        hwnd: HWND,
        context: &'a WindowContextSnapshot,
        snapshot: &'a UiAutomationSnapshot,
    }

    impl<'a> WindowsActionDriver<'a> {
        pub fn new(
            context: &'a WindowContextSnapshot,
            snapshot: &'a UiAutomationSnapshot,
        ) -> Result<Self, String> {
            if context.native_window_handle == 0 {
                return Err("the captured window handle is unavailable.".to_string());
            }
            let apartment = ComApartment::initialize()?;
            // SAFETY: COM is initialized and CUIAutomation8 is an in-process client class.
            let automation: IUIAutomation =
                unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) }
                    .map_err(|error| format!("Windows could not start UI Automation: {error}"))?;
            // SAFETY: automation is a live UIA client object.
            let walker = unsafe { automation.ControlViewWalker() }
                .map_err(|error| format!("Windows could not open the Control View: {error}"))?;
            let driver = Self {
                _apartment: apartment,
                automation,
                walker,
                hwnd: HWND(context.native_window_handle as *mut c_void),
                context,
                snapshot,
            };
            driver.root()?;
            Ok(driver)
        }

        fn root(&self) -> Result<IUIAutomationElement, String> {
            // SAFETY: HWND is never dereferenced; Windows validates the captured handle.
            if !unsafe { IsWindow(Some(self.hwnd)) }.as_bool() {
                return Err("the captured target window has closed.".to_string());
            }
            // SAFETY: HWND validity was checked immediately above.
            if unsafe { IsIconic(self.hwnd) }.as_bool() {
                return Err("the captured target is minimized.".to_string());
            }
            let mut process_id = 0_u32;
            // SAFETY: process_id is a valid writable pointer for the duration of the call.
            unsafe { GetWindowThreadProcessId(self.hwnd, Some(&mut process_id)) };
            if process_id == 0 || process_id != self.context.process.id {
                return Err("the captured window now belongs to a different process.".to_string());
            }
            // SAFETY: UIA validates the live HWND.
            let root = unsafe { self.automation.ElementFromHandle(self.hwnd) }
                .map_err(|error| format!("the captured target is no longer accessible: {error}"))?;
            // SAFETY: this is a read-only identity property on the UIA root.
            let uia_process_id = unsafe { root.CurrentProcessId() }
                .map_err(|error| format!("Windows could not verify the target process: {error}"))?;
            if uia_process_id <= 0 || uia_process_id as u32 != self.context.process.id {
                return Err("UI Automation resolved a different target process.".to_string());
            }
            Ok(root)
        }

        fn activate_for_input(&self) -> Result<(), String> {
            self.root()?;
            // SAFETY: the HWND was revalidated immediately before activation.
            let _ = unsafe { SetForegroundWindow(self.hwnd) };
            let deadline = Instant::now() + Duration::from_millis(650);
            while Instant::now() < deadline {
                // SAFETY: reads the current foreground HWND without dereferencing it.
                if unsafe { GetForegroundWindow() } == self.hwnd {
                    return Ok(());
                }
                thread::sleep(Duration::from_millis(25));
            }
            Err("Windows did not grant foreground focus; no synthetic input was sent.".to_string())
        }

        fn resolve(&self, target_id: &str) -> Result<ResolvedElement, String> {
            let expected = self
                .snapshot
                .elements
                .iter()
                .find(|element| element.id == target_id)
                .ok_or_else(|| format!("target '{target_id}' is not in the current snapshot."))?;
            let expected_parent = expected.parent_id.as_deref().and_then(|parent_id| {
                self.snapshot
                    .elements
                    .iter()
                    .find(|element| element.id == parent_id)
            });
            let root = self.root()?;
            let started = Instant::now();
            let mut stack = vec![PendingElement {
                element: root,
                raw_depth: 0,
                normalized_parent: None,
                is_root: true,
            }];
            let mut visited = 0_usize;
            let mut matches = Vec::new();

            while let Some(pending) = stack.pop() {
                if visited >= MAX_REVALIDATION_VISITS
                    || started.elapsed() >= MAX_REVALIDATION_DURATION
                {
                    return Err(format!(
                        "target '{target_id}' could not be revalidated inside the safety limit."
                    ));
                }
                visited += 1;
                let candidate = candidate(&pending.element);
                let include = should_include(&candidate, pending.is_root);
                if include
                    && matches_fingerprint(
                        &candidate,
                        pending.normalized_parent.as_ref(),
                        expected,
                        expected_parent,
                    )
                {
                    matches.push(ResolvedElement {
                        element: pending.element.clone(),
                        candidate: candidate.clone(),
                    });
                    if matches.len() > 1 {
                        return Err(format!(
                            "target '{target_id}' became ambiguous after the interface changed."
                        ));
                    }
                }

                if pending.raw_depth >= self.snapshot.limits.max_depth.min(12) {
                    continue;
                }
                let next_parent = if include {
                    Some(candidate)
                } else {
                    pending.normalized_parent
                };
                let (children, child_limit_reached) = children(
                    &self.walker,
                    &pending.element,
                    self.snapshot.limits.max_children_per_parent.min(250),
                );
                if child_limit_reached {
                    return Err(format!(
                        "target '{target_id}' could not be revalidated because its container exceeded the child safety limit."
                    ));
                }
                for child in children.into_iter().rev() {
                    stack.push(PendingElement {
                        element: child,
                        raw_depth: pending.raw_depth + 1,
                        normalized_parent: next_parent.clone(),
                        is_root: false,
                    });
                }
            }

            matches
                .pop()
                .ok_or_else(|| format!("target '{target_id}' moved, changed, or disappeared."))
        }

        fn root_resolved(&self) -> Result<ResolvedElement, String> {
            let root = self.root()?;
            let candidate = candidate(&root);
            Ok(ResolvedElement {
                element: root,
                candidate,
            })
        }

        fn validated_point(&self, candidate: &Candidate) -> Result<(i32, i32), String> {
            let bounds = candidate
                .bounds
                .as_ref()
                .ok_or_else(|| "the live target has no clickable screen rectangle.".to_string())?;
            if bounds.width == 0 || bounds.height == 0 {
                return Err("the live target has empty bounds.".to_string());
            }
            let center_x = i64::from(bounds.left) + i64::from(bounds.width) / 2;
            let center_y = i64::from(bounds.top) + i64::from(bounds.height) / 2;
            let x = i32::try_from(center_x)
                .map_err(|_| "the target x coordinate is outside the desktop.".to_string())?;
            let y = i32::try_from(center_y)
                .map_err(|_| "the target y coordinate is outside the desktop.".to_string())?;
            let mut window_rect = RECT::default();
            // SAFETY: HWND was revalidated and window_rect is writable for this call.
            unsafe { GetWindowRect(self.hwnd, &mut window_rect) }
                .map_err(|error| format!("Windows could not verify target bounds: {error}"))?;
            if x < window_rect.left
                || x >= window_rect.right
                || y < window_rect.top
                || y >= window_rect.bottom
            {
                return Err("the live target center is outside the captured window.".to_string());
            }
            Ok((x, y))
        }

        fn click_fallback(&self, target: &ResolvedElement) -> Result<String, String> {
            self.activate_for_input()?;
            let (x, y) = self.validated_point(&target.candidate)?;
            // SAFETY: coordinates were checked against the current target window.
            unsafe { SetCursorPos(x, y) }
                .map_err(|error| format!("Windows could not position the pointer: {error}"))?;
            send_inputs(&[
                mouse_input(MOUSEEVENTF_LEFTDOWN, 0),
                mouse_input(MOUSEEVENTF_LEFTUP, 0),
            ])?;
            Ok("validated_mouse_click".to_string())
        }

        fn type_text(&self, target: &ResolvedElement, text: &str) -> Result<String, String> {
            if target.candidate.is_password
                || crate::security::is_sensitive_control_indicator(
                    &target.candidate.name,
                    &target.candidate.automation_id,
                    &target.candidate.class_name,
                    &target.candidate.role,
                )
            {
                return Err("text entry into password controls is blocked.".to_string());
            }
            // SAFETY: the live element was resolved in this COM apartment.
            if let Ok(pattern) = unsafe {
                target
                    .element
                    .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
            } {
                // SAFETY: reads the current provider state before mutation.
                if unsafe { pattern.CurrentIsReadOnly() }
                    .map(|value| value.as_bool())
                    .unwrap_or(true)
                {
                    return Err("the target value is read-only.".to_string());
                }
                // SAFETY: text was schema-bounded and the pattern belongs to the resolved target.
                unsafe { pattern.SetValue(&BSTR::from(text)) }
                    .map_err(|error| format!("the Value pattern rejected text entry: {error}"))?;
                return Ok("uia_value_set".to_string());
            }

            self.activate_for_input()?;
            // SAFETY: focus is set only on the revalidated live element.
            unsafe { target.element.SetFocus() }
                .map_err(|error| format!("the target could not receive focus: {error}"))?;
            let mut inputs = Vec::with_capacity(text.encode_utf16().count() * 2);
            for unit in text.encode_utf16() {
                inputs.push(unicode_input(unit, false));
                inputs.push(unicode_input(unit, true));
            }
            send_inputs(&inputs)?;
            Ok("validated_unicode_input".to_string())
        }

        fn send_keys(&self, keys: &[String]) -> Result<String, String> {
            self.activate_for_input()?;
            ensure_modifiers_released()?;
            let mapped = keys
                .iter()
                .map(|key| virtual_key(key))
                .collect::<Result<Vec<_>, _>>()?;
            let mut inputs = Vec::with_capacity(mapped.len() * 2);
            for (key, extended) in &mapped {
                inputs.push(key_input(*key, false, *extended));
            }
            for (key, extended) in mapped.iter().rev() {
                inputs.push(key_input(*key, true, *extended));
            }
            send_inputs(&inputs)?;
            Ok(if keys.len() == 1 {
                "validated_key_press"
            } else {
                "validated_hotkey"
            }
            .to_string())
        }

        fn scroll(
            &self,
            target: Option<ResolvedElement>,
            direction: ScrollDirection,
            amount: u16,
        ) -> Result<String, String> {
            let target = match target {
                Some(target) => target,
                None => self.root_resolved()?,
            };
            // SAFETY: the live element was resolved in this COM apartment.
            if let Ok(pattern) = unsafe {
                target
                    .element
                    .GetCurrentPatternAs::<IUIAutomationScrollPattern>(UIA_ScrollPatternId)
            } {
                let (horizontal, vertical) = match direction {
                    ScrollDirection::Up => (ScrollAmount_NoAmount, ScrollAmount_SmallDecrement),
                    ScrollDirection::Down => (ScrollAmount_NoAmount, ScrollAmount_SmallIncrement),
                    ScrollDirection::Left => (ScrollAmount_SmallDecrement, ScrollAmount_NoAmount),
                    ScrollDirection::Right => (ScrollAmount_SmallIncrement, ScrollAmount_NoAmount),
                };
                for _ in 0..amount {
                    // SAFETY: amount and direction are bounded by the validated action schema.
                    unsafe { pattern.Scroll(horizontal, vertical) }.map_err(|error| {
                        format!("the Scroll pattern rejected movement: {error}")
                    })?;
                }
                return Ok("uia_scroll".to_string());
            }
            // SAFETY: the live element was resolved in this COM apartment.
            if let Ok(pattern) = unsafe {
                target
                    .element
                    .GetCurrentPatternAs::<IUIAutomationScrollItemPattern>(UIA_ScrollItemPatternId)
            } {
                // SAFETY: ScrollIntoView is constrained to the resolved target.
                unsafe { pattern.ScrollIntoView() }
                    .map_err(|error| format!("the ScrollItem pattern failed: {error}"))?;
                return Ok("uia_scroll_into_view".to_string());
            }

            self.activate_for_input()?;
            let (x, y) = self.validated_point(&target.candidate)?;
            // SAFETY: coordinates were checked against the current target window.
            unsafe { SetCursorPos(x, y) }
                .map_err(|error| format!("Windows could not position the pointer: {error}"))?;
            let delta = i32::from(amount) * 120;
            let (flags, signed_delta) = match direction {
                ScrollDirection::Up => (MOUSEEVENTF_WHEEL, delta),
                ScrollDirection::Down => (MOUSEEVENTF_WHEEL, -delta),
                ScrollDirection::Left => (MOUSEEVENTF_HWHEEL, -delta),
                ScrollDirection::Right => (MOUSEEVENTF_HWHEEL, delta),
            };
            send_inputs(&[mouse_input(flags, signed_delta as u32)])?;
            Ok("validated_mouse_wheel".to_string())
        }

        fn select(&self, target: &ResolvedElement, text: &str) -> Result<String, String> {
            if target.candidate.is_password
                || crate::security::is_sensitive_control_indicator(
                    &target.candidate.name,
                    &target.candidate.automation_id,
                    &target.candidate.class_name,
                    &target.candidate.role,
                )
            {
                return Err("selection in password controls is blocked.".to_string());
            }

            let mut matches = Vec::new();
            if target.candidate.name.eq_ignore_ascii_case(text) {
                // SAFETY: pattern lookup is read-only on the revalidated element.
                if let Ok(pattern) = unsafe {
                    target
                        .element
                        .GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(
                            UIA_SelectionItemPatternId,
                        )
                } {
                    matches.push(pattern);
                }
            }

            let (root_children, root_children_truncated) =
                children(&self.walker, &target.element, 100);
            if root_children_truncated {
                return Err("the selection container exceeded the child safety limit.".to_string());
            }
            let mut stack = root_children
                .into_iter()
                .map(|element| (element, 1_usize))
                .collect::<Vec<_>>();
            let mut visited = 0_usize;
            while let Some((element, depth)) = stack.pop() {
                if visited >= 350 {
                    return Err("the selection subtree exceeded the safety limit.".to_string());
                }
                visited += 1;
                let option = candidate(&element);
                if option.is_enabled && option.name.eq_ignore_ascii_case(text) {
                    // SAFETY: pattern lookup is read-only on a live target descendant.
                    if let Ok(pattern) = unsafe {
                        element.GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(
                            UIA_SelectionItemPatternId,
                        )
                    } {
                        matches.push(pattern);
                        if matches.len() > 1 {
                            return Err(format!(
                                "more than one option named '{text}' exists in the target."
                            ));
                        }
                    }
                }
                if depth < 8 {
                    let (descendants, descendant_limit_reached) =
                        children(&self.walker, &element, 100);
                    if descendant_limit_reached {
                        return Err(
                            "the selection subtree exceeded the child safety limit.".to_string()
                        );
                    }
                    for child in descendants.into_iter().rev() {
                        stack.push((child, depth + 1));
                    }
                }
            }

            if let Some(pattern) = matches.pop() {
                // SAFETY: this is the unique bounded option match beneath the target.
                unsafe { pattern.Select() }
                    .map_err(|error| format!("selection failed: {error}"))?;
                return Ok("uia_selection_item".to_string());
            }

            // Editable combo boxes can expose their selected text through Value instead.
            // SAFETY: pattern lookup is read-only on the revalidated target.
            if let Ok(pattern) = unsafe {
                target
                    .element
                    .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
            } {
                // SAFETY: reads the live provider state before mutation.
                if !unsafe { pattern.CurrentIsReadOnly() }
                    .map(|value| value.as_bool())
                    .unwrap_or(true)
                {
                    // SAFETY: text is schema-bounded and the pattern belongs to the target.
                    unsafe { pattern.SetValue(&BSTR::from(text)) }
                        .map_err(|error| format!("the selectable value was rejected: {error}"))?;
                    return Ok("uia_select_value".to_string());
                }
            }

            Err(format!(
                "no unique selectable option named '{text}' exists in the live target."
            ))
        }

        fn window_title(&self) -> Result<String, String> {
            self.root()?;
            // SAFETY: hwnd was revalidated above and Windows returns a bounded character count.
            let length = unsafe { GetWindowTextLengthW(self.hwnd) }.max(0) as usize;
            let mut buffer = vec![0_u16; length.saturating_add(1)];
            // SAFETY: buffer is writable and sized for the title plus its terminator.
            let copied = unsafe { GetWindowTextW(self.hwnd, &mut buffer) }.max(0) as usize;
            Ok(String::from_utf16_lossy(&buffer[..copied]))
        }

        fn check_verification(
            &self,
            verification: &VerificationSpec,
        ) -> Result<Option<&'static str>, String> {
            match verification.kind {
                VerificationKind::WindowExists => {
                    self.root()?;
                    Ok(Some("win32_window_identity"))
                }
                VerificationKind::WindowTitleContains => {
                    let expected = verification
                        .expected_text
                        .as_deref()
                        .ok_or_else(|| "the expected title fragment is missing.".to_string())?;
                    let matches = self
                        .window_title()?
                        .to_lowercase()
                        .contains(&expected.to_lowercase());
                    Ok(matches.then_some("win32_window_title"))
                }
                VerificationKind::ElementExists => {
                    let target_id = verification
                        .target_id
                        .as_deref()
                        .ok_or_else(|| "the verification target is missing.".to_string())?;
                    Ok(self
                        .resolve(target_id)
                        .is_ok()
                        .then_some("uia_element_identity"))
                }
                VerificationKind::HasKeyboardFocus => {
                    let target_id = verification
                        .target_id
                        .as_deref()
                        .ok_or_else(|| "the verification target is missing.".to_string())?;
                    let target = self.resolve(target_id)?;
                    Ok(target
                        .candidate
                        .has_keyboard_focus
                        .then_some("uia_keyboard_focus"))
                }
                VerificationKind::ValueEquals => {
                    let target_id = verification
                        .target_id
                        .as_deref()
                        .ok_or_else(|| "the verification target is missing.".to_string())?;
                    let expected = verification
                        .expected_text
                        .as_deref()
                        .ok_or_else(|| "the expected value is missing.".to_string())?;
                    let target = self.resolve(target_id)?;
                    // SAFETY: the live element was freshly resolved and the pattern is read-only.
                    let pattern = unsafe {
                        target
                            .element
                            .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
                    }
                    .map_err(|_| {
                        "the target no longer exposes a readable Value pattern.".to_string()
                    })?;
                    // SAFETY: CurrentValue reads provider state without mutation.
                    let current = unsafe { pattern.CurrentValue() }
                        .map_err(|_| "Windows could not read the target value.".to_string())?;
                    Ok((current == expected).then_some("uia_value_read"))
                }
                VerificationKind::ToggleState => {
                    let target_id = verification
                        .target_id
                        .as_deref()
                        .ok_or_else(|| "the verification target is missing.".to_string())?;
                    let expected = verification
                        .expected_bool
                        .ok_or_else(|| "the expected toggle state is missing.".to_string())?;
                    let target = self.resolve(target_id)?;
                    // SAFETY: the live element was freshly resolved and the pattern is read-only.
                    let pattern = unsafe {
                        target
                            .element
                            .GetCurrentPatternAs::<IUIAutomationTogglePattern>(UIA_TogglePatternId)
                    }
                    .map_err(|_| "the target no longer exposes Toggle state.".to_string())?;
                    // SAFETY: CurrentToggleState reads provider state without mutation.
                    let current = unsafe { pattern.CurrentToggleState() }
                        .map_err(|_| "Windows could not read the toggle state.".to_string())?;
                    Ok(((current == ToggleState_On) == expected).then_some("uia_toggle_state"))
                }
                VerificationKind::SelectionState => {
                    let target_id = verification
                        .target_id
                        .as_deref()
                        .ok_or_else(|| "the verification target is missing.".to_string())?;
                    let expected = verification
                        .expected_bool
                        .ok_or_else(|| "the expected selection state is missing.".to_string())?;
                    let target = self.resolve(target_id)?;
                    // SAFETY: the live element was freshly resolved and the pattern is read-only.
                    let pattern = unsafe {
                        target
                            .element
                            .GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(
                                UIA_SelectionItemPatternId,
                            )
                    }
                    .map_err(|_| "the target no longer exposes SelectionItem state.".to_string())?;
                    // SAFETY: CurrentIsSelected reads provider state without mutation.
                    let current = unsafe { pattern.CurrentIsSelected() }
                        .map_err(|_| "Windows could not read the selection state.".to_string())?;
                    Ok((current.as_bool() == expected).then_some("uia_selection_state"))
                }
            }
        }
    }

    impl ActionDriver for WindowsActionDriver<'_> {
        fn perform(&mut self, step: &PlannedAction) -> Result<String, String> {
            match step.kind {
                ActionKind::Wait => {
                    self.root()?;
                    let duration = step
                        .duration_ms
                        .filter(|duration| (50..=5_000).contains(duration))
                        .ok_or_else(|| {
                            "the wait duration is outside the safe range.".to_string()
                        })?;
                    thread::sleep(Duration::from_millis(duration));
                    self.root()?;
                    Ok("bounded_wait".to_string())
                }
                ActionKind::KeyPress | ActionKind::Hotkey => self.send_keys(&step.keys),
                ActionKind::Scroll => {
                    let target = step
                        .target_id
                        .as_deref()
                        .map(|id| self.resolve(id))
                        .transpose()?;
                    self.scroll(
                        target,
                        step.scroll_direction
                            .ok_or_else(|| "the scroll direction was not provided.".to_string())?,
                        step.amount
                            .filter(|amount| (1..=20).contains(amount))
                            .ok_or_else(|| {
                                "the scroll amount is outside the safe range.".to_string()
                            })?,
                    )
                }
                _ => {
                    let target_id = step
                        .target_id
                        .as_deref()
                        .ok_or_else(|| "the action has no target.".to_string())?;
                    let target = self.resolve(target_id)?;
                    match step.kind {
                        ActionKind::Focus => {
                            // SAFETY: the target was freshly resolved in this COM apartment.
                            unsafe { target.element.SetFocus() }.map_err(|error| {
                                format!("the target could not receive focus: {error}")
                            })?;
                            Ok("uia_set_focus".to_string())
                        }
                        ActionKind::Invoke => {
                            // SAFETY: the target was freshly resolved in this COM apartment.
                            let pattern = unsafe {
                                target
                                    .element
                                    .GetCurrentPatternAs::<IUIAutomationInvokePattern>(
                                        UIA_InvokePatternId,
                                    )
                            }
                            .map_err(|_| "the target no longer supports Invoke.".to_string())?;
                            // SAFETY: Invoke is constrained to the resolved target.
                            unsafe { pattern.Invoke() }
                                .map_err(|error| format!("Invoke failed: {error}"))?;
                            Ok("uia_invoke".to_string())
                        }
                        ActionKind::Click => {
                            // SAFETY: pattern lookup is read-only on the revalidated element.
                            if let Ok(pattern) = unsafe {
                                target
                                    .element
                                    .GetCurrentPatternAs::<IUIAutomationInvokePattern>(
                                        UIA_InvokePatternId,
                                    )
                            } {
                                // SAFETY: Invoke is constrained to the resolved target.
                                unsafe { pattern.Invoke() }
                                    .map_err(|error| format!("Invoke failed: {error}"))?;
                                Ok("uia_invoke".to_string())
                            } else {
                                self.click_fallback(&target)
                            }
                        }
                        ActionKind::TypeText => self.type_text(
                            &target,
                            step.text
                                .as_deref()
                                .ok_or_else(|| "the text value is missing.".to_string())?,
                        ),
                        ActionKind::Select => {
                            let expected_name = step
                                .text
                                .as_deref()
                                .ok_or_else(|| "the selection label is missing.".to_string())?;
                            self.select(&target, expected_name)
                        }
                        ActionKind::Toggle => {
                            // SAFETY: the target was freshly resolved in this COM apartment.
                            let pattern = unsafe {
                                target
                                    .element
                                    .GetCurrentPatternAs::<IUIAutomationTogglePattern>(
                                        UIA_TogglePatternId,
                                    )
                            }
                            .map_err(|_| "the target no longer supports Toggle.".to_string())?;
                            // SAFETY: Toggle is constrained to the resolved target.
                            unsafe { pattern.Toggle() }
                                .map_err(|error| format!("toggle failed: {error}"))?;
                            Ok("uia_toggle".to_string())
                        }
                        ActionKind::KeyPress
                        | ActionKind::Hotkey
                        | ActionKind::Scroll
                        | ActionKind::Wait => unreachable!(),
                    }
                }
            }
        }

        fn verify(
            &mut self,
            verification: &VerificationSpec,
            cancellation: Option<&std::sync::Arc<std::sync::atomic::AtomicBool>>,
        ) -> Result<VerificationEvidence, String> {
            let started = Instant::now();
            let timeout = Duration::from_millis(verification.timeout_ms);
            let mut attempts = 0_u32;
            let mut last_error = None;
            loop {
                if is_cancelled(cancellation) {
                    return Err("verification interrupted by cancellation".to_string());
                }
                attempts = attempts.saturating_add(1);
                match self.check_verification(verification) {
                    Ok(Some(method)) => {
                        return Ok(VerificationEvidence {
                            kind: verification.kind,
                            method: method.to_string(),
                            attempts,
                            duration_ms: started
                                .elapsed()
                                .as_millis()
                                .try_into()
                                .unwrap_or(u64::MAX),
                        });
                    }
                    Ok(None) => {}
                    Err(error) => last_error = Some(error),
                }
                if started.elapsed() >= timeout {
                    break;
                }
                for _ in 0..4 {
                    if is_cancelled(cancellation) {
                        return Err("verification interrupted by cancellation".to_string());
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
            Err(last_error.unwrap_or_else(|| {
                format!(
                    "the {:?} condition was not observed within {} ms.",
                    verification.kind, verification.timeout_ms
                )
            }))
        }
    }

    fn sanitize_text(value: &str) -> String {
        value
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(220)
            .collect()
    }

    fn role_for(control_type: UIA_CONTROLTYPE_ID, localized: String) -> String {
        let role = match control_type {
            value if value == UIA_AppBarControlTypeId => "app-bar",
            value if value == UIA_ButtonControlTypeId => "button",
            value if value == UIA_CalendarControlTypeId => "calendar",
            value if value == UIA_CheckBoxControlTypeId => "checkbox",
            value if value == UIA_ComboBoxControlTypeId => "combobox",
            value if value == UIA_CustomControlTypeId => "custom",
            value if value == UIA_DataGridControlTypeId => "data-grid",
            value if value == UIA_DataItemControlTypeId => "data-item",
            value if value == UIA_DocumentControlTypeId => "document",
            value if value == UIA_EditControlTypeId => "edit",
            value if value == UIA_GroupControlTypeId => "group",
            value if value == UIA_HeaderControlTypeId => "header",
            value if value == UIA_HeaderItemControlTypeId => "header-item",
            value if value == UIA_HyperlinkControlTypeId => "hyperlink",
            value if value == UIA_ImageControlTypeId => "image",
            value if value == UIA_ListControlTypeId => "list",
            value if value == UIA_ListItemControlTypeId => "list-item",
            value if value == UIA_MenuBarControlTypeId => "menu-bar",
            value if value == UIA_MenuControlTypeId => "menu",
            value if value == UIA_MenuItemControlTypeId => "menu-item",
            value if value == UIA_PaneControlTypeId => "pane",
            value if value == UIA_ProgressBarControlTypeId => "progress-bar",
            value if value == UIA_RadioButtonControlTypeId => "radio-button",
            value if value == UIA_ScrollBarControlTypeId => "scrollbar",
            value if value == UIA_SeparatorControlTypeId => "separator",
            value if value == UIA_SliderControlTypeId => "slider",
            value if value == UIA_SpinnerControlTypeId => "spinner",
            value if value == UIA_SplitButtonControlTypeId => "split-button",
            value if value == UIA_StatusBarControlTypeId => "status-bar",
            value if value == UIA_TabControlTypeId => "tab",
            value if value == UIA_TabItemControlTypeId => "tab-item",
            value if value == UIA_TableControlTypeId => "table",
            value if value == UIA_TextControlTypeId => "text",
            value if value == UIA_ThumbControlTypeId => "thumb",
            value if value == UIA_TitleBarControlTypeId => "title-bar",
            value if value == UIA_ToolBarControlTypeId => "toolbar",
            value if value == UIA_ToolTipControlTypeId => "tooltip",
            value if value == UIA_TreeControlTypeId => "tree",
            value if value == UIA_TreeItemControlTypeId => "tree-item",
            value if value == UIA_WindowControlTypeId => "window",
            _ => "",
        };
        if role.is_empty() {
            let localized = localized.to_lowercase().replace(' ', "-");
            if localized.is_empty() {
                "unknown".to_string()
            } else {
                localized
            }
        } else {
            role.to_string()
        }
    }

    fn string_property(result: windows::core::Result<BSTR>) -> String {
        result
            .map(|value| sanitize_text(&String::from_utf16_lossy(&value)))
            .unwrap_or_default()
    }

    fn bool_property(result: windows::core::Result<windows::core::BOOL>, fallback: bool) -> bool {
        result.map(|value| value.as_bool()).unwrap_or(fallback)
    }

    fn bounds_property(result: windows::core::Result<RECT>) -> Option<PixelRect> {
        let rect = result.ok()?;
        Some(PixelRect {
            left: rect.left,
            top: rect.top,
            width: rect.right.checked_sub(rect.left)?.try_into().ok()?,
            height: rect.bottom.checked_sub(rect.top)?.try_into().ok()?,
        })
        .filter(|bounds| bounds.width > 0 && bounds.height > 0)
    }

    fn candidate(element: &IUIAutomationElement) -> Candidate {
        // SAFETY: all calls are read-only UIA properties on a live element.
        unsafe {
            Candidate {
                name: string_property(element.CurrentName()),
                role: role_for(
                    element
                        .CurrentControlType()
                        .unwrap_or(UIA_CONTROLTYPE_ID(0)),
                    string_property(element.CurrentLocalizedControlType()),
                ),
                automation_id: string_property(element.CurrentAutomationId()),
                class_name: string_property(element.CurrentClassName()),
                framework_id: string_property(element.CurrentFrameworkId()),
                bounds: bounds_property(element.CurrentBoundingRectangle()),
                is_enabled: bool_property(element.CurrentIsEnabled(), false),
                is_offscreen: bool_property(element.CurrentIsOffscreen(), true),
                is_keyboard_focusable: bool_property(element.CurrentIsKeyboardFocusable(), false),
                has_keyboard_focus: bool_property(element.CurrentHasKeyboardFocus(), false),
                is_password: bool_property(element.CurrentIsPassword(), false),
            }
        }
    }

    fn should_include(candidate: &Candidate, is_root: bool) -> bool {
        if is_root {
            return true;
        }
        if candidate.is_offscreen && !candidate.has_keyboard_focus {
            return false;
        }
        if candidate.bounds.is_none()
            && !candidate.is_keyboard_focusable
            && !candidate.has_keyboard_focus
        {
            return false;
        }
        if matches!(
            candidate.role.as_str(),
            "button"
                | "calendar"
                | "checkbox"
                | "combobox"
                | "data-grid"
                | "data-item"
                | "document"
                | "edit"
                | "hyperlink"
                | "list"
                | "list-item"
                | "menu"
                | "menu-item"
                | "radio-button"
                | "scrollbar"
                | "slider"
                | "spinner"
                | "split-button"
                | "tab"
                | "tab-item"
                | "table"
                | "tree"
                | "tree-item"
                | "window"
        ) {
            return true;
        }
        if !candidate.name.is_empty() || !candidate.automation_id.is_empty() {
            return true;
        }
        !matches!(
            candidate.role.as_str(),
            "pane" | "group" | "custom" | "unknown"
        )
    }

    fn children(
        walker: &IUIAutomationTreeWalker,
        parent: &IUIAutomationElement,
        limit: usize,
    ) -> (Vec<IUIAutomationElement>, bool) {
        let mut result = Vec::new();
        // SAFETY: walker and parent are live interfaces in this apartment.
        let Ok(mut current) = (unsafe { walker.GetFirstChildElement(parent) }) else {
            return (result, false);
        };
        loop {
            result.push(current.clone());
            if result.len() >= limit {
                // SAFETY: current came from this walker and remains alive for the call.
                let truncated = unsafe { walker.GetNextSiblingElement(&current) }.is_ok();
                return (result, truncated);
            }
            // SAFETY: current came from this walker and remains alive for the call.
            match unsafe { walker.GetNextSiblingElement(&current) } {
                Ok(next) => current = next,
                Err(_) => return (result, false),
            }
        }
    }

    fn same_nonempty(expected: &str, actual: &str) -> bool {
        expected.is_empty() || expected == actual
    }

    fn bounds_match(expected: &Option<PixelRect>, actual: &Option<PixelRect>) -> bool {
        let (Some(expected), Some(actual)) = (expected, actual) else {
            return expected.is_none() && actual.is_none();
        };
        let expected_center_x = i64::from(expected.left) + i64::from(expected.width) / 2;
        let expected_center_y = i64::from(expected.top) + i64::from(expected.height) / 2;
        let actual_center_x = i64::from(actual.left) + i64::from(actual.width) / 2;
        let actual_center_y = i64::from(actual.top) + i64::from(actual.height) / 2;
        let x_tolerance = i64::from((expected.width / 4).max(16));
        let y_tolerance = i64::from((expected.height / 4).max(16));
        let width_tolerance = (expected.width / 3).max(8);
        let height_tolerance = (expected.height / 3).max(8);
        (expected_center_x - actual_center_x).abs() <= x_tolerance
            && (expected_center_y - actual_center_y).abs() <= y_tolerance
            && expected.width.abs_diff(actual.width) <= width_tolerance
            && expected.height.abs_diff(actual.height) <= height_tolerance
    }

    fn parent_matches(expected: &NormalizedUiElement, actual: &Candidate) -> bool {
        expected.role == actual.role
            && same_nonempty(&expected.name, &actual.name)
            && same_nonempty(&expected.automation_id, &actual.automation_id)
            && same_nonempty(&expected.class_name, &actual.class_name)
            && same_nonempty(&expected.framework_id, &actual.framework_id)
    }

    fn matches_fingerprint(
        actual: &Candidate,
        actual_parent: Option<&Candidate>,
        expected: &NormalizedUiElement,
        expected_parent: Option<&NormalizedUiElement>,
    ) -> bool {
        if !actual.is_enabled || actual.is_offscreen || actual.role != expected.role {
            return false;
        }
        if !same_nonempty(&expected.name, &actual.name)
            || !same_nonempty(&expected.automation_id, &actual.automation_id)
            || !same_nonempty(&expected.class_name, &actual.class_name)
            || !same_nonempty(&expected.framework_id, &actual.framework_id)
            || !bounds_match(&expected.bounds_physical, &actual.bounds)
        {
            return false;
        }
        match (expected_parent, actual_parent) {
            (Some(expected), Some(actual)) => parent_matches(expected, actual),
            (None, None) => true,
            _ => false,
        }
    }

    fn mouse_input(
        flags: windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS,
        data: u32,
    ) -> INPUT {
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    mouseData: data,
                    dwFlags: flags,
                    ..Default::default()
                },
            },
        }
    }

    fn unicode_input(unit: u16, key_up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wScan: unit,
                    dwFlags: if key_up {
                        KEYEVENTF_UNICODE | KEYEVENTF_KEYUP
                    } else {
                        KEYEVENTF_UNICODE
                    },
                    ..Default::default()
                },
            },
        }
    }

    fn key_input(key: VIRTUAL_KEY, key_up: bool, extended: bool) -> INPUT {
        let mut flags = if extended {
            KEYEVENTF_EXTENDEDKEY
        } else {
            Default::default()
        };
        if key_up {
            flags |= KEYEVENTF_KEYUP;
        }
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: key,
                    dwFlags: flags,
                    ..Default::default()
                },
            },
        }
    }

    fn send_inputs(inputs: &[INPUT]) -> Result<(), String> {
        if inputs.is_empty() {
            return Err("no input events were generated.".to_string());
        }
        // SAFETY: inputs is a valid contiguous slice and cbSize matches INPUT exactly.
        let sent = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };
        if sent as usize == inputs.len() {
            Ok(())
        } else {
            Err(format!(
                "Windows accepted {sent} of {} input events; execution stopped.",
                inputs.len()
            ))
        }
    }

    fn ensure_modifiers_released() -> Result<(), String> {
        for (name, key) in [
            ("Control", VK_CONTROL),
            ("Shift", VK_SHIFT),
            ("Alt", VK_MENU),
            ("Windows", VK_LWIN),
        ] {
            // SAFETY: GetAsyncKeyState reads the state for a known virtual key.
            if unsafe { GetAsyncKeyState(i32::from(key.0)) } < 0 {
                return Err(format!(
                    "the {name} key is already held; synthetic input was not sent."
                ));
            }
        }
        Ok(())
    }

    fn virtual_key(value: &str) -> Result<(VIRTUAL_KEY, bool), String> {
        let normalized = value.trim().to_ascii_lowercase();
        if normalized.len() == 1 {
            let byte = normalized.as_bytes()[0];
            if byte.is_ascii_alphanumeric() {
                return Ok((VIRTUAL_KEY(u16::from(byte.to_ascii_uppercase())), false));
            }
        }
        let result = match normalized.as_str() {
            "alt" => (VK_MENU, false),
            "control" | "ctrl" => (VK_CONTROL, false),
            "shift" => (VK_SHIFT, false),
            "win" => (VK_LWIN, true),
            "enter" => (VK_RETURN, false),
            "escape" | "esc" => (VK_ESCAPE, false),
            "tab" => (VK_TAB, false),
            "space" => (VK_SPACE, false),
            "backspace" => (VK_BACK, false),
            "delete" => (VK_DELETE, true),
            "home" => (VK_HOME, true),
            "end" => (VK_END, true),
            "pageup" => (VK_PRIOR, true),
            "pagedown" => (VK_NEXT, true),
            "arrowup" => (VK_UP, true),
            "arrowdown" => (VK_DOWN, true),
            "arrowleft" => (VK_LEFT, true),
            "arrowright" => (VK_RIGHT, true),
            key if key.starts_with('f') => {
                let number = key[1..]
                    .parse::<u16>()
                    .map_err(|_| format!("key '{value}' is not allowlisted."))?;
                if !(1..=12).contains(&number) {
                    return Err(format!("key '{value}' is not allowlisted."));
                }
                (VIRTUAL_KEY(0x6f + number), false)
            }
            _ => return Err(format!("key '{value}' is not allowlisted.")),
        };
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ai::{PlanStatus, ScrollDirection},
        context::{
            ActiveTargetSummary, LogicalRect, MonitorMetadata, PixelRect, ProcessMetadata,
            ScreenshotData,
        },
        uia::{NormalizedUiElement, UiAutomationLimits},
    };

    struct MockDriver {
        fail_on: Option<String>,
        fail_verification: bool,
        calls: Vec<String>,
    }

    impl ActionDriver for MockDriver {
        fn perform(&mut self, step: &PlannedAction) -> Result<String, String> {
            self.calls.push(step.id.clone());
            if self.fail_on.as_deref() == Some(step.id.as_str()) {
                Err("simulated stale target".to_string())
            } else {
                Ok("mock".to_string())
            }
        }

        fn verify(
            &mut self,
            verification: &VerificationSpec,
            _cancellation: Option<&std::sync::Arc<std::sync::atomic::AtomicBool>>,
        ) -> Result<VerificationEvidence, String> {
            if self.fail_verification {
                Err("simulated changed state".to_string())
            } else {
                Ok(VerificationEvidence {
                    kind: verification.kind,
                    method: "mock_observation".to_string(),
                    attempts: 1,
                    duration_ms: 0,
                })
            }
        }
    }

    fn context() -> WindowContextSnapshot {
        let bounds = PixelRect {
            left: 0,
            top: 0,
            width: 800,
            height: 600,
        };
        WindowContextSnapshot {
            native_window_handle: 1,
            captured_at_unix_ms: 1,
            window_handle: "0x1".to_string(),
            title: "Target".to_string(),
            class_name: "TargetClass".to_string(),
            process: ProcessMetadata {
                id: 42,
                name: Some("target.exe".to_string()),
                executable_path: None,
            },
            bounds_physical: bounds.clone(),
            bounds_logical: LogicalRect {
                left: 0.0,
                top: 0.0,
                width: 800.0,
                height: 600.0,
            },
            dpi: 96,
            scale_factor: 1.0,
            monitor: MonitorMetadata {
                device_name: "DISPLAY1".to_string(),
                bounds_physical: bounds.clone(),
                work_area_physical: bounds,
                is_primary: true,
            },
            screenshot: ScreenshotData {
                mime_type: "image/png".to_string(),
                data_url: "data:image/png;base64,".to_string(),
                width_px: 1,
                height_px: 1,
                byte_size: 0,
                capture_method: "test".to_string(),
            },
            warnings: Vec::new(),
        }
    }

    fn snapshot() -> UiAutomationSnapshot {
        UiAutomationSnapshot {
            captured_at_unix_ms: 1,
            target: ActiveTargetSummary {
                title: "Target".to_string(),
                process_name: Some("target.exe".to_string()),
                process_id: 42,
            },
            root_id: Some("uia-0001".to_string()),
            elements: vec![NormalizedUiElement {
                id: "uia-0001".to_string(),
                parent_id: None,
                depth: 0,
                name: "Target".to_string(),
                role: "window".to_string(),
                automation_id: String::new(),
                class_name: "TargetClass".to_string(),
                framework_id: "Win32".to_string(),
                bounds_physical: Some(PixelRect {
                    left: 0,
                    top: 0,
                    width: 800,
                    height: 600,
                }),
                is_enabled: true,
                is_offscreen: false,
                is_keyboard_focusable: true,
                has_keyboard_focus: false,
                is_password: false,
                supported_patterns: Vec::new(),
            }],
            visited_count: 1,
            filtered_count: 0,
            truncated: false,
            duration_ms: 1,
            limits: UiAutomationLimits::default(),
            warnings: Vec::new(),
        }
    }

    fn wait_step(id: &str, risk: RiskLevel) -> PlannedAction {
        PlannedAction {
            id: id.to_string(),
            kind: ActionKind::Wait,
            target_id: None,
            text: None,
            keys: Vec::new(),
            scroll_direction: None::<ScrollDirection>,
            amount: None,
            duration_ms: Some(50),
            description: "Wait briefly".to_string(),
            expected_result: "Target remains available".to_string(),
            verification: VerificationSpec {
                kind: VerificationKind::WindowExists,
                target_id: None,
                expected_text: None,
                expected_bool: None,
                timeout_ms: 100,
            },
            risk,
            requires_user_approval: risk == RiskLevel::High,
        }
    }

    fn plan(steps: Vec<PlannedAction>) -> ActionPlan {
        ActionPlan {
            status: PlanStatus::Ready,
            title: "Test plan".to_string(),
            summary: "Exercise deterministic execution.".to_string(),
            overall_risk: steps
                .iter()
                .map(|step| step.risk)
                .max()
                .unwrap_or(RiskLevel::Low),
            steps,
        }
    }

    #[test]
    fn stops_after_first_failed_step_and_reports_partial_progress() {
        let plan = plan(vec![
            wait_step("one", RiskLevel::Low),
            wait_step("two", RiskLevel::Low),
            wait_step("three", RiskLevel::Low),
        ]);
        let mut driver = MockDriver {
            fail_on: Some("two".to_string()),
            fail_verification: false,
            calls: Vec::new(),
        };
        let report = run_steps(&mut driver, &plan, Duration::ZERO, None);
        assert_eq!(report.status, ExecutionStatus::Failed);
        assert_eq!(report.completed_steps, 1);
        assert_eq!(driver.calls, vec!["one", "two"]);
        assert_eq!(report.step_results.len(), 2);
    }

    #[test]
    fn verification_failure_stops_before_the_next_stale_action() {
        let plan = plan(vec![
            wait_step("changed", RiskLevel::Low),
            wait_step("must-not-run", RiskLevel::Low),
        ]);
        let mut driver = MockDriver {
            fail_on: None,
            fail_verification: true,
            calls: Vec::new(),
        };
        let report = run_steps(&mut driver, &plan, Duration::ZERO, None);
        assert_eq!(report.status, ExecutionStatus::VerificationFailed);
        assert_eq!(report.completed_steps, 0);
        assert_eq!(driver.calls, vec!["changed"]);
        assert_eq!(
            report.step_results[0].status,
            ExecutionStepStatus::VerificationFailed
        );
    }

    #[test]
    fn blocks_high_risk_plans_without_approval() {
        let plan = plan(vec![wait_step("danger", RiskLevel::High)]);
        let error = validate_execution_policy(
            &context(),
            &snapshot(),
            &plan,
            12,
            &crate::settings::ApprovalPolicy::Balanced,
            &[],
        )
        .expect_err("high risk must remain blocked without approval");
        assert!(matches!(error, AppError::ExecutionPolicy(_)));
    }

    #[test]
    fn allows_high_risk_plans_with_granular_approval() {
        let plan = plan(vec![wait_step("danger", RiskLevel::High)]);
        let result = validate_execution_policy(
            &context(),
            &snapshot(),
            &plan,
            12,
            &crate::settings::ApprovalPolicy::Balanced,
            &["danger".to_string()],
        );
        assert!(result.is_ok());
    }

    #[test]
    fn always_ask_policy_requires_approval_for_all_steps() {
        let plan = plan(vec![wait_step("safe", RiskLevel::Low)]);
        assert!(
            validate_execution_policy(
                &context(),
                &snapshot(),
                &plan,
                12,
                &crate::settings::ApprovalPolicy::AlwaysAsk,
                &[],
            )
            .is_err()
        );
        assert!(
            validate_execution_policy(
                &context(),
                &snapshot(),
                &plan,
                12,
                &crate::settings::ApprovalPolicy::AlwaysAsk,
                &["safe".to_string()],
            )
            .is_ok()
        );
    }

    #[test]
    fn cancellation_stops_execution_and_marks_status() {
        let plan = plan(vec![
            wait_step("one", RiskLevel::Low),
            wait_step("two", RiskLevel::Low),
        ]);
        let mut driver = MockDriver {
            fail_on: None,
            fail_verification: false,
            calls: Vec::new(),
        };
        let token = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let report = run_steps(&mut driver, &plan, Duration::ZERO, Some(&token));
        assert_eq!(report.status, ExecutionStatus::Cancelled);
        assert_eq!(report.completed_steps, 0);
        assert!(driver.calls.is_empty());
    }

    #[test]
    fn enforces_current_step_limit_and_target_identity() {
        let plan = plan(vec![
            wait_step("one", RiskLevel::Low),
            wait_step("two", RiskLevel::Low),
        ]);
        assert!(
            validate_execution_policy(
                &context(),
                &snapshot(),
                &plan,
                1,
                &crate::settings::ApprovalPolicy::Balanced,
                &[],
            )
            .is_err()
        );
        let mut wrong_snapshot = snapshot();
        wrong_snapshot.target.process_id = 7;
        assert!(
            validate_execution_policy(
                &context(),
                &wrong_snapshot,
                &plan,
                12,
                &crate::settings::ApprovalPolicy::Balanced,
                &[],
            )
            .is_err()
        );
    }
}
