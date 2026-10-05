import { invoke, isTauri } from "@tauri-apps/api/core";

import {
  DEFAULT_RUNTIME_STATUS,
  DEFAULT_SETTINGS,
  type AppSettings,
  type AiProviderKind,
  type CommandError,
  type ExecutionReport,
  type RuntimeStatus,
  type PlanningModel,
  type PlanningResult,
  type ProviderCatalog,
  type TargetHighlight,
  type UiAutomationSnapshot,
  type WindowContextSnapshot,
} from "../types/settings";

const BROWSER_SETTINGS_KEY = "deskflow-browser-settings";

export type AppView = "overlay" | "settings" | "highlight";
export type HideableAppView = Exclude<AppView, "highlight">;

export function getAppView(): AppView {
  const requested = new URLSearchParams(window.location.search).get("view");
  if (requested === "settings" || requested === "highlight") return requested;
  return "overlay";
}

export async function loadSettings(): Promise<AppSettings> {
  if (isTauri()) {
    return invoke<AppSettings>("get_app_settings");
  }

  const saved = window.localStorage.getItem(BROWSER_SETTINGS_KEY);
  if (!saved) {
    return structuredClone(DEFAULT_SETTINGS);
  }

  try {
    return { ...DEFAULT_SETTINGS, ...(JSON.parse(saved) as Partial<AppSettings>) };
  } catch {
    return structuredClone(DEFAULT_SETTINGS);
  }
}

export async function loadRuntimeStatus(): Promise<RuntimeStatus> {
  if (isTauri()) {
    return invoke<RuntimeStatus>("get_runtime_status");
  }
  return structuredClone(DEFAULT_RUNTIME_STATUS);
}

export async function loadLastWindowContext(): Promise<WindowContextSnapshot | null> {
  if (isTauri()) {
    return invoke<WindowContextSnapshot | null>("get_last_window_context");
  }
  return null;
}

export async function captureActiveWindow(): Promise<WindowContextSnapshot> {
  if (isTauri()) {
    return invoke<WindowContextSnapshot>("capture_active_window");
  }
  throw {
    code: "desktop_required",
    message: "Active-window capture is available in the DeskFlow desktop app.",
  } satisfies CommandError;
}

export async function loadLastUiAutomation(): Promise<UiAutomationSnapshot | null> {
  if (isTauri()) {
    return invoke<UiAutomationSnapshot | null>("get_last_ui_automation");
  }
  return null;
}

export async function inspectTargetUi(): Promise<UiAutomationSnapshot> {
  if (isTauri()) {
    return invoke<UiAutomationSnapshot>("inspect_target_ui");
  }
  throw {
    code: "desktop_required",
    message: "UI Automation inspection is available in the DeskFlow desktop app.",
  } satisfies CommandError;
}

export async function highlightUiElement(elementId: string): Promise<TargetHighlight> {
  if (isTauri()) {
    return invoke<TargetHighlight>("highlight_ui_element", { elementId });
  }
  throw {
    code: "desktop_required",
    message: "Target highlighting is available in the DeskFlow desktop app.",
  } satisfies CommandError;
}

export async function clearTargetHighlight(): Promise<void> {
  if (isTauri()) {
    await invoke("clear_target_highlight");
  }
}

const BROWSER_PROVIDER_CATALOG: ProviderCatalog = {
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
    {
      provider: "opencode_zen",
      label: "OpenCode Zen",
      description: "OpenCode's pay-as-you-go curated model gateway.",
      notice: null,
      configured: false,
      credential_source: null,
      supports_screenshot: true,
      models: [
        { profile: "fast", id: "gpt-5.6-luna", label: "GPT 5.6 Luna", stability: "stable" },
        { profile: "reasoning", id: "gpt-6-astra", label: "GPT 6 Astra", stability: "stable" },
      ],
    },
    {
      provider: "opencode_go",
      label: "OpenCode Go",
      description: "OpenCode's subscription gateway for coding-agent models.",
      notice: "OpenCode Go is intended for coding-agent traffic; use it only for compatible workflows.",
      configured: false,
      credential_source: null,
      supports_screenshot: false,
      models: [
        { profile: "fast", id: "glm-5.3-flash", label: "GLM-5.3 Flash", stability: "current" },
        { profile: "reasoning", id: "gpt-5.6-luna", label: "GPT 5.6 Luna", stability: "current" },
      ],
    },
    {
      provider: "openrouter",
      label: "OpenRouter",
      description: "OpenAI-compatible routing across supported model providers.",
      notice: null,
      configured: false,
      credential_source: null,
      supports_screenshot: true,
      models: [
        { profile: "fast", id: "openai/gpt-5.6-luna", label: "GPT 5.6 Luna", stability: "routed" },
        { profile: "reasoning", id: "openai/gpt-5.6-sol", label: "GPT 5.6 Sol", stability: "routed" },
      ],
    },
    {
      provider: "nvidia",
      label: "NVIDIA NIM",
      description: "NVIDIA hosted NIM inference with guided JSON output.",
      notice: null,
      configured: false,
      credential_source: null,
      supports_screenshot: true,
      models: [
        { profile: "fast", id: "qwen/qwen3.5-122b-a10b", label: "Qwen 3.5 122B · Fast", stability: "hosted" },
        { profile: "reasoning", id: "qwen/qwen3.5-122b-a10b", label: "Qwen 3.5 122B · Reasoning", stability: "hosted" },
      ],
    },
    {
      provider: "openai",
      label: "OpenAI",
      description: "OpenAI Responses API with strict structured output.",
      notice: null,
      configured: false,
      credential_source: null,
      supports_screenshot: true,
      models: [
        { profile: "fast", id: "gpt-5.6-luna", label: "GPT 5.6 Luna", stability: "stable" },
        { profile: "reasoning", id: "gpt-5.6-sol", label: "GPT 5.6 Sol", stability: "stable" },
      ],
    },
    {
      provider: "anthropic",
      label: "Anthropic",
      description: "Anthropic Messages API with schema-constrained output.",
      notice: null,
      configured: false,
      credential_source: null,
      supports_screenshot: true,
      models: [
        { profile: "fast", id: "claude-haiku-4-5", label: "Claude Haiku 4.5", stability: "stable" },
        { profile: "reasoning", id: "claude-sonnet-5-5", label: "Claude Sonnet 5.5", stability: "stable" },
      ],
    },
  ],
};

export async function loadAiProviderStatus(): Promise<ProviderCatalog> {
  if (isTauri()) {
    return invoke<ProviderCatalog>("get_ai_provider_status");
  }
  return structuredClone(BROWSER_PROVIDER_CATALOG);
}

export async function saveAiProviderCredential(
  provider: AiProviderKind,
  apiKey: string,
): Promise<ProviderCatalog> {
  if (isTauri()) {
    return invoke<ProviderCatalog>("save_ai_provider_credential", {
      request: { provider, api_key: apiKey },
    });
  }
  throw {
    code: "desktop_required",
    message: "Secure credentials can be saved only in the DeskFlow desktop app.",
  } satisfies CommandError;
}

export async function deleteAiProviderCredential(
  provider: AiProviderKind,
): Promise<ProviderCatalog> {
  if (isTauri()) {
    return invoke<ProviderCatalog>("delete_ai_provider_credential", { provider });
  }
  throw {
    code: "desktop_required",
    message: "Secure credentials can be removed only in the DeskFlow desktop app.",
  } satisfies CommandError;
}

export async function loadLastActionPlan(): Promise<PlanningResult | null> {
  if (isTauri()) {
    return invoke<PlanningResult | null>("get_last_action_plan");
  }
  return null;
}

export async function createActionPlan(
  instruction: string,
  model: PlanningModel,
  includeScreenshot: boolean,
): Promise<PlanningResult> {
  if (isTauri()) {
    return invoke<PlanningResult>("create_action_plan", {
      request: {
        instruction,
        model,
        include_screenshot: includeScreenshot,
      },
    });
  }
  throw {
    code: "desktop_required",
    message: "AI planning is available in the DeskFlow desktop app.",
  } satisfies CommandError;
}

export async function executeActionPlan(
  providerRequestId: string,
  surface: "overlay" | "settings",
): Promise<ExecutionReport> {
  if (isTauri()) {
    return invoke<ExecutionReport>("execute_action_plan", {
      request: {
        provider_request_id: providerRequestId,
        surface,
        confirmed: true,
      },
    });
  }
  throw {
    code: "desktop_required",
    message: "Plan execution is available only in the DeskFlow desktop app.",
  } satisfies CommandError;
}

export async function setOverlayPlanMode(expanded: boolean): Promise<void> {
  if (isTauri()) {
    await invoke("set_overlay_plan_mode", { expanded });
  }
}

export async function saveSettings(settings: AppSettings): Promise<RuntimeStatus> {
  if (isTauri()) {
    return invoke<RuntimeStatus>("update_app_settings", { settings });
  }

  window.localStorage.setItem(BROWSER_SETTINGS_KEY, JSON.stringify(settings));
  return {
    ...DEFAULT_RUNTIME_STATUS,
    registered_hotkey: settings.global_hotkey,
  };
}

export async function hideWindow(label: HideableAppView): Promise<void> {
  if (isTauri()) {
    await invoke("hide_window", { label });
  }
}

export function toUserMessage(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }
  if (error && typeof error === "object" && "message" in error) {
    return String((error as CommandError).message);
  }
  return "DeskFlow could not complete that request. Please try again.";
}
