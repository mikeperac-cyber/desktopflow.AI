use std::{thread, time::Duration};

use deskflow_ai_lib::{context, uia};

fn main() {
    thread::sleep(Duration::from_secs(2));

    let context = context::capture_foreground_context().expect("foreground context capture failed");
    let tree = uia::inspect_captured_window(&context).expect("UI Automation inspection failed");

    println!(
        "target={} process={} pid={}",
        tree.target.title,
        tree.target.process_name.as_deref().unwrap_or("unknown"),
        tree.target.process_id
    );
    println!(
        "elements={} visited={} filtered={} truncated={} duration_ms={}",
        tree.elements.len(),
        tree.visited_count,
        tree.filtered_count,
        tree.truncated,
        tree.duration_ms
    );
    for element in tree.elements.iter().take(12) {
        println!(
            "{} parent={} depth={} role={} patterns={}",
            element.id,
            element.parent_id.as_deref().unwrap_or("root"),
            element.depth,
            element.role,
            element.supported_patterns.join(",")
        );
    }
    for warning in tree.warnings {
        println!("warning={warning}");
    }
}
