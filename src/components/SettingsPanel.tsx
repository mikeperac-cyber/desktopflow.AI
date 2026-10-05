import { type FormEvent, useEffect, useMemo, useState } from "react";

import { Icon } from "./Icon";
import { AiSettings } from "./AiSettings";
import { ContextInspector } from "./ContextInspector";
import { WindowChrome } from "./WindowChrome";
import { applyTheme, validateShortcut } from "../lib/theme";
import {
  captureActiveWindow,
  clearTargetHighlight,
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
  type RuntimeStatus,
  type ThemePreference,
  type UiAutomationSnapshot,
  type WindowContextSnapshot,
} from "../types/settings";

type SectionId = "general" | "ai" | "automation" | "privacy" | "advanced";

const sections: Array<{ id: SectionId; label: string; icon: Parameters<typeof Icon>[0]["name"] }> = [
  { id: "general", label: "General", icon: "general" },
  { id: "ai", label: "AI", icon: "ai" },
  { id: "automation", label: "Automation", icon: "automation" },
  { id: "privacy", label: "Privacy", icon: "privacy" },
  { id: "advanced", label: "Advanced", icon: "sliders" },
];

const sectionCopy: Record<Exclude<SectionId, "general" | "advanced" | "ai">, { title: string; body: string; milestone: string }> = {
  automation: {
    title: "Automation controls",
    body: "The deterministic executor already enforces delay and total action limits. Granular approval policy and global emergency cancellation remain the next safety milestone.",
    milestone: "Phase 8 safety controls next",
  },
  privacy: {
    title: "Privacy controls",
    body: "Provider keys are isolated in Windows Credential Manager. Screenshots remain in memory and are transmitted only after explicit opt-in; additional redaction, retention, and logging controls remain open.",
    milestone: "Phase 9 credential storage implemented · controls in progress",
  },
};

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

  const info = activeSection === "automation" || activeSection === "privacy"
    ? sectionCopy[activeSection]
    : null;
  const activeIcon = sections.find((section) => section.id === activeSection)?.icon ?? "sliders";

  return (
    <main className="settings-shell">
      <WindowChrome title="Settings" label="settings" />
      <form className="settings-form" onSubmit={handleSave}>
        <div className="settings-workspace">
          <nav className="settings-nav" aria-label="Settings sections">
            {sections.map((section) => (
              <button
                aria-current={activeSection === section.id ? "page" : undefined}
                className={activeSection === section.id ? "active" : ""}
                key={section.id}
                onClick={() => handleSectionChange(section.id)}
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
                onModelChange={(model) => updateDraft({ ai_model: model })}
                onProviderChange={(provider) => updateDraft({ ai_provider: provider })}
                onScreenshotChange={(enabled) => updateDraft({ screenshot_transmission: enabled })}
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
            ) : info ? (
              <section className="future-section">
                <span className="future-icon"><Icon name={activeIcon} size={28} /></span>
                <h2>{info.title}</h2>
                <p>{info.body}</p>
                <strong>{info.milestone}</strong>
              </section>
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
