export type ThemePreference = "system" | "light" | "dark";
export type ApprovalPolicy = "balanced" | "always_ask";
export type PlanningModel = "fast" | "reasoning";
export type AiProviderKind =
  | "local"
  | "gemini"
  | "opencode_zen"
  | "opencode_go"
  | "openrouter"
  | "nvidia"
  | "openai"
  | "anthropic";

export interface AppSettings {
  theme: ThemePreference;
  global_hotkey: string;
  emergency_hotkey: string;
  launch_at_startup: boolean;
  highlight_targets: boolean;
  execution_delay_ms: number;
  approval_policy: ApprovalPolicy;
  maximum_autonomous_steps: number;
  ai_provider: AiProviderKind;
  ai_model: PlanningModel;
  screenshot_transmission: boolean;
  diagnostic_logging: boolean;
  developer_mode: boolean;
}

export interface RuntimeStatus {
  paused: boolean;
  executing: boolean;
  emergency_stopped: boolean;
  registered_hotkey: string | null;
  hotkey_warning: string | null;
  registered_emergency_hotkey?: string | null;
  emergency_hotkey_warning?: string | null;
  active_target: ActiveTargetSummary | null;
  context_warning: string | null;
}

export interface ActiveTargetSummary {
  title: string;
  process_name: string | null;
  process_id: number;
}

export interface PixelRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface LogicalRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface ProcessMetadata {
  id: number;
  name: string | null;
  executable_path: string | null;
}

export interface MonitorMetadata {
  device_name: string;
  bounds_physical: PixelRect;
  work_area_physical: PixelRect;
  is_primary: boolean;
}

export interface ScreenshotData {
  mime_type: string;
  data_url: string;
  width_px: number;
  height_px: number;
  byte_size: number;
  capture_method: string;
}

export interface WindowContextSnapshot {
  captured_at_unix_ms: number;
  window_handle: string;
  title: string;
  class_name: string;
  process: ProcessMetadata;
  bounds_physical: PixelRect;
  bounds_logical: LogicalRect;
  dpi: number;
  scale_factor: number;
  monitor: MonitorMetadata;
  screenshot: ScreenshotData;
  warnings: string[];
}

export interface UiAutomationLimits {
  max_depth: number;
  max_visited: number;
  max_elements: number;
  max_children_per_parent: number;
  timeout_ms: number;
}

export interface NormalizedUiElement {
  id: string;
  parent_id: string | null;
  depth: number;
  name: string;
  role: string;
  automation_id: string;
  class_name: string;
  framework_id: string;
  bounds_physical: PixelRect | null;
  is_enabled: boolean;
  is_offscreen: boolean;
  is_keyboard_focusable: boolean;
  has_keyboard_focus: boolean;
  is_password: boolean;
  supported_patterns: string[];
}

export interface UiAutomationSnapshot {
  captured_at_unix_ms: number;
  target: ActiveTargetSummary;
  root_id: string | null;
  elements: NormalizedUiElement[];
  visited_count: number;
  filtered_count: number;
  truncated: boolean;
  duration_ms: number;
  limits: UiAutomationLimits;
  warnings: string[];
}

export interface TargetHighlight {
  element_id: string;
  bounds_physical: PixelRect;
}

export interface CommandError {
  code: string;
  message: string;
}

export type PlanStatus = "ready" | "needs_clarification" | "unsupported";
export type RiskLevel = "low" | "medium" | "high";
export type ActionKind =
  | "focus"
  | "click"
  | "invoke"
  | "type_text"
  | "key_press"
  | "hotkey"
  | "scroll"
  | "select"
  | "toggle"
  | "wait";
export type VerificationKind =
  | "window_exists"
  | "window_title_contains"
  | "element_exists"
  | "has_keyboard_focus"
  | "value_equals"
  | "toggle_state"
  | "selection_state";

export interface VerificationSpec {
  kind: VerificationKind;
  target_id: string | null;
  expected_text: string | null;
  expected_bool: boolean | null;
  timeout_ms: number;
}

export interface PlannedAction {
  id: string;
  kind: ActionKind;
  target_id: string | null;
  text: string | null;
  keys: string[];
  scroll_direction: "up" | "down" | "left" | "right" | null;
  amount: number | null;
  duration_ms: number | null;
  description: string;
  expected_result: string;
  verification: VerificationSpec;
  risk: RiskLevel;
  requires_user_approval: boolean;
}

export interface ActionPlan {
  status: PlanStatus;
  title: string;
  summary: string;
  overall_risk: RiskLevel;
  steps: PlannedAction[];
}

export interface ProviderUsage {
  total_input_tokens: number;
  total_output_tokens: number;
  total_thought_tokens: number;
  total_tokens: number;
}

export interface PlanningResult {
  created_at_unix_ms: number;
  provider: string;
  model: string;
  provider_request_id: string;
  screenshot_included: boolean;
  observed_element_count: number;
  usage: ProviderUsage;
  plan: ActionPlan;
}

export type ExecutionStatus =
  | "completed"
  | "failed"
  | "verification_failed"
  | "cancelled"
  | "emergency_stopped";
export type ExecutionStepStatus =
  | "completed"
  | "failed"
  | "verification_failed"
  | "cancelled";

export interface VerificationEvidence {
  kind: VerificationKind;
  method: string;
  attempts: number;
  duration_ms: number;
}

export interface ExecutionStepResult {
  plan_attempt: number;
  id: string;
  kind: ActionKind;
  target_id: string | null;
  status: ExecutionStepStatus;
  method: string | null;
  duration_ms: number;
  verification: VerificationEvidence | null;
  error: string | null;
}

export interface ExecutionReport {
  started_at_unix_ms: number;
  finished_at_unix_ms: number;
  status: ExecutionStatus;
  total_steps: number;
  completed_steps: number;
  plan_attempts: number;
  replan_attempts: number;
  recovered: boolean;
  step_results: ExecutionStepResult[];
  failure_message: string | null;
  recovery_failure_message: string | null;
}

export interface ProviderModel {
  profile: PlanningModel;
  id: string;
  label: string;
  stability: string;
}

export interface ProviderStatus {
  provider: AiProviderKind;
  label: string;
  description: string;
  notice: string | null;
  configured: boolean;
  credential_source: string | null;
  requires_credential: boolean;
  supports_screenshot: boolean;
  models: ProviderModel[];
}

export interface ProviderCatalog {
  selected: AiProviderKind;
  providers: ProviderStatus[];
}

export interface RecordingStatus {
  recording: boolean;
  event_count: number;
  skipped_count: number;
  elapsed_ms: number;
  target_title: string;
}

export const DEFAULT_SETTINGS: AppSettings = {
  theme: "system",
  global_hotkey: "Alt+Space",
  emergency_hotkey: "Esc",
  launch_at_startup: false,
  highlight_targets: true,
  execution_delay_ms: 250,
  approval_policy: "balanced",
  maximum_autonomous_steps: 12,
  ai_provider: "local",
  ai_model: "fast",
  screenshot_transmission: false,
  diagnostic_logging: false,
  developer_mode: false,
};

export const DEFAULT_RUNTIME_STATUS: RuntimeStatus = {
  paused: false,
  executing: false,
  emergency_stopped: false,
  registered_hotkey: "Alt+Space",
  hotkey_warning: null,
  registered_emergency_hotkey: "Esc",
  emergency_hotkey_warning: null,
  active_target: null,
  context_warning: null,
};

export interface DiagnosticLogEntry {
  timestamp_unix_ms: number;
  level: string;
  category: string;
  message: string;
}

