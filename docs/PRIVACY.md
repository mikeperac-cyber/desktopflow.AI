# Privacy

DeskFlow can capture the foreground application's visible pixels and basic Windows metadata, inspect the captured window's accessible Control View, display a selected physical rectangle, and submit a minimized observation to the selected allowlisted AI provider after an explicit planning request. It does not collect clipboard contents or persist observations/plans.

Stored locally:

- appearance preference
- requested global and emergency shortcuts
- opt-in startup preference
- future automation/privacy toggles at their safe defaults
- selected AI provider/model profile and screenshot-transmission preference

The settings file is written to the operating system application configuration directory. It contains no credentials. Browser-only UI preview stores an equivalent settings object in localStorage for visual testing, but its credential commands are disabled; that adapter is not used by the Tauri application.

API keys are stored as isolated generic credentials named `DeskFlow AI/provider/<provider>` in Windows Credential Manager. A key entered in AI settings exists transiently in the masked React field and one typed Tauri command, is written immediately by Rust, and is then cleared from React state. Saved secret values are never returned to React. Native-process environment variables remain supported as a development fallback, with Credential Manager taking precedence.

Held only in process memory for the latest capture:

- window title, class, handle, process ID, executable name/path when Windows permits access
- physical and DPI-normalized logical bounds
- monitor identity, bounds, work area, DPI, and scale factor
- one PNG screenshot of the active window
- one filtered UI Automation snapshot: accessible names, roles, IDs/classes/frameworks, physical bounds, state flags, hierarchy, and supported-pattern names

DeskFlow does not read Value or Text pattern contents in Phase 3. Accessible names can still contain visible labels or text exposed by the target application. Password controls are labeled as protected in the inspector. Capturing a new foreground window clears the previous accessibility snapshot, and UIA traversal is bounded and filtered before the result reaches the webview.

Phase 4 sends only a temporary element ID from the inspector back to Rust. The native host resolves its physical rectangle from the current in-memory snapshot and positions a click-through visual frame. Highlight selection and geometry are not persisted or transmitted.

Planning always sends the trusted user instruction plus up to 200 filtered, on-screen UI elements. UI labels are explicitly marked as untrusted observed content, password control names are replaced with `[protected]`, and Value/Text pattern contents remain excluded. Screenshot transmission is off by default and is included only after the user enables it for a compatible provider. OpenCode Go is currently UI-tree-only in DeskFlow.

Gemini, OpenAI, and OpenCode Responses requests set `store=false`. Other services use their documented request and retention controls. All transmitted observations are processed under the selected provider account and that provider's applicable privacy, retention, and billing terms. Switching providers changes the external data processor; it does not weaken DeskFlow's local schema validation or execution policy.

The diagnostics action hides DeskFlow before observing the restored foreground target. Same-process, minimized, invalid, and oversized captures are rejected. A protected application may cause Windows to reject the capture; DeskFlow reports that failure rather than substituting another window.

The executor revalidates the target and applies local policy after plan validation; model output remains untrusted regardless of provider. Provider responses cannot call Windows APIs, access credentials, or bypass the typed action vocabulary.
