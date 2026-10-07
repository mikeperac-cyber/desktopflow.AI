import { type FormEvent, useEffect, useRef, useState } from "react";

import { BrandMark } from "./Icon";
import { PlanInspector } from "./PlanInspector";
import { applyTheme } from "../lib/theme";
import {
  createActionPlan,
  emergencyStop,
  executeActionPlan,
  hideWindow,
  loadRuntimeStatus,
  loadSettings,
  setOverlayPlanMode,
  toUserMessage,
} from "../services/desktop";
import {
  DEFAULT_SETTINGS,
  type AppSettings,
  type ExecutionReport,
  type PlanningModel,
  type PlanningResult,
} from "../types/settings";

type OverlayState = "idle" | "paused" | "planning" | "planned" | "executing" | "executed" | "error";

export function Spotlight() {
  const inputRef = useRef<HTMLInputElement>(null);
  const [instruction, setInstruction] = useState("");
  const [state, setState] = useState<OverlayState>("idle");
  const stateRef = useRef<OverlayState>(state);

  useEffect(() => {
    stateRef.current = state;
  }, [state]);

  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [contextStatus, setContextStatus] = useState("Ready · No target captured");
  const [model, setModel] = useState<PlanningModel>("fast");
  const [includeScreenshot, setIncludeScreenshot] = useState(false);
  const [plan, setPlan] = useState<PlanningResult | null>(null);
  const [execution, setExecution] = useState<ExecutionReport | null>(null);
  const [executionError, setExecutionError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void Promise.all([loadSettings(), loadRuntimeStatus()]).then(([loadedSettings, runtime]) => {
      setSettings(loadedSettings);
      applyTheme(loadedSettings.theme);
      setModel(loadedSettings.ai_model);
      setIncludeScreenshot(loadedSettings.screenshot_transmission);
      setState(runtime.paused ? "paused" : "idle");
      const targetName = runtime.active_target?.process_name || runtime.active_target?.title;
      setContextStatus(
        targetName
          ? `Ready · ${targetName}`
          : runtime.context_warning
            ? "Ready · Target capture unavailable"
            : "Ready · No target captured",
      );
      window.requestAnimationFrame(() => inputRef.current?.focus());
    });
    void setOverlayPlanMode(false);

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        if (stateRef.current === "executing") {
          void emergencyStop();
        } else {
          void hideWindow("overlay");
        }
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    if (!instruction.trim()) {
      inputRef.current?.focus();
      return;
    }
    if (state === "paused") return;
    setState("planning");
    setError(null);
    setPlan(null);
    setExecution(null);
    setExecutionError(null);
    try {
      const result = await createActionPlan(instruction, model, includeScreenshot);
      setPlan(result);
      await setOverlayPlanMode(true);
      setState("planned");
    } catch (planError: unknown) {
      setError(toUserMessage(planError));
      setState("error");
    }
  }

  async function handleExecute(approvedStepIds: string[] = [], customMaxSteps?: number) {
    if (!plan) return;
    setState("executing");
    setExecutionError(null);
    try {
      const report =
        approvedStepIds.length > 0 ||
        (customMaxSteps !== undefined && customMaxSteps !== settings.maximum_autonomous_steps)
          ? await executeActionPlan(
              plan.provider_request_id,
              "overlay",
              approvedStepIds,
              customMaxSteps,
            )
          : await executeActionPlan(plan.provider_request_id, "overlay");
      setExecution(report);
      setState("executed");
    } catch (executionFailure: unknown) {
      setExecutionError(toUserMessage(executionFailure));
      setState("planned");
    }
  }

  async function handleEmergencyStop() {
    try {
      await emergencyStop();
    } catch (stopError: unknown) {
      setExecutionError(toUserMessage(stopError));
    }
  }

  const status = {
    idle: contextStatus,
    paused: "Paused · Resume automation from the tray",
    planning: "Planning · Inspecting the target and validating provider output",
    planned: "Plan validated · Review before running",
    executing: "Running · Observing, acting, and verifying each result",
    executed:
      execution?.status === "completed"
        ? execution.recovered
          ? "Execution recovered and completed"
          : "Execution verified"
        : execution?.status === "cancelled"
          ? "Execution cancelled"
          : execution?.status === "emergency_stopped"
            ? "Emergency stopped · Synthetic inputs released"
            : "Execution stopped safely",
    error: error ?? "Planning failed",
  }[state];

  return (
    <main className="overlay-shell" aria-label="DeskFlow command palette">
      <div className="overlay-heading" data-tauri-drag-region>
        <div className="brand-lockup" data-tauri-drag-region>
          <BrandMark size={34} />
          <h1 data-tauri-drag-region>DeskFlow AI</h1>
        </div>
      </div>

      <form onSubmit={(event) => void handleSubmit(event)}>
        <label className="sr-only" htmlFor="deskflow-command">
          Tell DeskFlow what to do
        </label>
        <input
          ref={inputRef}
          id="deskflow-command"
          className="command-input"
          disabled={state === "planning" || state === "executing"}
          value={instruction}
          onChange={(event) => {
            setInstruction(event.currentTarget.value);
            if (state === "planned" || state === "executed" || state === "error") {
              setPlan(null);
              setExecution(null);
              setExecutionError(null);
              setError(null);
              setState("idle");
              void setOverlayPlanMode(false);
            }
          }}
          placeholder="Ask DeskFlow what to do..."
          spellCheck="true"
          autoComplete="off"
        />
      </form>

      <div className="overlay-status-row" aria-live="polite">
        <div className="status-copy">
          <span className={`status-dot status-dot--${state}`} />
          <span>{status}</span>
        </div>
        <div className="key-hints" aria-label="Keyboard shortcuts">
          <span><kbd>Enter</kbd> Plan</span>
          <span><kbd>Esc</kbd> {state === "executing" ? "Emergency Stop" : "Close"}</span>
        </div>
      </div>

      {plan ? (
        <PlanInspector
          compact
          approvalPolicy={settings.approval_policy}
          defaultMaxSteps={settings.maximum_autonomous_steps}
          execution={execution}
          executionError={executionError}
          executing={state === "executing"}
          onEmergencyStop={() => void handleEmergencyStop()}
          onExecute={(approvedStepIds, customMaxSteps) => void handleExecute(approvedStepIds, customMaxSteps)}
          result={plan}
        />
      ) : null}
    </main>
  );
}
