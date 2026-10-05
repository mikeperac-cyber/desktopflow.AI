use tauri::{
    Manager,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::{error::AppResult, runtime::RuntimeState, windows};

pub fn install(app: &tauri::App) -> AppResult<()> {
    let open = MenuItem::with_id(app, "open", "Open DeskFlow", true, None::<&str>)
        .map_err(|error| crate::error::AppError::Window(error.to_string()))?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)
        .map_err(|error| crate::error::AppError::Window(error.to_string()))?;
    let paused = app.state::<RuntimeState>().status().paused;
    let pause =
        CheckMenuItem::with_id(app, "pause", "Pause automation", true, paused, None::<&str>)
            .map_err(|error| crate::error::AppError::Window(error.to_string()))?;
    let separator = PredefinedMenuItem::separator(app)
        .map_err(|error| crate::error::AppError::Window(error.to_string()))?;
    let quit = MenuItem::with_id(app, "quit", "Quit DeskFlow", true, None::<&str>)
        .map_err(|error| crate::error::AppError::Window(error.to_string()))?;
    let menu = Menu::with_items(app, &[&open, &settings, &pause, &separator, &quit])
        .map_err(|error| crate::error::AppError::Window(error.to_string()))?;

    let pause_for_menu = pause.clone();
    let mut builder = TrayIconBuilder::with_id("deskflow-tray")
        .tooltip("DeskFlow AI")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "open" => {
                if let Err(error) = windows::show_overlay(app) {
                    eprintln!("DESKFLOW_TRAY_OPEN_FAILED code={error}");
                }
            }
            "settings" => {
                if let Err(error) = windows::show_settings(app) {
                    eprintln!("DESKFLOW_TRAY_SETTINGS_FAILED code={error}");
                }
            }
            "pause" => {
                let paused = app.state::<RuntimeState>().toggle_paused();
                if let Err(error) = pause_for_menu.set_checked(paused) {
                    eprintln!("DESKFLOW_TRAY_PAUSE_STATE_FAILED code={error}");
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
                && let Err(error) = windows::show_overlay(tray.app_handle())
            {
                eprintln!("DESKFLOW_TRAY_CLICK_FAILED code={error}");
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder
        .build(app)
        .map_err(|error| crate::error::AppError::Window(error.to_string()))?;
    Ok(())
}
