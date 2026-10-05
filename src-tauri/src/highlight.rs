use serde::Serialize;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize};

use crate::{
    context::PixelRect,
    error::{AppError, AppResult},
    uia::{NormalizedUiElement, UiAutomationSnapshot},
};

pub const HIGHLIGHT_LABEL: &str = "highlight";
const MAX_HIGHLIGHT_PIXELS: u64 = 100_000_000;
const MAX_HIGHLIGHT_EDGE: u32 = 32_768;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TargetHighlight {
    pub element_id: String,
    pub bounds_physical: PixelRect,
}

fn validate_bounds(bounds: &PixelRect) -> Result<(), String> {
    if bounds.width == 0 || bounds.height == 0 {
        return Err("The selected element does not have a visible rectangle.".to_string());
    }
    if bounds.width > MAX_HIGHLIGHT_EDGE || bounds.height > MAX_HIGHLIGHT_EDGE {
        return Err("The selected element rectangle is too large to highlight safely.".to_string());
    }
    let area = u64::from(bounds.width)
        .checked_mul(u64::from(bounds.height))
        .ok_or_else(|| "The selected element rectangle is invalid.".to_string())?;
    if area > MAX_HIGHLIGHT_PIXELS {
        return Err("The selected element rectangle is too large to highlight safely.".to_string());
    }
    Ok(())
}

fn resolve_element(
    elements: &[NormalizedUiElement],
    element_id: &str,
) -> Result<TargetHighlight, String> {
    if element_id.is_empty() || element_id.len() > 64 {
        return Err("The selected element ID is invalid.".to_string());
    }
    let element = elements
        .iter()
        .find(|element| element.id == element_id)
        .ok_or_else(|| {
            "The selected element is not part of the current UI snapshot.".to_string()
        })?;
    let bounds = element
        .bounds_physical
        .clone()
        .ok_or_else(|| "The selected element does not expose screen bounds.".to_string())?;
    validate_bounds(&bounds)?;
    Ok(TargetHighlight {
        element_id: element.id.clone(),
        bounds_physical: bounds,
    })
}

pub fn resolve(snapshot: &UiAutomationSnapshot, element_id: &str) -> AppResult<TargetHighlight> {
    resolve_element(&snapshot.elements, element_id).map_err(AppError::Highlight)
}

fn window(app: &AppHandle) -> AppResult<tauri::WebviewWindow> {
    app.get_webview_window(HIGHLIGHT_LABEL)
        .ok_or(AppError::WindowUnavailable)
}

pub fn initialize(app: &AppHandle) -> AppResult<()> {
    let window = window(app)?;
    window
        .set_ignore_cursor_events(true)
        .map_err(|error| AppError::Highlight(error.to_string()))?;
    window
        .set_focusable(false)
        .map_err(|error| AppError::Highlight(error.to_string()))?;
    window
        .set_always_on_top(true)
        .map_err(|error| AppError::Highlight(error.to_string()))
}

pub fn show(app: &AppHandle, target: &TargetHighlight) -> AppResult<()> {
    validate_bounds(&target.bounds_physical).map_err(AppError::Highlight)?;
    let window = window(app)?;
    window
        .hide()
        .map_err(|error| AppError::Highlight(error.to_string()))?;
    window
        .set_size(PhysicalSize::new(
            target.bounds_physical.width,
            target.bounds_physical.height,
        ))
        .map_err(|error| AppError::Highlight(error.to_string()))?;
    window
        .set_position(PhysicalPosition::new(
            target.bounds_physical.left,
            target.bounds_physical.top,
        ))
        .map_err(|error| AppError::Highlight(error.to_string()))?;
    window
        .set_ignore_cursor_events(true)
        .map_err(|error| AppError::Highlight(error.to_string()))?;
    window
        .show()
        .map_err(|error| AppError::Highlight(error.to_string()))
}

pub fn hide(app: &AppHandle) -> AppResult<()> {
    window(app)?
        .hide()
        .map_err(|error| AppError::Highlight(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(bounds: Option<PixelRect>) -> NormalizedUiElement {
        NormalizedUiElement {
            id: "uia-0042".to_string(),
            parent_id: Some("uia-0001".to_string()),
            depth: 2,
            name: "Save".to_string(),
            role: "button".to_string(),
            automation_id: "SaveButton".to_string(),
            class_name: "Button".to_string(),
            framework_id: "Win32".to_string(),
            bounds_physical: bounds,
            is_enabled: true,
            is_offscreen: false,
            is_keyboard_focusable: true,
            has_keyboard_focus: false,
            is_password: false,
            supported_patterns: vec!["invoke".to_string()],
        }
    }

    #[test]
    fn preserves_negative_physical_coordinates_for_secondary_monitors() {
        let bounds = PixelRect {
            left: -1830,
            top: 140,
            width: 220,
            height: 48,
        };
        let target = resolve_element(&[element(Some(bounds.clone()))], "uia-0042").unwrap();
        assert_eq!(target.bounds_physical, bounds);
    }

    #[test]
    fn refuses_missing_or_unbounded_geometry() {
        assert!(resolve_element(&[element(None)], "uia-0042").is_err());
        assert!(
            resolve_element(
                &[element(Some(PixelRect {
                    left: 0,
                    top: 0,
                    width: 100_000,
                    height: 100_000,
                }))],
                "uia-0042",
            )
            .is_err()
        );
    }

    #[test]
    fn refuses_ids_outside_the_current_snapshot() {
        let error = resolve_element(
            &[element(Some(PixelRect {
                left: 0,
                top: 0,
                width: 100,
                height: 40,
            }))],
            "uia-9999",
        )
        .unwrap_err();
        assert!(error.contains("current UI snapshot"));
    }
}
