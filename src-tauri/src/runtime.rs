use std::sync::{
    Mutex, MutexGuard,
    atomic::{AtomicBool, Ordering},
};

use serde::Serialize;

use crate::{
    ai::{PlanRequest, PlanningResult},
    context::{ActiveTargetSummary, WindowContextSnapshot},
    settings::AppSettings,
    uia::UiAutomationSnapshot,
};

#[derive(Clone, Debug, Serialize)]
pub struct RuntimeStatus {
    pub paused: bool,
    pub executing: bool,
    pub emergency_stopped: bool,
    pub registered_hotkey: Option<String>,
    pub hotkey_warning: Option<String>,
    pub registered_emergency_hotkey: Option<String>,
    pub emergency_hotkey_warning: Option<String>,
    pub active_target: Option<ActiveTargetSummary>,
    pub context_warning: Option<String>,
}

pub struct RuntimeState {
    settings: Mutex<AppSettings>,
    hotkey: Mutex<(Option<String>, Option<String>)>,
    emergency_hotkey: Mutex<(Option<String>, Option<String>)>,
    context: Mutex<(Option<WindowContextSnapshot>, Option<String>)>,
    ui_automation: Mutex<Option<UiAutomationSnapshot>>,
    plan_request: Mutex<Option<PlanRequest>>,
    action_plan: Mutex<Option<PlanningResult>>,
    paused: AtomicBool,
    executing: AtomicBool,
    emergency_stop_requested: std::sync::Arc<AtomicBool>,
    cancellation_requested: std::sync::Arc<AtomicBool>,
    diagnostic_logs: Mutex<Vec<crate::security::DiagnosticLogEntry>>,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            settings: Mutex::new(AppSettings::default()),
            hotkey: Mutex::new((None, None)),
            emergency_hotkey: Mutex::new((None, None)),
            context: Mutex::new((None, None)),
            ui_automation: Mutex::new(None),
            plan_request: Mutex::new(None),
            action_plan: Mutex::new(None),
            paused: AtomicBool::new(false),
            executing: AtomicBool::new(false),
            emergency_stop_requested: std::sync::Arc::new(AtomicBool::new(false)),
            cancellation_requested: std::sync::Arc::new(AtomicBool::new(false)),
            diagnostic_logs: Mutex::new(Vec::new()),
        }
    }
}

fn recover_lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl RuntimeState {
    pub fn settings(&self) -> AppSettings {
        recover_lock(&self.settings).clone()
    }

    pub fn replace_settings(&self, settings: AppSettings) {
        *recover_lock(&self.settings) = settings;
    }

    pub fn status(&self) -> RuntimeStatus {
        let (registered_hotkey, hotkey_warning) = recover_lock(&self.hotkey).clone();
        let (registered_emergency_hotkey, emergency_hotkey_warning) =
            recover_lock(&self.emergency_hotkey).clone();
        let context = recover_lock(&self.context);
        RuntimeStatus {
            paused: self.paused.load(Ordering::SeqCst),
            executing: self.executing.load(Ordering::SeqCst),
            emergency_stopped: self.emergency_stop_requested.load(Ordering::SeqCst),
            registered_hotkey,
            hotkey_warning,
            registered_emergency_hotkey,
            emergency_hotkey_warning,
            active_target: context.0.as_ref().map(ActiveTargetSummary::from),
            context_warning: context.1.clone(),
        }
    }

    pub fn set_hotkey_status(&self, registered: Option<String>, warning: Option<String>) {
        *recover_lock(&self.hotkey) = (registered, warning);
    }

    pub fn set_emergency_hotkey_status(&self, registered: Option<String>, warning: Option<String>) {
        *recover_lock(&self.emergency_hotkey) = (registered, warning);
    }

    pub fn window_context(&self) -> Option<WindowContextSnapshot> {
        recover_lock(&self.context).0.clone()
    }

    pub fn set_window_context(&self, context: WindowContextSnapshot) {
        *recover_lock(&self.context) = (Some(context), None);
        *recover_lock(&self.ui_automation) = None;
        *recover_lock(&self.plan_request) = None;
        *recover_lock(&self.action_plan) = None;
    }

    pub fn set_context_warning(&self, warning: String) {
        *recover_lock(&self.context) = (None, Some(warning));
        *recover_lock(&self.ui_automation) = None;
        *recover_lock(&self.plan_request) = None;
        *recover_lock(&self.action_plan) = None;
    }

    pub fn ui_automation(&self) -> Option<UiAutomationSnapshot> {
        recover_lock(&self.ui_automation).clone()
    }

    pub fn set_ui_automation(&self, snapshot: UiAutomationSnapshot) {
        *recover_lock(&self.ui_automation) = Some(snapshot);
        *recover_lock(&self.plan_request) = None;
        *recover_lock(&self.action_plan) = None;
    }

    pub fn action_plan(&self) -> Option<PlanningResult> {
        recover_lock(&self.action_plan).clone()
    }

    pub fn plan_request(&self) -> Option<PlanRequest> {
        recover_lock(&self.plan_request).clone()
    }

    pub fn set_action_plan(&self, request: PlanRequest, plan: PlanningResult) {
        *recover_lock(&self.plan_request) = Some(request);
        *recover_lock(&self.action_plan) = Some(plan);
    }

    pub fn set_recovery_snapshot(
        &self,
        context: WindowContextSnapshot,
        snapshot: UiAutomationSnapshot,
        plan: PlanningResult,
    ) {
        *recover_lock(&self.context) = (Some(context), None);
        *recover_lock(&self.ui_automation) = Some(snapshot);
        *recover_lock(&self.action_plan) = Some(plan);
    }

    pub fn toggle_paused(&self) -> bool {
        let next = !self.paused.load(Ordering::SeqCst);
        self.paused.store(next, Ordering::SeqCst);
        next
    }

    pub fn begin_execution(&self) -> bool {
        self.reset_execution_signals();
        self.executing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn finish_execution(&self) {
        self.executing.store(false, Ordering::SeqCst);
    }

    pub fn request_emergency_stop(&self) -> bool {
        self.emergency_stop_requested.store(true, Ordering::SeqCst);
        self.cancellation_requested.store(true, Ordering::SeqCst);
        self.executing.load(Ordering::SeqCst)
    }

    pub fn request_cancellation(&self) -> bool {
        self.cancellation_requested.store(true, Ordering::SeqCst);
        self.executing.load(Ordering::SeqCst)
    }

    pub fn reset_execution_signals(&self) {
        self.emergency_stop_requested.store(false, Ordering::SeqCst);
        self.cancellation_requested.store(false, Ordering::SeqCst);
    }

    pub fn is_emergency_stopped(&self) -> bool {
        self.emergency_stop_requested.load(Ordering::SeqCst)
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancellation_requested.load(Ordering::SeqCst)
    }

    pub fn cancellation_flag(&self) -> std::sync::Arc<AtomicBool> {
        std::sync::Arc::clone(&self.cancellation_requested)
    }

    #[allow(dead_code)]
    pub fn emergency_stop_flag(&self) -> std::sync::Arc<AtomicBool> {
        std::sync::Arc::clone(&self.emergency_stop_requested)
    }

    pub fn clear_cache(&self) {
        *recover_lock(&self.context) = (None, None);
        *recover_lock(&self.ui_automation) = None;
        *recover_lock(&self.plan_request) = None;
        *recover_lock(&self.action_plan) = None;
    }

    pub fn log_diagnostic(&self, level: &str, category: &str, message: &str) {
        if !self.settings().diagnostic_logging {
            return;
        }
        let sanitized = crate::security::sanitize_sensitive_text(message);
        let mut logs = recover_lock(&self.diagnostic_logs);
        if logs.len() >= 500 {
            logs.remove(0);
        }
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        logs.push(crate::security::DiagnosticLogEntry {
            timestamp_unix_ms: timestamp,
            level: level.to_string(),
            category: category.to_string(),
            message: sanitized,
        });
    }

    pub fn diagnostic_logs(&self) -> Vec<crate::security::DiagnosticLogEntry> {
        recover_lock(&self.diagnostic_logs).clone()
    }

    pub fn clear_diagnostic_logs(&self) {
        recover_lock(&self.diagnostic_logs).clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_state_diagnostic_logs_are_bounded_at_500() {
        let state = RuntimeState::default();
        let mut settings = state.settings();
        settings.diagnostic_logging = true;
        state.replace_settings(settings);

        for i in 0..600 {
            state.log_diagnostic("info", "test", &format!("Message {i}"));
        }

        let logs = state.diagnostic_logs();
        assert_eq!(logs.len(), 500);
        assert_eq!(logs[0].message, "Message 100");
        assert_eq!(logs[499].message, "Message 599");

        state.clear_diagnostic_logs();
        assert!(state.diagnostic_logs().is_empty());
    }

    #[test]
    fn diagnostic_logging_drops_messages_when_disabled() {
        let state = RuntimeState::default();
        assert!(!state.settings().diagnostic_logging);

        state.log_diagnostic("info", "test", "Should not be logged");
        assert!(state.diagnostic_logs().is_empty());
    }

    #[test]
    fn clear_cache_purges_all_volatile_structures() {
        let state = RuntimeState::default();
        *recover_lock(&state.context) = (None, Some("warning".to_string()));
        state.clear_cache();
        let status = state.status();
        assert!(status.context_warning.is_none());
        assert!(status.active_target.is_none());
    }
}
