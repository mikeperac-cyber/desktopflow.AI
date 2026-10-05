# DeskFlow AI

DeskFlow AI is a keyboard-first Windows workflow copilot. This repository currently contains **Phases 1–7**, through the verified recovery loop, plus a user-requested secure multi-provider credential slice from Phase 9. The Tauri v2 application can identify, capture, inspect, and visually highlight accessible controls in a foreground Windows application, ask one selected provider for a typed plan, run a user-confirmed low/medium-risk plan through a local Rust executor, verify every action, and recover from a changed interface with bounded fresh-state replanning.

The product is intentionally honest about its current boundary. It can execute only the allowlist (`focus`, `click`, `invoke`, `type_text`, `key_press`, `hotkey`, `scroll`, `select`, `toggle`, and `wait`) after a second explicit confirmation. Every action revalidates the captured HWND, process, live UIA fingerprint, hierarchy, state, and bounds, then checks a typed postcondition. A failure triggers a fresh capture/UIA inspection and at most two replans within the existing total action limit. High-risk execution and the global emergency stop remain Phase 8 work.

## Implemented capabilities

- Tauri v2 host with Rust services and React 19 + strict TypeScript frontend
- hidden-by-default 680×250 command overlay and separate settings window
- global `Alt + Space` registration with graceful `Win + Shift + A` fallback
- tray actions: Open DeskFlow, Settings, Pause automation, Quit
- close-to-tray lifecycle; only Quit terminates the process
- opt-in Windows startup registration (off by default)
- locally persisted settings under the OS app configuration directory
- light, dark, and system appearance modes
- typed, user-facing command errors and shortcut validation
- least-privilege Tauri capability file
- explicit foreground-window capture when the overlay opens
- process ID, executable identity, title, class, window handle, monitor, and DPI metadata
- physical-pixel and DPI-normalized logical bounds, including negative multi-monitor coordinates
- local in-memory PNG capture with guarded failure states for minimized, oversized, restricted, or protected targets
- Advanced settings diagnostics view with screenshot preview and captured metadata
- bounded Windows UI Automation Control View traversal for the exact captured window
- normalized temporary element/parent IDs, roles, physical bounds, states, and supported patterns
- searchable Advanced Developer Inspector with large-tree filtering and truncation reporting
- keyboard-selectable inspector rows and a non-focusable, click-through physical-pixel target frame
- provider-neutral native AI interface with allowlisted Gemini, OpenCode Zen, OpenCode Go, OpenRouter, NVIDIA NIM, OpenAI, and Anthropic adapters
- provider-specific fast/reasoning profiles and API dialects, including strict Responses, chat-completions, guided JSON, and Anthropic structured output
- per-provider Windows Credential Manager storage with environment-variable fallback and no secret material in settings, localStorage, logs, or Git
- strict structured action schema, current-target ID validation, password-name redaction, bounded payloads, and token usage reporting
- command-overlay and AI-settings plan inspection with two-stage execution confirmation and local step reports
- UIA-first executor with bounded input fallback, stale/ambiguous target rejection, and stop-on-first-failure behavior
- typed post-action verification, fresh-state recovery planning, a two-replan ceiling, and a total action budget across attempts
- ESLint, TypeScript, Vitest/Testing Library, and Rust adapter/unit-test gates

## Requirements

- Windows 10 x64 or Windows 11 x64
- Microsoft Edge WebView2 Runtime
- Microsoft C++ Build Tools with **Desktop development with C++**
- Rust stable MSVC toolchain
- Node.js 20 or newer and npm

The validated local environment used Node 24 and Rust 1.99. See the current Tauri [Windows prerequisites](https://v2.tauri.app/start/prerequisites/) before setting up a new machine.

## Download

Download the latest Windows installers from the [GitHub Releases page](https://github.com/mikeperac-cyber/desktopflow.AI/releases/latest). The NSIS setup executable is the recommended per-user installation; the MSI is provided for managed Windows deployment. Both packages target Windows 10/11 x64 and require the WebView2 Runtime.

These direct-download installers are currently unsigned development releases, so Windows SmartScreen may show a warning on first launch. A Microsoft Store release requires a Partner Center submission and Microsoft certification; it is not created by a GitHub upload alone.

## Develop

```powershell
npm install
npm run tauri dev
```

The app starts in the system tray. Press `Alt + Space` or left-click the tray icon to open the overlay. Right-click the tray icon for Settings, Pause automation, and Quit.

Open **Settings → AI**, choose a provider, enter its API key, and save the provider selection. Each key is stored under an isolated DeskFlow target in Windows Credential Manager. The password field is cleared immediately after the native save command and the key is never added to `settings.json`.

For development, native-process environment variables remain supported: `GOOGLE_API_KEY`/`GEMINI_API_KEY`, `OPENCODE_ZEN_API_KEY`, `OPENCODE_GO_API_KEY`, `OPENROUTER_API_KEY`, `NVIDIA_API_KEY`, `OPENAI_API_KEY`, and `ANTHROPIC_API_KEY`. A Credential Manager value takes precedence. `OPENCODE_API_KEY` is a shared fallback for either OpenCode service.

For browser-only UI work:

```powershell
npm run dev
```

Then open `http://127.0.0.1:1420/?view=overlay` or `?view=settings`. Browser mode uses localStorage only as a design/test adapter; the desktop app uses Rust-owned settings persistence.

## Verify

```powershell
npm run check
npm run build
npm run tauri build -- --no-bundle
```

`npm run check` runs ESLint, strict TypeScript, frontend tests, and Rust tests. Native lifecycle and hotkey behavior still require the Windows smoke checks in [Testing](docs/TESTING.md).

## Repository map

```text
src/
  components/        React overlay and settings surfaces
  lib/               browser-safe validation and theme helpers
  services/          typed Tauri command adapter
  types/             shared frontend contracts
  test/              test environment setup
src-tauri/src/
  context.rs         Windows foreground metadata, DPI geometry, and bounded GDI capture
  uia.rs             bounded read-only UI Automation traversal and normalization
  highlight.rs       validated physical geometry and highlight-window lifecycle
  ai.rs              allowlisted provider adapters, shared schemas, and plan validation
  credentials.rs     Windows Credential Manager read/write/delete boundary
  executor.rs        execution policy, live target revalidation, and allowlisted actions
  workflow.rs        bounded retry policy and multi-attempt execution reports
  commands.rs        narrow frontend-to-Rust command boundary
  error.rs           serializable user-facing errors
  hotkeys.rs         registration, normalization, fallback, rollback
  runtime.rs         synchronized in-memory runtime state
  settings.rs        validation and local persistence
  tray.rs            tray menu and tray events
  windows.rs         allowlisted window operations
docs/                 architecture, roadmap, testing, privacy, design
```

## Security and privacy

DeskFlow observes only the explicitly captured foreground target. Planning sends a filtered UI tree only after the user submits an instruction; screenshot transmission is separately off by default. Credentials are retrieved only by the Rust host from Windows Credential Manager or the native process environment. Plans plus verification rules are validated against the current in-memory element IDs before display. Execution uses only the current Rust-held plan, requires its provider request ID and explicit confirmation, blocks high-risk plans, and never accepts shell/script/binary commands. Recovery reuses the confirmed instruction only after a local fresh-state inspection and remains inside both retry and action limits. Read [Privacy](docs/PRIVACY.md) before extending context collection.

## Next milestone

Phase 8 adds risk classification refinements, granular approvals, and the global emergency stop without weakening the Phase 7 verification and recovery boundary.
