use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{
    ai::{AiProviderKind, PlanningModel},
    error::{AppError, AppResult},
};

const SETTINGS_FILE_NAME: &str = "settings.json";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalPolicy {
    Balanced,
    AlwaysAsk,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SavedWorkflow {
    pub id: String,
    pub name: String,
    pub instruction: String,
    pub created_at_unix_ms: u64,
}

pub const MAX_SAVED_WORKFLOWS: usize = 50;
const MAX_WORKFLOW_ID_CHARS: usize = 80;
const MAX_WORKFLOW_NAME_CHARS: usize = 80;
const MAX_WORKFLOW_INSTRUCTION_CHARS: usize = 4_000;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScheduleTrigger {
    Once { at_unix_ms: u64 },
    Daily { hour: u8, minute: u8 },
    Weekly { weekdays: u8, hour: u8, minute: u8 },
    FileAppears { folder: String, pattern: String },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ScheduledWorkflow {
    pub id: String,
    pub name: String,
    pub instruction: String,
    pub trigger: ScheduleTrigger,
    pub autonomous: bool,
    pub enabled: bool,
    pub expected_process: Option<String>,
    pub created_at_unix_ms: u64,
}

pub const MAX_SCHEDULES: usize = 20;
const MAX_SCHEDULE_ID_CHARS: usize = 80;
const MAX_SCHEDULE_NAME_CHARS: usize = 80;
const MAX_EXPECTED_PROCESS_CHARS: usize = 64;
const MAX_WATCH_FOLDER_CHARS: usize = 260;
const MAX_FILE_PATTERN_CHARS: usize = 80;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AppSettings {
    pub theme: ThemePreference,
    pub global_hotkey: String,
    pub emergency_hotkey: String,
    pub launch_at_startup: bool,
    pub highlight_targets: bool,
    pub execution_delay_ms: u32,
    pub approval_policy: ApprovalPolicy,
    pub maximum_autonomous_steps: u16,
    #[serde(default)]
    pub ai_provider: AiProviderKind,
    #[serde(default = "default_ai_model")]
    pub ai_model: PlanningModel,
    pub screenshot_transmission: bool,
    pub diagnostic_logging: bool,
    pub developer_mode: bool,
    #[serde(default)]
    pub saved_workflows: Vec<SavedWorkflow>,
    #[serde(default)]
    pub schedules: Vec<ScheduledWorkflow>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: ThemePreference::System,
            global_hotkey: "Alt+Space".to_string(),
            emergency_hotkey: "Esc".to_string(),
            launch_at_startup: false,
            highlight_targets: true,
            execution_delay_ms: 250,
            approval_policy: ApprovalPolicy::Balanced,
            maximum_autonomous_steps: 12,
            ai_provider: AiProviderKind::Local,
            ai_model: default_ai_model(),
            screenshot_transmission: false,
            diagnostic_logging: false,
            developer_mode: false,
            saved_workflows: Vec::new(),
            schedules: Vec::new(),
        }
    }
}

fn default_ai_model() -> PlanningModel {
    PlanningModel::Fast
}

impl AppSettings {
    pub fn validate(&self) -> AppResult<()> {
        validate_shortcut_label("global shortcut", &self.global_hotkey)?;
        validate_shortcut_label("emergency shortcut", &self.emergency_hotkey)?;

        if !(50..=2_000).contains(&self.execution_delay_ms) {
            return Err(AppError::InvalidSettings(
                "execution delay must be between 50 and 2000 milliseconds".to_string(),
            ));
        }

        if !(1..=100).contains(&self.maximum_autonomous_steps) {
            return Err(AppError::InvalidSettings(
                "maximum autonomous steps must be between 1 and 100".to_string(),
            ));
        }

        if self.saved_workflows.len() > MAX_SAVED_WORKFLOWS {
            return Err(AppError::InvalidSettings(format!(
                "at most {MAX_SAVED_WORKFLOWS} saved workflows are kept"
            )));
        }
        let mut workflow_ids = std::collections::HashSet::new();
        for workflow in &self.saved_workflows {
            validate_workflow_text("workflow id", &workflow.id, MAX_WORKFLOW_ID_CHARS)?;
            validate_workflow_text("workflow name", &workflow.name, MAX_WORKFLOW_NAME_CHARS)?;
            validate_workflow_text(
                "workflow instruction",
                &workflow.instruction,
                MAX_WORKFLOW_INSTRUCTION_CHARS,
            )?;
            if !workflow_ids.insert(workflow.id.as_str()) {
                return Err(AppError::InvalidSettings(format!(
                    "workflow id '{}' is duplicated",
                    workflow.id
                )));
            }
        }

        self.validate_schedules()?;

        Ok(())
    }
}

impl AppSettings {
    fn validate_schedules(&self) -> AppResult<()> {
        if self.schedules.len() > MAX_SCHEDULES {
            return Err(AppError::InvalidSettings(format!(
                "at most {MAX_SCHEDULES} schedules are kept"
            )));
        }
        let mut schedule_ids = std::collections::HashSet::new();
        for schedule in &self.schedules {
            validate_workflow_text("schedule id", &schedule.id, MAX_SCHEDULE_ID_CHARS)?;
            validate_workflow_text("schedule name", &schedule.name, MAX_SCHEDULE_NAME_CHARS)?;
            validate_workflow_text(
                "schedule instruction",
                &schedule.instruction,
                MAX_WORKFLOW_INSTRUCTION_CHARS,
            )?;
            if !schedule_ids.insert(schedule.id.as_str()) {
                return Err(AppError::InvalidSettings(format!(
                    "schedule id '{}' is duplicated",
                    schedule.id
                )));
            }
            match &schedule.trigger {
                ScheduleTrigger::Once { at_unix_ms } => {
                    if *at_unix_ms == 0 {
                        return Err(AppError::InvalidSettings(
                            "one-time schedules need a future run time".to_string(),
                        ));
                    }
                }
                ScheduleTrigger::Daily { hour, minute } => {
                    validate_clock("daily schedule", *hour, *minute)?;
                }
                ScheduleTrigger::Weekly {
                    weekdays,
                    hour,
                    minute,
                } => {
                    if *weekdays == 0 || *weekdays > 0x7F {
                        return Err(AppError::InvalidSettings(
                            "weekly schedules need at least one weekday".to_string(),
                        ));
                    }
                    validate_clock("weekly schedule", *hour, *minute)?;
                }
                ScheduleTrigger::FileAppears { folder, pattern } => {
                    validate_workflow_text("watched folder", folder, MAX_WATCH_FOLDER_CHARS)?;
                    validate_file_pattern(pattern)?;
                }
            }
            if let Some(process) = schedule.expected_process.as_deref() {
                validate_workflow_text("expected process", process, MAX_EXPECTED_PROCESS_CHARS)?;
            }
        }
        Ok(())
    }
}

fn validate_workflow_text(label: &str, value: &str, maximum: usize) -> AppResult<()> {
    let length = value.trim().chars().count();
    if length == 0 || length > maximum {
        return Err(AppError::InvalidSettings(format!(
            "{label} must contain between 1 and {maximum} characters"
        )));
    }
    Ok(())
}

fn validate_clock(label: &str, hour: u8, minute: u8) -> AppResult<()> {
    if hour > 23 || minute > 59 {
        return Err(AppError::InvalidSettings(format!(
            "{label} needs an hour of 0-23 and a minute of 0-59"
        )));
    }
    Ok(())
}

/// File patterns match against file *names* only, never paths, so directory
/// traversal is impossible by construction. The charset is still bounded to
/// keep matching predictable and logs clean.
fn validate_file_pattern(pattern: &str) -> AppResult<()> {
    let trimmed = pattern.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_FILE_PATTERN_CHARS {
        return Err(AppError::InvalidSettings(format!(
            "file patterns must contain between 1 and {MAX_FILE_PATTERN_CHARS} characters"
        )));
    }
    if !trimmed
        .chars()
        .all(|value| value.is_alphanumeric() || " *?._-".contains(value))
    {
        return Err(AppError::InvalidSettings(
            "file patterns may only contain letters, digits, spaces, and * ? . _ -".to_string(),
        ));
    }
    Ok(())
}

fn validate_shortcut_label(label: &str, value: &str) -> AppResult<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return Err(AppError::InvalidSettings(format!(
            "{label} must contain between 1 and 64 characters"
        )));
    }
    Ok(())
}

fn settings_path(app: &AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_config_dir()
        .map(|directory| directory.join(SETTINGS_FILE_NAME))
        .map_err(|_| AppError::SettingsStorage)
}

pub fn load_or_default(app: &AppHandle) -> AppSettings {
    let Ok(path) = settings_path(app) else {
        eprintln!("DESKFLOW_SETTINGS_PATH_UNAVAILABLE");
        return AppSettings::default();
    };

    match fs::read_to_string(path) {
        Ok(contents) => match serde_json::from_str::<AppSettings>(&contents) {
            Ok(settings) if settings.validate().is_ok() => settings,
            Ok(_) | Err(_) => {
                eprintln!("DESKFLOW_SETTINGS_INVALID using safe defaults");
                AppSettings::default()
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => AppSettings::default(),
        Err(_) => {
            eprintln!("DESKFLOW_SETTINGS_READ_FAILED using safe defaults");
            AppSettings::default()
        }
    }
}

pub fn persist(app: &AppHandle, settings: &AppSettings) -> AppResult<()> {
    settings.validate()?;
    let path = settings_path(app)?;
    let directory = path.parent().ok_or(AppError::SettingsStorage)?;
    fs::create_dir_all(directory).map_err(|_| AppError::SettingsStorage)?;

    let payload = serde_json::to_vec_pretty(settings).map_err(|_| AppError::SettingsStorage)?;
    fs::write(path, payload).map_err(|_| AppError::SettingsStorage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_keep_startup_and_screen_transmission_off() {
        let settings = AppSettings::default();
        assert!(!settings.launch_at_startup);
        assert!(!settings.screenshot_transmission);
        assert_eq!(settings.global_hotkey, "Alt+Space");
    }

    #[test]
    fn rejects_unbounded_automation_settings() {
        let settings = AppSettings {
            maximum_autonomous_steps: 0,
            ..AppSettings::default()
        };
        assert!(settings.validate().is_err());

        let delayed = AppSettings {
            execution_delay_ms: 2_001,
            ..AppSettings::default()
        };
        assert!(delayed.validate().is_err());
    }

    fn workflow_fixture(id: &str) -> SavedWorkflow {
        SavedWorkflow {
            id: id.to_string(),
            name: "Rename selected file".to_string(),
            instruction:
                "Rename the currently selected file to 'meeting-notes.txt' and confirm with Enter."
                    .to_string(),
            created_at_unix_ms: 1_800_000_000_000,
        }
    }

    #[test]
    fn accepts_a_bounded_workflow_library() {
        let settings = AppSettings {
            saved_workflows: vec![workflow_fixture("user-1")],
            ..AppSettings::default()
        };
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn rejects_unbounded_or_duplicate_workflows() {
        let empty_name = AppSettings {
            saved_workflows: vec![SavedWorkflow {
                name: "   ".to_string(),
                ..workflow_fixture("user-1")
            }],
            ..AppSettings::default()
        };
        assert!(empty_name.validate().is_err());

        let duplicated = AppSettings {
            saved_workflows: vec![workflow_fixture("user-1"), workflow_fixture("user-1")],
            ..AppSettings::default()
        };
        assert!(duplicated.validate().is_err());

        let oversized = AppSettings {
            saved_workflows: (0..=MAX_SAVED_WORKFLOWS)
                .map(|index| workflow_fixture(&format!("user-{index}")))
                .collect(),
            ..AppSettings::default()
        };
        assert!(oversized.validate().is_err());
    }

    fn schedule_fixture(id: &str) -> ScheduledWorkflow {
        ScheduledWorkflow {
            id: id.to_string(),
            name: "Morning inbox".to_string(),
            instruction: "Open the Downloads folder and sort by Date modified, newest first."
                .to_string(),
            trigger: ScheduleTrigger::Daily { hour: 9, minute: 0 },
            autonomous: false,
            enabled: true,
            expected_process: Some("explorer.exe".to_string()),
            created_at_unix_ms: 1_800_000_000_000,
        }
    }

    #[test]
    fn accepts_bounded_schedules() {
        let settings = AppSettings {
            schedules: vec![
                schedule_fixture("sched-1"),
                ScheduledWorkflow {
                    trigger: ScheduleTrigger::Weekly {
                        weekdays: 0b001_1111,
                        hour: 18,
                        minute: 30,
                    },
                    ..schedule_fixture("sched-2")
                },
                ScheduledWorkflow {
                    trigger: ScheduleTrigger::FileAppears {
                        folder: "C:\\Temp".to_string(),
                        pattern: "*.pdf".to_string(),
                    },
                    autonomous: true,
                    ..schedule_fixture("sched-3")
                },
            ],
            ..AppSettings::default()
        };
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn rejects_invalid_schedules() {
        let bad_clock = AppSettings {
            schedules: vec![ScheduledWorkflow {
                trigger: ScheduleTrigger::Daily {
                    hour: 25,
                    minute: 0,
                },
                ..schedule_fixture("sched-1")
            }],
            ..AppSettings::default()
        };
        assert!(bad_clock.validate().is_err());

        let no_weekday = AppSettings {
            schedules: vec![ScheduledWorkflow {
                trigger: ScheduleTrigger::Weekly {
                    weekdays: 0,
                    hour: 9,
                    minute: 0,
                },
                ..schedule_fixture("sched-1")
            }],
            ..AppSettings::default()
        };
        assert!(no_weekday.validate().is_err());

        let bad_pattern = AppSettings {
            schedules: vec![ScheduledWorkflow {
                trigger: ScheduleTrigger::FileAppears {
                    folder: "C:\\Temp".to_string(),
                    pattern: "../evil".to_string(),
                },
                ..schedule_fixture("sched-1")
            }],
            ..AppSettings::default()
        };
        assert!(bad_pattern.validate().is_err());

        let duplicated = AppSettings {
            schedules: vec![schedule_fixture("sched-1"), schedule_fixture("sched-1")],
            ..AppSettings::default()
        };
        assert!(duplicated.validate().is_err());
    }
}
