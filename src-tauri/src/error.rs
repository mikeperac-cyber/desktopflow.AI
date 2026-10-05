use serde::ser::{Serialize, SerializeStruct, Serializer};
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("DeskFlow could not read or save its settings.")]
    SettingsStorage,
    #[error("The selected shortcut is not valid: {0}")]
    InvalidShortcut(String),
    #[error("DeskFlow could not register the shortcut: {0}")]
    Hotkey(String),
    #[error("DeskFlow could not update the Windows startup setting: {0}")]
    Autostart(String),
    #[error("DeskFlow could not update secure credentials: {0}")]
    CredentialStorage(String),
    #[error("DeskFlow could not open or close the requested window: {0}")]
    Window(String),
    #[error("The requested window is not available.")]
    WindowUnavailable,
    #[error("The settings are not valid: {0}")]
    InvalidSettings(String),
    #[error("DeskFlow could not capture the active application: {0}")]
    Context(String),
    #[error("DeskFlow could not inspect the target interface: {0}")]
    UiAutomation(String),
    #[error("DeskFlow could not highlight the selected control: {0}")]
    Highlight(String),
    #[error("DeskFlow AI is not configured: {0}")]
    AiConfiguration(String),
    #[error("DeskFlow could not create an AI plan: {0}")]
    AiProvider(String),
    #[error("The AI plan was rejected by DeskFlow: {0}")]
    InvalidPlan(String),
    #[error("DeskFlow blocked execution: {0}")]
    ExecutionPolicy(String),
    #[error("DeskFlow could not execute the plan safely: {0}")]
    Execution(String),
}

impl AppError {
    fn code(&self) -> &'static str {
        match self {
            Self::SettingsStorage => "settings_storage",
            Self::InvalidShortcut(_) => "invalid_shortcut",
            Self::Hotkey(_) => "hotkey_registration",
            Self::Autostart(_) => "autostart",
            Self::CredentialStorage(_) => "credential_storage",
            Self::Window(_) | Self::WindowUnavailable => "window",
            Self::InvalidSettings(_) => "invalid_settings",
            Self::Context(_) => "window_context",
            Self::UiAutomation(_) => "ui_automation",
            Self::Highlight(_) => "target_highlight",
            Self::AiConfiguration(_) => "ai_configuration",
            Self::AiProvider(_) => "ai_provider",
            Self::InvalidPlan(_) => "invalid_ai_plan",
            Self::ExecutionPolicy(_) => "execution_policy",
            Self::Execution(_) => "execution",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut value = serializer.serialize_struct("CommandError", 2)?;
        value.serialize_field("code", self.code())?;
        value.serialize_field("message", &self.to_string())?;
        value.end()
    }
}
