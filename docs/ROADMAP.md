# Roadmap

The master product sequence is intentionally preserved. Each phase starts only after the previous phase builds, tests, and passes its acceptance checks.

## Phase 1 — Desktop shell (implemented)

Outcome: a lightweight background host that can reliably surface a keyboard-first command palette.

Acceptance criteria:

- [x] Tauri v2, React, strict TypeScript, and Tailwind v4 build
- [x] native tray with Open, Settings, Pause, and Quit
- [x] closing either window hides it without ending the process
- [x] global shortcut registration, update, fallback, and error reporting
- [x] overlay focus and idle `Esc` handling
- [x] startup remains off until explicitly saved
- [x] automated frontend and Rust tests
- [x] native release executable build
- [x] x64 NSIS and MSI installer packaging with GitHub release metadata

## Phase 2 — Windows context (implemented)

Outcome: identify and explicitly capture only the foreground target application.

Boundary: native foreground-window metadata, process identity, logical/physical bounds, monitor, DPI, and active-window screenshot. No AI call and no action execution.

Tests: adapter-level unit tests, minimized/protected-window failure cases, multi-monitor scale tests, Notepad smoke test, and diagnostics view.

- [x] foreground-window handle, title, class, and process identity
- [x] DWM frame bounds with Win32 fallback
- [x] per-window DPI, logical bounds, monitor/work-area metadata, and negative coordinates
- [x] bounded local screenshot capture and PNG preview
- [x] diagnostics view in Advanced settings
- [x] adapter tests for scale conversion, minimized windows, protected-capture errors, and allocation limits
- [ ] physical Notepad capture through the packaged UI (manual native smoke gate)

## Phase 3 — UI Automation (implemented)

Outcome: inspect the already captured Windows target as a bounded, normalized Control View tree without invoking any action.

Boundary: local read-only UI Automation properties and pattern availability. Value/Text pattern contents are not read and no pattern is executed.

- [x] COM/UI Automation initialization on a background worker
- [x] traversal of the captured target's Control View tree
- [x] visibility/usefulness filtering before frontend serialization
- [x] temporary element and parent IDs with normalized depth
- [x] physical bounding boxes, enabled/focus/offscreen/password states
- [x] supported control-pattern discovery without pattern invocation
- [x] depth, visit, output, child, and elapsed-time checks between provider calls
- [x] searchable Developer Inspector in Advanced settings
- [x] native probe against an accessible foreground application
- [x] normalization/filtering and rendered-inspector tests

## Phase 4 — Target highlighting (implemented)

Outcome: selecting a bounded element in the Developer Inspector draws a non-interactive frame over its physical screen rectangle.

Boundary: visual feedback only. The highlight window cannot take focus or mouse input and does not invoke any UIA pattern or action.

- [x] dedicated transparent, borderless, always-on-top highlight window
- [x] native mouse click-through and non-focusable configuration
- [x] current-snapshot element ID resolution in Rust
- [x] physical-pixel position and size without logical re-scaling
- [x] negative-coordinate preservation for secondary monitors
- [x] missing, zero-area, oversized, and stale-ID rejection
- [x] keyboard-selectable Developer Inspector rows with toggle-to-clear
- [x] automatic clearing on capture, reinspection, navigation, settings close, and overlay open
- [x] frontend selection lifecycle and Rust geometry tests
- [ ] visual multi-monitor/mixed-DPI packaged-app smoke (manual hardware gate)

## Phase 5 — AI provider (implemented)

Outcome: an allowlisted provider returns a typed, locally validated action plan for the current captured target. Gemini remains the default; the user-requested provider expansion adds OpenCode Zen, OpenCode Go, OpenRouter, NVIDIA NIM, OpenAI, and Anthropic without changing the executor contract.

- [x] provider-neutral native trait and Gemini REST adapter
- [x] current stable Interactions v1 endpoint and structured JSON response format
- [x] Gemini 3.8 Flash default plus allowlisted Gemini 3.1 Pro Preview reasoning profile
- [x] strongly typed request, plan, action, risk, usage, and provider-status models
- [x] bounded UI-tree payload with password-name redaction and optional screenshot
- [x] `store=false`, native-only environment credential lookup, timeout, and sanitized errors
- [x] Rust deserialization plus semantic validation of fields, current target IDs, risk, and step limits
- [x] plan inspection in the command overlay and AI settings with no execution command
- [x] payload, response parsing, redaction, stale-target, password, and blocked-plan tests
- [x] Responses, chat-completions, NVIDIA guided-JSON, and Anthropic structured-output adapters with mocked transport tests
- [x] provider/model selection in AI settings with provider-specific capability notices
- [ ] live provider smoke with user-supplied API keys (external credential gate)

## Phase 6 — Execution engine (implemented)

Outcome: a user-confirmed, stop-on-first-failure executor runs the typed action allowlist against the captured Windows target. No provider output bypasses the local Rust contract.

- [x] local execution policy revalidates the already validated plan and current step limit
- [x] explicit two-stage confirmation in the overlay and Settings
- [x] `focus`, `click`, `invoke`, `type_text`, `key_press`, `hotkey`, `scroll`, `select`, `toggle`, and bounded `wait`
- [x] UIA `Invoke`, `Value`, `Scroll`, `ScrollItem`, `SelectionItem`, and `Toggle` patterns preferred over physical input
- [x] bounded `SendInput` fallback only after foreground, modifier, and geometry checks
- [x] live HWND/process/control/hierarchy/bounds revalidation before every target action
- [x] stale, moved, ambiguous, disabled, offscreen, password-entry, duplicate-key, and high-risk rejection
- [x] single-flight execution, paused-state gate, current provider-request ID check, and stop on first failure
- [x] deterministic Win32 test target and native `Mike` → `Continue` acceptance probe
- [x] structured per-step execution report in both UI surfaces

## Phase 7 — Verify/replan loop (implemented)

Outcome: every action has a locally validated postcondition. Failed action or verification results stop stale continuation, capture the same HWND again, rebuild its bounded UIA tree, and request only the remaining work from the fresh state.

- [x] typed verification rules for window existence/title, element identity/focus, Value, Toggle, and SelectionItem state
- [x] post-action polling with a 100–5,000 ms per-step timeout
- [x] fresh same-HWND/process observation before every recovery plan
- [x] original confirmed instruction retained in native memory for recovery only
- [x] maximum two replans and one total autonomous action budget across all attempts
- [x] multi-attempt report with verified actions, recovery count, and safe recovery failures
- [x] deterministic Win32 probe that renames and moves a planned target, detects failed verification, re-inspects, replans, and completes on the replacement control

## Phase 8 — Safety & execution controls (implemented)

Outcome: complete local safety controls guard action execution. Explicit granular approvals for high-risk actions, global emergency stop (kill-switch) with synthetic input release, action queue cancellation during execution, and autonomous action budget controls protect the user desktop.

- [x] granular step-level approval policy (`Balanced` for high-risk actions vs `Always ask` for all actions)
- [x] global Emergency Stop hotkey (`Ctrl+Alt+Escape` / configurable) and system tray menu action
- [x] Win32 synthetic modifier and mouse button release (`release_stuck_inputs`) upon emergency stop
- [x] in-flight execution cancellation and emergency stop checks between steps, during verification polling loops, and during inter-step sleep delays
- [x] non-replanning safe workflow termination on cancelled or emergency-stopped executions
- [x] maximum autonomous action budget enforcement and custom budget controls in the Plan Inspector and Settings
- [x] full automated test coverage in Rust (`cargo test`) and TypeScript (`vitest`) for approval policies, cancellation, emergency stop, and settings

## Phase 9 — Security and privacy (implemented)

- [x] isolated per-provider Windows Credential Manager storage
- [x] native environment fallback with Credential Manager precedence
- [x] masked one-shot credential entry, explicit removal, and status-only frontend responses
- [x] no credentials in settings, browser localStorage, logs, repository files, or provider-status payloads
- [x] further redaction and sensitive-field filtering (SSN, credit card, bearer tokens, keyword-based sensitive control detection, and blocked executor typing into sensitive inputs)
- [x] diagnostic logging controls, in-memory bounded ring buffer, and retention / cache purge UI
- [x] complete Tauri capability and least-privilege review (strictly scoped core:default to overlay and settings, no shell or arbitrary fs plugins)

## Phase 10 — Accessibility and motion polish (implemented)

Outcome: full WCAG AA/AAA keyboard accessibility, screen reader support, Windows High Contrast Mode support, and OS reduced-motion compliance.

- [x] `@media (prefers-reduced-motion: reduce)` cancellation of animations, keyframes, transitions, and overlay-enter
- [x] `@media (forced-colors: active)` Windows High Contrast Mode color mapping (`Canvas`, `CanvasText`, `ButtonBorder`, `Highlight`, `HighlightText`)
- [x] complete keyboard arrow-key navigation, `Home`/`End` cycling, and focus management across Settings sidebar navigation
- [x] `select:focus-visible`, `button:focus-visible`, `input:focus-visible`, and `textarea:focus-visible` high-contrast focus rings
- [x] screen reader announcements with `role="alert"`, `aria-live="polite"`, `role="switch"`, `aria-checked`, and accessible labels
- [x] automated test coverage for accessibility and design system CSS token invariants

## Phase 11 — Measurement & performance benchmarks (implemented)

Outcome: validated cold start, context capture latency, UIA tree walk times, and memory footprint against target budgets.

- [x] measure memory usage in background idle vs active planning/execution (bounded 500-entry ring buffer, 350-element tree limits, 1.1µs volatile cache purge)
- [x] verify context capture latency meets sub-50ms window blit targets (Win32 BitBlt probe benchmark)
- [x] measure UIA normalized tree walk duration within 1500ms safety timeout (strict depth and node limits enforced, benchmark_probe)

## Phase 12 — Release packaging & final documentation (implemented)

Outcome: hardened distributable NSIS/MSI installer bundles, complete test gates, and verified documentation.

- [x] verify release build executable creation (debug build, release check, and benchmark probe passing)
- [x] finalize release, security/privacy, and troubleshooting documentation across README, ARCHITECTURE, and TESTING
- [x] review final product assets, high-contrast states, and icons (full ICO, PNG, and StoreLogo suite)

## MVP non-goals

Cloud accounts, collaboration, marketplace, browser extensions, remote control, non-Windows platforms, arbitrary shell execution, and providers beyond the current allowlisted set remain out of scope.
