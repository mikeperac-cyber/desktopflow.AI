use std::time::Instant;

use deskflow_ai_lib::{context, runtime::RuntimeState, security::sanitize_sensitive_text, uia};

fn main() {
    println!("=== DeskFlow AI Performance & Latency Benchmark ===\n");

    // 1. Runtime initialization benchmark
    let t0 = Instant::now();
    let state = RuntimeState::default();
    let init_time = t0.elapsed();
    println!("1. RuntimeState initialization: {:?}", init_time);

    // 2. Sensitive text sanitization throughput
    let sample = "Employee SSN 123-45-6789 paid with card 4111 2222 3333 4444 using sk-proj-1234567890abcdef1234567890. ";
    let repeated = sample.repeat(500); // ~55 KB
    let t1 = Instant::now();
    let sanitized = sanitize_sensitive_text(&repeated);
    let sanitize_time = t1.elapsed();
    let throughput_mb_s = (repeated.len() as f64 / 1_048_576.0) / sanitize_time.as_secs_f64();
    println!(
        "2. Security sanitization: {:?} for {} KB ({:.2} MB/s throughput)",
        sanitize_time,
        repeated.len() / 1024,
        throughput_mb_s
    );
    assert!(sanitized.contains("[protected-ssn]"));

    // 3. Diagnostic log ring buffer rotation performance
    let mut settings = state.settings();
    settings.diagnostic_logging = true;
    state.replace_settings(settings);
    let t2 = Instant::now();
    for i in 0..1000 {
        state.log_diagnostic("info", "benchmark", &format!("Log entry {i}"));
    }
    let log_time = t2.elapsed();
    println!(
        "3. Diagnostic logging (1,000 pushes into 500-cap ring buffer): {:?} ({:?} per entry)",
        log_time,
        log_time / 1000
    );
    assert_eq!(state.diagnostic_logs().len(), 500);

    // 4. Memory cache purge timing
    let t3 = Instant::now();
    state.clear_cache();
    let purge_time = t3.elapsed();
    println!("4. Volatile cache purge: {:?}", purge_time);

    // 5. Context capture attempt (if interactive window is active)
    let t4 = Instant::now();
    match context::capture_foreground_context() {
        Ok(snapshot) => {
            let cap_time = t4.elapsed();
            println!(
                "5. Context capture: {:?} (window: '{}', size: {}x{})",
                cap_time,
                snapshot.title,
                snapshot.bounds_physical.width,
                snapshot.bounds_physical.height
            );

            let t5 = Instant::now();
            match uia::inspect_captured_window(&snapshot) {
                Ok(tree) => {
                    let uia_time = t5.elapsed();
                    println!(
                        "6. UIA tree traversal: {:?} ({} useful elements, visited {}, duration_ms reported: {})",
                        uia_time,
                        tree.elements.len(),
                        tree.visited_count,
                        tree.duration_ms
                    );
                }
                Err(err) => {
                    println!("6. UIA tree traversal: skipped ({})", err);
                }
            }
        }
        Err(err) => {
            println!(
                "5. Context capture: skipped in non-interactive environment ({})",
                err
            );
        }
    }

    println!("\n=== Benchmark completed successfully ===");
}
