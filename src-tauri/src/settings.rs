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
}
