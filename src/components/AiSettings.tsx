import { useEffect, useState } from "react";

import { PlanInspector } from "./PlanInspector";
import {
  createActionPlan,
  deleteAiProviderCredential,
  emergencyStop,
  executeActionPlan,
  loadAiProviderStatus,
  loadLastActionPlan,
  saveAiProviderCredential,
  toUserMessage,
} from "../services/desktop";
import type {
  AiProviderKind,
  ExecutionReport,
  PlanningModel,
  PlanningResult,
  ProviderCatalog,
} from "../types/settings";

interface AiSettingsProps {
  model: PlanningModel;
  provider: AiProviderKind;
  providerSelectionSaved: boolean;
  includeScreenshot: boolean;
  hasContext: boolean;
  hasUiTree: boolean;
  onModelChange: (model: PlanningModel) => void;
  onProviderChange: (provider: AiProviderKind) => void;
  onScreenshotChange: (enabled: boolean) => void;
}

export function AiSettings({
  model,
  provider,
  providerSelectionSaved,
  includeScreenshot,
  hasContext,
  hasUiTree,
  onModelChange,
  onProviderChange,
  onScreenshotChange,
}: AiSettingsProps) {
  const [catalog, setCatalog] = useState<ProviderCatalog | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [credentialBusy, setCredentialBusy] = useState(false);
  const [credentialMessage, setCredentialMessage] = useState<string | null>(null);
  const [credentialError, setCredentialError] = useState<string | null>(null);
  const [instruction, setInstruction] = useState("");
  const [plan, setPlan] = useState<PlanningResult | null>(null);
  const [planning, setPlanning] = useState(false);
  const [executing, setExecuting] = useState(false);
  const [execution, setExecution] = useState<ExecutionReport | null>(null);
  const [executionError, setExecutionError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void Promise.all([loadAiProviderStatus(), loadLastActionPlan()])
      .then(([status, lastPlan]) => {
        setCatalog(status);
        setPlan(lastPlan);
      })
      .catch((loadError: unknown) => setError(toUserMessage(loadError)));
  }, []);

  const activeProvider = catalog?.providers.find((option) => option.provider === provider) ?? null;
  const planningAvailable = Boolean(activeProvider?.configured && providerSelectionSaved);

  function handleProviderChange(nextProvider: AiProviderKind) {
    const next = catalog?.providers.find((option) => option.provider === nextProvider);
    if (next && !next.supports_screenshot && includeScreenshot) {
      onScreenshotChange(false);
    }
    onProviderChange(nextProvider);
    setCredentialMessage(null);
    setCredentialError(null);
  }

  async function handleSaveCredential() {
    if (!apiKey.trim()) return;
    setCredentialBusy(true);
    setCredentialError(null);
    setCredentialMessage(null);
    try {
      setCatalog(await saveAiProviderCredential(provider, apiKey));
      setCredentialMessage(`${activeProvider?.label ?? "Provider"} key saved in Windows Credential Manager.`);
    } catch (saveError: unknown) {
      setCredentialError(toUserMessage(saveError));
    } finally {
      setApiKey("");
      setCredentialBusy(false);
    }
  }

  async function handleDeleteCredential() {
    setCredentialBusy(true);
    setCredentialError(null);
    setCredentialMessage(null);
    try {
      setCatalog(await deleteAiProviderCredential(provider));
      setCredentialMessage(`${activeProvider?.label ?? "Provider"} saved key removed.`);
    } catch (deleteError: unknown) {
      setCredentialError(toUserMessage(deleteError));
    } finally {
      setApiKey("");
      setCredentialBusy(false);
    }
  }

  async function handlePlan() {
    if (!instruction.trim()) return;
    setPlanning(true);
    setError(null);
    setExecution(null);
    setExecutionError(null);
    try {
      setPlan(await createActionPlan(instruction, model, includeScreenshot));
    } catch (planError: unknown) {
      setError(toUserMessage(planError));
    } finally {
      setPlanning(false);
    }
  }

  async function handleExecute(approvedStepIds: string[] = [], customMaxSteps?: number) {
    if (!plan) return;
    setExecuting(true);
    setExecutionError(null);
    try {
      const report =
        approvedStepIds.length > 0 || customMaxSteps !== undefined
          ? await executeActionPlan(
              plan.provider_request_id,
              "settings",
              approvedStepIds,
              customMaxSteps,
            )
          : await executeActionPlan(plan.provider_request_id, "settings");
      setExecution(report);
    } catch (executionFailure: unknown) {
      setExecutionError(toUserMessage(executionFailure));
    } finally {
      setExecuting(false);
    }
  }

  async function handleEmergencyStop() {
    try {
      await emergencyStop();
    } catch (stopError: unknown) {
      setExecutionError(toUserMessage(stopError));
    }
  }

  return (
    <div className="ai-settings settings-content-section">
      <section className="settings-group ai-provider-card" aria-labelledby="ai-provider-heading">
        <div className="ai-provider-heading">
          <div>
            <span className="eyebrow">Secure multi-provider planning</span>
            <h2 id="ai-provider-heading">{activeProvider?.label ?? "AI provider"}</h2>
          </div>
          <span className={`provider-status ${activeProvider?.configured ? "provider-status--ready" : ""}`}>
            {activeProvider
              ? activeProvider.configured
                ? activeProvider.requires_credential
                  ? "Configured"
                  : "Ready · on this PC"
                : activeProvider.requires_credential
                  ? "Needs API key"
                  : "Needs local model"
              : "Needs API key"}
          </span>
        </div>
        <p className="ai-provider-copy">
          {activeProvider?.description ?? "Choose a provider for native planning."} Only the Rust executor can run a locally validated plan after your confirmation.
        </p>
        <div className="setting-row setting-row--input provider-picker-row">
          <div>
            <label htmlFor="ai-provider">Provider</label>
            <p>Save the selection before generating a plan.</p>
          </div>
          <select
            id="ai-provider"
            onChange={(event) => handleProviderChange(event.currentTarget.value as AiProviderKind)}
            value={provider}
          >
            {(catalog?.providers ?? []).map((option) => (
              <option key={option.provider} value={option.provider}>{option.label}</option>
            ))}
          </select>
        </div>
        {!providerSelectionSaved ? (
          <div className="provider-setup-note">Save settings to make {activeProvider?.label ?? "this provider"} active.</div>
        ) : null}
        {activeProvider?.notice ? <div className="provider-setup-note">{activeProvider.notice}</div> : null}
        {activeProvider && !activeProvider.requires_credential ? (
          <div className="credential-editor">
            <div className="credential-heading">
              <div>
                <strong>Free local engine</strong>
                <small>
                  {activeProvider.configured
                    ? "Local model server reachable"
                    : "Local model server not detected"}
                </small>
              </div>
            </div>
            <p>No API key is needed and plans never leave this PC. To enable free planning:</p>
            <p>1. Install Ollama from ollama.com. 2. Run <code>ollama pull qwen3:8b</code>. 3. Keep <code>ollama serve</code> running.</p>
            <p>DeskFlow talks only to 127.0.0.1:11434 and validates every local plan with the same Rust safety checks as hosted providers.</p>
          </div>
        ) : (
        <div className="credential-editor">
          <div className="credential-heading">
            <div>
              <strong>API credential</strong>
              <small>
                {activeProvider?.credential_source === "windows_credential_manager"
                  ? "Stored in Windows Credential Manager"
                  : activeProvider?.credential_source === "process_environment"
                    ? "Provided by the native process environment"
                    : "No credential configured"}
              </small>
            </div>
          </div>
          <div className="credential-controls">
            <input
              aria-label={`${activeProvider?.label ?? "Provider"} API key`}
              autoComplete="new-password"
              onChange={(event) => setApiKey(event.currentTarget.value)}
              placeholder={activeProvider?.configured ? "Enter a replacement key" : "Paste key locally"}
              spellCheck={false}
              type="password"
              value={apiKey}
            />
            <button
              className="button button--primary"
              disabled={!apiKey.trim() || credentialBusy}
              onClick={() => void handleSaveCredential()}
              type="button"
            >
              {credentialBusy ? "Saving…" : activeProvider?.configured ? "Replace key" : "Save key"}
            </button>
            {activeProvider?.credential_source === "windows_credential_manager" ? (
              <button
                className="button button--secondary"
                disabled={credentialBusy}
                onClick={() => void handleDeleteCredential()}
                type="button"
              >
                Remove
              </button>
            ) : null}
          </div>
          <p>Sent once to the native host, then cleared from this field. It is never written to DeskFlow settings or browser storage.</p>
          {credentialMessage ? <div className="inline-success" role="status">{credentialMessage}</div> : null}
          {credentialError ? <div className="inline-error" role="alert">{credentialError}</div> : null}
        </div>
        )}
      </section>

      <section className="settings-group" aria-labelledby="ai-model-heading">
        <h2 id="ai-model-heading">Planning model</h2>
        <div className="model-options" role="radiogroup" aria-label="Planning model">
          {(activeProvider?.models ?? []).map((option) => (
            <label className="model-option" key={option.profile}>
              <input
                checked={model === option.profile}
                name="ai-model"
                onChange={() => onModelChange(option.profile)}
                type="radio"
              />
              <span>
                <strong>{option.label}</strong>
                <small>{option.id} · {option.stability}</small>
              </span>
            </label>
          ))}
        </div>
        <div className="setting-row ai-screenshot-row">
          <div>
            <span className="setting-label">Include the captured screenshot</span>
            <p>Off by default. The filtered UI tree is always sent when you explicitly generate a plan.</p>
          </div>
          <button
            aria-checked={includeScreenshot}
            aria-label="Include captured screenshot in AI planning"
            className={`toggle ${includeScreenshot ? "toggle--on" : ""}`}
            disabled={!activeProvider?.supports_screenshot}
            onClick={() => onScreenshotChange(!includeScreenshot)}
            role="switch"
            type="button"
          >
            <span />
          </button>
        </div>
        {!activeProvider?.supports_screenshot ? (
          <p className="provider-capability-note">Screenshot transmission is unavailable for this provider profile; the filtered UI tree is still used.</p>
        ) : null}
      </section>

      <section className="settings-group ai-plan-builder" aria-labelledby="plan-builder-heading">
        <div className="context-section-heading">
          <div><span className={`context-status-dot ${hasContext ? "" : "context-status-dot--idle"}`} />Context input</div>
          <span>{hasContext ? (hasUiTree ? "Capture + UI tree ready" : "UI tree will be inspected on request") : "Capture a target in Advanced"}</span>
        </div>
        <h2 id="plan-builder-heading">Generate an action plan</h2>
        <label htmlFor="ai-instruction">Instruction</label>
        <textarea
          id="ai-instruction"
          maxLength={4000}
          onChange={(event) => setInstruction(event.currentTarget.value)}
          placeholder="For example: open the File menu and choose Save As"
          value={instruction}
        />
        <div className="ai-plan-actions">
          <span>{instruction.length.toLocaleString()} / 4,000</span>
          <button
            className="button button--primary"
            disabled={!planningAvailable || !hasContext || !instruction.trim() || planning}
            onClick={() => void handlePlan()}
            type="button"
          >
            {planning ? "Planning…" : "Generate plan"}
          </button>
        </div>
        {error ? <div className="inline-error" role="alert">{error}</div> : null}
      </section>

      {plan ? (
        <PlanInspector
          execution={execution}
          executionError={executionError}
          executing={executing}
          onEmergencyStop={() => void handleEmergencyStop()}
          onExecute={(approvedStepIds, customMaxSteps) => void handleExecute(approvedStepIds, customMaxSteps)}
          result={plan}
        />
      ) : null}
    </div>
  );
}
