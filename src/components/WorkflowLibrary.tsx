import { useMemo, useState } from "react";

import {
  BUILT_IN_WORKFLOWS,
  MAX_SAVED_WORKFLOWS,
  MAX_WORKFLOW_INSTRUCTION_CHARS,
  MAX_WORKFLOW_NAME_CHARS,
  WORKFLOW_CATEGORIES,
  type WorkflowCategory,
} from "../lib/workflows";
import type { SavedWorkflow } from "../types/settings";

interface WorkflowLibraryProps {
  savedWorkflows: SavedWorkflow[];
  currentInstruction: string;
  onUse: (instruction: string) => void;
  onSave: (name: string) => void;
  onDelete: (id: string) => void;
}

export function WorkflowLibrary({
  savedWorkflows,
  currentInstruction,
  onUse,
  onSave,
  onDelete,
}: WorkflowLibraryProps) {
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<WorkflowCategory | "All" | "Mine">("All");
  const [workflowName, setWorkflowName] = useState("");

  const visibleBuiltIn = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return BUILT_IN_WORKFLOWS.filter((workflow) => {
      if (category !== "All" && category !== "Mine" && workflow.category !== category) return false;
      if (category === "Mine") return false;
      if (!needle) return true;
      return `${workflow.name} ${workflow.description} ${workflow.app_hint} ${workflow.instruction}`
        .toLowerCase()
        .includes(needle);
    });
  }, [query, category]);

  const visibleSaved = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return savedWorkflows.filter((workflow) => {
      if (category !== "All" && category !== "Mine") return false;
      if (!needle) return true;
      return `${workflow.name} ${workflow.instruction}`.toLowerCase().includes(needle);
    });
  }, [query, category, savedWorkflows]);

  const canSave =
    currentInstruction.trim().length > 0 &&
    currentInstruction.trim().length <= MAX_WORKFLOW_INSTRUCTION_CHARS &&
    workflowName.trim().length > 0 &&
    workflowName.trim().length <= MAX_WORKFLOW_NAME_CHARS &&
    savedWorkflows.length < MAX_SAVED_WORKFLOWS;

  function handleSave() {
    if (!canSave) return;
    onSave(workflowName.trim());
    setWorkflowName("");
  }

  return (
    <section className="settings-group" aria-labelledby="workflow-library-heading">
      <h2 id="workflow-library-heading">Workflow library</h2>
      <p className="ai-provider-copy">
        Each recipe fills the instruction below and still produces a validated plan that needs your
        confirmation. Recorded action plans cannot be saved — element IDs expire with each capture,
        so reusables are always instructions, never stale steps.
      </p>

      <div className="setting-row setting-row--input provider-picker-row">
        <div>
          <label htmlFor="workflow-search">Search recipes</label>
          <p>{BUILT_IN_WORKFLOWS.length} built-in, {savedWorkflows.length} saved.</p>
        </div>
        <input
          id="workflow-search"
          onChange={(event) => setQuery(event.currentTarget.value)}
          placeholder="Name, app, or task…"
          spellCheck={false}
          type="search"
          value={query}
        />
      </div>

      <div className="model-options" role="radiogroup" aria-label="Recipe category">
        {(["All", "Mine", ...WORKFLOW_CATEGORIES] as const).map((option) => (
          <label className="model-option" key={option}>
            <input
              checked={category === option}
              name="workflow-category"
              onChange={() => setCategory(option)}
              type="radio"
            />
            <span>
              <strong>{option}</strong>
            </span>
          </label>
        ))}
      </div>

      <ul className="workflow-list">
        {visibleSaved.map((workflow) => (
          <li className="workflow-card" key={workflow.id}>
            <div>
              <strong>{workflow.name}</strong>
              <small>Saved by you · {workflow.instruction.length.toLocaleString()} chars</small>
            </div>
            <div className="workflow-actions">
              <button className="button button--secondary" onClick={() => onUse(workflow.instruction)} type="button">
                Use
              </button>
              <button className="button button--secondary" onClick={() => onDelete(workflow.id)} type="button">
                Delete
              </button>
            </div>
          </li>
        ))}
        {visibleBuiltIn.map((workflow) => (
          <li className="workflow-card" key={workflow.id}>
            <div>
              <strong>{workflow.name}</strong>
              <small>
                {workflow.category} · {workflow.app_hint}
              </small>
              <p>{workflow.description}</p>
            </div>
            <div className="workflow-actions">
              <button className="button button--secondary" onClick={() => onUse(workflow.instruction)} type="button">
                Use
              </button>
            </div>
          </li>
        ))}
      </ul>
      {visibleSaved.length === 0 && visibleBuiltIn.length === 0 ? (
        <p className="provider-setup-note">No recipes match this filter.</p>
      ) : null}

      <div className="setting-row setting-row--input provider-picker-row">
        <div>
          <label htmlFor="workflow-name">Save the current instruction</label>
          <p>Stored locally in settings; runs through planning like any other instruction.</p>
        </div>
        <input
          id="workflow-name"
          maxLength={MAX_WORKFLOW_NAME_CHARS}
          onChange={(event) => setWorkflowName(event.currentTarget.value)}
          placeholder="Name this workflow…"
          spellCheck={false}
          type="text"
          value={workflowName}
        />
      </div>
      <button className="button button--secondary" disabled={!canSave} onClick={handleSave} type="button">
        Save current instruction
      </button>
    </section>
  );
}
