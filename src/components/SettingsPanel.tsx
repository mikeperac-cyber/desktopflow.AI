import { type FormEvent, type KeyboardEvent, useEffect, useMemo, useState } from "react";

import { Icon } from "./Icon";
import { AiSettings } from "./AiSettings";
import { MemoryManager } from "./MemoryManager";
import { SchedulesSection } from "./SchedulesSection";
import { ContextInspector } from "./ContextInspector";
import { WindowChrome } from "./WindowChrome";
import { applyTheme, validateShortcut } from "../lib/theme";
import {
  captureActiveWindow,
  clearDiagnosticLogs,
  clearLocalCache,
  clearTargetHighlight,
  getDiagnosticLogs,
  hideWindow,
  highlightUiElement,
  inspectTargetUi,
  loadLastUiAutomation,
  loadLastWindowContext,
  loadRuntimeStatus,
  loadSettings,
  saveSettings,
  toUserMessage,
} from "../services/desktop";
import {
  DEFAULT_RUNTIME_STATUS,
  DEFAULT_SETTINGS,
  type AppSettings,
  type ApprovalPolicy,
  type DiagnosticLogEntry,
  type RuntimeStatus,
  type ThemePreference,
  type UiAutomationSnapshot,
  type WindowContextSnapshot,
} from "../types/settings";

type SectionId = "general" | "ai" | "schedules" | "automation" | "privacy" | "advanced";

const sections: Array<{ id: SectionId; label: string; icon: Parameters<typeof Icon>[0]["name"] }> = [
  { id: "general", label: "General", icon: "general" },
  { id: "ai", label: "AI", icon: "ai" },
  { id: "schedules", label: "Schedules", icon: "clock" },
  { id: "automation", label: "Automation", icon: "automation" },
  { id: "privacy", label: "Privacy", icon: "privacy" },
  { id: "advanced", label: "Advanced", icon: "sliders" },
];

function Toggle({ checked, label, onChange }: { checked: boolean; label: string; onChange: (checked: boolean) => void }) {
  return (
    <button
      aria-checked={checked}
      aria-label={label}
      className={`toggle ${checked ? "toggle--on" : ""}`}
      onClick={() => onChange(!checked)}
      role="switch"
      type="button"
    >
      <span />
    </button>
  );
}

function GeneralSettings({
  draft,
  onChange,
  shortcutError,
}: {
  draft: AppSettings;
  onChange: (patch: Partial<AppSettings>) => void;
  shortcutError: string | null;
}) {
  const themes: Array<{ value: ThemePreference; label: string }> = [
    { value: "system", label: "System" },
    { value: "light", label: "Light" },
    { value: "dark", label: "Dark" },
  ];

  return (
    <div className="settings-content-section">
      <section className="settings-group" aria-labelledby="appearance-heading">
        <h2 id="appearance-heading">Appearance</h2>
        <div className="theme-options" role="radiogroup" aria-label="Theme">
          {themes.map((theme) => (
            <label key={theme.value} className="radio-option">
              <input
                checked={draft.theme === theme.value}
                name="theme"
                onChange={() => {
                  onChange({ theme: theme.value });
                  applyTheme(theme.value);
                }}
                type="radio"
              />
              <span className="radio-control" />
              {theme.label}
            </label>
          ))}
        </div>
      </section>

      <section className="settings-group" aria-labelledby="keyboard-heading">
        <h2 id="keyboard-heading">Keyboard shortcuts</h2>
        <div className="setting-row setting-row--input">
          <div>
            <label htmlFor="global-hotkey">Open DeskFlow</label>
            <p>Use Alt + Space or another Windows shortcut.</p>
          </div>
          <input
            aria-invalid={Boolean(shortcutError)}
            id="global-hotkey"
            onChange={(event) => onChange({ global_hotkey: event.currentTarget.value })}
            value={draft.global_hotkey}
          />
        </div>
        {shortcutError ? <p className="field-error">{shortcutError}</p> : null}
        <div className="setting-row setting-row--input">
          <div>
            <label htmlFor="emergency-hotkey">Emergency stop</label>
            <p>Becomes global while an automation is running.</p>
          </div>
          <input
            id="emergency-hotkey"
            onChange={(event) => onChange({ emergency_hotkey: event.currentTarget.value })}
            value={draft.emergency_hotkey}
          />
        </div>
      </section>

      <section className="settings-group" aria-labelledby="startup-heading">
        <h2 id="startup-heading">Startup</h2>
        <div className="setting-row">
          <div>
            <span className="setting-label">Launch DeskFlow when Windows starts</span>
            <p>Off by default. This changes only after you save.</p>
          </div>
          <Toggle
            checked={draft.launch_at_startup}
            label="Launch DeskFlow when Windows starts"
            onChange={(checked) => onChange({ launch_at_startup: checked })}
          />
        </div>
      </section>
    </div>
  );
}

function AutomationSettings({
  draft,
  onChange,
}: {
  draft: AppSettings;
  onChange: (patch: Partial<AppSettings>) => void;
}) {
  const policies: Array<{ value: ApprovalPolicy; label: string; description: string }> = [
    {
      value: "balanced",
      label: "Balanced",
      description: "Require explicit approval only for high-risk or destructive actions.",
    },
    {
      value: "always_ask",
      label: "Always ask",
      description: "Require explicit user approval for every individual step before execution.",
    },
  ];

  return (
    <div className="settings-content-section">
      <section className="settings-group" aria-labelledby="approval-policy-heading">
        <h2 id="approval-policy-heading">Approval policy</h2>
        <div className="approval-policy-options" role="radiogroup" aria-label="Approval policy">
          {policies.map((policy) => (
            <label key={policy.value} className="radio-option">
              <input
                checked={draft.approval_policy === policy.value}
                name="approval_policy"
                onChange={() => onChange({ approval_policy: policy.value })}
                type="radio"
              />
              <span className="radio-control" />
              <div className="approval-policy-copy">
                <strong>{policy.label}</strong>
                <p>{policy.description}</p>
              </div>
            </label>
          ))}
        </div>
      </section>

      <section className="settings-group" aria-labelledby="automation-safety-heading">
        <h2 id="automation-safety-heading">Execution safety & limits</h2>
        <div className="setting-row setting-row--input">
          <div>
            <label htmlFor="max-autonomous-steps">Maximum autonomous actions</label>
            <p>Maximum total steps permitted per plan across all recovery attempts (1–100).</p>
          </div>
          <input
            id="max-autonomous-steps"
            max={100}
            min={1}
            onChange={(event) => {
              const raw = event.currentTarget.value;
              if (raw === "") {
                onChange({ maximum_autonomous_steps: 0 });
                return;
              }
              const value = Number.parseInt(raw, 10);
              if (!Number.isNaN(value)) {
                onChange({ maximum_autonomous_steps: Math.min(100, Math.max(0, value)) });
              }
            }}
            type="number"
            value={draft.maximum_autonomous_steps || ""}
          />
        </div>

        <div className="setting-row setting-row--input">
          <div>
            <label htmlFor="execution-delay-ms">Execution delay</label>
            <p>Pause between simulated inputs to allow Windows UI to settle (50–2000 ms).</p>
          </div>
          <input
            id="execution-delay-ms"
            max={2000}
            min={50}
            step={50}
            onChange={(event) => {
              const raw = event.currentTarget.value;
              if (raw === "") {
                onChange({ execution_delay_ms: 0 });
                return;
              }
              const value = Number.parseInt(raw, 10);
              if (!Number.isNaN(value)) {
                onChange({ execution_delay_ms: Math.min(2000, Math.max(0, value)) });
              }
            }}
            type="number"
            value={draft.execution_delay_ms || ""}
          />
        </div>

        <div className="setting-row">
          <div>
            <span className="setting-label">Highlight target controls</span>
            <p>Draw a non-interactive visual indicator around targeted elements during execution.</p>
          </div>
          <Toggle
            checked={draft.highlight_targets}
            label="Highlight target controls"
            onChange={(checked) => onChange({ highlight_targets: checked })}
          />
        </div>
      </section>
    </div>
  );
}

function PrivacySettings({
  draft,
  onChange,
  onCacheCleared,
}: {
  draft: AppSettings;
  onChange: (patch: Partial<AppSettings>) => void;
  onCacheCleared?: () => void;
}) {
  const [logs, setLogs] = useState<DiagnosticLogEntry[]>([]);
  const [loadingLogs, setLoadingLogs] = useState(false);
  const [actionMessage, setActionMessage] = useState<string | null>(null);

  const handleRefreshLogs = async () => {
    setLoadingLogs(true);
    try {
      const entries = await getDiagnosticLogs();
      setLogs(entries);
    } finally {
      setLoadingLogs(false);
    }
  };

  const handleClearLogs = async () => {
    await clearDiagnosticLogs();
    setLogs([]);
    setActionMessage("Diagnostic logs cleared.");
  };

  const handleClearCache = async () => {
    await clearLocalCache();
    onCacheCleared?.();
    setActionMessage("In-memory window context and UI trees cleared.");
  };

  return (
    <div className="settings-content-section">
      <MemoryManager />
      <section className="settings-group" aria-labelledby="privacy-storage-heading">
        <h2 id="privacy-storage-heading">Credential & context security</h2>
        <div className="privacy-feature-list">
          <div className="privacy-feature-card">
            <strong>Windows Credential Manager</strong>
            <p>API keys are isolated in encrypted platform storage under per-provider targets. Never written to settings files, webview storage, or repository code.</p>
          </div>
          <div className="privacy-feature-card">
            <strong>Volatile in-memory context</strong>
            <p>Foreground window screenshots and UI Automation trees are held in volatile native memory only and discarded when windows change.</p>
          </div>
          <div className="privacy-feature-card">
            <strong>Sensitive field redaction</strong>
            <p>Passwords, credit card numbers, SSNs, and API keys are automatically detected and replaced with [protected] placeholders before prompts leave your machine.</p>
          </div>
        </div>

        <div className="setting-row">
          <div>
            <span className="setting-label">Clear in-memory context cache</span>
            <p>Purge the currently held foreground screenshot and inspected UI Automation tree.</p>
          </div>
          <button
            className="button button--secondary"
            onClick={() => void handleClearCache()}
            type="button"
          >
            Clear context cache
          </button>
        </div>
      </section>

      <section className="settings-group" aria-labelledby="logging-heading">
        <h2 id="logging-heading">Diagnostic logging</h2>
        <div className="setting-row">
          <div>
            <span className="setting-label">Enable local diagnostic logging</span>
            <p>Records high-level operational events locally for troubleshooting. Passwords and credentials are never logged.</p>
          </div>
          <Toggle
            checked={draft.diagnostic_logging}
            label="Enable local diagnostic logging"
            onChange={(checked) => onChange({ diagnostic_logging: checked })}
          />
        </div>

        {draft.diagnostic_logging ? (
          <div className="diagnostic-log-section">
            <div className="diagnostic-log-actions">
              <button
                className="button button--secondary"
                disabled={loadingLogs}
                onClick={() => void handleRefreshLogs()}
                type="button"
              >
                {loadingLogs ? "Loading…" : `View logs (${logs.length})`}
              </button>
              <button
                className="button button--secondary"
                disabled={logs.length === 0}
                onClick={() => void handleClearLogs()}
                type="button"
              >
                Clear logs
              </button>
            </div>
            {logs.length > 0 ? (
              <div className="diagnostic-log-viewer" role="region" aria-label="Diagnostic logs">
                <ol className="diagnostic-log-list">
                  {logs.slice(-20).map((log, idx) => (
                    <li key={`${log.timestamp_unix_ms}-${idx}`}>
                      <span className={`log-level log-level--${log.level}`}>{log.level}</span>
                      <span className="log-category">[{log.category}]</span>
                      <span className="log-message">{log.message}</span>
                    </li>
                  ))}
                </ol>
              </div>
            ) : null}
          </div>
        ) : null}

        {actionMessage ? (
          <div className="inline-success" role="status" style={{ marginTop: 12 }}>
            {actionMessage}
          </div>
        ) : null}
      </section>
    </div>
  );
}

export function SettingsPanel() {
  const [activeSection, setActiveSection] = useState<SectionId>("general");
  const [saved, setSaved] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [draft, setDraft] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [runtime, setRuntime] = useState<RuntimeStatus>(DEFAULT_RUNTIME_STATUS);
  const [windowContext, setWindowContext] = useState<WindowContextSnapshot | null>(null);
  const [uiAutomation, setUiAutomation] = useState<UiAutomationSnapshot | null>(null);
  const [highlightedElementId, setHighlightedElementId] = useState<string | null>(null);
  const [highlightingElementId, setHighlightingElementId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [capturing, setCapturing] = useState(false);
  const [inspecting, setInspecting] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void Promise.all([
      loadSettings(),
      loadRuntimeStatus(),
      loadLastWindowContext(),
      loadLastUiAutomation(),
    ])
      .then(([settings, status, context, automation]) => {
        setSaved(settings);
        setDraft(settings);
        setRuntime(status);
        setWindowContext(context);
        setUiAutomation(automation);
        applyTheme(settings.theme);
      })
      .catch((loadError: unknown) => setError(toUserMessage(loadError)))
      .finally(() => setLoading(false));
  }, []);

  const dirty = useMemo(() => JSON.stringify(draft) !== JSON.stringify(saved), [draft, saved]);
  const shortcutError = validateShortcut(draft.global_hotkey);

  function updateDraft(patch: Partial<AppSettings>) {
    setDraft((current) => ({ ...current, ...patch }));
    setMessage(null);
    setError(null);
  }

  async function handleSave(event: FormEvent) {
    event.preventDefault();
    if (shortcutError) return;
    setSaving(true);
    setError(null);
    setMessage(null);
    try {
      const nextRuntime = await saveSettings(draft);
      setRuntime(nextRuntime);
      setSaved(draft);
      setMessage("Settings saved.");
    } catch (saveError: unknown) {
      setError(toUserMessage(saveError));
    } finally {
      setSaving(false);
    }
  }

  function handleCancel() {
    setDraft(saved);
    applyTheme(saved.theme);
    setError(null);
    setMessage(null);
    setHighlightedElementId(null);
    void clearTargetHighlight();
    void hideWindow("settings");
  }

  function handleSectionChange(section: SectionId) {
    setActiveSection(section);
    if (section !== "advanced" && highlightedElementId) {
      setHighlightedElementId(null);
      void clearTargetHighlight();
    }
  }

  function handleNavKeyDown(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    let nextIndex = -1;
    if (event.key === "ArrowDown" || event.key === "ArrowRight") {
      event.preventDefault();
      nextIndex = (index + 1) % sections.length;
    } else if (event.key === "ArrowUp" || event.key === "ArrowLeft") {
      event.preventDefault();
      nextIndex = (index - 1 + sections.length) % sections.length;
    } else if (event.key === "Home") {
      event.preventDefault();
      nextIndex = 0;
    } else if (event.key === "End") {
      event.preventDefault();
      nextIndex = sections.length - 1;
    }
    if (nextIndex >= 0) {
      const nextSection = sections[nextIndex];
      handleSectionChange(nextSection.id);
      const navButtons = event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>("button");
      navButtons?.[nextIndex]?.focus();
    }
  }

  async function handleContextCapture() {
    setCapturing(true);
    setError(null);
    setMessage(null);
    setHighlightedElementId(null);
    try {
      const context = await captureActiveWindow();
      setWindowContext(context);
      setUiAutomation(null);
      setRuntime(await loadRuntimeStatus());
      setMessage("Active application captured locally.");
    } catch (captureError: unknown) {
      setError(toUserMessage(captureError));
    } finally {
      setCapturing(false);
    }
  }

  async function handleUiInspection() {
    setInspecting(true);
    setError(null);
    setMessage(null);
    setHighlightedElementId(null);
    try {
      const snapshot = await inspectTargetUi();
      setUiAutomation(snapshot);
      setMessage(`UI tree ready with ${snapshot.elements.length} useful elements.`);
    } catch (inspectionError: unknown) {
      setError(toUserMessage(inspectionError));
    } finally {
      setInspecting(false);
    }
  }

  async function handleElementHighlight(elementId: string) {
    setHighlightingElementId(elementId);
    setError(null);
    setMessage(null);
    try {
      if (highlightedElementId === elementId) {
        await clearTargetHighlight();
        setHighlightedElementId(null);
        setMessage("Target highlight cleared.");
      } else {
        const target = await highlightUiElement(elementId);
        setHighlightedElementId(target.element_id);
        setMessage(`Highlighting ${target.element_id} at ${target.bounds_physical.left}, ${target.bounds_physical.top}.`);
      }
    } catch (highlightError: unknown) {
      setError(toUserMessage(highlightError));
    } finally {
      setHighlightingElementId(null);
    }
  }

  return (
    <main className="settings-shell">
      <WindowChrome title="Settings" label="settings" />
      <form className="settings-form" onSubmit={handleSave}>
        <div className="settings-workspace">
          <nav className="settings-nav" aria-label="Settings sections">
            {sections.map((section, index) => (
              <button
                aria-current={activeSection === section.id ? "page" : undefined}
                className={activeSection === section.id ? "active" : ""}
                key={section.id}
                onClick={() => handleSectionChange(section.id)}
                onKeyDown={(event) => handleNavKeyDown(event, index)}
                type="button"
              >
                <Icon name={section.icon} size={20} />
                <span>{section.label}</span>
                <Icon className="nav-chevron" name="chevron" size={16} />
              </button>
            ))}
          </nav>

          <div className="settings-main">
            {loading ? (
              <div className="settings-loading" role="status">Loading settings…</div>
            ) : activeSection === "general" ? (
              <GeneralSettings draft={draft} onChange={updateDraft} shortcutError={shortcutError} />
            ) : activeSection === "ai" ? (
              <AiSettings
                hasContext={Boolean(windowContext)}
                hasUiTree={Boolean(uiAutomation)}
                includeScreenshot={draft.screenshot_transmission}
                model={draft.ai_model}
                provider={draft.ai_provider}
                providerSelectionSaved={draft.ai_provider === saved.ai_provider}
                savedWorkflows={draft.saved_workflows}
                onModelChange={(model) => updateDraft({ ai_model: model })}
                onProviderChange={(provider) => updateDraft({ ai_provider: provider })}
                onScreenshotChange={(enabled) => updateDraft({ screenshot_transmission: enabled })}
                onSaveWorkflow={(workflow) =>
                  updateDraft({ saved_workflows: [...draft.saved_workflows, workflow] })
                }
                onDeleteWorkflow={(id) =>
                  updateDraft({
                    saved_workflows: draft.saved_workflows.filter((workflow) => workflow.id !== id),
                  })
                }
              />
            ) : activeSection === "schedules" ? (
              <SchedulesSection
                onAdd={(schedule) =>
                  updateDraft({ schedules: [...draft.schedules, schedule] })
                }
                onDelete={(id) =>
                  updateDraft({
                    schedules: draft.schedules.filter((schedule) => schedule.id !== id),
                  })
                }
                onToggle={(id, enabled) =>
                  updateDraft({
                    schedules: draft.schedules.map((schedule) =>
                      schedule.id === id ? { ...schedule, enabled } : schedule,
                    ),
                  })
                }
                savedWorkflows={draft.saved_workflows}
                schedules={draft.schedules}
              />
            ) : activeSection === "automation" ? (
              <AutomationSettings draft={draft} onChange={updateDraft} />
            ) : activeSection === "privacy" ? (
              <PrivacySettings
                draft={draft}
                onChange={updateDraft}
                onCacheCleared={() => {
                  setWindowContext(null);
                  setUiAutomation(null);
                  setHighlightedElementId(null);
                }}
              />
            ) : activeSection === "advanced" ? (
              <ContextInspector
                capturing={capturing}
                context={windowContext}
                highlightedElementId={highlightedElementId}
                highlightingElementId={highlightingElementId}
                inspecting={inspecting}
                onCapture={() => void handleContextCapture()}
                onHighlight={(elementId) => void handleElementHighlight(elementId)}
                onInspect={() => void handleUiInspection()}
                uiAutomation={uiAutomation}
              />
            ) : null}

            {runtime.hotkey_warning ? (
              <div className="message-banner message-banner--warning" role="status">
                {runtime.hotkey_warning}
              </div>
            ) : null}
            {error ? <div className="message-banner message-banner--error" role="alert">{error}</div> : null}
            {message ? <div className="message-banner message-banner--success" role="status"><Icon name="check" size={16} />{message}</div> : null}
          </div>
        </div>

        <footer className="settings-footer">
          <span className="runtime-note">
            {runtime.registered_hotkey ? `Active shortcut: ${runtime.registered_hotkey.split("+").join(" + ")}` : "No global shortcut registered"}
          </span>
          <div>
            <button className="button button--secondary" onClick={handleCancel} type="button">Cancel</button>
            <button className="button button--primary" disabled={!dirty || saving || Boolean(shortcutError)} type="submit">
              {saving ? "Saving…" : "Save settings"}
            </button>
          </div>
        </footer>
      </form>
    </main>
  );
}
