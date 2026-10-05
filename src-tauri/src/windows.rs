use tauri::{AppHandle, LogicalSize, Manager, Size};

use crate::{
    context,
    error::{AppError, AppResult},
    highlight,
    runtime::RuntimeState,
};

const OVERLAY_LABEL: &str = "overlay";
const SETTINGS_LABEL: &str = "settings";

fn window(app: &AppHandle, label: &str) -> AppResult<tauri::WebviewWindow> {
    app.get_webview_window(label)
        .ok_or(AppError::WindowUnavailable)
}

fn show_and_focus(app: &AppHandle, label: &str, center: bool) -> AppResult<()> {
    let window = window(app, label)?;
    if center {
        window
            .center()
            .map_err(|error| AppError::Window(error.to_string()))?;
    }
    window
        .show()
        .map_err(|error| AppError::Window(error.to_string()))?;
    window
        .set_focus()
        .map_err(|error| AppError::Window(error.to_string()))
}

pub fn show_overlay(app: &AppHandle) -> AppResult<()> {
    highlight::hide(app)?;
    let state = app.state::<RuntimeState>();
    match context::capture_foreground_context() {
        Ok(snapshot) => state.set_window_context(snapshot),
        Err(error) => state.set_context_warning(error.to_string()),
    }
    show_and_focus(app, OVERLAY_LABEL, true)
}

pub fn restore_overlay(app: &AppHandle) -> AppResult<()> {
    set_overlay_plan_mode(app, true)?;
    show_and_focus(app, OVERLAY_LABEL, true)
}

pub fn set_overlay_plan_mode(app: &AppHandle, expanded: bool) -> AppResult<()> {
    let overlay = window(app, OVERLAY_LABEL)?;
    let height = if expanded { 620.0 } else { 250.0 };
    overlay
        .set_size(Size::Logical(LogicalSize::new(680.0, height)))
        .map_err(|error| AppError::Window(error.to_string()))?;
    overlay
        .center()
        .map_err(|error| AppError::Window(error.to_string()))
}

pub fn toggle_overlay(app: &AppHandle) -> AppResult<()> {
    let overlay = window(app, OVERLAY_LABEL)?;
    let visible = overlay
        .is_visible()
        .map_err(|error| AppError::Window(error.to_string()))?;
    if visible {
        overlay
            .hide()
            .map_err(|error| AppError::Window(error.to_string()))
    } else {
        show_overlay(app)
    }
}

pub fn show_settings(app: &AppHandle) -> AppResult<()> {
    show_and_focus(app, SETTINGS_LABEL, true)
}

pub fn restore_execution_surface(app: &AppHandle, surface: &str) -> AppResult<()> {
    match surface {
        OVERLAY_LABEL => restore_overlay(app),
        SETTINGS_LABEL => show_settings(app),
        _ => Err(AppError::WindowUnavailable),
    }
}

pub fn hide_known_window(app: &AppHandle, label: &str) -> AppResult<()> {
    if !matches!(label, OVERLAY_LABEL | SETTINGS_LABEL) {
        return Err(AppError::WindowUnavailable);
    }

    if label == SETTINGS_LABEL {
        highlight::hide(app)?;
    }

    window(app, label)?
        .hide()
        .map_err(|error| AppError::Window(error.to_string()))
}
