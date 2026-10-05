import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { DEFAULT_RUNTIME_STATUS, DEFAULT_SETTINGS, type PlanningResult } from "../types/settings";
import {
  createActionPlan,
  executeActionPlan,
  hideWindow,
  loadRuntimeStatus,
  loadSettings,
  setOverlayPlanMode,
} from "../services/desktop";
import { Spotlight } from "./Spotlight";

vi.mock("../services/desktop", () => ({
  createActionPlan: vi.fn(),
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

  it("closes the overlay when Escape is pressed", async () => {
    const user = userEvent.setup();
    render(<Spotlight />);

    await user.keyboard("{Escape}");
    expect(hideWindow).toHaveBeenCalledWith("overlay");
  });
});
