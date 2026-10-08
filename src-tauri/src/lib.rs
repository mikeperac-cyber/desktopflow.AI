pub mod ai;
mod commands;
pub mod context;
mod credentials;
mod error;
pub mod executor;
mod highlight;
mod hotkeys;
pub mod recorder;
pub mod runtime;
pub mod security;
pub mod settings;
mod tray;
pub mod uia;
mod windows;
pub mod workflow;

use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

use runtime::RuntimeState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let application = tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(RuntimeState::default())
        .setup(|app| {
            let mut app_settings = settings::load_or_default(app.handle());

            match app.autolaunch().is_enabled() {
                Ok(enabled) => app_settings.launch_at_startup = enabled,
                Err(error) => eprintln!("DESKFLOW_AUTOSTART_STATUS_FAILED code={error}"),
            }

            let (registered, warning) =
                hotkeys::register_with_fallback(app.handle(), &app_settings.global_hotkey);
            let (registered_emergency, emergency_warning) =
                hotkeys::register_emergency_with_fallback(
                    app.handle(),
                    &app_settings.emergency_hotkey,
                );
            let state = app.state::<RuntimeState>();
            state.replace_settings(app_settings);
            state.set_hotkey_status(registered, warning);
            state.set_emergency_hotkey_status(registered_emergency, emergency_warning);

            tray::install(app)?;
            highlight::initialize(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if window.label() == "settings" {
                    let _ = highlight::hide(window.app_handle());
                }
                if let Err(error) = window.hide() {
                    eprintln!("DESKFLOW_WINDOW_HIDE_FAILED code={error}");
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_settings,
            commands::get_runtime_status,
            commands::get_last_window_context,
            commands::get_last_ui_automation,
            commands::get_ai_provider_status,
            commands::save_ai_provider_credential,
            commands::delete_ai_provider_credential,
            commands::get_last_action_plan,
            commands::hide_window,
            commands::capture_active_window,
            commands::inspect_target_ui,
            commands::highlight_ui_element,
            commands::clear_target_highlight,
            commands::create_action_plan,
            commands::get_recording_status,
            commands::start_recording,
            commands::stop_recording,
            commands::execute_action_plan,
            commands::set_overlay_plan_mode,
            commands::update_app_settings,
            commands::emergency_stop,
            commands::cancel_execution,
            commands::get_diagnostic_logs,
            commands::clear_diagnostic_logs,
            commands::clear_local_cache,
        ])
        .run(tauri::generate_context!());

    if let Err(error) = application {
        eprintln!("DESKFLOW_FATAL_STARTUP_ERROR code={error}");
        std::process::exit(1);
    }
}
