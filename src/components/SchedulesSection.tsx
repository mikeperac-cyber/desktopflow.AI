import { useEffect, useMemo, useState } from "react";

import { BUILT_IN_WORKFLOWS } from "../lib/workflows";
import { describeTrigger } from "../lib/schedules";
import { loadScheduleRuns } from "../services/desktop";
import type { SavedWorkflow, ScheduleRun, ScheduledWorkflow, ScheduleTrigger } from "../types/settings";

interface SchedulesSectionProps {
  schedules: ScheduledWorkflow[];
  savedWorkflows: SavedWorkflow[];
  onAdd: (schedule: ScheduledWorkflow) => void;
  onToggle: (id: string, enabled: boolean) => void;
  onDelete: (id: string) => void;
}

type TriggerKind = "once" | "daily" | "weekly" | "file_appears";

const TRIGGER_LABELS: Record<TriggerKind, string> = {
  once: "Once",
  daily: "Daily",
  weekly: "Weekly",
  file_appears: "File appears",
};

const WEEKDAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

function makeScheduleId(): string {
  return `sched-${Date.now().toString(36)}-${Math.floor(Math.random() * 0xffff).toString(36)}`;
}

function toDateTimeLocal(value: number): string {
  const date = new Date(value);
  const offset = date.getTimezoneOffset();
  return new Date(value - offset * 60_000).toISOString().slice(0, 16);
}

export function SchedulesSection({ schedules, savedWorkflows, onAdd, onToggle, onDelete }: SchedulesSectionProps) {
  const [name, setName] = useState("");
  const [instruction, setInstruction] = useState("");
  const [recipe, setRecipe] = useState("");
  const [triggerKind, setTriggerKind] = useState<TriggerKind>("daily");
  const [onceAt, setOnceAt] = useState(() => toDateTimeLocal(Date.now() + 3_600_000));
  const [hour, setHour] = useState("09");
  const [minute, setMinute] = useState("00");
  const [weekdays, setWeekdays] = useState<number>(0b001_1111);
  const [folder, setFolder] = useState("");
  const [pattern, setPattern] = useState("*.pdf");
  const [expectedProcess, setExpectedProcess] = useState("");
  const [autonomous, setAutonomous] = useState(false);
  const [runs, setRuns] = useState<ScheduleRun[]>([]);

  useEffect(() => {
    void loadScheduleRuns().then(setRuns).catch(() => undefined);
  }, []);

  const lastRunBySchedule = useMemo(() => {
    const latest = new Map<string, ScheduleRun>();
    for (const run of runs) {
      if (!latest.has(run.schedule_id)) latest.set(run.schedule_id, run);
    }
    return latest;
  }, [runs]);

  function applyRecipe(value: string) {
    setRecipe(value);
    if (!value) return;
    const [source, id] = value.split(":", 2);
    const found =
      source === "saved"
        ? savedWorkflows.find((workflow) => workflow.id === id)
        : BUILT_IN_WORKFLOWS.find((workflow) => workflow.id === id);
    if (found) {
      setInstruction(found.instruction);
      if (!name.trim()) setName(found.name);
    }
  }

  function buildTrigger(): ScheduleTrigger | null {
    switch (triggerKind) {
      case "once": {
        const at = new Date(onceAt).getTime();
        if (!Number.isFinite(at)) return null;
        return { kind: "once", at_unix_ms: at };
      }
      case "daily": {
        const parsedHour = Number(hour);
        const parsedMinute = Number(minute);
        if (!Number.isInteger(parsedHour) || parsedHour < 0 || parsedHour > 23) return null;
        if (!Number.isInteger(parsedMinute) || parsedMinute < 0 || parsedMinute > 59) return null;
        return { kind: "daily", hour: parsedHour, minute: parsedMinute };
      }
      case "weekly": {
        const parsedHour = Number(hour);
        const parsedMinute = Number(minute);
        if (weekdays === 0) return null;
        if (!Number.isInteger(parsedHour) || parsedHour < 0 || parsedHour > 23) return null;
        if (!Number.isInteger(parsedMinute) || parsedMinute < 0 || parsedMinute > 59) return null;
        return { kind: "weekly", weekdays, hour: parsedHour, minute: parsedMinute };
      }
      case "file_appears": {
        if (!folder.trim() || !pattern.trim()) return null;
        return { kind: "file_appears", folder: folder.trim(), pattern: pattern.trim() };
      }
    }
  }

  const trigger = buildTrigger();
  const canAdd =
    name.trim().length > 0 &&
    name.trim().length <= 80 &&
    instruction.trim().length > 0 &&
    instruction.trim().length <= 4000 &&
    trigger !== null &&
    schedules.length < 20;

  function handleAdd() {
    if (!canAdd || !trigger) return;
    if (trigger.kind === "once" && trigger.at_unix_ms <= Date.now()) return;
    onAdd({
      id: makeScheduleId(),
      name: name.trim(),
      instruction: instruction.trim(),
      trigger,
      autonomous,
      enabled: true,
      expected_process: expectedProcess.trim() ? expectedProcess.trim() : null,
      created_at_unix_ms: Date.now(),
    });
    setName("");
    setInstruction("");
    setRecipe("");
    setExpectedProcess("");
    setAutonomous(false);
  }

  function toggleWeekday(index: number) {
    setWeekdays((current) => current ^ (1 << index));
  }

  return (
    <div className="settings-content-section">
      <section className="settings-group" aria-labelledby="schedules-heading">
        <h2 id="schedules-heading">Scheduled workflows</h2>
        <p className="ai-provider-copy">
          Schedules plan with your selected provider and either wait for your confirmation
          (attended) or run alone when the plan is low-risk (autonomous). Anything riskier is
          downgraded to attended. Runs never start while paused, executing, recording, or while
          another plan awaits review.
        </p>

        <ul className="workflow-list">
          {schedules.map((schedule) => {
            const lastRun = lastRunBySchedule.get(schedule.id);
            return (
              <li className="workflow-card" key={schedule.id}>
                <div>
                  <strong>{schedule.name}</strong>
                  <small>
                    {describeTrigger(schedule.trigger)} · {schedule.autonomous ? "Autonomous" : "Attended"} ·{" "}
                    {schedule.enabled ? "Armed" : "Paused"}
                    {schedule.expected_process ? ` · ${schedule.expected_process}` : ""}
                  </small>
                  {lastRun ? (
                    <p>
                      Last run: {lastRun.outcome} — {lastRun.detail}
                    </p>
                  ) : (
                    <p>No runs yet.</p>
                  )}
                </div>
                <div className="workflow-actions">
                  <button
                    className="button button--secondary"
                    onClick={() => onToggle(schedule.id, !schedule.enabled)}
                    type="button"
                  >
                    {schedule.enabled ? "Pause" : "Arm"}
                  </button>
                  <button className="button button--secondary" onClick={() => onDelete(schedule.id)} type="button">
                    Delete
                  </button>
                </div>
              </li>
            );
          })}
        </ul>
        {schedules.length === 0 ? (
          <p className="provider-setup-note">No schedules yet. Arm one below.</p>
        ) : null}
      </section>

      <section className="settings-group" aria-labelledby="new-schedule-heading">
        <h2 id="new-schedule-heading">New schedule</h2>

        <div className="setting-row setting-row--input provider-picker-row">
          <div>
            <label htmlFor="schedule-recipe">Start from a recipe</label>
            <p>Optional. Fills the instruction and name.</p>
          </div>
          <select id="schedule-recipe" onChange={(event) => applyRecipe(event.currentTarget.value)} value={recipe}>
            <option value="">Custom instruction…</option>
            <optgroup label="Built-in recipes">
              {BUILT_IN_WORKFLOWS.map((workflow) => (
                <option key={workflow.id} value={`built-in:${workflow.id}`}>
                  {workflow.name}
                </option>
              ))}
            </optgroup>
            {savedWorkflows.length > 0 ? (
              <optgroup label="Saved workflows">
                {savedWorkflows.map((workflow) => (
                  <option key={workflow.id} value={`saved:${workflow.id}`}>
                    {workflow.name}
                  </option>
                ))}
              </optgroup>
            ) : null}
          </select>
        </div>

        <div className="setting-row setting-row--input provider-picker-row">
          <div>
            <label htmlFor="schedule-name">Name</label>
          </div>
          <input
            id="schedule-name"
            maxLength={80}
            onChange={(event) => setName(event.currentTarget.value)}
            placeholder="Morning downloads"
            spellCheck={false}
            type="text"
            value={name}
          />
        </div>

        <label htmlFor="schedule-instruction">Instruction</label>
        <textarea
          id="schedule-instruction"
          maxLength={4000}
          onChange={(event) => setInstruction(event.currentTarget.value)}
          placeholder="What should run on schedule…"
          value={instruction}
        />

        <div className="model-options" role="radiogroup" aria-label="Trigger type">
          {(Object.keys(TRIGGER_LABELS) as TriggerKind[]).map((option) => (
            <label className="model-option" key={option}>
              <input
                checked={triggerKind === option}
                name="schedule-trigger"
                onChange={() => setTriggerKind(option)}
                type="radio"
              />
              <span>
                <strong>{TRIGGER_LABELS[option]}</strong>
              </span>
            </label>
          ))}
        </div>

        {triggerKind === "once" ? (
          <div className="setting-row setting-row--input provider-picker-row">
            <div>
              <label htmlFor="schedule-once">Run at</label>
            </div>
            <input
              id="schedule-once"
              onChange={(event) => setOnceAt(event.currentTarget.value)}
              type="datetime-local"
              value={onceAt}
            />
          </div>
        ) : null}

        {triggerKind === "daily" || triggerKind === "weekly" ? (
          <div className="setting-row setting-row--input provider-picker-row">
            <div>
              <label htmlFor="schedule-hour">Time of day</label>
            </div>
            <div className="schedule-time-inputs">
              <input
                aria-label="Hour"
                id="schedule-hour"
                max={23}
                min={0}
                onChange={(event) => setHour(event.currentTarget.value)}
                type="number"
                value={hour}
              />
              <span>:</span>
              <input
                aria-label="Minute"
                max={59}
                min={0}
                onChange={(event) => setMinute(event.currentTarget.value)}
                type="number"
                value={minute}
              />
            </div>
          </div>
        ) : null}

        {triggerKind === "weekly" ? (
          <div className="setting-row setting-row--input provider-picker-row">
            <div>
              <span className="setting-label" id="schedule-weekdays-label">Weekdays</span>
            </div>
            <div aria-labelledby="schedule-weekdays-label" className="schedule-weekday-inputs" role="group">
              {WEEKDAYS.map((day, index) => (
                <label key={day}>
                  <input
                    checked={(weekdays & (1 << index)) !== 0}
                    onChange={() => toggleWeekday(index)}
                    type="checkbox"
                  />
                  {day}
                </label>
              ))}
            </div>
          </div>
        ) : null}

        {triggerKind === "file_appears" ? (
          <>
            <div className="setting-row setting-row--input provider-picker-row">
              <div>
                <label htmlFor="schedule-folder">Watched folder</label>
                <p>Only file names are read; contents are never opened.</p>
              </div>
              <input
                id="schedule-folder"
                maxLength={260}
                onChange={(event) => setFolder(event.currentTarget.value)}
                placeholder="C:\Users\you\Downloads"
                spellCheck={false}
                type="text"
                value={folder}
              />
            </div>
            <div className="setting-row setting-row--input provider-picker-row">
              <div>
                <label htmlFor="schedule-pattern">File pattern</label>
                <p>Letters, digits, spaces and * ? . _ - only.</p>
              </div>
              <input
                id="schedule-pattern"
                maxLength={80}
                onChange={(event) => setPattern(event.currentTarget.value)}
                spellCheck={false}
                type="text"
                value={pattern}
              />
            </div>
          </>
        ) : null}

        <div className="setting-row setting-row--input provider-picker-row">
          <div>
            <label htmlFor="schedule-process">Expected app (optional)</label>
            <p>The run is skipped unless this process is in the foreground.</p>
          </div>
          <input
            id="schedule-process"
            maxLength={64}
            onChange={(event) => setExpectedProcess(event.currentTarget.value)}
            placeholder="notepad.exe"
            spellCheck={false}
            type="text"
            value={expectedProcess}
          />
        </div>

        <div className="setting-row">
          <div>
            <span className="setting-label">Run without asking</span>
            <p>Autonomous runs execute only low-risk plans inside your action budget.</p>
          </div>
          <button
            aria-checked={autonomous}
            aria-label="Run without asking"
            className={`toggle ${autonomous ? "toggle--on" : ""}`}
            onClick={() => setAutonomous((current) => !current)}
            role="switch"
            type="button"
          >
            <span />
          </button>
        </div>

        <button className="button button--primary" disabled={!canAdd} onClick={handleAdd} type="button">
          Arm schedule
        </button>
      </section>
    </div>
  );
}
