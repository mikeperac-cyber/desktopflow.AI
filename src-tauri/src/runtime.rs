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
    pub registered_hotkey: Option<String>,
    pub hotkey_warning: Option<String>,
    pub active_target: Option<ActiveTargetSummary>,
    pub context_warning: Option<String>,
}

pub struct RuntimeState {
    settings: Mutex<AppSettings>,
    hotkey: Mutex<(Option<String>, Option<String>)>,
    context: Mutex<(Option<WindowContextSnapshot>, Option<String>)>,
    ui_automation: Mutex<Option<UiAutomationSnapshot>>,
    plan_request: Mutex<Option<PlanRequest>>,
    action_plan: Mutex<Option<PlanningResult>>,
    paused: AtomicBool,
    executing: AtomicBool,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            settings: Mutex::new(AppSettings::default()),
            hotkey: Mutex::new((None, None)),
            context: Mutex::new((None, None)),
            ui_automation: Mutex::new(None),
            plan_request: Mutex::new(None),
            action_plan: Mutex::new(None),
            paused: AtomicBool::new(false),
            executing: AtomicBool::new(false),
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
        let context = recover_lock(&self.context);
        RuntimeStatus {
            paused: self.paused.load(Ordering::SeqCst),
            executing: self.executing.load(Ordering::SeqCst),
            registered_hotkey,
            hotkey_warning,
            active_target: context.0.as_ref().map(ActiveTargetSummary::from),
            context_warning: context.1.clone(),
        }
    }

    pub fn set_hotkey_status(&self, registered: Option<String>, warning: Option<String>) {
        *recover_lock(&self.hotkey) = (registered, warning);
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
        self.executing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn finish_execution(&self) {
        self.executing.store(false, Ordering::SeqCst);
    }
}
