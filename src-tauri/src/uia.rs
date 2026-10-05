use serde::Serialize;

use crate::{
    context::{ActiveTargetSummary, PixelRect, WindowContextSnapshot},
    error::{AppError, AppResult},
};

const MAX_TEXT_CHARS: usize = 220;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UiAutomationLimits {
    pub max_depth: usize,
    pub max_visited: usize,
    pub max_elements: usize,
    pub max_children_per_parent: usize,
    pub timeout_ms: u64,
}

impl Default for UiAutomationLimits {
    fn default() -> Self {
        Self {
            max_depth: 12,
            max_visited: 1_500,
            max_elements: 350,
            max_children_per_parent: 250,
            timeout_ms: 1_500,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NormalizedUiElement {
    pub id: String,
    pub parent_id: Option<String>,
    pub depth: usize,
    pub name: String,
    pub role: String,
    pub automation_id: String,
    pub class_name: String,
    pub framework_id: String,
    pub bounds_physical: Option<PixelRect>,
    pub is_enabled: bool,
    pub is_offscreen: bool,
    pub is_keyboard_focusable: bool,
    pub has_keyboard_focus: bool,
    pub is_password: bool,
    pub supported_patterns: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct UiAutomationSnapshot {
    pub captured_at_unix_ms: u64,
    pub target: ActiveTargetSummary,
    pub root_id: Option<String>,
    pub elements: Vec<NormalizedUiElement>,
    pub visited_count: usize,
    pub filtered_count: usize,
    pub truncated: bool,
    pub duration_ms: u64,
    pub limits: UiAutomationLimits,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug)]
struct ElementCandidate {
    name: String,
    role: String,
    automation_id: String,
    class_name: String,
    framework_id: String,
    bounds_physical: Option<PixelRect>,
    is_enabled: bool,
    is_offscreen: bool,
    is_keyboard_focusable: bool,
    has_keyboard_focus: bool,
    is_password: bool,
}

fn sanitize_text(value: &str) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= MAX_TEXT_CHARS {
        return normalized;
    }

    let mut truncated = normalized
        .chars()
        .take(MAX_TEXT_CHARS.saturating_sub(1))
        .collect::<String>();
    truncated.push('…');
    truncated
}

fn is_interactive_role(role: &str) -> bool {
    matches!(
        role,
        "button"
            | "calendar"
            | "checkbox"
            | "combobox"
            | "data-grid"
            | "data-item"
            | "document"
            | "edit"
            | "hyperlink"
            | "list"
            | "list-item"
            | "menu"
            | "menu-item"
            | "radio-button"
            | "scrollbar"
            | "slider"
            | "spinner"
            | "split-button"
            | "tab"
            | "tab-item"
            | "table"
            | "tree"
            | "tree-item"
            | "window"
    )
}

fn should_include(candidate: &ElementCandidate, is_root: bool) -> bool {
    if is_root {
        return true;
    }
    if candidate.is_offscreen && !candidate.has_keyboard_focus {
        return false;
    }

    let has_area = candidate
        .bounds_physical
        .as_ref()
        .is_some_and(|bounds| bounds.width > 0 && bounds.height > 0);
    if !has_area && !candidate.is_keyboard_focusable && !candidate.has_keyboard_focus {
        return false;
    }

    if is_interactive_role(&candidate.role) {
        return true;
    }

    if !candidate.name.is_empty() || !candidate.automation_id.is_empty() {
        return true;
    }

    !matches!(
        candidate.role.as_str(),
        "pane" | "group" | "custom" | "unknown"
    )
}

fn timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

pub fn inspect_captured_window(context: &WindowContextSnapshot) -> AppResult<UiAutomationSnapshot> {
    #[cfg(windows)]
    {
        platform::inspect(context, UiAutomationLimits::default()).map_err(AppError::UiAutomation)
    }

    #[cfg(not(windows))]
    {
        let _ = context;
        Err(AppError::UiAutomation(
            "Windows UI Automation is available only on Windows.".to_string(),
        ))
    }
}

#[cfg(windows)]
mod platform {
    use std::{ffi::c_void, time::Instant};

    use windows::{
        Win32::{
            Foundation::{HWND, RECT},
            System::Com::{
                CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
                CoUninitialize,
            },
            UI::Accessibility::{
                CUIAutomation8, IUIAutomation, IUIAutomationElement, IUIAutomationTreeWalker,
                UIA_AppBarControlTypeId, UIA_ButtonControlTypeId, UIA_CONTROLTYPE_ID,
                UIA_CalendarControlTypeId, UIA_CheckBoxControlTypeId, UIA_ComboBoxControlTypeId,
                UIA_CustomControlTypeId, UIA_DataGridControlTypeId, UIA_DataItemControlTypeId,
                UIA_DocumentControlTypeId, UIA_EditControlTypeId, UIA_ExpandCollapsePatternId,
                UIA_GridItemPatternId, UIA_GridPatternId, UIA_GroupControlTypeId,
                UIA_HeaderControlTypeId, UIA_HeaderItemControlTypeId, UIA_HyperlinkControlTypeId,
                UIA_ImageControlTypeId, UIA_InvokePatternId, UIA_ItemContainerPatternId,
                UIA_ListControlTypeId, UIA_ListItemControlTypeId, UIA_MenuBarControlTypeId,
                UIA_MenuControlTypeId, UIA_MenuItemControlTypeId, UIA_PATTERN_ID,
                UIA_PaneControlTypeId, UIA_ProgressBarControlTypeId, UIA_RadioButtonControlTypeId,
                UIA_RangeValuePatternId, UIA_ScrollBarControlTypeId, UIA_ScrollItemPatternId,
                UIA_ScrollPatternId, UIA_SelectionItemPatternId, UIA_SelectionPatternId,
                UIA_SeparatorControlTypeId, UIA_SliderControlTypeId, UIA_SpinnerControlTypeId,
                UIA_SplitButtonControlTypeId, UIA_StatusBarControlTypeId, UIA_TabControlTypeId,
                UIA_TabItemControlTypeId, UIA_TableControlTypeId, UIA_TableItemPatternId,
                UIA_TablePatternId, UIA_TextControlTypeId, UIA_TextPatternId,
                UIA_ThumbControlTypeId, UIA_TitleBarControlTypeId, UIA_TogglePatternId,
                UIA_ToolBarControlTypeId, UIA_ToolTipControlTypeId, UIA_TransformPatternId,
                UIA_TreeControlTypeId, UIA_TreeItemControlTypeId, UIA_ValuePatternId,
                UIA_VirtualizedItemPatternId, UIA_WindowControlTypeId, UIA_WindowPatternId,
            },
        },
        core::BSTR,
    };

    use super::{
        ElementCandidate, NormalizedUiElement, PixelRect, UiAutomationLimits, UiAutomationSnapshot,
        WindowContextSnapshot, sanitize_text, should_include, timestamp_ms,
    };

    struct ComApartment;

    impl ComApartment {
        fn initialize() -> Result<Self, String> {
            // SAFETY: this worker owns its COM apartment for the lifetime of this guard.
            unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
                .ok()
                .map_err(|error| format!("Windows could not initialize UI Automation: {error}"))?;
            Ok(Self)
        }
    }

    impl Drop for ComApartment {
        fn drop(&mut self) {
            // SAFETY: initialization succeeded on this thread and is balanced exactly once.
            unsafe { CoUninitialize() };
        }
    }

    struct PendingElement {
        element: IUIAutomationElement,
        raw_depth: usize,
        parent_id: Option<String>,
        normalized_depth: usize,
        is_root: bool,
    }

    fn string_property(result: windows::core::Result<BSTR>) -> String {
        result
            .map(|value| sanitize_text(&String::from_utf16_lossy(&value)))
            .unwrap_or_default()
    }

    fn bool_property(result: windows::core::Result<windows::core::BOOL>, fallback: bool) -> bool {
        result.map(|value| value.as_bool()).unwrap_or(fallback)
    }

    fn bounds_property(result: windows::core::Result<RECT>) -> Option<PixelRect> {
        let rect = result.ok()?;
        let width = rect.right.checked_sub(rect.left)?.try_into().ok()?;
        let height = rect.bottom.checked_sub(rect.top)?.try_into().ok()?;
        if width == 0 || height == 0 {
            return None;
        }
        Some(PixelRect {
            left: rect.left,
            top: rect.top,
            width,
            height,
        })
    }

    fn role_for(control_type: UIA_CONTROLTYPE_ID, localized: String) -> String {
        let role = match control_type {
            value if value == UIA_AppBarControlTypeId => "app-bar",
            value if value == UIA_ButtonControlTypeId => "button",
            value if value == UIA_CalendarControlTypeId => "calendar",
            value if value == UIA_CheckBoxControlTypeId => "checkbox",
            value if value == UIA_ComboBoxControlTypeId => "combobox",
            value if value == UIA_CustomControlTypeId => "custom",
            value if value == UIA_DataGridControlTypeId => "data-grid",
            value if value == UIA_DataItemControlTypeId => "data-item",
            value if value == UIA_DocumentControlTypeId => "document",
            value if value == UIA_EditControlTypeId => "edit",
            value if value == UIA_GroupControlTypeId => "group",
            value if value == UIA_HeaderControlTypeId => "header",
            value if value == UIA_HeaderItemControlTypeId => "header-item",
            value if value == UIA_HyperlinkControlTypeId => "hyperlink",
            value if value == UIA_ImageControlTypeId => "image",
            value if value == UIA_ListControlTypeId => "list",
            value if value == UIA_ListItemControlTypeId => "list-item",
            value if value == UIA_MenuBarControlTypeId => "menu-bar",
            value if value == UIA_MenuControlTypeId => "menu",
            value if value == UIA_MenuItemControlTypeId => "menu-item",
            value if value == UIA_PaneControlTypeId => "pane",
            value if value == UIA_ProgressBarControlTypeId => "progress-bar",
            value if value == UIA_RadioButtonControlTypeId => "radio-button",
            value if value == UIA_ScrollBarControlTypeId => "scrollbar",
            value if value == UIA_SeparatorControlTypeId => "separator",
            value if value == UIA_SliderControlTypeId => "slider",
            value if value == UIA_SpinnerControlTypeId => "spinner",
            value if value == UIA_SplitButtonControlTypeId => "split-button",
            value if value == UIA_StatusBarControlTypeId => "status-bar",
            value if value == UIA_TabControlTypeId => "tab",
            value if value == UIA_TabItemControlTypeId => "tab-item",
            value if value == UIA_TableControlTypeId => "table",
            value if value == UIA_TextControlTypeId => "text",
            value if value == UIA_ThumbControlTypeId => "thumb",
            value if value == UIA_TitleBarControlTypeId => "title-bar",
            value if value == UIA_ToolBarControlTypeId => "toolbar",
            value if value == UIA_ToolTipControlTypeId => "tooltip",
            value if value == UIA_TreeControlTypeId => "tree",
            value if value == UIA_TreeItemControlTypeId => "tree-item",
            value if value == UIA_WindowControlTypeId => "window",
            _ => "",
        };

        if role.is_empty() {
            let localized = localized.to_lowercase().replace(' ', "-");
            if localized.is_empty() {
                "unknown".to_string()
            } else {
                localized
            }
        } else {
            role.to_string()
        }
    }

    fn candidate(element: &IUIAutomationElement) -> ElementCandidate {
        // SAFETY: UIA element getters are read-only COM calls on the worker's initialized apartment.
        unsafe {
            let localized = string_property(element.CurrentLocalizedControlType());
            ElementCandidate {
                name: string_property(element.CurrentName()),
                role: role_for(
                    element
                        .CurrentControlType()
                        .unwrap_or(UIA_CONTROLTYPE_ID(0)),
                    localized,
                ),
                automation_id: string_property(element.CurrentAutomationId()),
                class_name: string_property(element.CurrentClassName()),
                framework_id: string_property(element.CurrentFrameworkId()),
                bounds_physical: bounds_property(element.CurrentBoundingRectangle()),
                is_enabled: bool_property(element.CurrentIsEnabled(), true),
                is_offscreen: bool_property(element.CurrentIsOffscreen(), false),
                is_keyboard_focusable: bool_property(element.CurrentIsKeyboardFocusable(), false),
                has_keyboard_focus: bool_property(element.CurrentHasKeyboardFocus(), false),
                is_password: bool_property(element.CurrentIsPassword(), false),
            }
        }
    }

    fn supported_patterns(element: &IUIAutomationElement) -> Vec<String> {
        const PATTERNS: [(&str, UIA_PATTERN_ID); 18] = [
            ("invoke", UIA_InvokePatternId),
            ("value", UIA_ValuePatternId),
            ("range-value", UIA_RangeValuePatternId),
            ("toggle", UIA_TogglePatternId),
            ("expand-collapse", UIA_ExpandCollapsePatternId),
            ("selection", UIA_SelectionPatternId),
            ("selection-item", UIA_SelectionItemPatternId),
            ("scroll", UIA_ScrollPatternId),
            ("scroll-item", UIA_ScrollItemPatternId),
            ("grid", UIA_GridPatternId),
            ("grid-item", UIA_GridItemPatternId),
            ("table", UIA_TablePatternId),
            ("table-item", UIA_TableItemPatternId),
            ("text", UIA_TextPatternId),
            ("window", UIA_WindowPatternId),
            ("transform", UIA_TransformPatternId),
            ("item-container", UIA_ItemContainerPatternId),
            ("virtualized-item", UIA_VirtualizedItemPatternId),
        ];

        let mut supported = Vec::new();
        for (name, pattern) in PATTERNS {
            // SAFETY: only pattern availability is queried; no pattern operation is invoked.
            if unsafe { element.GetCurrentPattern(pattern) }.is_ok() {
                supported.push(name.to_string());
            }
        }
        supported
    }

    fn children(
        walker: &IUIAutomationTreeWalker,
        parent: &IUIAutomationElement,
        limit: usize,
    ) -> (Vec<IUIAutomationElement>, bool) {
        let mut result = Vec::new();
        // SAFETY: walker and parent are valid COM interfaces in the current apartment.
        let Ok(mut current) = (unsafe { walker.GetFirstChildElement(parent) }) else {
            return (result, false);
        };

        loop {
            result.push(current.clone());
            if result.len() >= limit {
                return (result, true);
            }
            // SAFETY: current came from this walker and remains alive for the call.
            match unsafe { walker.GetNextSiblingElement(&current) } {
                Ok(next) => current = next,
                Err(_) => return (result, false),
            }
        }
    }

    pub fn inspect(
        context: &WindowContextSnapshot,
        limits: UiAutomationLimits,
    ) -> Result<UiAutomationSnapshot, String> {
        if context.native_window_handle == 0 {
            return Err("The captured window handle is no longer available.".to_string());
        }

        let started = Instant::now();
        let _apartment = ComApartment::initialize()?;
        // SAFETY: COM is initialized and CUIAutomation8 is an in-process UIA client class.
        let automation: IUIAutomation =
            unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) }
                .map_err(|error| format!("Windows could not start UI Automation: {error}"))?;
        let hwnd = HWND(context.native_window_handle as *mut c_void);
        // SAFETY: the handle was captured from GetForegroundWindow and UIA validates it.
        let root = unsafe { automation.ElementFromHandle(hwnd) }.map_err(|error| {
            format!("The captured application is no longer accessible: {error}")
        })?;
        // SAFETY: this is a read-only identity property on the root returned for the captured HWND.
        let root_process_id = unsafe { root.CurrentProcessId() }
            .map_err(|error| format!("Windows could not verify the captured target: {error}"))?;
        if root_process_id <= 0 || root_process_id as u32 != context.process.id {
            return Err(
                "The captured window changed before inspection. Capture the target again."
                    .to_string(),
            );
        }
        // SAFETY: automation is a live UIA client object.
        let walker = unsafe { automation.ControlViewWalker() }.map_err(|error| {
            format!("Windows could not create the Control View walker: {error}")
        })?;

        let mut stack = vec![PendingElement {
            element: root,
            raw_depth: 0,
            parent_id: None,
            normalized_depth: 0,
            is_root: true,
        }];
        let mut elements = Vec::new();
        let mut visited_count = 0_usize;
        let mut filtered_count = 0_usize;
        let mut truncated = false;
        let mut warnings = Vec::new();

        while let Some(pending) = stack.pop() {
            if started.elapsed().as_millis() >= u128::from(limits.timeout_ms) {
                truncated = true;
                warnings.push(format!(
                    "Inspection stopped after the {} ms safety limit.",
                    limits.timeout_ms
                ));
                break;
            }
            if visited_count >= limits.max_visited {
                truncated = true;
                warnings.push(format!(
                    "Inspection stopped after visiting {} UI elements.",
                    limits.max_visited
                ));
                break;
            }
            visited_count += 1;

            let candidate = candidate(&pending.element);
            let include = should_include(&candidate, pending.is_root);
            let (child_parent_id, child_depth) = if include {
                if elements.len() >= limits.max_elements {
                    truncated = true;
                    warnings.push(format!(
                        "The normalized tree was limited to {} useful elements.",
                        limits.max_elements
                    ));
                    break;
                }
                let id = format!("uia-{:04}", elements.len() + 1);
                let patterns = supported_patterns(&pending.element);
                elements.push(NormalizedUiElement {
                    id: id.clone(),
                    parent_id: pending.parent_id.clone(),
                    depth: pending.normalized_depth,
                    name: candidate.name,
                    role: candidate.role,
                    automation_id: candidate.automation_id,
                    class_name: candidate.class_name,
                    framework_id: candidate.framework_id,
                    bounds_physical: candidate.bounds_physical,
                    is_enabled: candidate.is_enabled,
                    is_offscreen: candidate.is_offscreen,
                    is_keyboard_focusable: candidate.is_keyboard_focusable,
                    has_keyboard_focus: candidate.has_keyboard_focus,
                    is_password: candidate.is_password,
                    supported_patterns: patterns,
                });
                (Some(id), pending.normalized_depth + 1)
            } else {
                filtered_count += 1;
                (pending.parent_id, pending.normalized_depth)
            };

            if pending.raw_depth >= limits.max_depth {
                truncated = true;
                continue;
            }
            let (child_elements, child_truncated) =
                children(&walker, &pending.element, limits.max_children_per_parent);
            if child_truncated {
                truncated = true;
                if !warnings
                    .iter()
                    .any(|warning| warning.contains("child limit"))
                {
                    warnings.push(format!(
                        "At least one container exceeded the {}-child limit.",
                        limits.max_children_per_parent
                    ));
                }
            }
            for child in child_elements.into_iter().rev() {
                stack.push(PendingElement {
                    element: child,
                    raw_depth: pending.raw_depth + 1,
                    parent_id: child_parent_id.clone(),
                    normalized_depth: child_depth,
                    is_root: false,
                });
            }
        }

        if elements.is_empty() {
            return Err(
                "Windows returned an empty accessibility tree for this application.".to_string(),
            );
        }

        Ok(UiAutomationSnapshot {
            captured_at_unix_ms: timestamp_ms(),
            target: context.into(),
            root_id: elements.first().map(|element| element.id.clone()),
            elements,
            visited_count,
            filtered_count,
            truncated,
            duration_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
            limits,
            warnings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(role: &str) -> ElementCandidate {
        ElementCandidate {
            name: String::new(),
            role: role.to_string(),
            automation_id: String::new(),
            class_name: String::new(),
            framework_id: String::new(),
            bounds_physical: Some(PixelRect {
                left: -100,
                top: 20,
                width: 80,
                height: 30,
            }),
            is_enabled: true,
            is_offscreen: false,
            is_keyboard_focusable: false,
            has_keyboard_focus: false,
            is_password: false,
        }
    }

    #[test]
    fn keeps_interactive_controls_without_accessible_names() {
        assert!(should_include(&candidate("button"), false));
        assert!(should_include(&candidate("edit"), false));
    }

    #[test]
    fn filters_empty_structural_and_offscreen_nodes() {
        assert!(!should_include(&candidate("pane"), false));
        let mut button = candidate("button");
        button.is_offscreen = true;
        assert!(!should_include(&button, false));
    }

    #[test]
    fn keeps_root_and_focused_zero_area_nodes() {
        let mut node = candidate("custom");
        node.bounds_physical = None;
        assert!(should_include(&node, true));
        node.has_keyboard_focus = true;
        assert!(!should_include(&node, false));
        node.name = "Caret host".to_string();
        assert!(should_include(&node, false));
    }

    #[test]
    fn normalizes_whitespace_and_caps_provider_text() {
        assert_eq!(sanitize_text("  Save\r\n   as  "), "Save as");
        let long = "x".repeat(MAX_TEXT_CHARS + 50);
        let sanitized = sanitize_text(&long);
        assert_eq!(sanitized.chars().count(), MAX_TEXT_CHARS);
        assert!(sanitized.ends_with('…'));
    }
}
