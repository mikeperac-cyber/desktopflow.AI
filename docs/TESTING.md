# Testing

## Automated gate

Run before every milestone handoff:

```powershell
npm run check
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```

The frontend suite covers shortcut validation, plan rendering, two-stage execution confirmation, execution reports, Escape handling, settings edits, settings saves, provider configuration states, one-shot credential submission/field clearing, navigation, close behavior, context diagnostics rendering, normalized UI-tree inspection/filtering, and highlight selection/clearing. Rust tests additionally cover provider payload opt-in, password-name redaction, all allowlisted model profiles, Gemini Interactions, OpenAI Responses, OpenCode session headers, OpenRouter schema-capable routing, NVIDIA guided JSON, Anthropic structured output, Windows Credential Manager round trips, typed-plan validation, stale targets, password entry rejection, blocked plans, high-risk execution rejection, step limits, target identity, partial progress, and stop-on-first-failure behavior. Mock provider tests make no external or billable calls.

## Browser UI gate

Verify both routes with the development server:

- overlay at 680×250 and 390×250
- settings at 940×720 and 390×844
- no horizontal document overflow
- input is focused when the overlay opens
- Enter requests a plan; execution remains behind a separate confirmation and browser-only execution reports the desktop requirement
- system/light/dark selection persists in the browser adapter
- every settings navigation item changes the visible section
- Save is disabled when clean and after a successful save
- no error-level console messages

## Phase 2 browser diagnostics gate

- Advanced displays the empty local-capture state without overflow
- browser-only Capture reports that the desktop app is required
- mocked captured state renders the preview, process identity, DPI, physical/logical bounds, monitor metadata, and executable path
- no screenshot is stored in browser localStorage

## Windows native smoke gate

1. Start `npm run tauri dev` or the release executable.
2. Confirm no application window appears on startup and the tray icon exists.
3. Press `Alt + Space`; confirm the overlay appears centered and the input owns keyboard focus.
4. Press `Alt + Space` again; confirm the overlay hides.
5. Open the overlay and press `Esc`; confirm it hides while the process remains alive.
6. Open Settings from the tray, close it, and confirm the process remains alive.
7. Save `Win + Shift + A`; confirm the old binding is removed and the new binding opens the overlay.
8. Revert to `Alt + Space`.
9. Toggle Pause in the tray and confirm the check state changes.
10. Select Quit and confirm the process exits.

## Windows context smoke gate

1. Open a blank Notepad window and leave it unminimized in the foreground.
2. Press `Alt + Space`; confirm the overlay status names `Notepad.exe` (or the installed Notepad process name).
3. Close the overlay, open Settings from the tray, and select Advanced.
4. Click **Capture active application**; confirm Settings briefly hides and returns.
5. Confirm the title, process ID/path, window class, handle, DPI/scale, physical/logical bounds, monitor, and screenshot describe the same Notepad window.
6. Move Notepad to a monitor with a different scale or negative desktop coordinates and repeat; confirm physical bounds track the window and logical bounds scale by the reported DPI.
7. Minimize the target and repeat; confirm DeskFlow reports a capture error and does not reuse stale metadata.
8. Try a protected-content window if available; confirm a blocked capture is reported rather than replaced with another target.

For a developer-only core probe, build `cargo build --manifest-path src-tauri/Cargo.toml --example context_probe`, focus a safe test window during its two-second delay, and run the example with an output PNG path. The example is not part of the application bundle and should never be pointed at sensitive content.

Hotkey registration can legitimately fail when another application owns the binding. That state passes only if Settings displays the warning and the fallback binding is active.

## Phase 3 UI Automation smoke gate

1. Capture a blank Notepad, Paint, or another safe accessible Windows application using the context gate above.
2. In Advanced, click **Inspect captured UI**.
3. Confirm the target title/process still match the captured application and the inspector lists a root window plus useful descendant controls.
4. Confirm each displayed element has a temporary ID, role, parent indentation, available physical bounds/state details, and any supported patterns.
5. Filter by a visible control name, role, automation ID, class, or pattern and confirm only matching normalized rows remain.
6. Confirm clicking or focusing an inspector row performs no target action beyond the Phase 4 visual frame; execution begins in Phase 6.
7. Capture another application and confirm the previous tree disappears before inspecting the new target.
8. Inspect a large application and confirm truncation is reported rather than returning an unbounded tree.

For the read-only native core probe, build `cargo build --manifest-path src-tauri/Cargo.toml --example uia_probe`, focus a safe accessible application during its two-second delay, and run `src-tauri\target\debug\examples\uia_probe.exe`. It prints only target identity, counts, roles, hierarchy, and pattern names; it does not save the captured screenshot.

## Phase 4 highlight smoke gate

1. Complete the Phase 3 gate with a safe target positioned away from Settings.
2. Select a bounded inspector row; confirm the row becomes selected and a blue frame matches that control without focusing the highlight window.
3. Click through the frame on the target and confirm the target receives the pointer input. Do not activate a destructive target during this check.
4. Select another bounded row and confirm the frame moves without leaving the previous frame behind.
5. Select the same row again and confirm the frame disappears.
6. Repeat on every connected monitor, including one with a different Windows scale and one with a negative virtual-desktop origin when available.
7. Confirm the frame follows the UIA physical rectangle exactly; it must not apply the context snapshot's logical scale factor again.
8. Capture or inspect again, leave Advanced, close Settings, and open the command overlay; confirm each transition clears the frame.
9. Confirm rows without screen bounds are disabled and cannot request a highlight.

## AI provider and credential smoke gate

1. With no API key, confirm **Settings → AI** defaults to **Local (Free)** with status **Needs local model** and no password field.
2. Install Ollama, run `ollama pull qwen3:8b`, and start `ollama serve`. Reopen AI settings and confirm the local provider reports **Ready · on this PC**.
3. Submit a harmless instruction on a safe target and confirm a validated local plan arrives with a `local-*` request ID and no credential prompt.
4. Open **Settings → AI**, select a provider, enter its API key locally, and save the provider selection. Never paste a real key into chat, a repository file, browser localStorage, or test output.
2. Confirm the password field clears, the provider reports **Configured**, and Windows Credential Manager contains an isolated `DeskFlow AI/provider/<provider>` generic credential. The UI must never reveal the saved value.
3. Restart DeskFlow and confirm the provider remains configured. Remove the saved key and confirm the status becomes unconfigured unless a supported native environment variable exists.
4. Repeat credential save/remove for each intended provider without overwriting another provider's status.
5. Open a safe, non-sensitive target such as blank Notepad, then open DeskFlow so the target is captured.
6. Submit a harmless instruction. Confirm the overlay expands and labels the result **Validated plan · Awaiting confirmation**.
7. Confirm every target ID shown in the plan exists in the current Developer Inspector and that no target application state changes before the second confirmation.
8. In AI settings, switch between the fast and reasoning profiles and confirm the returned provider/model metadata matches the selection.
9. Keep screenshot transmission off and confirm plan metadata says **UI tree only**. Enable it only for a provider/profile that supports image input, save, and confirm metadata says **Screenshot included**.
10. Capture a new target and confirm the prior plan is cleared. Submit again and confirm stale IDs are never accepted.
11. For OpenCode Go, confirm the coding-traffic notice is visible and screenshot transmission is disabled. Use it only for workflows compatible with the provider's traffic policy.

Automated tests use synthetic observations and mocked provider responses; they make no billable external call. A live smoke for each configured service remains a credential-gated manual check.

## Phase 6–7 execution and recovery smoke gate

1. Build the deterministic tools with `cargo build --manifest-path src-tauri/Cargo.toml --example executor_target --example executor_probe`.
2. Start `src-tauri\target\debug\examples\executor_target.exe` and then run `src-tauri\target\debug\examples\executor_probe.exe`.
3. Confirm the probe prints `status=completed recovered=true replans=1 verified=1 attempted=2 planned=3 methods=uia_value_set,uia_invoke` and the target title becomes **DeskFlow Executor Test — Complete**. The probe deliberately renames and moves Continue after the initial snapshot.
4. In the product UI, generate a low-risk plan and confirm **Review and run plan** reveals a second confirmation without executing.
5. Confirming must hide DeskFlow, execute only the current Rust-held provider request, restore the originating surface, and display action plus verification methods in the report.
6. Move, disable, hide, duplicate, or close a target after planning and confirm execution stops without using the old coordinate.
7. Confirm a high-risk plan is displayed but cannot run. Confirm paused or concurrent execution requests are rejected.
8. Confirm a failed action or verification stops all later steps in that plan. A recovery must first recapture the same HWND, require the same PID, and rebuild the UIA tree.
9. Confirm recovered runs say **Recovered and verified**, identify the number of plan attempts, and never exceed two replans or the configured total autonomous action limit.
10. With a user-supplied API key, repeat a harmless changed-layout scenario and confirm the recovery provider request plans only remaining work. Remove the key during recovery and confirm DeskFlow returns a safe recovery failure rather than continuing.

The specific-HWND capture helper exists only for local developer probes and is not registered as a Tauri command. Production capture remains foreground-only.

## Phase 14 recorder smoke gate

1. Capture a safe target (blank Notepad) and inspect it in Advanced settings.
2. Press `Alt + Space`, click **Record**, and confirm the overlay hides.
3. Click a control, type a short word, press `Enter`, and scroll once — all inside the target.
4. Press `Alt + Space` and confirm the overlay shows **Recording · 4+ actions** with a live count.
5. Click **Stop** and confirm a validated recorded plan appears with `recorder-*` request ID, ready for the unchanged second confirmation.
6. Never type real secrets during recording. If the target has a password field, type a fake value there and confirm the stopped plan reports dropped sensitive keystrokes with no corresponding step.
7. Confirm clicking outside the target (e.g., the desktop) adds no steps.
8. Confirm replay requires the same explicit confirmation and supports emergency stop.

## Release build

```powershell
npm run tauri build
```

The release produces `src-tauri\target\release\bundle\nsis\DeskFlow AI_0.1.0_x64-setup.exe` and `src-tauri\target\release\bundle\msi\DeskFlow AI_0.1.0_x64_en-US.msi`. Verify both files exist, record SHA-256 hashes, and attach them to the matching GitHub release. MSI packaging also depends on the Windows VBSCRIPT optional feature documented by Tauri.
