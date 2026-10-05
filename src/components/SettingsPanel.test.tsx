import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  captureActiveWindow,
  clearTargetHighlight,
  hideWindow,
  highlightUiElement,
  inspectTargetUi,
  loadAiProviderStatus,
  loadLastActionPlan,
  loadLastUiAutomation,
  loadLastWindowContext,
  loadRuntimeStatus,
  loadSettings,
  saveAiProviderCredential,
  saveSettings,
} from "../services/desktop";
import {
  DEFAULT_RUNTIME_STATUS,
  DEFAULT_SETTINGS,
  type UiAutomationSnapshot,
  type WindowContextSnapshot,
} from "../types/settings";
import { SettingsPanel } from "./SettingsPanel";

vi.mock("../services/desktop", () => ({
  captureActiveWindow: vi.fn(),
  clearTargetHighlight: vi.fn().mockResolvedValue(undefined),
  hideWindow: vi.fn().mockResolvedValue(undefined),
  highlightUiElement: vi.fn(),
  inspectTargetUi: vi.fn(),
  loadAiProviderStatus: vi.fn(),
  loadLastActionPlan: vi.fn(),
  loadLastUiAutomation: vi.fn(),
  loadLastWindowContext: vi.fn(),
  loadRuntimeStatus: vi.fn(),
  loadSettings: vi.fn(),
  saveAiProviderCredential: vi.fn(),
  deleteAiProviderCredential: vi.fn(),
  saveSettings: vi.fn(),
  toUserMessage: (error: unknown) => String(error),
}));

const capturedContext: WindowContextSnapshot = {
  captured_at_unix_ms: 1_800_000_000_000,
  window_handle: "0x1234",
  title: "Untitled - Notepad",
  class_name: "ApplicationFrameWindow",
  process: {
    id: 4242,
    name: "Notepad.exe",
    executable_path: "C:\\Windows\\System32\\Notepad.exe",
  },
  bounds_physical: { left: 100, top: 80, width: 1200, height: 800 },
  bounds_logical: { left: 80, top: 64, width: 960, height: 640 },
  dpi: 120,
  scale_factor: 1.25,
  monitor: {
    device_name: "\\\\.\\DISPLAY1",
    bounds_physical: { left: 0, top: 0, width: 1920, height: 1080 },
    work_area_physical: { left: 0, top: 0, width: 1920, height: 1040 },
    is_primary: true,
  },
  screenshot: {
    mime_type: "image/png",
    data_url: "data:image/png;base64,iVBORw0KGgo=",
    width_px: 1200,
    height_px: 800,
    byte_size: 2048,
    capture_method: "screen_bitblt",
  },
  warnings: [],
};

const inspectedUi: UiAutomationSnapshot = {
  captured_at_unix_ms: 1_800_000_000_500,
  target: { title: "Untitled - Notepad", process_name: "Notepad.exe", process_id: 4242 },
  root_id: "uia-0001",
  elements: [
    {
      id: "uia-0001",
      parent_id: null,
      depth: 0,
      name: "Untitled - Notepad",
      role: "window",
      automation_id: "",
      class_name: "ApplicationFrameWindow",
      framework_id: "Win32",
      bounds_physical: { left: 100, top: 80, width: 1200, height: 800 },
      is_enabled: true,
      is_offscreen: false,
      is_keyboard_focusable: true,
      has_keyboard_focus: false,
      is_password: false,
      supported_patterns: ["window", "transform"],
    },
    {
      id: "uia-0002",
      parent_id: "uia-0001",
      depth: 1,
      name: "Text editor",
      role: "document",
      automation_id: "TextArea",
      class_name: "RichEditD2DPT",
      framework_id: "Win32",
      bounds_physical: { left: 110, top: 130, width: 1180, height: 730 },
      is_enabled: true,
      is_offscreen: false,
      is_keyboard_focusable: true,
      has_keyboard_focus: true,
      is_password: false,
      supported_patterns: ["text", "value"],
    },
  ],
  visited_count: 4,
  filtered_count: 2,
  truncated: false,
  duration_ms: 18,
  limits: {
    max_depth: 12,
    max_visited: 1500,
    max_elements: 350,
    max_children_per_parent: 250,
    timeout_ms: 1500,
  },
  warnings: [],
};

describe("SettingsPanel", () => {
  beforeEach(() => {
    vi.mocked(loadSettings).mockResolvedValue({ ...DEFAULT_SETTINGS });
    vi.mocked(loadRuntimeStatus).mockResolvedValue({ ...DEFAULT_RUNTIME_STATUS });
    vi.mocked(loadLastWindowContext).mockResolvedValue(null);
    vi.mocked(loadLastUiAutomation).mockResolvedValue(null);
    vi.mocked(loadLastActionPlan).mockResolvedValue(null);
    vi.mocked(loadAiProviderStatus).mockResolvedValue({
      selected: "gemini",
      providers: [
        {
          provider: "gemini",
          label: "Google Gemini",
          description: "Google's native multimodal Interactions API.",
          notice: null,
          configured: false,
          credential_source: null,
          supports_screenshot: true,
          models: [
            { profile: "fast", id: "gemini-3.8-flash", label: "Gemini 3.8 Flash", stability: "stable" },
            { profile: "reasoning", id: "gemini-3.1-pro-preview", label: "Gemini 3.1 Pro", stability: "preview" },
          ],
        },
      ],
    });
    vi.mocked(saveSettings).mockResolvedValue({ ...DEFAULT_RUNTIME_STATUS });
    vi.mocked(captureActiveWindow).mockResolvedValue(capturedContext);
    vi.mocked(clearTargetHighlight).mockResolvedValue(undefined);
    vi.mocked(highlightUiElement).mockResolvedValue({
      element_id: "uia-0002",
      bounds_physical: { left: 110, top: 130, width: 1180, height: 730 },
    });
    vi.mocked(inspectTargetUi).mockResolvedValue(inspectedUi);
  });

  it("edits and saves the global shortcut", async () => {
    const user = userEvent.setup();
    render(<SettingsPanel />);

    const hotkey = await screen.findByLabelText("Open DeskFlow");
    await user.clear(hotkey);
    await user.type(hotkey, "Win + Shift + A");
    await user.click(screen.getByRole("button", { name: "Save settings" }));

    await waitFor(() => {
      expect(saveSettings).toHaveBeenCalledWith(
        expect.objectContaining({ global_hotkey: "Win + Shift + A" }),
      );
    });
    expect(await screen.findByText("Settings saved.")).toBeInTheDocument();
  });

  it("shows the native AI provider configuration and closes without saving", async () => {
    const user = userEvent.setup();
    render(<SettingsPanel />);

    await user.click(await screen.findByRole("button", { name: "AI" }));
    expect(await screen.findByRole("heading", { name: "Google Gemini" })).toBeInTheDocument();
    expect(screen.getByText("Needs API key")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Generate plan" })).toBeDisabled();

    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(hideWindow).toHaveBeenCalledWith("settings");
  });

  it("sends a credential once and clears the password field", async () => {
    const user = userEvent.setup();
    vi.mocked(saveAiProviderCredential).mockResolvedValue({
      selected: "gemini",
      providers: [{
        provider: "gemini",
        label: "Google Gemini",
        description: "Google's native multimodal Interactions API.",
        notice: null,
        configured: true,
        credential_source: "windows_credential_manager",
        supports_screenshot: true,
        models: [
          { profile: "fast", id: "gemini-3.8-flash", label: "Gemini 3.8 Flash", stability: "stable" },
          { profile: "reasoning", id: "gemini-3.1-pro-preview", label: "Gemini 3.1 Pro", stability: "preview" },
        ],
      }],
    });
    render(<SettingsPanel />);

    await user.click(await screen.findByRole("button", { name: "AI" }));
    const key = screen.getByLabelText("Google Gemini API key");
    await user.type(key, "test-secret-key");
    await user.click(screen.getByRole("button", { name: "Save key" }));

    await waitFor(() => expect(saveAiProviderCredential).toHaveBeenCalledWith("gemini", "test-secret-key"));
    expect(key).toHaveValue("");
    expect(screen.getByText("Stored in Windows Credential Manager")).toBeInTheDocument();
  });

  it("persists the selected provider before planning", async () => {
    const user = userEvent.setup();
    vi.mocked(loadAiProviderStatus).mockResolvedValue({
      selected: "gemini",
      providers: [
        {
          provider: "gemini",
          label: "Google Gemini",
          description: "Google's native multimodal Interactions API.",
          notice: null,
          configured: false,
          credential_source: null,
          supports_screenshot: true,
          models: [],
        },
        {
          provider: "openai",
          label: "OpenAI",
          description: "OpenAI Responses API with strict structured output.",
          notice: null,
          configured: true,
          credential_source: "windows_credential_manager",
          supports_screenshot: true,
          models: [
            { profile: "fast", id: "gpt-5.6-luna", label: "GPT 5.6 Luna", stability: "stable" },
            { profile: "reasoning", id: "gpt-5.6-sol", label: "GPT 5.6 Sol", stability: "stable" },
          ],
        },
      ],
    });
    render(<SettingsPanel />);

    await user.click(await screen.findByRole("button", { name: "AI" }));
    await user.selectOptions(screen.getByLabelText("Provider"), "openai");
    expect(screen.getByText("Save settings to make OpenAI active.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Generate plan" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Save settings" }));

    await waitFor(() => {
      expect(saveSettings).toHaveBeenCalledWith(expect.objectContaining({ ai_provider: "openai" }));
    });
    expect(screen.queryByText("Save settings to make OpenAI active.")).not.toBeInTheDocument();
  });

  it("captures and displays foreground window diagnostics", async () => {
    const user = userEvent.setup();
    render(<SettingsPanel />);

    await user.click(await screen.findByRole("button", { name: "Advanced" }));
    expect(screen.getByText("No context captured yet")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Capture active application" }));

    expect(await screen.findByText("Untitled - Notepad")).toBeInTheDocument();
    expect(screen.getAllByText("Notepad.exe")).toHaveLength(2);
    expect(screen.getByText("120 DPI · 1.25×")).toBeInTheDocument();
    expect(captureActiveWindow).toHaveBeenCalledOnce();
  });

  it("inspects and filters a normalized UI Automation tree", async () => {
    const user = userEvent.setup();
    vi.mocked(loadLastWindowContext).mockResolvedValue(capturedContext);
    render(<SettingsPanel />);

    await user.click(await screen.findByRole("button", { name: "Advanced" }));
    await user.click(screen.getByRole("button", { name: "Inspect captured UI" }));

    expect(await screen.findByText("Text editor")).toBeInTheDocument();
    expect(screen.getByText("included").parentElement).toHaveTextContent("2included");
    expect(screen.getByText("text")).toBeInTheDocument();
    expect(inspectTargetUi).toHaveBeenCalledOnce();

    await user.type(screen.getByRole("searchbox", { name: "Filter normalized elements" }), "document");
    expect(screen.queryByText("Untitled - Notepad", { selector: ".uia-node-main strong" })).not.toBeInTheDocument();
    expect(screen.getByText("Text editor")).toBeInTheDocument();
  });

  it("highlights a bounded inspector element and clears it on second selection", async () => {
    const user = userEvent.setup();
    vi.mocked(loadLastWindowContext).mockResolvedValue(capturedContext);
    vi.mocked(loadLastUiAutomation).mockResolvedValue(inspectedUi);
    render(<SettingsPanel />);

    await user.click(await screen.findByRole("button", { name: "Advanced" }));
    const target = screen.getByRole("button", { name: "Highlight Text editor" });
    await user.click(target);

    expect(highlightUiElement).toHaveBeenCalledWith("uia-0002");
    expect(await screen.findByRole("button", { name: "Clear highlight for Text editor" })).toHaveAttribute("aria-pressed", "true");

    await user.click(screen.getByRole("button", { name: "Clear highlight for Text editor" }));
    expect(clearTargetHighlight).toHaveBeenCalledOnce();
    expect(await screen.findByRole("button", { name: "Highlight Text editor" })).toHaveAttribute("aria-pressed", "false");
  });
});
