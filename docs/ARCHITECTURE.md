# Architecture

## Current boundary

DeskFlow is split into a trusted local host and a webview UI. The React frontend may request a small allowlisted set of operations; Rust owns OS integration, validates settings, and controls persistent state.

```text
Keyboard / tray
      |
      v
Rust desktop host -----> overlay + settings windows
      |                         |
      |                         v
      +<---- typed Tauri commands ---- React UI
      |
      +---- local settings file
      +---- Windows autostart registration (opt-in)
      +---- foreground context adapter
              |-- process + window metadata
              |-- monitor + DPI geometry
              +-- bounded in-memory PNG capture
      +---- Windows UI Automation inspector
              |-- captured HWND only
              |-- bounded Control View traversal
              +-- filtered normalized element tree
      +---- target highlight window
              |-- current snapshot IDs only
              |-- physical pixel geometry
              +-- non-focusable + mouse click-through
      +---- AI provider interface
              |-- minimized labeled observation
              |-- seven allowlisted native adapters
              |-- structured action schema
              +-- Rust semantic validation
      +---- credential boundary
              |-- isolated Windows Credential Manager targets
              |-- native environment fallback
              +-- status only returned to React
      +---- deterministic executor
              |-- explicit confirmation + current plan identity
              |-- HWND/process/control fingerprint revalidation
              |-- UIA patterns before bounded SendInput fallback
              |-- typed post-action verification
              +-- stop-before-stale-continuation report
      +---- bounded recovery controller
              |-- fresh same-HWND capture + UIA inspection
              |-- remaining-work replanning from current state
              +-- two-replan and total-action ceilings
```

The filtered UI tree is transmitted only on an explicit planning request. Screenshot transmission is a separate saved opt-in and remains off by default. Captured pixels, metadata, accessibility properties, model output, and highlight geometry are observations, never local policy. A plan remains inert until the user opens the second confirmation state; the command then references only the current Rust-held plan by provider request ID.

## Native modules

- `lib.rs` assembles plugins, state, startup initialization, close interception, and commands.
- `context.rs` owns the mockable foreground-window adapter, process metadata, physical/logical geometry, monitor context, guarded GDI screenshot capture, PNG encoding, and data contracts.
- `uia.rs` owns read-only COM initialization, captured-HWND lookup, bounded Control View traversal, normalization, filtering, temporary IDs, control-state geometry, and supported-pattern discovery.
- `highlight.rs` resolves only current-snapshot element IDs, validates highlight bounds, and owns physical positioning plus click-through lifecycle for the native highlight window.
- `ai.rs` owns the provider interface, provider/model allowlists, Gemini Interactions, OpenAI/OpenCode Responses, OpenRouter/OpenCode chat-completions, NVIDIA guided-JSON, and Anthropic Messages request/response contracts, payload minimization, typed verification schema, recovery context, response parsing, and authoritative semantic plan validation.
- `credentials.rs` owns bounded per-provider Windows generic credentials. It exposes save/read/delete/existence operations to Rust only; React receives configured/source status but never a saved secret.
- `executor.rs` owns execution policy, the action allowlist, live UIA fingerprint resolution, pattern-first mutations, validated input fallback, step sequencing, post-action observation, verification polling, and structured reports.
- `workflow.rs` owns the two-replan ceiling, attempt aggregation, recovery classification, and safe recovery-failure reporting.
- `runtime.rs` owns synchronized settings, registered-hotkey status, warnings, paused/executing state, current observations, the confirmed planning request, and the latest validated plan. Poisoned mutexes are recovered explicitly to preserve tray control.
- `hotkeys.rs` normalizes user-facing Windows shortcut names, registers callbacks, falls back safely at startup, and restores the previous binding if an update fails.
- `tray.rs` creates the native menu. Left-click opens the overlay; right-click exposes the complete menu.
- `windows.rs` exposes only the two known window labels. Arbitrary labels from the frontend are rejected.
- `settings.rs` owns defaults, validation, OS-directory resolution, and serialization.
- `commands.rs` is the narrow IPC boundary. Hotkey and autostart changes are rolled back when later persistence steps fail.
- `error.rs` maps internal failure categories to structured `{ code, message }` responses without stack traces.

## Windows

- `overlay`: transparent, undecorated, always on top, skipped from the taskbar, 680×250 logical pixels at rest and 680×620 while inspecting a plan, hidden at startup.
- `settings`: undecorated settings surface, resizable with an 820×640 minimum, hidden at startup.
- `highlight`: transparent, borderless, non-focusable, click-through, always on top, skipped from the taskbar, and hidden until a bounded inspector element is selected.

Close requests are prevented and converted to `hide()`. The explicit tray Quit action calls the Tauri exit lifecycle.

When the overlay opens, Rust snapshots the foreground target before DeskFlow takes focus. The Advanced diagnostics action hides Settings for 350 ms, captures the window Windows restores to the foreground, then restores Settings. Same-process, minimized, invalid, and oversized targets are rejected. Process-path access may degrade to a PID-only observation without failing an otherwise valid capture.

`DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)` supplies physical bounds with `GetWindowRect` as a fallback. `GetDpiForWindow` supplies the scale used for logical bounds, and monitor rectangles retain negative coordinates. Pixel capture uses a screen-compatible GDI bitmap with `CAPTUREBLT`, a 40-million-pixel allocation limit, scoped handle cleanup, and local PNG encoding.

The UI Automation inspector always starts from the numeric HWND retained in the latest in-memory context snapshot and verifies that the UIA root still belongs to the captured process. It does not rediscover or silently switch targets. The traversal uses Control View and stops at 12 levels, 1,500 visited nodes, 350 normalized nodes, 250 children per container, or a 1.5-second elapsed check between provider calls. Structural, empty, zero-area, and offscreen nodes are filtered unless their role, name, focus, or keyboard behavior makes them useful. Temporary IDs (`uia-0001`, and so on) are valid only for the current snapshot. Inspection queries pattern availability without invoking it; only the confirmed native executor may invoke an allowlisted pattern after revalidation.

Value and Text pattern contents are deliberately excluded. The frontend receives UIA accessible names (which can reflect visible labels or text supplied by the target), roles, automation/class/framework IDs, physical bounds, state flags, and supported-pattern names for the searchable Developer Inspector. Capturing a new target clears the old UI tree so stale element identities cannot cross target boundaries.

Highlight requests contain only a temporary element ID. Rust resolves that ID from the current in-memory UIA snapshot and rejects missing or unsafe geometry. The highlight window receives `PhysicalPosition<i32>` and `PhysicalSize<u32>` directly, preserving negative virtual-desktop origins and avoiding a second DPI conversion. It is non-focusable and has cursor events disabled at both startup and display time. Highlighting is cleared before any new context capture or UIA inspection and whenever Settings closes or leaves Advanced.

All provider adapters use a 45-second timeout, at most 200 normalized on-screen elements, password-name replacement with `[protected]`, and the same typed action/verification schema. Screenshot transmission remains a saved opt-in, is rejected above 12 MiB, and is disabled for the OpenCode Go profile currently exposed by DeskFlow. Gemini, OpenAI, and OpenCode Responses requests set `store=false`; other adapters use the provider's documented stateless request shape and applicable retention policy. OpenCode Go sends a stable `x-opencode-session` value derived from the captured process/window, as required for compatible coding-agent traffic. Recovery requests include only bounded failure classification, verified step IDs, retry counters, the original confirmed instruction, and a fresh observation. The prompt explicitly requires remaining work rather than replay.

Rust deserializes with unknown fields denied, caps plans at 12 steps, checks action-specific fields, rejects unknown/disabled target IDs and password entry, verifies aggregate risk, validates every verification target/value/pattern and its 100–5,000 ms timeout, and rejects steps attached to unsupported or clarification responses. Provider schema compliance is never accepted as sufficient by itself. A locally entered key crosses the webview boundary once in a password-field command, is written immediately to its isolated Windows Credential Manager target, and is cleared from React state; it is never returned to React, serialized into settings, or logged. Native environment variables remain an optional fallback.

Execution is single-flight and unavailable while paused. It reruns semantic plan validation, enforces the current settings action limit across all attempts, blocks all high-risk plans until Phase 8, hides DeskFlow, and verifies the captured HWND and process. Each target is resolved against a fresh bounded Control View traversal using automation ID, accessible name, role, class, framework, nearest normalized parent, and geometry. No action uses a stale coordinate. `click` prefers Invoke; text prefers Value; select/toggle/scroll require their UIA patterns when available. Mouse, keyboard, and Unicode input are fallback paths guarded by foreground ownership, released modifier state, current window bounds, and `SendInput` completion counts.

After each mutation, the executor polls its typed postcondition and marks the step complete only after observation succeeds. A failed action or postcondition stops the current plan before any later action. The command layer may then capture the same numeric HWND, require the same process ID, rebuild the UIA snapshot, and ask the original selected provider for a new remaining-work plan. Recovery is capped at two replans; each attempted action consumes the original settings budget, including an action whose postcondition failed. Re-inspection, provider, policy, worker, and budget failures become an explicit safe-stop report rather than silently resuming stale steps.

## Security evolution

Phases 1–12 implement the complete pipeline through verified action, bounded fresh-state recovery, granular step-level approval policy, emergency stop input release, sensitive field & keyword redaction, memory-bounded diagnostic logging, and accessible high-contrast UI polish. The runtime enforces these gates:

```text
User instruction
  -> minimum sufficient observation
  -> untrusted model plan
  -> Rust schema validation
  -> local risk policy
  -> user approval when required
  -> target revalidation
  -> deterministic action
  -> fresh observation and verification
  -> bounded fresh-state replan when required
```

Model output must never reach a shell, script host, registry API, or arbitrary executable path. UI content must be labeled as observed, untrusted content in every provider request.
