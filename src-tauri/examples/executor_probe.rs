#[cfg(windows)]
mod windows_probe {
    use std::{thread, time::Duration};

    use deskflow_ai_lib::{
        ai::{
            ActionKind, ActionPlan, PlanStatus, PlannedAction, RiskLevel, VerificationKind,
            VerificationSpec,
        },
        context,
        executor::{self, ExecutionStatus},
        uia, workflow,
    };
    use windows::{
        Win32::UI::WindowsAndMessaging::{FindWindowExW, FindWindowW, MoveWindow, SetWindowTextW},
        core::{PCWSTR, w},
    };

    fn action(id: &str, kind: ActionKind, target_id: String, text: Option<&str>) -> PlannedAction {
        PlannedAction {
            id: id.to_string(),
            kind,
            target_id: Some(target_id.clone()),
            text: text.map(str::to_string),
            keys: Vec::new(),
            scroll_direction: None,
            amount: None,
            duration_ms: None,
            description: id.replace('-', " "),
            expected_result: "The deterministic target advances.".to_string(),
            verification: VerificationSpec {
                kind: match kind {
                    ActionKind::TypeText => VerificationKind::ValueEquals,
                    ActionKind::Focus => VerificationKind::HasKeyboardFocus,
                    _ => VerificationKind::WindowTitleContains,
                },
                target_id: matches!(kind, ActionKind::TypeText | ActionKind::Focus)
                    .then(|| target_id.clone()),
                expected_text: match kind {
                    ActionKind::TypeText => Some(text.unwrap_or_default().to_string()),
                    ActionKind::Focus => None,
                    _ => Some("Complete".to_string()),
                },
                expected_bool: None,
                timeout_ms: 1_000,
            },
            risk: RiskLevel::Low,
            requires_user_approval: false,
        }
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        // SAFETY: searches for the fixed class owned by the local deterministic target.
        let hwnd = unsafe { FindWindowW(w!("DeskFlowExecutorTarget"), PCWSTR::null()) }?;
        let context = context::capture_window_context(hwnd.0 as usize)?;
        if context.class_name != "DeskFlowExecutorTarget" {
            return Err(format!("captured unexpected class: {}", context.class_name).into());
        }
        let snapshot = uia::inspect_captured_window(&context)?;
        let edit = snapshot
            .elements
            .iter()
            .find(|element| element.role == "edit")
            .ok_or("the Name edit control was not exposed through UIA")?;
        let button = snapshot
            .elements
            .iter()
            .find(|element| element.role == "button" && element.name == "Continue")
            .ok_or("the Continue button was not exposed through UIA")?;

        let mut plan = ActionPlan {
            status: PlanStatus::Ready,
            title: "Enter a name and continue".to_string(),
            summary: "Enter Mike into Name and invoke Continue.".to_string(),
            overall_risk: RiskLevel::Low,
            steps: vec![
                action(
                    "enter-name",
                    ActionKind::TypeText,
                    edit.id.clone(),
                    Some("Mike"),
                ),
                action("continue", ActionKind::Click, button.id.clone(), None),
            ],
        };
        plan.steps[0].verification = VerificationSpec {
            kind: VerificationKind::ElementExists,
            target_id: Some(button.id.clone()),
            expected_text: None,
            expected_bool: None,
            timeout_ms: 400,
        };

        // Simulate the target application refreshing after planning but before execution.
        // SAFETY: searches beneath the deterministic target for its known child button.
        let live_button = unsafe { FindWindowExW(Some(hwnd), None, w!("BUTTON"), w!("Continue")) }?;
        // SAFETY: live_button is owned by the deterministic local target.
        unsafe { SetWindowTextW(live_button, w!("Continue after refresh")) }?;
        // SAFETY: the new geometry remains inside the deterministic target window.
        unsafe { MoveWindow(live_button, 124, 96, 190, 34, true) }?;
        thread::sleep(Duration::from_millis(100));

        let first_report = executor::execute_plan(
            &context,
            &snapshot,
            &plan,
            12,
            100,
            None,
            &deskflow_ai_lib::settings::ApprovalPolicy::Balanced,
            &[],
        )?;
        if first_report.status != ExecutionStatus::VerificationFailed
            || first_report
                .step_results
                .last()
                .is_none_or(|step| step.id != "enter-name")
        {
            return Err(format!(
                "the predictable interface change was not detected at Continue: {first_report:?}"
            )
            .into());
        }

        let fresh_context = context::capture_window_context(hwnd.0 as usize)?;
        let fresh_snapshot = uia::inspect_captured_window(&fresh_context)?;
        let refreshed_button = fresh_snapshot
            .elements
            .iter()
            .find(|element| element.role == "button" && element.name == "Continue after refresh")
            .ok_or_else(|| {
                let buttons = fresh_snapshot
                    .elements
                    .iter()
                    .filter(|element| element.role == "button")
                    .map(|element| format!("{}@{:?}", element.name, element.bounds_physical))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "the changed Continue button was not found during re-inspection; report={first_report:?}; buttons={buttons}"
                )
            })?;
        let recovery_plan = ActionPlan {
            status: PlanStatus::Ready,
            title: "Continue from the changed layout".to_string(),
            summary: "Use the newly observed Continue control.".to_string(),
            overall_risk: RiskLevel::Low,
            steps: vec![action(
                "continue-after-refresh",
                ActionKind::Click,
                refreshed_button.id.clone(),
                None,
            )],
        };
        let recovery_report = executor::execute_plan(
            &fresh_context,
            &fresh_snapshot,
            &recovery_plan,
            10,
            100,
            None,
            &deskflow_ai_lib::settings::ApprovalPolicy::Balanced,
            &[],
        )?;
        let report = workflow::merge_attempt(Some(first_report), recovery_report);
        if report.status != ExecutionStatus::Completed || !report.recovered {
            return Err(format!("bounded recovery failed: {:?}", report.failure_message).into());
        }
        println!(
            "status=completed recovered={} replans={} verified={} attempted={} planned={} methods={}",
            report.recovered,
            report.replan_attempts,
            report.completed_steps,
            report.step_results.len(),
            report.total_steps,
            report
                .step_results
                .iter()
                .filter_map(|step| step.method.as_deref())
                .collect::<Vec<_>>()
                .join(",")
        );
        Ok(())
    }
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    windows_probe::run()
}

#[cfg(not(windows))]
fn main() {
    eprintln!("The DeskFlow executor probe is available only on Windows.");
}
