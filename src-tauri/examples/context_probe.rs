use std::{env, fs, path::PathBuf, thread, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use deskflow_ai_lib::context::capture_foreground_context;
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("context-probe.png"));

    thread::sleep(Duration::from_secs(2));
    let snapshot = capture_foreground_context()?;
    let (_, encoded) = snapshot
        .screenshot
        .data_url
        .split_once(',')
        .ok_or("screenshot data URL is invalid")?;
    fs::write(&output, STANDARD.decode(encoded)?)?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "output": output,
            "title": snapshot.title,
            "process": snapshot.process,
            "bounds_physical": snapshot.bounds_physical,
            "bounds_logical": snapshot.bounds_logical,
            "dpi": snapshot.dpi,
            "scale_factor": snapshot.scale_factor,
            "monitor": snapshot.monitor,
            "screenshot": {
                "width_px": snapshot.screenshot.width_px,
                "height_px": snapshot.screenshot.height_px,
                "byte_size": snapshot.screenshot.byte_size,
                "capture_method": snapshot.screenshot.capture_method,
            },
            "warnings": snapshot.warnings,
        }))?
    );
    Ok(())
}
