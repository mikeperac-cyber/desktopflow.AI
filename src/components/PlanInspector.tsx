import { useMemo, useState } from "react";

import type { ApprovalPolicy, ExecutionReport, PlanningResult } from "../types/settings";

function actionDetails(step: PlanningResult["plan"]["steps"][number]) {
  const details: string[] = [];
  if (step.target_id) details.push(step.target_id);
  if (step.text) details.push(`“${step.text}”`);
  if (step.keys.length) details.push(step.keys.join(" + "));
  if (step.scroll_direction && step.amount) details.push(`${step.scroll_direction} × ${step.amount}`);
  if (step.duration_ms) details.push(`${step.duration_ms} ms`);
  return details.join(" · ");
}

function verificationDetails(step: PlanningResult["plan"]["steps"][number]) {
  const verification = step.verification;
  const expected = verification.expected_text ?? (
    verification.expected_bool === null ? null : String(verification.expected_bool)
  );
  return [
    verification.kind.split("_").join(" "),
    verification.target_id,
    expected ? `“${expected}”` : null,
    `${verification.timeout_ms} ms`,
  ].filter(Boolean).join(" · ");
}

interface PlanInspectorProps {
  result: PlanningResult;
  compact?: boolean;
  execution?: ExecutionReport | null;
  executing?: boolean;
  executionError?: string | null;
  onExecute?: (approvedStepIds?: string[], customMaxSteps?: number) => void;
  onEmergencyStop?: () => void;
  approvalPolicy?: ApprovalPolicy;
  defaultMaxSteps?: number;
}

export function PlanInspector({
  result,
  compact = false,
  execution = null,
  executing = false,
  executionError = null,
  onExecute,
  onEmergencyStop,
  approvalPolicy = "balanced",
  defaultMaxSteps = 12,
}: PlanInspectorProps) {
  const { plan } = result;
  const [confirmationFor, setConfirmationFor] = useState<string | null>(null);
  const [approvedStepIds, setApprovedStepIds] = useState<string[]>([]);
  const [customMaxSteps, setCustomMaxSteps] = useState<number>(defaultMaxSteps);

  const confirming = confirmationFor === result.provider_request_id;

  const stepsRequiringApproval = useMemo(() => {
    if (plan.status !== "ready") return [];
    if (approvalPolicy === "always_ask") {
      return plan.steps;
    }
    return plan.steps.filter((s) => s.risk === "high" || s.requires_user_approval);
  }, [plan, approvalPolicy]);

  const unapprovedSteps = useMemo(() => {
    return stepsRequiringApproval.filter((step) => !approvedStepIds.includes(step.id));
  }, [stepsRequiringApproval, approvedStepIds]);

  const hasUnapprovedSteps = unapprovedSteps.length > 0;
  const budgetExceeded = plan.steps ? plan.steps.length > customMaxSteps : false;

  const toggleStepApproval = (stepId: string) => {
    setApprovedStepIds((prev) =>
      prev.includes(stepId) ? prev.filter((id) => id !== stepId) : [...prev, stepId]
    );
  };

  const approveAllRequired = () => {
    setApprovedStepIds(stepsRequiringApproval.map((s) => s.id));
  };

  return (
    <section className={`plan-inspector ${compact ? "plan-inspector--compact" : ""}`} aria-label="AI action plan">
      <header className="plan-heading">
        <div>
          <span className="eyebrow">
            {execution ? "Local execution report" : "Validated plan · Awaiting confirmation"}
          </span>
          <h2>{plan.title}</h2>
          <p>{plan.summary}</p>
        </div>
        <span className={`plan-risk plan-risk--${plan.overall_risk}`}>{plan.overall_risk} risk</span>
      </header>

      {plan.status === "ready" ? (
        <>
          <ol className="plan-steps">
            {plan.steps.map((step, index) => {
              const requiresApproval = stepsRequiringApproval.some((s) => s.id === step.id);
              const isApproved = approvedStepIds.includes(step.id);
              return (
                <li key={step.id}>
                  <span className="plan-step-number">{index + 1}</span>
                  <div className="plan-step-copy">
                    <div className="plan-step-title">
                      <strong>{step.kind.split("_").join(" ")}</strong>
                      <span className={`plan-risk plan-risk--${step.risk}`}>{step.risk}</span>
                      {step.requires_user_approval ? <span className="plan-approval">approval</span> : null}
                    </div>
                    <p>{step.description}</p>
                    {actionDetails(step) ? <code>{actionDetails(step)}</code> : null}
                    <small>Expected: {step.expected_result}</small>
                    <small>Verify: {verificationDetails(step)}</small>
                    {requiresApproval ? (
                      <label className="step-approval-checkbox">
                        <input
                          checked={isApproved}
                          disabled={executing}
                          onChange={() => toggleStepApproval(step.id)}
                          type="checkbox"
                        />
                        <span>Approve {step.risk === "high" ? "high-risk" : ""} action</span>
                      </label>
                    ) : null}
                  </div>
                </li>
              );
            })}
          </ol>

          {stepsRequiringApproval.length > 0 ? (
            <div className="plan-approval-summary" aria-label="Approval status">
              <span>
                {hasUnapprovedSteps
                  ? `${unapprovedSteps.length} of ${stepsRequiringApproval.length} actions require explicit approval.`
                  : `All ${stepsRequiringApproval.length} required actions approved.`}
              </span>
              {hasUnapprovedSteps ? (
                <button
                  className="button button--secondary"
                  disabled={executing}
                  onClick={approveAllRequired}
                  type="button"
                >
                  Approve all required
                </button>
              ) : null}
            </div>
          ) : null}

          <div className="plan-budget-row" aria-label="Action budget controls">
            <label htmlFor="plan-budget-slider">
              Action budget limit: <strong>{customMaxSteps}</strong> steps (Plan has {plan.steps.length})
            </label>
            <input
              id="plan-budget-slider"
              max={100}
              min={1}
              disabled={executing}
              onChange={(e) => setCustomMaxSteps(Number(e.currentTarget.value))}
              type="range"
              value={customMaxSteps}
            />
          </div>

          {budgetExceeded ? (
            <div className="inline-error" role="alert">
              Plan step count ({plan.steps.length}) exceeds the maximum autonomous step budget ({customMaxSteps}).
            </div>
          ) : null}
        </>
      ) : (
        <div className="plan-blocked" role="status">
          {plan.status === "needs_clarification" ? "The planner needs clarification." : "The request is outside the safe action vocabulary."}
        </div>
      )}

      {execution ? (
        <section className={`execution-report execution-report--${execution.status}`} aria-label="Execution report" role="status">
          <strong>
            {execution.status === "completed"
              ? execution.recovered
                ? `Recovered and verified ${execution.completed_steps} actions across ${execution.plan_attempts} plans`
                : `Verified ${execution.completed_steps} of ${execution.total_steps} actions`
              : execution.status === "cancelled"
                ? `Execution cancelled by user after ${execution.completed_steps} verified actions`
                : execution.status === "emergency_stopped"
                  ? `Emergency stop triggered · Released inputs after ${execution.completed_steps} verified actions`
                  : `Stopped safely after ${execution.completed_steps} verified actions`}
          </strong>
          {execution.replan_attempts > 0 ? (
            <p>{execution.replan_attempts} of 2 bounded replan attempts used.</p>
          ) : null}
          {execution.failure_message ? <p>{execution.failure_message}</p> : null}
          {execution.recovery_failure_message ? <p>{execution.recovery_failure_message}</p> : null}
          <ul>
            {execution.step_results.map((step) => (
              <li key={`${step.plan_attempt}-${step.id}`}>
                <span>{step.status === "completed" ? "✓" : step.status === "cancelled" ? "⊘" : "!"} {step.id}</span>
                <code>
                  {step.method ?? "no action"}
                  {step.verification ? ` · ${step.verification.method}` : ""}
                  {step.error ? ` · ${step.error}` : ""}
                </code>
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      {executionError ? <div className="inline-error" role="alert">{executionError}</div> : null}

      {onExecute && plan.status === "ready" && !execution ? (
        <div className="execution-controls">
          {executing ? (
            <div className="execution-active-bar">
              {onEmergencyStop ? (
                <button className="button button--danger" onClick={onEmergencyStop} type="button">
                  Emergency Stop (Esc)
                </button>
              ) : null}
            </div>
          ) : confirming ? (
            <div className="execution-confirmation">
              <p>
                DeskFlow will hide this window, revalidate and verify every action, then re-inspect and replan up to 2 times if the observed state changes. The total action limit still applies.
              </p>
              {hasUnapprovedSteps ? (
                <p className="inline-error" role="alert" style={{ marginTop: 8 }}>
                  Please approve all required actions before running.
                </p>
              ) : null}
              <div>
                <button className="button" disabled={executing} onClick={() => setConfirmationFor(null)} type="button">Cancel</button>
                <button
                  className="button button--primary"
                  disabled={executing || hasUnapprovedSteps || budgetExceeded}
                  onClick={() => onExecute(approvedStepIds, customMaxSteps)}
                  type="button"
                >
                  {executing ? "Running…" : `Confirm & run ${plan.steps.length} ${plan.steps.length === 1 ? "step" : "steps"}`}
                </button>
              </div>
            </div>
          ) : (
            <button
              className="button button--primary"
              disabled={budgetExceeded}
              onClick={() => setConfirmationFor(result.provider_request_id)}
              type="button"
            >
              Review and run plan
            </button>
          )}
        </div>
      ) : null}

      <footer className="plan-metadata">
        <span>{result.model}</span>
        <span>{result.observed_element_count} UI elements</span>
        <span>{result.screenshot_included ? "Screenshot included" : "UI tree only"}</span>
        <span>{result.usage.total_tokens.toLocaleString()} tokens</span>
      </footer>
    </section>
  );
}
