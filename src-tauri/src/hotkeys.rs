use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::{
    error::{AppError, AppResult},
    windows,
};

pub const FALLBACK_HOTKEY: &str = "Super+Shift+A";

pub fn normalize_shortcut(value: &str) -> AppResult<String> {
    let parts: Vec<&str> = value
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();

    if parts.is_empty() || parts.len() > 5 {
        return Err(AppError::InvalidShortcut(value.to_string()));
    }

    let normalized: Vec<String> = parts
        .iter()
        .map(|part| match part.to_ascii_lowercase().as_str() {
            "alt" => "Alt".to_string(),
            "ctrl" | "control" => "Control".to_string(),
            "shift" => "Shift".to_string(),
            "win" | "windows" | "super" | "meta" => "Super".to_string(),
            "space" | "spacebar" => "Space".to_string(),
            "esc" | "escape" => "Escape".to_string(),
            key if key.len() == 1
                && key
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric()) =>
            {
                key.to_ascii_uppercase()
            }
            key if key.starts_with('f')
                && key[1..]
                    .parse::<u8>()
                    .is_ok_and(|number| (1..=24).contains(&number)) =>
            {
                key.to_ascii_uppercase()
            }
            _ => part.to_string(),
        })
        .collect();

    Ok(normalized.join("+"))
}

pub fn display_shortcut(value: &str) -> String {
    value
        .split('+')
        .map(|part| match part {
            "Super" => "Win",
            "Control" => "Ctrl",
            "Escape" => "Esc",
            other => other,
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

fn register(app: &AppHandle, shortcut: &str) -> AppResult<()> {
    let normalized = normalize_shortcut(shortcut)?;
    app.global_shortcut()
        .on_shortcut(normalized.as_str(), |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed
                && let Err(error) = windows::toggle_overlay(app)
            {
                eprintln!("DESKFLOW_HOTKEY_WINDOW_FAILED code={error}");
            }
        })
        .map_err(|error| AppError::Hotkey(error.to_string()))
}

pub fn register_with_fallback(
    app: &AppHandle,
    requested: &str,
) -> (Option<String>, Option<String>) {
    let requested = match normalize_shortcut(requested) {
        Ok(value) => value,
        Err(error) => return (None, Some(error.to_string())),
    };

    match register(app, &requested) {
        Ok(()) => (Some(requested), None),
        Err(primary_error) => match register(app, FALLBACK_HOTKEY) {
            Ok(()) => (
                Some(FALLBACK_HOTKEY.to_string()),
                Some(format!(
                    "{} is unavailable. DeskFlow is using {} instead.",
                    display_shortcut(&requested),
                    display_shortcut(FALLBACK_HOTKEY)
                )),
            ),
            Err(_) => (None, Some(primary_error.to_string())),
        },
    }
}

fn register_emergency(app: &AppHandle, shortcut: &str) -> AppResult<()> {
    use tauri::Manager;
    let normalized = normalize_shortcut(shortcut)?;
    app.global_shortcut()
        .on_shortcut(normalized.as_str(), |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                let state = app.state::<crate::runtime::RuntimeState>();
                state.request_emergency_stop();
                if !state.status().executing {
                    let _ = windows::hide_known_window(app, "overlay");
                }
            }
        })
        .map_err(|error| AppError::Hotkey(error.to_string()))
}

pub fn register_emergency_with_fallback(
    app: &AppHandle,
    requested: &str,
) -> (Option<String>, Option<String>) {
    let requested = match normalize_shortcut(requested) {
        Ok(value) => value,
        Err(error) => return (None, Some(error.to_string())),
    };

    match register_emergency(app, &requested) {
        Ok(()) => (Some(requested), None),
        Err(error) => (None, Some(error.to_string())),
    }
}

pub fn replace_emergency_shortcut(
    app: &AppHandle,
    current: Option<&str>,
    requested: &str,
) -> AppResult<String> {
    let requested = normalize_shortcut(requested)?;
    if current == Some(requested.as_str()) {
        return Ok(requested);
    }

    if let Some(current) = current {
        let _ = app.global_shortcut().unregister(current);
    }

    if let Err(error) = register_emergency(app, &requested) {
        if let Some(current) = current {
            let _ = register_emergency(app, current);
        }
        return Err(error);
    }

    Ok(requested)
}

pub fn replace_shortcut(
    app: &AppHandle,
    current: Option<&str>,
    requested: &str,
) -> AppResult<String> {
    let requested = normalize_shortcut(requested)?;
    if current == Some(requested.as_str()) {
        return Ok(requested);
    }

    if let Some(current) = current {
        app.global_shortcut()
            .unregister(current)
            .map_err(|error| AppError::Hotkey(error.to_string()))?;
    }

    if let Err(error) = register(app, &requested) {
        if let Some(current) = current
            && let Err(rollback_error) = register(app, current)
        {
            eprintln!("DESKFLOW_HOTKEY_ROLLBACK_FAILED code={rollback_error}");
        }
        return Err(error);
    }

    Ok(requested)
}

pub fn restore_shortcut(app: &AppHandle, current: &str, previous: Option<&str>) -> AppResult<()> {
    app.global_shortcut()
        .unregister(current)
        .map_err(|error| AppError::Hotkey(error.to_string()))?;

    if let Some(previous) = previous {
        register(app, previous)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_windows_shortcut_names() {
        assert_eq!(
            normalize_shortcut("Win + shift + a").unwrap(),
            "Super+Shift+A"
        );
        assert_eq!(normalize_shortcut("alt+space").unwrap(), "Alt+Space");
        assert_eq!(display_shortcut("Super+Shift+A"), "Win + Shift + A");
    }

    #[test]
    fn rejects_empty_shortcuts() {
        assert!(normalize_shortcut(" + ").is_err());
    }
}
