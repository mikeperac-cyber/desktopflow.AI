use std::{thread, time::Duration};

use serde::Deserialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;

use crate::{
    ai::{self, AiProviderKind, PlanRequest, PlanningResult, ProviderCatalog},
    context::{self, WindowContextSnapshot},
    error::{AppError, AppResult},
    executor::{self, ExecutionReport},
    highlight::{self, TargetHighlight},
    hotkeys,
    memory::{self, MemoryEntry},
    recorder::{self, RecordingStatus},
    runtime::{RuntimeState, RuntimeStatus},
    settings::{self, AppSettings},
    uia::{self, UiAutomationSnapshot},
    windows, workflow,
};

#[derive(Debug, Deserialize)]
pub struct ExecutePlanRequest {
    pub(crate) provider_request_id: String,
    pub(crate) surface: String,
    pub(crate) confirmed: bool,
    #[serde(default)]
    pub(crate) approved_step_ids: Vec<String>,
    #[serde(default)]
    pub(crate) custom_max_steps: Option<u16>,
}

#[derive(Debug, Deserialize)]
pub struct ProviderCredentialRequest {
    provider: AiProviderKind,
    api_key: String,
}

#[tauri::command]
pub fn get_app_settings(state: State<'_, RuntimeState>) -> AppSettings {
    state.settings()
}

#[tauri::command]
pub fn get_runtime_status(state: State<'_, RuntimeState>) -> RuntimeStatus {
    state.status()
}

#[tauri::command]
pub fn get_last_window_context(state: State<'_, RuntimeState>) -> Option<WindowContextSnapshot> {
    state.window_context()
}

#[tauri::command]
pub fn get_last_ui_automation(state: State<'_, RuntimeState>) -> Option<UiAutomationSnapshot> {
    state.ui_automation()
}

#[tauri::command]
pub fn get_ai_provider_status(state: State<'_, RuntimeState>) -> AppResult<ProviderCatalog> {
    ai::provider_catalog(state.settings().ai_provider)
}

#[tauri::command]
pub fn save_ai_provider_credential(
    state: State<'_, RuntimeState>,
    request: ProviderCredentialRequest,
) -> AppResult<ProviderCatalog> {
    ai::save_provider_credential(request.provider, &request.api_key)?;
    ai::provider_catalog(state.settings().ai_provider)
}

#[tauri::command]
pub fn delete_ai_provider_credential(
    state: State<'_, RuntimeState>,
    provider: AiProviderKind,
) -> AppResult<ProviderCatalog> {
    ai::delete_provider_credential(provider)?;
    ai::provider_catalog(state.settings().ai_provider)
}

#[tauri::command]
pub fn get_last_action_plan(state: State<'_, RuntimeState>) -> Option<PlanningResult> {
    state.action_plan()
}

#[tauri::command]
pub fn hide_window(app: AppHandle, label: String) -> AppResult<()> {
    windows::hide_known_window(&app, &label)
}

#[tauri::command]
pub async fn capture_active_window(app: AppHandle) -> AppResult<WindowContextSnapshot> {
    highlight::hide(&app)?;
    windows::hide_known_window(&app, "settings")?;

    let capture = match tauri::async_runtime::spawn_blocking(|| {
        thread::sleep(Duration::from_millis(350));
        context::capture_foreground_context()
    })
    .await
    {
        Ok(result) => result,
        Err(error) => Err(AppError::Context(format!(
            "the capture worker stopped: {error}"
        ))),
    };

    let restore_result = windows::show_settings(&app);
    match capture {
        Ok(snapshot) => {
            let state = app.state::<RuntimeState>();
            state.set_window_context(snapshot.clone());
            state.log_diagnostic(
                "info",
                "context",
                &format!("Captured active window: {}", snapshot.title),
            );
            restore_result?;
            Ok(snapshot)
        }
        Err(error) => {
            let state = app.state::<RuntimeState>();
            state.set_context_warning(error.to_string());
            state.log_diagnostic(
                "warn",
                "context",
                &format!("Target capture failed: {error}"),
            );
            restore_result?;
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn inspect_target_ui(
    app: AppHandle,
    state: State<'_, RuntimeState>,
) -> AppResult<UiAutomationSnapshot> {
    highlight::hide(&app)?;
    let context = state.window_context().ok_or_else(|| {
        AppError::UiAutomation(
            "Capture an active application before opening the UI tree.".to_string(),
        )
    })?;

    let snapshot =
        tauri::async_runtime::spawn_blocking(move || uia::inspect_captured_window(&context))
            .await
            .map_err(|error| {
                AppError::UiAutomation(format!("the inspection worker stopped: {error}"))
            })??;

    state.set_ui_automation(snapshot.clone());
    state.log_diagnostic(
        "info",
        "uia",
        &format!("Inspected target UI: {} elements", snapshot.elements.len()),
    );
    Ok(snapshot)
}

#[tauri::command]
pub fn highlight_ui_element(
    app: AppHandle,
    state: State<'_, RuntimeState>,
    element_id: String,
) -> AppResult<TargetHighlight> {
    let snapshot = state.ui_automation().ok_or_else(|| {
        AppError::Highlight("Inspect the target interface before selecting a control.".to_string())
    })?;
    let target = highlight::resolve(&snapshot, &element_id)?;
    highlight::show(&app, &target)?;
    Ok(target)
}

#[tauri::command]
pub fn clear_target_highlight(app: AppHandle) -> AppResult<()> {
    highlight::hide(&app)
}

#[tauri::command]
pub fn set_overlay_plan_mode(app: AppHandle, expanded: bool) -> AppResult<()> {
    windows::set_overlay_plan_mode(&app, expanded)
}

#[tauri::command]
pub async fn create_action_plan(
    app: AppHandle,
    state: State<'_, RuntimeState>,
    request: PlanRequest,
) -> AppResult<PlanningResult> {
    highlight::hide(&app)?;
    request.validate()?;

    let context = state.window_context().ok_or_else(|| {
        AppError::AiConfiguration(
            "Open DeskFlow over a target application or capture one in Advanced settings first."
                .to_string(),
        )
    })?;

    let automation = if let Some(snapshot) = state.ui_automation() {
        snapshot
    } else {
        let inspection_context = context.clone();
        let snapshot = tauri::async_runtime::spawn_blocking(move || {
            uia::inspect_captured_window(&inspection_context)
        })
        .await
        .map_err(|error| {
            AppError::UiAutomation(format!("the planning inspection worker stopped: {error}"))
        })??;
        state.set_ui_automation(snapshot.clone());
        snapshot
    };

    let provider_kind = state.settings().ai_provider;
    let provider = ai::create_provider(provider_kind)?;
    let stored_request = request.clone();
    let memory_context = memory::context_for_process(&app, context.process.name.as_deref());
    let result = provider
        .create_plan(ai::ProviderPlanningInput {
            request,
            context,
            automation,
            recovery: None,
            memory_context,
        })
        .await?;
    state.set_action_plan(stored_request, result.clone());
    state.log_diagnostic(
        "info",
        "ai",
        &format!(
            "Created action plan: {} ({} steps)",
            result.plan.title,
            result.plan.steps.len()
        ),
    );
    Ok(result)
}

#[tauri::command]
pub async fn get_recording_status(
    state: State<'_, RuntimeState>,
) -> AppResult<Option<RecordingStatus>> {
    Ok(state.recording_status())
}

#[tauri::command]
pub fn get_schedule_runs(state: State<'_, RuntimeState>) -> Vec<crate::scheduler::ScheduleRun> {
    let mut runs = state.schedule_runs();
    runs.reverse();
    runs
}

#[derive(Debug, Deserialize)]
pub struct MemoryEntryRequest {
    subject: String,
    content: String,
}

#[tauri::command]
pub fn get_memories(app: AppHandle) -> AppResult<Vec<MemoryEntry>> {
    memory::load_all(&app)
}

#[tauri::command]
pub fn add_memory(app: AppHandle, request: MemoryEntryRequest) -> AppResult<Vec<MemoryEntry>> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or(0);
    memory::add(
        &app,
        MemoryEntry {
            id: format!("mem-{timestamp}"),
            subject: request.subject,
            content: request.content,
            created_at_unix_ms: timestamp,
        },
    )
}

#[tauri::command]
pub fn delete_memory(app: AppHandle, id: String) -> AppResult<Vec<MemoryEntry>> {
    memory::remove(&app, &id)
}

#[tauri::command]
pub fn purge_memories(app: AppHandle) -> AppResult<Vec<MemoryEntry>> {
    memory::purge(&app)
}

#[tauri::command]
pub async fn start_recording(
    app: AppHandle,
    state: State<'_, RuntimeState>,
) -> AppResult<RecordingStatus> {
    if state.status().paused {
        return Err(AppError::ExecutionPolicy(
            "DeskFlow is paused. Resume it from the tray before recording.".to_string(),
        ));
    }
    if state.status().executing {
        return Err(AppError::ExecutionPolicy(
            "a plan is currently executing. Recording starts only when idle.".to_string(),
        ));
    }
    if state.recording_status().is_some() {
        return Err(AppError::ExecutionPolicy(
            "a recording is already in progress.".to_string(),
        ));
    }
    let context = state.window_context().ok_or_else(|| {
        AppError::InvalidPlan(
            "Capture a target first: open the overlay on the application you want to record."
                .to_string(),
        )
    })?;
    let automation = state.ui_automation().ok_or_else(|| {
        AppError::InvalidPlan(
            "Inspect the captured target first in Advanced settings, then start recording."
                .to_string(),
        )
    })?;
    if context.process.id != automation.target.process_id {
        return Err(AppError::InvalidPlan(
            "The captured window and UI tree no longer identify the same process. Capture the target again."
                .to_string(),
        ));
    }
    let recording = recorder::ActiveRecording::begin(&context, &automation)?;
    let status = recording.status();
    if !state.begin_recording(recording) {
        return Err(AppError::ExecutionPolicy(
            "a recording is already in progress.".to_string(),
        ));
    }
    windows::hide_known_window(&app, "overlay")?;
    state.log_diagnostic(
        "info",
        "recorder",
        &format!("Started recording input on '{}'.", context.title),
    );
    Ok(status)
}

#[tauri::command]
pub async fn stop_recording(
    app: AppHandle,
    state: State<'_, RuntimeState>,
) -> AppResult<PlanningResult> {
    let mut recording = state
        .take_recording()
        .ok_or_else(|| AppError::InvalidPlan("there is no recording in progress.".to_string()))?;
    // The overlay recaptures on open; refuse to build a plan when the user
    // wandered to a different window instead of failing opaquely at replay.
    if let Some(current) = state.window_context()
        && current.native_window_handle != recording.hwnd()
    {
        let _ = recording.shutdown();
        return Err(AppError::InvalidPlan(format!(
            "the recording targeted '{}', but the overlay now shows '{}'. Return to the recorded application and record again.",
            recording.title(),
            current.title
        )));
    }
    let events = recording.shutdown()?;
    let snapshot = recording.snapshot().clone();
    let (plan, outcome) = recorder::build_plan(&snapshot, events)?;
    let title = recording.title().to_string();
    let result = recorder::build_result(&snapshot, plan);
    // Restore the recording-time snapshot so the executor revalidates recorded
    // targets by fingerprint against the interface that produced them.
    state.set_ui_automation(snapshot);
    state.set_action_plan(
        PlanRequest {
            instruction: format!("Recorded workflow on '{title}' ({} steps)", outcome.steps),
            model: state.settings().ai_model,
            include_screenshot: false,
        },
        result.clone(),
    );
    state.log_diagnostic(
        "info",
        "recorder",
        &format!(
            "Stopped recording on '{}': {} steps, {} ignored, {} redacted.",
            title, outcome.steps, outcome.skipped, outcome.redacted
        ),
    );
    windows::set_overlay_plan_mode(&app, true)?;
    Ok(result)
}

#[tauri::command]
pub async fn execute_action_plan(
    app: AppHandle,
    state: State<'_, RuntimeState>,
    request: ExecutePlanRequest,
) -> AppResult<ExecutionReport> {
    if !request.confirmed {
        return Err(AppError::ExecutionPolicy(
            "execution requires an explicit confirmation.".to_string(),
        ));
    }
    if !matches!(request.surface.as_str(), "overlay" | "settings") {
        return Err(AppError::ExecutionPolicy(
            "the execution surface is not allowlisted.".to_string(),
        ));
    }
    if state.status().paused {
        return Err(AppError::ExecutionPolicy(
            "DeskFlow is paused. Resume it from the tray before running a plan.".to_string(),
        ));
    }
    if !state.begin_execution() {
        return Err(AppError::ExecutionPolicy(
            "another plan is already executing.".to_string(),
        ));
    }

    let result = async {
        let mut planning_result = state.action_plan().ok_or_else(|| {
            AppError::ExecutionPolicy("there is no current validated plan.".to_string())
        })?;
        // Recorded plans are validated observations, not provider output. They
        // execute through the identical policy and revalidation gates; only
        // recovery replanning needs a planning provider, which falls back to
        // the user's currently selected one.
        let provider_kind = if planning_result.provider == recorder::RECORDER_PROVIDER_ID {
            state.settings().ai_provider
        } else {
            AiProviderKind::from_id(&planning_result.provider)?
        };
        if request.provider_request_id.trim().is_empty()
            || request.provider_request_id != planning_result.provider_request_id
        {
            return Err(AppError::ExecutionPolicy(
                "the displayed plan is stale. Generate a new plan before running it.".to_string(),
            ));
        }
        let original_request = state.plan_request().ok_or_else(|| {
            AppError::ExecutionPolicy(
                "the instruction for this plan is no longer available. Generate a new plan."
                    .to_string(),
            )
        })?;
        let mut context = state.window_context().ok_or_else(|| {
            AppError::ExecutionPolicy("the captured target is no longer available.".to_string())
        })?;
        let mut automation = state.ui_automation().ok_or_else(|| {
            AppError::ExecutionPolicy("the inspected target is no longer available.".to_string())
        })?;
        let settings = state.settings();

        highlight::hide(&app)?;
        windows::hide_known_window(&app, &request.surface)?;
        let maximum_actions = usize::from(
            request
                .custom_max_steps
                .unwrap_or(settings.maximum_autonomous_steps),
        );
        let cancellation_token = state.cancellation_flag();
        let approved_step_ids = request.approved_step_ids.clone();
        let approval_policy = settings.approval_policy.clone();
        let mut aggregate: Option<ExecutionReport> = None;
        loop {
            if state.is_cancelled() || state.is_emergency_stopped() {
                break;
            }
            let used_actions = aggregate
                .as_ref()
                .map_or(0, |report| report.step_results.len());
            let remaining_actions = maximum_actions.saturating_sub(used_actions);
            if remaining_actions == 0 {
                if let Some(report) = &mut aggregate {
                    workflow::mark_recovery_failure(
                        report,
                        "The total autonomous action limit was reached.".to_string(),
                    );
                    break;
                }
                return Err(AppError::ExecutionPolicy(
                    "the autonomous action limit is zero.".to_string(),
                ));
            }

            let execution_context = context.clone();
            let execution_automation = automation.clone();
            let execution_plan = planning_result.plan.clone();
            let execution_cancellation = std::sync::Arc::clone(&cancellation_token);
            let execution_approved_ids = approved_step_ids.clone();
            let execution_policy = approval_policy.clone();
            let execution = match tauri::async_runtime::spawn_blocking(move || {
                thread::sleep(Duration::from_millis(180));
                executor::execute_plan(
                    &execution_context,
                    &execution_automation,
                    &execution_plan,
                    remaining_actions,
                    settings.execution_delay_ms,
                    Some(&execution_cancellation),
                    &execution_policy,
                    &execution_approved_ids,
                )
            })
            .await
            {
                Ok(Ok(report)) => report,
                Ok(Err(error)) => {
                    if let Some(report) = &mut aggregate {
                        workflow::mark_recovery_failure(report, error.to_string());
                        break;
                    }
                    return Err(error);
                }
                Err(error) => {
                    let message = format!("the execution worker stopped: {error}");
                    if let Some(report) = &mut aggregate {
                        workflow::mark_recovery_failure(report, message);
                        break;
                    }
                    return Err(AppError::Execution(message));
                }
            };
            aggregate = Some(workflow::merge_attempt(aggregate, execution));
            let report = aggregate.as_ref().expect("attempt report was just stored");
            if report.status == executor::ExecutionStatus::Completed
                || !workflow::should_replan(report)
                || state.is_cancelled()
                || state.is_emergency_stopped()
            {
                break;
            }

            let recovery_attempt = report.replan_attempts.saturating_add(1);
            let failure_kind = workflow::recovery_failure_kind(report);
            let completed_step_ids = report
                .step_results
                .iter()
                .filter(|step| step.status == executor::ExecutionStepStatus::Completed)
                .map(|step| step.id.clone())
                .collect::<Vec<_>>();
            let native_window_handle = context.native_window_handle;
            let expected_process_id = context.process.id;
            let observation = tauri::async_runtime::spawn_blocking(move || {
                let fresh_context = context::capture_window_context(native_window_handle)?;
                if fresh_context.process.id != expected_process_id {
                    return Err(AppError::Execution(
                        "the captured window now belongs to a different process.".to_string(),
                    ));
                }
                let fresh_automation = uia::inspect_captured_window(&fresh_context)?;
                Ok((fresh_context, fresh_automation))
            })
            .await;
            let (fresh_context, fresh_automation) = match observation {
                Ok(Ok(value)) => value,
                Ok(Err(error)) => {
                    workflow::mark_recovery_failure(
                        aggregate.as_mut().expect("attempt report exists"),
                        format!("Fresh observation failed: {error}"),
                    );
                    break;
                }
                Err(error) => {
                    workflow::mark_recovery_failure(
                        aggregate.as_mut().expect("attempt report exists"),
                        format!("The recovery inspection worker stopped: {error}"),
                    );
                    break;
                }
            };

            let provider = match ai::create_provider(provider_kind) {
                Ok(provider) => provider,
                Err(error) => {
                    workflow::mark_recovery_failure(
                        aggregate.as_mut().expect("attempt report exists"),
                        error.to_string(),
                    );
                    break;
                }
            };
            let recovery_plan = provider
                .create_plan(ai::ProviderPlanningInput {
                    request: original_request.clone(),
                    context: fresh_context.clone(),
                    automation: fresh_automation.clone(),
                    recovery: Some(ai::RecoveryPlanningContext {
                        attempt: recovery_attempt,
                        maximum_attempts: workflow::MAX_REPLAN_ATTEMPTS,
                        failure_kind,
                        completed_step_ids,
                    }),
                    memory_context: memory::context_for_process(
                        &app,
                        fresh_context.process.name.as_deref(),
                    ),
                })
                .await;
            let fresh_plan = match recovery_plan {
                Ok(plan) => plan,
                Err(error) => {
                    workflow::mark_recovery_failure(
                        aggregate.as_mut().expect("attempt report exists"),
                        format!("Replanning failed: {error}"),
                    );
                    break;
                }
            };
            state.set_recovery_snapshot(
                fresh_context.clone(),
                fresh_automation.clone(),
                fresh_plan.clone(),
            );
            context = fresh_context;
            automation = fresh_automation;
            planning_result = fresh_plan;
        }

        let execution = aggregate.ok_or_else(|| {
            AppError::Execution("execution ended without an attempt report.".to_string())
        });

        let restore_result = windows::restore_execution_surface(&app, &request.surface);
        match execution {
            Ok(report) => {
                state.log_diagnostic(
                    "info",
                    "executor",
                    &format!("Execution completed with status {:?}", report.status),
                );
                restore_result?;
                Ok(report)
            }
            Err(error) => {
                state.log_diagnostic("error", "executor", &format!("Execution error: {error}"));
                restore_result?;
                Err(error)
            }
        }
    }
    .await;

    state.finish_execution();
    result
}

#[tauri::command]
pub fn update_app_settings(
    app: AppHandle,
    state: State<'_, RuntimeState>,
    settings: AppSettings,
) -> AppResult<RuntimeStatus> {
    settings.validate()?;
    let previous = state.settings();
    let previous_status = state.status();

    let registered_hotkey = hotkeys::replace_shortcut(
        &app,
        previous_status.registered_hotkey.as_deref(),
        &settings.global_hotkey,
    )?;

    if settings.launch_at_startup != previous.launch_at_startup {
        let autostart = app.autolaunch();
        let result = if settings.launch_at_startup {
            autostart.enable()
        } else {
            autostart.disable()
        };

        if let Err(error) = result {
            if let Err(rollback_error) = hotkeys::restore_shortcut(
                &app,
                &registered_hotkey,
                previous_status.registered_hotkey.as_deref(),
            ) {
                eprintln!("DESKFLOW_HOTKEY_ROLLBACK_FAILED code={rollback_error}");
            }
            return Err(AppError::Autostart(error.to_string()));
        }
    }

    if let Err(error) = settings::persist(&app, &settings) {
        if settings.launch_at_startup != previous.launch_at_startup {
            let autostart = app.autolaunch();
            if previous.launch_at_startup {
                let _ = autostart.enable();
            } else {
                let _ = autostart.disable();
            }
        }
        if let Err(rollback_error) = hotkeys::restore_shortcut(
            &app,
            &registered_hotkey,
            previous_status.registered_hotkey.as_deref(),
        ) {
            eprintln!("DESKFLOW_HOTKEY_ROLLBACK_FAILED code={rollback_error}");
        }
        return Err(error);
    }

    let (registered_emergency, emergency_warning) =
        if settings.emergency_hotkey != previous.emergency_hotkey {
            match hotkeys::replace_emergency_shortcut(
                &app,
                previous_status.registered_emergency_hotkey.as_deref(),
                &settings.emergency_hotkey,
            ) {
                Ok(hotkey) => (Some(hotkey), None),
                Err(error) => (None, Some(error.to_string())),
            }
        } else {
            (
                previous_status.registered_emergency_hotkey,
                previous_status.emergency_hotkey_warning,
            )
        };

    state.replace_settings(settings);
    state.set_hotkey_status(Some(registered_hotkey), None);
    state.set_emergency_hotkey_status(registered_emergency, emergency_warning);
    Ok(state.status())
}

#[tauri::command]
pub fn emergency_stop(app: AppHandle, state: State<'_, RuntimeState>) -> AppResult<bool> {
    let was_executing = state.request_emergency_stop();
    executor::release_stuck_inputs();
    state.log_diagnostic("warn", "safety", "Emergency stop triggered");
    if !was_executing {
        let _ = windows::hide_known_window(&app, "overlay");
    }
    Ok(was_executing)
}

#[tauri::command]
pub fn cancel_execution(state: State<'_, RuntimeState>) -> AppResult<bool> {
    let was_executing = state.request_cancellation();
    executor::release_stuck_inputs();
    state.log_diagnostic("info", "safety", "Execution cancellation requested");
    Ok(was_executing)
}

#[tauri::command]
pub fn get_diagnostic_logs(
    state: State<'_, RuntimeState>,
) -> AppResult<Vec<crate::security::DiagnosticLogEntry>> {
    Ok(state.diagnostic_logs())
}

#[tauri::command]
pub fn clear_diagnostic_logs(state: State<'_, RuntimeState>) -> AppResult<bool> {
    state.clear_diagnostic_logs();
    Ok(true)
}

#[tauri::command]
pub fn clear_local_cache(state: State<'_, RuntimeState>) -> AppResult<bool> {
    state.clear_cache();
    Ok(true)
}
