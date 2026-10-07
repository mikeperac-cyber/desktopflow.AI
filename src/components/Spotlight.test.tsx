import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { DEFAULT_RUNTIME_STATUS, DEFAULT_SETTINGS, type PlanningResult } from "../types/settings";
import {
  createActionPlan,
  emergencyStop,
  executeActionPlan,
  hideWindow,
  loadRuntimeStatus,
  loadSettings,
  setOverlayPlanMode,
} from "../services/desktop";
import { Spotlight } from "./Spotlight";

vi.mock("../services/desktop", () => ({
  createActionPlan: vi.fn(),
  emergencyStop: vi.fn().mockResolvedValue(true),
  cancelExecution: vi.fn().mockResolvedValue(true),
  executeActionPlan: vi.fn(),
  hideWindow: vi.fn().mockResolvedValue(undefined),
  loadRuntimeStatus: vi.fn(),
  loadSettings: vi.fn(),
  setOverlayPlanMode: vi.fn().mockResolvedValue(undefined),
  toUserMessage: (error: unknown) => String(error),
}));

const planningResult: PlanningResult = {
  created_at_unix_ms: 1,
  provider: "gemini",
  model: "gemini-3.8-flash",
  provider_request_id: "interaction-1",
  screenshot_included: false,
  observed_element_count: 2,
  usage: { total_input_tokens: 100, total_output_tokens: 30, total_thought_tokens: 10, total_tokens: 140 },
  plan: {
    status: "ready",
    title: "Open settings",
    summary: "Would open the settings control.",
    overall_risk: "low",
    steps: [{
      id: "step-1",
      kind: "invoke",
      target_id: "uia-0002",
      text: null,
      keys: [],
      scroll_direction: null,
      amount: null,
      duration_ms: null,
      description: "Invoke Settings",
      expected_result: "Settings opens",
      verification: {
        kind: "window_title_contains",
        target_id: null,
        expected_text: "Settings",
        expected_bool: null,
        timeout_ms: 1000,
      },
      risk: "low",
      requires_user_approval: false,
    }],
  },
};

const highRiskPlanningResult: PlanningResult = {
  ...planningResult,
  provider_request_id: "interaction-high-risk",
  plan: {
    status: "ready",
    title: "Delete file",
    summary: "Would delete a selected file.",
    overall_risk: "high",
    steps: [{
      id: "step-del",
      kind: "click",
      target_id: "uia-0005",
      text: null,
      keys: [],
      scroll_direction: null,
      amount: null,
      duration_ms: null,
      description: "Click Delete button",
      expected_result: "File is deleted",
      verification: {
        kind: "element_exists",
        target_id: "uia-0005",
        expected_text: null,
        expected_bool: false,
        timeout_ms: 1000,
      },
      risk: "high",
      requires_user_approval: true,
    }],
  },
};

describe("Spotlight", () => {
  beforeEach(() => {
    vi.mocked(loadSettings).mockResolvedValue({ ...DEFAULT_SETTINGS });
    vi.mocked(loadRuntimeStatus).mockResolvedValue({ ...DEFAULT_RUNTIME_STATUS });
    vi.mocked(createActionPlan).mockResolvedValue(planningResult);
    vi.mocked(executeActionPlan).mockResolvedValue({
      started_at_unix_ms: 2,
      finished_at_unix_ms: 3,
      status: "completed",
      total_steps: 1,
      completed_steps: 1,
      plan_attempts: 1,
      replan_attempts: 0,
      recovered: false,
      step_results: [{
        plan_attempt: 1,
        id: "step-1",
        kind: "invoke",
        target_id: "uia-0002",
        status: "completed",
        method: "uia_invoke",
        duration_ms: 5,
        verification: {
          kind: "window_title_contains",
          method: "win32_window_title",
          attempts: 1,
          duration_ms: 1,
        },
        error: null,
      }],
      failure_message: null,
      recovery_failure_message: null,
    });
  });

  it("requires a second confirmation before executing a validated plan", async () => {
    const user = userEvent.setup();
    render(<Spotlight />);

    const input = await screen.findByPlaceholderText("Ask DeskFlow what to do...");
    expect(input).toHaveFocus();
    await user.type(input, "Open settings{enter}");

    expect(await screen.findByText("Open settings", { selector: "h2" })).toBeInTheDocument();
    expect(screen.getByText(/Awaiting confirmation/)).toBeInTheDocument();
    expect(screen.getByText(/Review before running/)).toBeInTheDocument();
    expect(createActionPlan).toHaveBeenCalledWith("Open settings", "fast", false);
    expect(setOverlayPlanMode).toHaveBeenCalledWith(true);

    await user.click(screen.getByRole("button", { name: "Review and run plan" }));
    expect(executeActionPlan).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Confirm & run 1 step" }));

    expect(executeActionPlan).toHaveBeenCalledWith("interaction-1", "overlay");
    expect(await screen.findByText("Verified 1 of 1 actions")).toBeInTheDocument();
  });

  it("requires granular approval for high-risk actions before execution", async () => {
    const user = userEvent.setup();
    vi.mocked(createActionPlan).mockResolvedValue(highRiskPlanningResult);
    render(<Spotlight />);

    const input = await screen.findByPlaceholderText("Ask DeskFlow what to do...");
    await user.type(input, "Delete file{enter}");

    expect(await screen.findByText("Delete file", { selector: "h2" })).toBeInTheDocument();
    expect(screen.getByText(/1 of 1 actions require explicit approval/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Review and run plan" }));
    const confirmButton = screen.getByRole("button", { name: "Confirm & run 1 step" });
    expect(confirmButton).toBeDisabled();
    expect(screen.getByText("Please approve all required actions before running.")).toBeInTheDocument();

    const approvalCheckbox = screen.getByRole("checkbox", { name: "Approve high-risk action" });
    await user.click(approvalCheckbox);
    expect(confirmButton).toBeEnabled();

    await user.click(confirmButton);
    expect(executeActionPlan).toHaveBeenCalledWith(
      "interaction-high-risk",
      "overlay",
      ["step-del"],
      12,
    );
  });

  it("surfaces emergency stopped status copy and report", async () => {
    const user = userEvent.setup();
    vi.mocked(executeActionPlan).mockResolvedValue({
      started_at_unix_ms: 2,
      finished_at_unix_ms: 3,
      status: "emergency_stopped",
      total_steps: 1,
      completed_steps: 0,
      plan_attempts: 1,
      replan_attempts: 0,
      recovered: false,
      step_results: [],
      failure_message: "Emergency stop signal received",
      recovery_failure_message: null,
    });
    render(<Spotlight />);

    const input = await screen.findByPlaceholderText("Ask DeskFlow what to do...");
    await user.type(input, "Open settings{enter}");

    await user.click(await screen.findByRole("button", { name: "Review and run plan" }));
    await user.click(screen.getByRole("button", { name: "Confirm & run 1 step" }));

    expect(await screen.findByText("Emergency stopped · Synthetic inputs released")).toBeInTheDocument();
    expect(screen.getByText(/Emergency stop triggered · Released inputs after 0 verified actions/)).toBeInTheDocument();
  });

  it("closes the overlay when Escape is pressed while idle", async () => {
    const user = userEvent.setup();
    render(<Spotlight />);

    await user.keyboard("{Escape}");
    expect(hideWindow).toHaveBeenCalledWith("overlay");
    expect(emergencyStop).not.toHaveBeenCalled();
  });
});
