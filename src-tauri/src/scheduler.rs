//! Scheduled workflows (Phase 16).
//!
//! Schedules turn saved instructions into runs without the user typing them.
//! Every run reuses the exact pipeline an interactive plan uses: fresh
//! foreground capture, bounded inspection, provider planning, and — for
//! autonomous runs — the same confirmation-free execution gate the executor
//! already enforces. Two modes:
//!
//! - Attended (default): the validated plan opens in the overlay and waits
//!   for the unchanged second confirmation. Nothing runs by itself.
//! - Autonomous: only plans whose overall risk is [`RiskLevel::Low`] run, and
//!   only inside the configured autonomous action budget. Anything riskier is
//!   downgraded to attended instead of failing silently.
//!
//! A run never starts while paused, executing, recording, or while another
//! plan is awaiting review, so the scheduler can never clobber user state.

use std::collections::HashSet;

use chrono::{DateTime, Datelike, Local, TimeZone};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::{
    ai::{PlanRequest, RiskLevel},
    commands::{self, ExecutePlanRequest},
    context,
    error::AppResult,
    runtime::RuntimeState,
    settings::{ScheduleTrigger, ScheduledWorkflow},
    uia,
};

const TICK_INTERVAL_SECS: u64 = 30;
const STARTUP_GRACE_SECS: u64 = 15;
/// A run that starts up to 30 minutes late (sleep, restart) still fires once.
/// Older slots are consumed as missed so a laptop waking at noon does not
/// replay the whole morning.
const LATE_WINDOW_MS: i64 = 30 * 60 * 1_000;
pub const MAX_RUN_HISTORY: usize = 20;
const MAX_SEEN_FILES: usize = 2_000;
const MAX_LISTED_FILES: usize = 5;

#[derive(Clone, Debug, Default)]
pub struct ScheduleMark {
    pub slot_ms: u64,
    pub seen_files: Option<HashSet<String>>,
    pub last_outcome: Option<(String, String)>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScheduleRun {
    pub schedule_id: String,
    pub schedule_name: String,
    pub fired_at_unix_ms: u64,
    pub outcome: String,
    pub detail: String,
}

/// Most recent scheduled slot at or before `now`, in local time.
pub fn previous_slot(trigger: &ScheduleTrigger, now: DateTime<Local>) -> Option<DateTime<Local>> {
    match trigger {
        ScheduleTrigger::Once { at_unix_ms } => {
            let at = Local.timestamp_millis_opt(*at_unix_ms as i64).single()?;
            (at <= now).then_some(at)
        }
        ScheduleTrigger::Daily { hour, minute } => {
            let today = day_at(now, 0, *hour, *minute)?;
            Some(if today <= now {
                today
            } else {
                day_at(now, -1, *hour, *minute)?
            })
        }
        ScheduleTrigger::Weekly {
            weekdays,
            hour,
            minute,
        } => (0..=7)
            .filter_map(|back| day_at(now, -back, *hour, *minute))
            .find(|candidate| *candidate <= now && weekday_bit(candidate) & weekdays != 0),
        // File arrivals are polled, not slotted: this function only serves time triggers.
        ScheduleTrigger::FileAppears { .. } => None,
    }
}

fn day_at(now: DateTime<Local>, back_days: i64, hour: u8, minute: u8) -> Option<DateTime<Local>> {
    let date = now.date_naive() + chrono::Duration::days(back_days);
    let naive = date.and_hms_opt(hour as u32, minute as u32, 0)?;
    Local.from_local_datetime(&naive).single()
}

/// Bit 0 is Monday through bit 6 Sunday, matching the settings bitmask.
fn weekday_bit(moment: &DateTime<Local>) -> u8 {
    1 << (moment.weekday().number_from_monday() - 1)
}

/// Case-insensitive `*`/`?` glob over a file name (never a path).
pub fn match_file_pattern(pattern: &str, file_name: &str) -> bool {
    let pattern = pattern.to_lowercase().chars().collect::<Vec<_>>();
    let name = file_name.to_lowercase().chars().collect::<Vec<_>>();
    match_glob(&pattern, &name)
}

fn match_glob(pattern: &[char], name: &[char]) -> bool {
    if pattern.is_empty() {
        return name.is_empty();
    }
    match pattern[0] {
        '*' => (0..=name.len()).any(|skip| match_glob(&pattern[1..], &name[skip..])),
        '?' => !name.is_empty() && match_glob(&pattern[1..], &name[1..]),
        exact => {
            name.first().is_some_and(|first| *first == exact)
                && match_glob(&pattern[1..], &name[1..])
        }
    }
}

fn now_ms() -> u64 {
    Local::now().timestamp_millis().max(0) as u64
}

fn record_run(state: &RuntimeState, run: ScheduleRun) {
    {
        let mut mark_guard = state.schedule_marks_lock();
        let entry = mark_guard.entry(run.schedule_id.clone()).or_default();
        // Consecutive identical outcomes (e.g. a missing folder every 30 s) are
        // logged once so one broken schedule cannot flood the run history.
        if entry.last_outcome.as_ref() == Some(&(run.outcome.clone(), run.detail.clone())) {
            return;
        }
        entry.last_outcome = Some((run.outcome.clone(), run.detail.clone()));
    }
    state.push_schedule_run(run);
}

async fn fire_time_schedule(
    app: &AppHandle,
    state: &tauri::State<'_, RuntimeState>,
    schedule: &ScheduledWorkflow,
    slot_ms: u64,
) {
    let _ = run_workflow(app, state, schedule, None).await;
    {
        let mut marks = state.schedule_marks_lock();
        let entry = marks.entry(schedule.id.clone()).or_default();
        entry.slot_ms = entry.slot_ms.max(slot_ms);
    }
}

async fn run_workflow(
    app: &AppHandle,
    state: &tauri::State<'_, RuntimeState>,
    schedule: &ScheduledWorkflow,
    context_note: Option<String>,
) -> AppResult<()> {
    let status = state.status();
    if status.paused {
        return failed(state, schedule, "skipped_paused", "DeskFlow is paused.");
    }
    if status.executing {
        return failed(
            state,
            schedule,
            "skipped_busy",
            "Another plan is executing.",
        );
    }
    if state.recording_status().is_some() {
        return failed(
            state,
            schedule,
            "skipped_busy",
            "A recording is in progress.",
        );
    }
    if state.action_plan().is_some() {
        return failed(
            state,
            schedule,
            "skipped_review",
            "Another plan is awaiting review.",
        );
    }

    let context = match capture_foreground().await {
        Ok(context) => context,
        Err(detail) => return failed(state, schedule, "skipped_no_target", &detail),
    };
    if let Some(expected) = schedule.expected_process.as_deref() {
        let actual = context.process.name.as_deref().unwrap_or_default();
        if !actual.eq_ignore_ascii_case(expected.trim()) {
            return failed(
                state,
                schedule,
                "skipped_wrong_target",
                &format!("Expected '{expected}', but '{actual}' is in the foreground."),
            );
        }
    }
    state.set_window_context(context.clone());

    let automation = match inspect_target(context.clone()).await {
        Ok(automation) => automation,
        Err(detail) => return failed(state, schedule, "inspect_failed", &detail),
    };
    state.set_ui_automation(automation);

    let settings = state.settings();
    let request = PlanRequest {
        instruction: schedule.instruction.clone(),
        model: settings.ai_model,
        include_screenshot: false,
    };
    let result = match commands::create_action_plan(app.clone(), state.clone(), request).await {
        Ok(result) => result,
        Err(error) => {
            return failed(state, schedule, "plan_failed", &error.to_string());
        }
    };

    if schedule.autonomous && result.plan.overall_risk == RiskLevel::Low {
        match commands::execute_action_plan(
            app.clone(),
            state.clone(),
            ExecutePlanRequest {
                provider_request_id: result.provider_request_id.clone(),
                surface: "overlay".to_string(),
                confirmed: true,
                approved_step_ids: Vec::new(),
                custom_max_steps: None,
            },
        )
        .await
        {
            Ok(report) => {
                let mut detail = format!(
                    "Executed {} of {} actions ({:?}).",
                    report.completed_steps, report.total_steps, report.status
                );
                if let Some(note) = context_note {
                    detail = format!("{note} {detail}");
                }
                record_run(
                    state,
                    ScheduleRun {
                        schedule_id: schedule.id.clone(),
                        schedule_name: schedule.name.clone(),
                        fired_at_unix_ms: now_ms(),
                        outcome: "ran_autonomous".to_string(),
                        detail,
                    },
                );
                Ok(())
            }
            Err(error) => failed(state, schedule, "run_failed", &error.to_string()),
        }
    } else {
        let downgraded = schedule.autonomous;
        if let Err(error) = crate::windows::restore_overlay(app) {
            return failed(state, schedule, "present_failed", &error.to_string());
        }
        let mut detail = if downgraded {
            "The plan was not low-risk, so it waits for confirmation instead of running alone."
                .to_string()
        } else {
            "The validated plan is open in the overlay.".to_string()
        };
        if let Some(note) = context_note {
            detail = format!("{note} {detail}");
        }
        record_run(
            state,
            ScheduleRun {
                schedule_id: schedule.id.clone(),
                schedule_name: schedule.name.clone(),
                fired_at_unix_ms: now_ms(),
                outcome: if downgraded {
                    "downgraded_to_attended".to_string()
                } else {
                    "awaiting_confirmation".to_string()
                },
                detail,
            },
        );
        Ok(())
    }
}

fn failed(
    state: &RuntimeState,
    schedule: &ScheduledWorkflow,
    outcome: &str,
    detail: &str,
) -> AppResult<()> {
    record_run(
        state,
        ScheduleRun {
            schedule_id: schedule.id.clone(),
            schedule_name: schedule.name.clone(),
            fired_at_unix_ms: now_ms(),
            outcome: outcome.to_string(),
            detail: detail.to_string(),
        },
    );
    Ok(())
}

async fn capture_foreground() -> Result<crate::context::WindowContextSnapshot, String> {
    tauri::async_runtime::spawn_blocking(context::capture_foreground_context)
        .await
        .map_err(|error| format!("the capture worker stopped: {error}"))?
        .map_err(|error| error.to_string())
}

async fn inspect_target(
    context: crate::context::WindowContextSnapshot,
) -> Result<crate::uia::UiAutomationSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || uia::inspect_captured_window(&context))
        .await
        .map_err(|error| format!("the inspection worker stopped: {error}"))?
        .map_err(|error| error.to_string())
}

/// One scheduler pass over every enabled schedule.
pub async fn tick(app: &AppHandle) {
    let state = app.state::<RuntimeState>();
    let settings = state.settings();
    let now = Local::now();
    let now_timestamp = now.timestamp_millis();
    for schedule in settings.schedules.iter().filter(|item| item.enabled) {
        match &schedule.trigger {
            ScheduleTrigger::Once { .. }
            | ScheduleTrigger::Daily { .. }
            | ScheduleTrigger::Weekly { .. } => {
                let Some(slot) = previous_slot(&schedule.trigger, now) else {
                    continue;
                };
                let slot_ms = slot.timestamp_millis().max(0) as u64;
                let marked = state
                    .schedule_marks_lock()
                    .get(&schedule.id)
                    .map(|mark| mark.slot_ms)
                    .unwrap_or(0);
                if slot_ms <= marked {
                    continue;
                }
                if now_timestamp - slot.timestamp_millis() > LATE_WINDOW_MS {
                    // Consume stale slots silently; the miss is self-evident.
                    state.set_schedule_slot(&schedule.id, slot_ms);
                    continue;
                }
                fire_time_schedule(app, &state, schedule, slot_ms).await;
                if matches!(schedule.trigger, ScheduleTrigger::Once { .. }) {
                    disable_once_schedule(app, &state, schedule).await;
                }
            }
            ScheduleTrigger::FileAppears { folder, pattern } => {
                tick_file_schedule(app, &state, schedule, folder, pattern).await;
            }
        }
    }
}

async fn tick_file_schedule(
    app: &AppHandle,
    state: &tauri::State<'_, RuntimeState>,
    schedule: &ScheduledWorkflow,
    folder: &str,
    pattern: &str,
) {
    let entries = match std::fs::read_dir(folder.trim()) {
        Ok(entries) => entries,
        Err(_) => {
            record_deduped(
                state,
                schedule,
                "watch_unavailable",
                &format!("The watched folder '{}' cannot be read.", folder.trim()),
            );
            return;
        }
    };
    let mut current = HashSet::new();
    for entry in entries.flatten() {
        if let Ok(kind) = entry.file_type()
            && kind.is_file()
        {
            let name = entry.file_name().to_string_lossy().to_string();
            if match_file_pattern(pattern, &name) {
                current.insert(name);
            }
        }
        if current.len() >= MAX_SEEN_FILES {
            break;
        }
    }
    let Some(seen) = state.file_baseline(&schedule.id) else {
        // First sighting seeds the baseline: files already present never fire.
        state.set_file_baseline(&schedule.id, current);
        return;
    };
    let mut fresh: Vec<String> = current.difference(&seen).cloned().collect();
    fresh.sort();
    if fresh.is_empty() {
        state.set_file_baseline(&schedule.id, current);
        return;
    }
    let listed = fresh
        .iter()
        .take(MAX_LISTED_FILES)
        .cloned()
        .collect::<Vec<_>>();
    let note = format!(
        "{} new file{}: {}.",
        fresh.len(),
        if fresh.len() == 1 { "" } else { "s" },
        listed.join(", ")
    );
    drop(fresh);
    // The baseline advances even when the run itself is skipped, so one busy
    // afternoon cannot replay the same arrival forever.
    state.set_file_baseline(&schedule.id, current);
    let _ = run_workflow(app, state, schedule, Some(note)).await;
}

fn record_deduped(state: &RuntimeState, schedule: &ScheduledWorkflow, outcome: &str, detail: &str) {
    record_run(
        state,
        ScheduleRun {
            schedule_id: schedule.id.clone(),
            schedule_name: schedule.name.clone(),
            fired_at_unix_ms: now_ms(),
            outcome: outcome.to_string(),
            detail: detail.to_string(),
        },
    );
}

async fn disable_once_schedule(
    app: &AppHandle,
    state: &tauri::State<'_, RuntimeState>,
    schedule: &ScheduledWorkflow,
) {
    let mut settings = state.settings();
    let mut changed = false;
    for item in settings.schedules.iter_mut() {
        if item.id == schedule.id && item.enabled {
            item.enabled = false;
            changed = true;
        }
    }
    if changed {
        state.replace_settings(settings.clone());
        let _ = crate::settings::persist(app, &settings);
    }
}

/// Background loop spawned once from `setup`. It never panics out: every fallible
/// step degrades to a recorded run outcome.
pub async fn run_loop(app: AppHandle) {
    tokio::time::sleep(std::time::Duration::from_secs(STARTUP_GRACE_SECS)).await;
    loop {
        tick(&app).await;
        tokio::time::sleep(std::time::Duration::from_secs(TICK_INTERVAL_SECS)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> DateTime<Local> {
        Local
            .from_local_datetime(
                &chrono::NaiveDate::from_ymd_opt(year, month, day)
                    .expect("date")
                    .and_hms_opt(hour, minute, 0)
                    .expect("time"),
            )
            .single()
            .expect("local time")
    }

    #[test]
    fn daily_slot_is_today_when_passed_and_yesterday_when_not() {
        let trigger = ScheduleTrigger::Daily { hour: 9, minute: 0 };
        // 2026-10-08 is a Thursday.
        let morning = previous_slot(&trigger, local(2026, 10, 8, 10, 30)).expect("slot");
        assert_eq!(
            morning.format("%Y-%m-%d %H:%M").to_string(),
            "2026-10-08 09:00"
        );
        let early = previous_slot(&trigger, local(2026, 10, 8, 8, 0)).expect("slot");
        assert_eq!(
            early.format("%Y-%m-%d %H:%M").to_string(),
            "2026-10-07 09:00"
        );
    }

    #[test]
    fn weekly_slot_respects_the_weekday_mask() {
        // Bit 3 (Thursday) only.
        let trigger = ScheduleTrigger::Weekly {
            weekdays: 0b000_1000,
            hour: 9,
            minute: 0,
        };
        let thursday = previous_slot(&trigger, local(2026, 10, 8, 12, 0)).expect("slot");
        assert_eq!(
            thursday.format("%Y-%m-%d %H:%M").to_string(),
            "2026-10-08 09:00"
        );
        // Friday noon: the most recent Thursday slot, not Friday.
        let friday = previous_slot(&trigger, local(2026, 10, 9, 12, 0)).expect("slot");
        assert_eq!(
            friday.format("%Y-%m-%d %H:%M").to_string(),
            "2026-10-08 09:00"
        );
        // Thursday before 09:00: the previous Thursday.
        let early = previous_slot(&trigger, local(2026, 10, 8, 8, 0)).expect("slot");
        assert_eq!(
            early.format("%Y-%m-%d %H:%M").to_string(),
            "2026-10-01 09:00"
        );
    }

    #[test]
    fn once_slots_fire_only_after_their_time() {
        let at = local(2026, 10, 8, 9, 0).timestamp_millis() as u64;
        let trigger = ScheduleTrigger::Once { at_unix_ms: at };
        assert!(previous_slot(&trigger, local(2026, 10, 8, 8, 59)).is_none());
        assert!(previous_slot(&trigger, local(2026, 10, 8, 9, 0)).is_some());
    }

    #[test]
    fn file_patterns_match_names_case_insensitively() {
        assert!(match_file_pattern("*.pdf", "Invoice.PDF"));
        assert!(match_file_pattern("report-*.csv", "report-2026-10.csv"));
        assert!(match_file_pattern("exact.txt", "EXACT.TXT"));
        assert!(!match_file_pattern("*.pdf", "invoice.pdf.bak"));
        assert!(!match_file_pattern("a?c", "ac"));
        assert!(match_file_pattern("a?c", "abc"));
    }
}
