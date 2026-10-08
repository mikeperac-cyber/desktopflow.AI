import { type FormEvent, useEffect, useRef, useState } from "react";

import { BrandMark } from "./Icon";
import { PlanInspector } from "./PlanInspector";
import { applyTheme } from "../lib/theme";
import {
  createActionPlan,
  emergencyStop,
  executeActionPlan,
  getRecordingStatus,
  hideWindow,
  loadRuntimeStatus,
  loadSettings,
  setOverlayPlanMode,
  startRecording,
  stopRecording,
  toUserMessage,
} from "../services/desktop";
import {
  DEFAULT_SETTINGS,
  type AppSettings,
  type ExecutionReport,
  type PlanningModel,
  type PlanningResult,
  type RecordingStatus,
} from "../types/settings";

type OverlayState = "idle" | "paused" | "planning" | "planned" | "executing" | "executed" | "error" | "recording";

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
  const [recording, setRecording] = useState<RecordingStatus | null>(null);

  useEffect(() => {
    void Promise.all([loadSettings(), loadRuntimeStatus(), getRecordingStatus()]).then(([loadedSettings, runtime, recordingStatus]) => {
      setSettings(loadedSettings);
      applyTheme(loadedSettings.theme);
      setModel(loadedSettings.ai_model);
      setIncludeScreenshot(loadedSettings.screenshot_transmission);
      if (recordingStatus?.recording) {
        setRecording(recordingStatus);
        setState("recording");
      } else {
        setState(runtime.paused ? "paused" : "idle");
      }
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

  async function handleStartRecording() {
    if (state !== "idle") return;
    setError(null);
    try {
      const status = await startRecording();
      setRecording(status);
      setState("recording");
      await hideWindow("overlay");
    } catch (recordError: unknown) {
      setError(toUserMessage(recordError));
      setState("error");
    }
  }

  async function handleStopRecording() {
    setError(null);
    try {
      const result = await stopRecording();
      setRecording(null);
      setPlan(result);
      await setOverlayPlanMode(true);
      setState("planned");
    } catch (recordError: unknown) {
      setRecording(null);
      setError(toUserMessage(recordError));
      setState("error");
    }
  }

  useEffect(() => {
    if (state !== "recording") return;
    const timer = window.setInterval(() => {
      void getRecordingStatus()
        .then((status) => {
          if (status?.recording) {
            setRecording(status);
          } else {
            setRecording(null);
            setState("idle");
          }
        })
        .catch(() => undefined);
    }, 1000);
    return () => window.clearInterval(timer);
  }, [state]);

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
    recording: recording
      ? `Recording · ${recording.target_title} · ${recording.event_count} actions`
      : "Recording",
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
          disabled={state === "planning" || state === "executing" || state === "recording"}
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
          placeholder={state === "recording" ? "Recording your actions…" : "Ask DeskFlow what to do..."}
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
          {state === "idle" ? (
            <button className="button button--secondary" onClick={() => void handleStartRecording()} type="button">
              Record
            </button>
          ) : null}
          <span><kbd>Esc</kbd> {state === "executing" ? "Emergency Stop" : "Close"}</span>
        </div>
      </div>

      {state === "recording" ? (
        <div className="overlay-status-row" aria-live="polite">
          <div className="status-copy">
            <span className="status-dot status-dot--recording" />
            <span>Interact with {recording?.target_title ?? "the target"}, then stop here to review the plan.</span>
          </div>
          <div className="key-hints" aria-label="Recording controls">
            <button className="button button--secondary" onClick={() => void handleStopRecording()} type="button">
              Stop ({recording?.event_count ?? 0})
            </button>
          </div>
        </div>
      ) : null}

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
