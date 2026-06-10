import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export type DesktopHealth = {
  status: string;
  version: string;
  cwd: string;
};

export type DesktopContextSnapshot = {
  history_messages: number;
  history_tokens: number;
  tool_schema_tokens: number;
  memory_snapshot_tokens: number;
  total_estimated_tokens: number;
  max_context_tokens: number;
  usage_percent: number;
  stable_prefix_fingerprint: string;
  prompt_cache_cached_tokens: number;
  prompt_cache_miss_tokens: number;
  prompt_cache_hit_rate_percent: number;
  prompt_cache_diagnostic_count: number;
  prompt_cache_last_reason?: string | null;
  compact: DesktopCompactState;
};

export type DesktopWorkbenchSnapshot = {
  selected_project: string;
  project_map: DesktopProjectMapSnapshot;
  symbol_index: DesktopSymbolIndexSnapshot;
  runtime_context?: DesktopContextSnapshot | null;
};

export type DesktopProjectMapSnapshot = {
  available: boolean;
  source?: string | null;
  freshness: string;
  chars: number;
  truncated: boolean;
  content_preview: string;
};

export type DesktopSymbolIndexSnapshot = {
  schema_version: number;
  total_symbols: number;
  files: DesktopIndexedFile[];
  truncated: boolean;
};

export type DesktopIndexedFile = {
  path: string;
  hash: string;
  lines: number;
  summary: string;
  symbols: DesktopIndexedSymbol[];
};

export type DesktopIndexedSymbol = {
  name: string;
  kind: string;
  line: number;
  signature: string;
};

export type DesktopCompactState = {
  compression_count: number;
  circuit_open: boolean;
  latest_strategy?: string | null;
  latest_boundary_id?: string | null;
  latest_attempt_decision?: string | null;
  latest_attempt_reason?: string | null;
  latest_attempt_trigger?: string | null;
  latest_attempt_tokens_before?: number | null;
  latest_attempt_tokens_after?: number | null;
};

export type DesktopCompactionAttempt = {
  trigger: string;
  strategy: string;
  decision: string;
  before_tokens: number;
  after_tokens?: number | null;
  messages_before: number;
  messages_after?: number | null;
  reason: string;
  attempt_index: number;
  consecutive_no_gain: number;
  consecutive_failures: number;
  circuit_open: boolean;
  boundary_id?: string | null;
};

export type SelectedProject = {
  path: string;
};

export type RecentSession = {
  id: string;
  title: string;
  updated_at: string;
  model: string;
  message_count: number;
};

export type DesktopMessage = {
  id: number;
  role: string;
  content: string;
  created_at: string;
};

export type DesktopCompactBoundary = {
  boundary_id: string;
  strategy: string;
  trigger: string;
  before_tokens: number;
  after_tokens: number;
  messages_before: number;
  messages_after: number;
  summary: string;
  created_at: string;
};

export type ResumedSession = {
  session_id: string;
  messages: DesktopMessage[];
  compact_boundaries: DesktopCompactBoundary[];
  session_parts: DesktopSessionPart[];
};

export type DesktopSessionPart = {
  id: number;
  part_index: number;
  part_id: string;
  kind: string;
  tool_call_id?: string | null;
  tool_name?: string | null;
  status?: string | null;
  payload: Record<string, unknown>;
  projected_to_seq: number;
  updated_at: string;
};

export type DesktopSessionRevertRecord = {
  id: number;
  session_id: string;
  operation: string;
  status: string;
  message_id?: string | null;
  target_part_id?: string | null;
  part_ids: string[];
  checkpoint_ids: string[];
  snapshot_checkpoint_id?: string | null;
  paths: string[];
  restored_files: string[];
  removed_files: string[];
  errors: string[];
  diff_summary?: string | null;
  unrevert_possible: boolean;
  unreverted: boolean;
  payload: Record<string, unknown>;
  created_at: string;
};

export type DesktopToolOutputPage = {
  id: string;
  uri: string;
  tool_name: string;
  mime: string;
  content: string;
  offset: number;
  limit: number;
  total_bytes: number;
  has_more: boolean;
};

export type DesktopToolOutputMeta = {
  id: string;
  uri: string;
  tool_call_id: string;
  tool_name: string;
  mime: string;
  original_bytes: number;
  created_at_ms: number;
};

export type DesktopRevertResult = {
  session_id: string;
  status: string;
  message_id?: string | null;
  part_ids: string[];
  tool_round_id?: string | null;
  file_change_ids: string[];
  checkpoint_ids: string[];
  paths: string[];
  restored_files: string[];
  removed_files: string[];
  errors: string[];
  change_count: number;
};

export type DesktopSettings = {
  selected_project: string;
  active_session_id?: string | null;
  permission_mode: PermissionModeId;
  detail_level: DetailLevelId;
  agent_mode: AgentModeId;
  provider_name?: string | null;
  model?: string | null;
  settings_path: string;
  diagnostic_logs_path: string;
  recent_projects: string[];
  archived_session_ids: string[];
  startup_state: DesktopStartupState;
};

export type DesktopStartupState = {
  status: string;
  detail: string;
};

export type DetailLevelId = "coding" | "daily";

export type AgentModeId = "auto" | "build" | "plan" | "explore" | "review";

export type AgentModeOption = {
  id: AgentModeId;
  label: string;
  description: string;
};

export type PermissionModeId = "default" | "auto_low_risk" | "auto" | "read_only";

export type PermissionModeOption = {
  id: PermissionModeId;
  label: string;
  description: string;
};

export type DiagnosticStatus = "ok" | "warning" | "error";

export type DesktopDiagnostic = {
  id: string;
  label: string;
  status: DiagnosticStatus;
  detail: string;
};

export type DesktopDiagnosticsResponse = {
  items: DesktopDiagnostic[];
};

export type DesktopExportResult = {
  session_id: string;
  path: string;
  format: string;
  privacy: string;
};

export type ProviderSetupInfo = {
  shell_profile_path: string;
  provider_env_vars: string[];
  example: string;
};

export type ProviderModelStatus = {
  active_provider?: string | null;
  active_provider_label?: string | null;
  active_model: string;
  active_base_url: string;
  runtime_model?: string | null;
  runtime_provider_ready: boolean;
  selection_source: string;
  configured_count: number;
  providers: DesktopProviderOption[];
  models: DesktopModelOption[];
};

export type DesktopCurrentDiffContextDetail = {
  type: "current_diff";
  label: string;
  shortstat: string;
  files: string[];
  stat: string;
  patch_preview: string;
  truncated: boolean;
};

export type DesktopFileContextDetail = {
  type: "file";
  label: string;
  path: string;
  relative_path: string;
  size_bytes: number;
  line_count: number;
  line_start?: number | null;
  line_end?: number | null;
  preview: string;
  truncated: boolean;
};

export type DesktopRunContextDetail =
  | DesktopCurrentDiffContextDetail
  | DesktopFileContextDetail;

export type DesktopRunContext =
  | {
      type: "current_diff";
      label: string;
      detail?: DesktopRunContextDetail | null;
    }
  | {
      type: "file";
      label: string;
      path: string;
      line_start?: number | null;
      line_end?: number | null;
      selection_text?: string | null;
      detail?: DesktopRunContextDetail | null;
    };

export type DesktopProviderOption = {
  id: string;
  label: string;
  provider_type: string;
  model: string;
  base_url: string;
  configured: boolean;
  active: boolean;
  note: string;
};

export type DesktopModelOption = {
  id: string;
  label: string;
  provider_id: string;
  active: boolean;
  note: string;
};

export type DesktopRuntimeDiagnostic = {
  schema?: string;
  task_state?: Record<string, unknown>;
  verification_proof?: Record<string, unknown>;
  control_loop?: Record<string, unknown>;
  [key: string]: unknown;
};

export type DesktopRunEvent =
  | { type: "run_started"; run_id: string; session_id?: string | null }
  | { type: "assistant_delta"; text: string }
  | { type: "thinking_started" }
  | { type: "thinking_delta"; text: string }
  | { type: "thinking_completed" }
  | { type: "tool_started"; id: string; name: string }
  | { type: "tool_args_delta"; id: string; delta: string }
  | { type: "tool_call_completed"; id: string }
  | { type: "tool_execution_progress"; id: string; progress: string }
  | { type: "tool_completed"; id: string; result_preview: string; metadata?: unknown }
  | {
      type: "permission_request";
      id: string;
      tool_name: string;
      arguments: unknown;
      prompt: string;
      metadata?: unknown;
      review?: unknown;
    }
  | {
      type: "usage";
      prompt_tokens: number;
      completion_tokens: number;
      reasoning_tokens?: number | null;
      cached_tokens?: number | null;
    }
  | { type: "runtime_diagnostic"; diagnostic: DesktopRuntimeDiagnostic }
  | { type: "closeout"; status: string; evidence_summary?: string | null }
  | { type: "run_completed" }
  | { type: "output_truncated" }
  | { type: "run_error"; message: string };

const webPreviewListeners = new Set<(event: DesktopRunEvent) => void>();
let webPreviewSettings: DesktopSettings = {
  selected_project: "/Users/georgexu/Desktop/rust-agent",
  active_session_id: "web-preview",
  permission_mode: "auto",
  detail_level: "coding",
  agent_mode: "auto",
  provider_name: "minimax",
  model: "MiniMax-M3",
  settings_path: "web-preview",
  diagnostic_logs_path: "web-preview/logs/desktop.log",
  recent_projects: ["/Users/georgexu/Desktop/rust-agent", "/Users/georgexu/Desktop/bioclaw"],
  archived_session_ids: [],
  startup_state: {
    status: "restored_session",
    detail: "Restored web-preview in rust-agent",
  },
};
let webPreviewSessions: RecentSession[] = [
  {
    id: "web-preview",
    title: "Desktop app Phase 1",
    updated_at: "preview",
    model: "web-preview",
    message_count: 2,
  },
  {
    id: "web-preview-release",
    title: "Release readiness notes",
    updated_at: "preview",
    model: "web-preview",
    message_count: 5,
  },
];
let webPreviewArchivedSessions: RecentSession[] = [];

// ══ DTO types aligned with src/api/dto/* ──────────────────────

export type ProviderCatalogEntry = {
  provider_id: string;
  label: string;
  enabled: boolean;
  source: string;
  base_url_host: string;
  default_model: string;
  available_model_ids: string[];
  context_limit: number | null;
  output_limit: number | null;
  protocol_family: string;
  supports_streaming: boolean;
  requires_nonstreaming: boolean;
  last_health_status: string | null;
  last_latency_ms: number | null;
  recent_timeout_category: string | null;
};

export type FileMutationResult = {
  operation: string;
  changed_paths: string[];
  checkpoint_id: string | null;
  diff_preview: string | null;
  additions: number;
  deletions: number;
  stale_state: string | null;
  diagnostics_delta: unknown;
  rollback_status: string | null;
  error_hint: string | null;
};

export type SessionJobItem = {
  job_id: string;
  session_id: string;
  command: string;
  cwd: string | null;
  status: string;
  started_at: string;
  completed_at: string | null;
  exit_code: number | null;
  timed_out: boolean;
  tool_output_uri: string | null;
  cancelled: boolean;
};

export type SessionContext = {
  session_id: string;
  compact_boundary_id: string | null;
  estimated_history_tokens: number;
  tool_schema_tokens: number;
  memory_snapshot_tokens: number;
  stable_prefix_hash: string | null;
  dynamic_tail_hash: string | null;
  latest_compaction: CompactionSummary | null;
  message_count_after_compaction: number;
};

export type CompactionSummary = {
  boundary_id: string;
  strategy: string;
  trigger: string;
  before_tokens: number;
  after_tokens: number;
  messages_before: number;
  messages_after: number;
  preserved_tail_count: number;
};

export type SessionRunStatus = {
  session_id: string;
  status: string;
};

export function desktopHealth(): Promise<DesktopHealth> {
  if (!isTauriRuntime()) {
    return Promise.resolve({
      status: "web-preview",
      version: "0.1.0",
      cwd: "/Users/georgexu/Desktop/rust-agent",
    });
  }

  return invoke("desktop_health");
}

export function desktopContextSnapshot(): Promise<DesktopContextSnapshot> {
  if (!isTauriRuntime()) {
    return Promise.resolve({
      history_messages: webPreviewSessions.find((session) => session.id === webPreviewSettings.active_session_id)?.message_count || 0,
      history_tokens: 1200,
      tool_schema_tokens: 2200,
      memory_snapshot_tokens: 0,
      total_estimated_tokens: 3400,
      max_context_tokens: 128000,
      usage_percent: 3,
      stable_prefix_fingerprint: "web-preview",
      prompt_cache_cached_tokens: 16,
      prompt_cache_miss_tokens: 112,
      prompt_cache_hit_rate_percent: 12.5,
      prompt_cache_diagnostic_count: 1,
      prompt_cache_last_reason: "cold-start",
      compact: {
        compression_count: 0,
        circuit_open: false,
      },
    });
  }

  return invoke("desktop_context_snapshot");
}

export function desktopWorkbenchSnapshot(): Promise<DesktopWorkbenchSnapshot> {
  if (!isTauriRuntime()) {
    return Promise.resolve({
      selected_project: webPreviewSettings.selected_project,
      project_map: {
        available: true,
        source: `${webPreviewSettings.selected_project}/docs/PROJECT_MAP.md`,
        freshness: "current",
        chars: 2974,
        truncated: false,
        content_preview:
          "Project map source: docs/PROJECT_MAP.md\nFreshness: current\nPolicy: use this as navigation before broad repo scans; verify exact code with file_read/symbol_query before editing.\n\n## Runtime Navigation Contract\n\n- Start here for orientation before broad repository scans.\n- Use `project_list` action `map` or the injected project-map zone to pick likely files.\n- Use `symbol_query` for functions, structs, traits, enums, and impls before broad `grep`.",
      },
      symbol_index: {
        schema_version: 1,
        total_symbols: 2148,
        truncated: true,
        files: [
          {
            path: "src/engine/conversation_loop/request_preparation_controller.rs",
            hash: "cfbbe16c9a1f4024669ab6d3d9210c2f",
            lines: 1908,
            summary: "42 symbols: struct RequestPreparationController, function prepare, function inject_project_map_zone",
            symbols: [
              {
                name: "RequestPreparationController",
                kind: "struct",
                line: 47,
                signature: "pub(super) struct RequestPreparationController;",
              },
              {
                name: "inject_project_map_zone",
                kind: "function",
                line: 466,
                signature: "fn inject_project_map_zone(",
              },
            ],
          },
          {
            path: "src/tools/project_tool/mod.rs",
            hash: "930b1d540c135a768a0f8d3d716acccb",
            lines: 736,
            summary: "31 symbols: struct ProjectScanner, struct ProjectListTool, function execute",
            symbols: [
              {
                name: "ProjectScanner",
                kind: "struct",
                line: 95,
                signature: "pub struct ProjectScanner {",
              },
              {
                name: "ProjectListTool",
                kind: "struct",
                line: 327,
                signature: "pub struct ProjectListTool;",
              },
            ],
          },
        ],
      },
      runtime_context: {
        history_messages: webPreviewSessions.find((session) => session.id === webPreviewSettings.active_session_id)?.message_count || 0,
        history_tokens: 1200,
        tool_schema_tokens: 2200,
        memory_snapshot_tokens: 0,
        total_estimated_tokens: 3400,
        max_context_tokens: 128000,
        usage_percent: 3,
        stable_prefix_fingerprint: "web-preview",
        prompt_cache_cached_tokens: 16,
        prompt_cache_miss_tokens: 112,
        prompt_cache_hit_rate_percent: 12.5,
        prompt_cache_diagnostic_count: 1,
        prompt_cache_last_reason: "cold-start",
        compact: {
          compression_count: 0,
          circuit_open: false,
        },
      },
    });
  }

  return invoke("desktop_workbench_snapshot");
}

export function selectProject(path: string): Promise<SelectedProject> {
  if (!isTauriRuntime()) {
    webPreviewSettings = {
      ...webPreviewSettings,
      selected_project: path,
      active_session_id: null,
      recent_projects: [path, ...webPreviewSettings.recent_projects.filter((project) => project !== path)].slice(0, 8),
      startup_state: {
        status: "new_conversation",
        detail: `Ready for a new conversation in ${basename(path)}`,
      },
    };
    return Promise.resolve({ path });
  }

  return invoke("select_project", { path });
}

export function desktopSettings(): Promise<DesktopSettings> {
  if (!isTauriRuntime()) {
    return Promise.resolve(webPreviewSettings);
  }

  return invoke("desktop_settings");
}

export function newConversation(): Promise<DesktopSettings> {
  if (!isTauriRuntime()) {
    webPreviewSettings = {
      ...webPreviewSettings,
      active_session_id: null,
      startup_state: {
        status: "new_conversation",
        detail: `Ready for a new conversation in ${basename(webPreviewSettings.selected_project)}`,
      },
    };
    return Promise.resolve(webPreviewSettings);
  }

  return invoke("new_conversation");
}

export function setPermissionMode(mode: PermissionModeId): Promise<DesktopSettings> {
  if (!isTauriRuntime()) {
    webPreviewSettings = { ...webPreviewSettings, permission_mode: mode };
    return Promise.resolve(webPreviewSettings);
  }

  return invoke("set_permission_mode", { mode });
}

export function setDetailLevel(level: DetailLevelId): Promise<DesktopSettings> {
  if (!isTauriRuntime()) {
    webPreviewSettings = { ...webPreviewSettings, detail_level: level };
    return Promise.resolve(webPreviewSettings);
  }

  return invoke("set_detail_level", { level });
}

export function setAgentMode(mode: AgentModeId): Promise<DesktopSettings> {
  if (!isTauriRuntime()) {
    webPreviewSettings = { ...webPreviewSettings, agent_mode: mode };
    return Promise.resolve(webPreviewSettings);
  }

  return invoke("set_agent_mode", { mode });
}

export function agentModeOptions(): Promise<AgentModeOption[]> {
  if (!isTauriRuntime()) {
    return Promise.resolve([
      { id: "auto", label: "Auto", description: "Let the agent choose the right mode" },
      { id: "build", label: "Build", description: "Full coding — read, edit, shell, validation" },
      { id: "plan", label: "Plan", description: "Explore and plan — no file changes" },
      { id: "explore", label: "Explore", description: "Read and search — no edits" },
      { id: "review", label: "Review", description: "Diff analysis and findings — no edits" },
    ]);
  }

  return invoke("agent_mode_options");
}

export function permissionModeOptions(): Promise<PermissionModeOption[]> {
  if (!isTauriRuntime()) {
    return Promise.resolve([
      {
        id: "default",
        label: "Ask every time",
        description: "Ask before tool actions that require approval.",
      },
      {
        id: "auto_low_risk",
        label: "Auto low risk",
        description: "Allow low-risk read/search actions and ask for writes.",
      },
      {
        id: "auto",
        label: "Developer auto",
        description: "Allow normal development actions while guarding high-risk operations.",
      },
      {
        id: "read_only",
        label: "Read only",
        description: "Hide write tools and only allow read-oriented work.",
      },
    ]);
  }

  return invoke("permission_mode_options");
}

export function desktopDiagnostics(): Promise<DesktopDiagnosticsResponse> {
  if (!isTauriRuntime()) {
    return Promise.resolve({
      items: [
        {
          id: "provider_keys",
          label: "Provider keys",
          status: "error",
          detail: "Web preview cannot inspect local provider keys.",
        },
        {
          id: "project_access",
          label: "Project access",
          status: "ok",
          detail: "Preview project path is available.",
        },
        {
          id: "diagnostic_logs",
          label: "Diagnostic logs",
          status: "ok",
          detail: "Preview diagnostic logs are available.",
        },
      ],
    });
  }

  return invoke("desktop_diagnostics");
}

export function exportSession(
  sessionId?: string | null,
  format: "json" | "markdown" = "markdown",
  privacy: "full" | "redacted" | "summary" = "redacted",
): Promise<DesktopExportResult> {
  if (!isTauriRuntime()) {
    return Promise.resolve({
      session_id: sessionId || "preview-session",
      path: `${webPreviewSettings.selected_project}/session-preview.md`,
      format,
      privacy,
    });
  }

  return invoke("export_session", {
    sessionId: sessionId || null,
    format,
    privacy,
  });
}

export function desktopRunContextDetail(
  context: DesktopRunContext,
): Promise<DesktopRunContextDetail> {
  if (!isTauriRuntime()) {
    if (context.type === "file") {
      return Promise.resolve({
        type: "file",
        label: context.label,
        path: context.path,
        relative_path: "apps/desktop/src/app/App.tsx",
        size_bytes: 18422,
        line_count: 584,
        preview:
          "import { FormEvent, useEffect, useState, type ReactNode } from \"react\";\n\nexport function App() {\n  return <DesktopShell />;\n}\n",
        truncated: false,
      });
    }

    return Promise.resolve({
      type: "current_diff",
      label: context.label,
      shortstat: "unstaged:\n2 files changed, 42 insertions(+), 8 deletions(-)",
      files: ["apps/desktop/src/app/components/Composer.tsx", "apps/desktop/src/styles/global.css"],
      stat:
        "unstaged:\n apps/desktop/src/app/components/Composer.tsx | 24 +++++++++++++++++++-----\n apps/desktop/src/styles/global.css             | 18 ++++++++++++++++--",
      patch_preview:
        "@@ -1,3 +1,6 @@\n+type DesktopRunContextDetail = {\n+  patch_preview: string;\n+};\n",
      truncated: false,
    });
  }

  return invoke("desktop_run_context_detail", { context });
}

export function providerSetupInfo(): Promise<ProviderSetupInfo> {
  if (!isTauriRuntime()) {
    return Promise.resolve({
      shell_profile_path: "~/.zshrc",
      provider_env_vars: [
        "MINIMAX_API_KEY",
        "KIMI_CODE_API_KEY",
        "DEEPSEEK_API_KEY",
        "GLM_API_KEY",
        "ZAI_API_KEY",
        "ZHIPUAI_API_KEY",
        "BIGMODEL_API_KEY",
        "MOONSHOT_API_KEY",
        "OPENAI_API_KEY",
      ],
      example: 'export MINIMAX_API_KEY="your-key-here"',
    });
  }

  return invoke("provider_setup_info");
}

export function providerModelStatus(): Promise<ProviderModelStatus> {
  if (!isTauriRuntime()) {
    return Promise.resolve({
      active_provider: "minimax",
      active_provider_label: "MiniMax",
      active_model: "MiniMax-M3",
      active_base_url: "https://api.minimax.io/v1",
      runtime_model: "MiniMax-M3",
      runtime_provider_ready: true,
      selection_source: "preview",
      configured_count: 1,
      providers: [
        {
          id: "minimax",
          label: "MiniMax",
          provider_type: "Minimax",
          model: "MiniMax-M3",
          base_url: "https://api.minimax.io/v1",
          configured: true,
          active: true,
          note: "current",
        },
        {
          id: "kimi-code",
          label: "Kimi Code",
          provider_type: "KimiCode",
          model: "kimi-for-coding",
          base_url: "",
          configured: false,
          active: false,
          note: "missing KIMI_CODE_API_KEY",
        },
        {
          id: "deepseek",
          label: "DeepSeek",
          provider_type: "DeepSeek",
          model: "deepseek-v4-pro",
          base_url: "",
          configured: false,
          active: false,
          note: "missing DEEPSEEK_API_KEY",
        },
        {
          id: "glm",
          label: "GLM",
          provider_type: "Glm",
          model: "glm-5.1",
          base_url: "",
          configured: false,
          active: false,
          note: "missing GLM_API_KEY or ZAI_API_KEY or ZHIPUAI_API_KEY or BIGMODEL_API_KEY",
        },
        {
          id: "openai",
          label: "OpenAI",
          provider_type: "OpenAI",
          model: "gpt-4o",
          base_url: "",
          configured: false,
          active: false,
          note: "missing OPENAI_API_KEY",
        },
        {
          id: "kimi",
          label: "Kimi",
          provider_type: "Kimi",
          model: "kimi-k2.5",
          base_url: "",
          configured: false,
          active: false,
          note: "missing MOONSHOT_API_KEY",
        },
      ],
      models: [
        {
          id: "MiniMax-M3",
          label: "MiniMax-M3",
          provider_id: "minimax",
          active: false,
          note: "latest generation",
        },
        {
          id: "MiniMax-M2.7",
          label: "MiniMax-M2.7",
          provider_id: "minimax",
          active: true,
          note: "current",
        },
        {
          id: "MiniMax-M2.7-highspeed",
          label: "MiniMax-M2.7-highspeed",
          provider_id: "minimax",
          active: false,
          note: "takes effect next request",
        },
      ],
    });
  }

  return invoke("provider_model_status");
}

export function setProviderModel(providerId: string, model: string): Promise<ProviderModelStatus> {
  if (!isTauriRuntime()) {
    return providerModelStatus().then((status) => ({
      ...status,
      active_provider: providerId,
      active_provider_label: status.providers.find((provider) => provider.id === providerId)?.label ?? providerId,
      active_model: model,
      active_base_url: status.providers.find((provider) => provider.id === providerId)?.base_url ?? "",
      runtime_model: model,
      runtime_provider_ready: true,
      selection_source: "desktop_settings",
      providers: status.providers.map((provider) => ({
        ...provider,
        active: provider.id === providerId,
        note: provider.id === providerId ? "current" : provider.note,
      })),
      models: status.models.map((option) => ({
        ...option,
        active: option.id === model,
        note: option.id === model ? "current" : option.note,
      })),
    }));
  }

  return invoke("set_provider_model", { providerId, model });
}

export function openSettingsFolder(): Promise<void> {
  if (!isTauriRuntime()) {
    return Promise.resolve();
  }

  return invoke("open_settings_folder");
}

export function openDiagnosticsFolder(): Promise<void> {
  if (!isTauriRuntime()) {
    return Promise.resolve();
  }

  return invoke("open_diagnostics_folder");
}

export function openShellProfile(): Promise<void> {
  if (!isTauriRuntime()) {
    return Promise.resolve();
  }

  return invoke("open_shell_profile");
}

export function saveProviderCredential(providerId: string, key: string): Promise<string> {
  return invoke("save_provider_credential", { providerId, key });
}

export function listRecentSessions(limit = 20): Promise<RecentSession[]> {
  if (!isTauriRuntime()) {
    return Promise.resolve(webPreviewSessions.slice(0, limit));
  }

  return invoke("list_recent_sessions", { limit });
}

export function searchSessions(query: string, limit = 20): Promise<RecentSession[]> {
  if (!isTauriRuntime()) {
    const needle = query.trim().toLocaleLowerCase();
    if (!needle) {
      return listRecentSessions(limit);
    }
    return Promise.resolve(
      webPreviewSessions
        .filter((session) =>
          [session.title, session.id, session.model].join(" ").toLocaleLowerCase().includes(needle),
        )
        .slice(0, limit),
    );
  }

  return invoke("search_sessions", { query, limit });
}

export function renameSession(sessionId: string, title: string): Promise<RecentSession> {
  if (!isTauriRuntime()) {
    webPreviewSessions = webPreviewSessions.map((session) =>
      session.id === sessionId ? { ...session, title } : session,
    );
    const renamed = webPreviewSessions.find((session) => session.id === sessionId);
    if (!renamed) {
      return Promise.reject(new Error(`session not found: ${sessionId}`));
    }
    return Promise.resolve(renamed);
  }

  return invoke("rename_session", { sessionId, title });
}

export function archiveSession(sessionId: string): Promise<DesktopSettings> {
  if (!isTauriRuntime()) {
    const archived = webPreviewSessions.find((session) => session.id === sessionId);
    webPreviewSessions = webPreviewSessions.filter((session) => session.id !== sessionId);
    if (archived && !webPreviewArchivedSessions.some((session) => session.id === sessionId)) {
      webPreviewArchivedSessions = [archived, ...webPreviewArchivedSessions];
    }
    webPreviewSettings = {
      ...webPreviewSettings,
      active_session_id:
        webPreviewSettings.active_session_id === sessionId
          ? null
          : webPreviewSettings.active_session_id,
      archived_session_ids: [...webPreviewSettings.archived_session_ids, sessionId],
    };
    return Promise.resolve(webPreviewSettings);
  }

  return invoke("archive_session", { sessionId });
}

export function restoreArchivedSession(sessionId: string): Promise<DesktopSettings> {
  if (!isTauriRuntime()) {
    const restored = webPreviewArchivedSessions.find((session) => session.id === sessionId);
    webPreviewArchivedSessions = webPreviewArchivedSessions.filter((session) => session.id !== sessionId);
    if (restored && !webPreviewSessions.some((session) => session.id === sessionId)) {
      webPreviewSessions = [restored, ...webPreviewSessions];
    }
    webPreviewSettings = {
      ...webPreviewSettings,
      archived_session_ids: webPreviewSettings.archived_session_ids.filter((id) => id !== sessionId),
    };
    return Promise.resolve(webPreviewSettings);
  }

  return invoke("restore_archived_session", { sessionId });
}

export function deleteSession(sessionId: string): Promise<DesktopSettings> {
  if (!isTauriRuntime()) {
    webPreviewSessions = webPreviewSessions.filter((session) => session.id !== sessionId);
    webPreviewArchivedSessions = webPreviewArchivedSessions.filter((session) => session.id !== sessionId);
    webPreviewSettings = {
      ...webPreviewSettings,
      active_session_id:
        webPreviewSettings.active_session_id === sessionId
          ? null
          : webPreviewSettings.active_session_id,
      archived_session_ids: webPreviewSettings.archived_session_ids.filter((id) => id !== sessionId),
    };
    return Promise.resolve(webPreviewSettings);
  }

  return invoke("delete_session", { sessionId });
}

export function loadSessionMessages(sessionId: string): Promise<DesktopMessage[]> {
  if (!isTauriRuntime()) {
    return Promise.resolve([
      {
        id: 1,
        role: "user",
        content: `Loaded preview session: ${sessionId}`,
        created_at: "preview",
      },
      {
        id: 2,
        role: "assistant",
        content: "Real session history is available inside the Tauri app.",
        created_at: "preview",
      },
    ]);
  }

  return invoke("load_session_messages", { sessionId });
}

export function resumeSession(sessionId: string): Promise<ResumedSession> {
  if (!isTauriRuntime()) {
    return loadSessionMessages(sessionId).then((messages) => ({
      session_id: sessionId,
      messages,
      compact_boundaries: [],
      session_parts: [],
    }));
  }

  return invoke("resume_session", { sessionId });
}

export function listSessionReverts(
  sessionId: string,
  limit = 20,
): Promise<DesktopSessionRevertRecord[]> {
  if (!isTauriRuntime()) {
    return Promise.resolve([]);
  }

  return invoke("list_session_reverts", { sessionId, limit });
}

export function loadDesktopToolOutputPage(
  sessionId: string,
  idOrUri: string,
  offset = 0,
  limit = 64 * 1024,
): Promise<DesktopToolOutputPage> {
  if (!isTauriRuntime()) {
    return Promise.resolve({
      id: idOrUri,
      uri: idOrUri,
      tool_name: "preview",
      mime: "text/plain",
      content: "",
      offset,
      limit,
      total_bytes: 0,
      has_more: false,
    });
  }

  return invoke("desktop_tool_output_page", { sessionId, idOrUri, offset, limit });
}

export function loadDesktopToolOutputIndex(sessionId: string): Promise<DesktopToolOutputMeta[]> {
  if (!isTauriRuntime()) {
    return Promise.resolve([]);
  }

  return invoke("desktop_tool_output_index", { sessionId });
}

export function revertLastTurn(sessionId: string): Promise<DesktopRevertResult> {
  if (!isTauriRuntime()) {
    return Promise.resolve({
      session_id: sessionId,
      status: "completed",
      part_ids: [],
      file_change_ids: [],
      checkpoint_ids: [],
      paths: [],
      restored_files: [],
      removed_files: [],
      errors: [],
      change_count: 0,
    });
  }

  return invoke("revert_last_turn", { sessionId });
}

export async function pickProjectDirectory(): Promise<string | null> {
  if (!isTauriRuntime()) {
    return null;
  }

  const selected = await open({
    directory: true,
    multiple: false,
    title: "Select Priority Agent project",
  });

  return typeof selected === "string" ? selected : null;
}

export async function pickProjectFile(): Promise<string | null> {
  if (!isTauriRuntime()) {
    return "/Users/georgexu/Desktop/rust-agent/apps/desktop/src/app/App.tsx";
  }

  const selected = await open({
    directory: false,
    multiple: false,
    title: "Select file context",
  });

  return typeof selected === "string" ? selected : null;
}

export function sendMessage(message: string, contexts: DesktopRunContext[] = []): Promise<void> {
  if (!isTauriRuntime()) {
    if (!shouldUseWebPreviewFixtureRun(message, contexts)) {
      emitWebPreviewUnavailableResponse(message, contexts);
      return Promise.resolve();
    }

    const runId = crypto.randomUUID();
    const toolId = crypto.randomUUID();
    const fileToolId = crypto.randomUUID();
    const failedToolId = crypto.randomUUID();
    const permissionId = crypto.randomUUID();
    emitWebPreview({ type: "run_started", run_id: runId });
    emitWebPreview({ type: "thinking_started" });
    emitWebPreview({ type: "thinking_completed" });
    emitWebPreview({ type: "tool_started", id: toolId, name: "bash" });
    emitWebPreview({
      type: "tool_execution_progress",
      id: toolId,
      progress: "Scanning project context",
    });
    emitWebPreview({
      type: "tool_completed",
      id: toolId,
      result_preview: "Found desktop app workspace and active web preview fixtures.",
      metadata: {
        tool: "bash",
        call_id: toolId,
        success: true,
        command: "corepack pnpm --dir apps/desktop test:ui-smoke",
        command_category: "validation",
        validation_family: "pnpm_test",
        command_kind: "package_script",
        duration_ms: 1240,
        output_chars: 63,
        terminal_task: {
          status: "completed",
          exit_code: 0,
          duration_ms: 1240,
        },
      },
    });
    emitWebPreview({ type: "tool_started", id: fileToolId, name: "file_edit" });
    emitWebPreview({
      type: "tool_completed",
      id: fileToolId,
      result_preview: "Edited apps/desktop/src/app/runEventState.ts",
      metadata: {
        tool: "file_edit",
        call_id: fileToolId,
        success: true,
        path: "apps/desktop/src/app/runEventState.ts",
        replacements: 2,
        additions: 8,
        deletions: 3,
        diff_preview:
          "@@ -18,6 +18,9 @@\n type ToolSummary = {\n   replacements?: number;\n+  additions?: number;\n+  deletions?: number;\n+  diff_preview?: string;\n };\n",
        diff_preview_truncated: false,
        duration_ms: 48,
        output_chars: 44,
      },
    });
    emitWebPreview({ type: "tool_started", id: failedToolId, name: "bash" });
    emitWebPreview({
      type: "tool_completed",
      id: failedToolId,
      result_preview:
        "cargo test failed with exit code 101\n\nfailures:\n  desktop_smoke::timeline_cards_show_diff_preview\n\nthread 'desktop_smoke::timeline_cards_show_diff_preview' panicked at assertion failed\n\nexpected diff preview to be visible\nreceived empty preview block\n\nstack backtrace:\n  0: rust_begin_unwind\n  1: core::panicking::panic_fmt\n  2: desktop_smoke::timeline_cards_show_diff_preview\n\nrerun with RUST_BACKTRACE=1 for a backtrace",
      metadata: {
        tool: "bash",
        call_id: failedToolId,
        success: false,
        command: "cargo test -q desktop_smoke",
        command_category: "validation",
        validation_family: "cargo_test",
        command_kind: "cargo",
        duration_ms: 820,
        output_chars: 91,
        error_preview: "cargo test failed with exit code 101",
        user_note: "Inspect the failing test output, fix the regression, then rerun the same command.",
        terminal_task: {
          status: "failed",
          exit_code: 101,
          duration_ms: 820,
        },
      },
    });
    emitWebPreview({
      type: "assistant_delta",
      text: `Web preview received: ${message}\n\n${
        contexts.length
          ? `Structured context attached: ${contexts.map((context) => context.label).join(", ")}\n\n`
          : ""
      }Run this inside Tauri to stream the Rust agent runtime.`,
    });
    emitWebPreview({
      type: "runtime_diagnostic",
      diagnostic: {
        schema: "desktop_runtime_diagnostic.v1",
        task_state: {
          goal: message,
          mode: "full",
          stage: "closeout",
          mode_score: {
            confidence: 82,
            complexity: 7,
            risk: 5,
            uncertainty: 3,
            tool_need: 8,
            user_impact: 7,
          },
          lightweight_plan: null,
          verification: {
            status: "verified",
            required_checks: ["corepack pnpm --dir apps/desktop test:ui-smoke"],
          },
          done: {
            satisfied: true,
            summary: "preview run completed",
          },
          active_files: [
            "apps/desktop/src/app/runEventState.ts",
            "apps/desktop/src/app/components/TraceDrawer.tsx",
          ],
          recent_steps: [
            { stage: "validate", summary: "desktop smoke passed" },
            { stage: "edit", summary: "runtime diagnostic event rendered" },
          ],
          stop_check: {
            status: "stop",
            reason: "verification_ready",
            summary: "ready for closeout",
          },
        },
        verification_proof: {
          status: "verified",
          summary: "validation passed 1/1 current checks",
          closeout_status: "passed",
          changed_files: 2,
          validation_items: 1,
          acceptance_items: 1,
          residual_risks: 0,
        },
        control_loop: {
          coverage: "7/7",
          summary:
            "context=2 latest=runtime.diet -> decision=1 latest=action.decision -> verification=1 latest=verify.done -> closeout=2 latest=assistant",
          phases: [
            { phase: "context", events: 2, latest_label: "runtime.diet" },
            { phase: "decision", events: 1, latest_label: "action.decision" },
            { phase: "permission", events: 1, latest_label: "permission.resolve" },
            { phase: "tool_execution", events: 3, latest_label: "tool.done" },
            { phase: "state_update", events: 1, latest_label: "stop.check" },
            { phase: "verification", events: 1, latest_label: "verify.done" },
            { phase: "closeout", events: 2, latest_label: "assistant" },
          ],
        },
      },
    });
    emitWebPreview({
      type: "usage",
      prompt_tokens: 128,
      completion_tokens: 42,
      cached_tokens: 16,
    });
    emitWebPreview({
      type: "permission_request",
      id: permissionId,
      tool_name: "bash",
      arguments: {
        command: "git push origin claude",
      },
      prompt: "Allow git push to update the remote branch?",
      metadata: {
        permission_evidence: {
          schema: "permission_decision_evidence.v1",
          request_kind: "runtime_rule",
          permission_family: "shell",
          decision: "ask",
          risk_level: "medium",
          reasons: ["command matched git remote mutation policy"],
          matched_patterns: ["git push"],
          recovery: {
            recommended_action: "Approve once if this push is expected, otherwise reject and inspect the command.",
          },
          command_classification: {
            parser_status: "simple",
            category: "git_remote_mutation",
            mutation: true,
          },
        },
        action_review: {
          schema: "action_review.v1",
          tool: "bash",
          call_id: permissionId,
          decision: "ask_user",
          primary_reason: "network_requires_confirmation",
          permission: {
            allowed_by_context: true,
            requires_confirmation: true,
            decision: "Ask",
            risk_level: "Medium",
            confidence: 0.86,
            warnings: ["REMOTE_SIDE_EFFECT"],
          },
          scope: {
            allowed: true,
            reason: "command stays within the active repository task",
          },
          budget: {
            allowed: true,
            scheduled_count: 1,
            max_tool_calls: 4,
            reason: "tool-call budget still has room",
          },
          checkpoint: {
            required: true,
            status: "unavailable",
            enforcement: "user_approval",
            rollback_scope: "remote",
            requires_user_approval: true,
            reason: "git push mutates remote state and cannot be rolled back locally",
          },
          side_effects: {
            schema: "action_side_effect_profile.v1",
            external_side_effect: "git_remote_publication",
            network: {
              class: "remote_service",
              target: "git push origin claude",
              trusted: false,
              reason: "git push publishes to a remote service",
            },
            mutates_local_workspace: true,
            mutates_local_machine: false,
            remote_side_effect: true,
            paths: [],
            summary: "external_effect=GitRemotePublication network=RemoteService paths=0",
          },
          user_reason:
            "Action requires user confirmation before execution: network_requires_confirmation.",
          model_recovery:
            "Wait for the permission result and do not claim the tool ran until it succeeds.",
        },
      },
    });
    emitWebPreview({ type: "run_completed" });
    return Promise.resolve();
  }

  return invoke("send_message", { contexts, message });
}

function shouldUseWebPreviewFixtureRun(message: string, contexts: DesktopRunContext[]) {
  if (!webPreviewFixtureMode()) {
    return false;
  }

  const normalized = message.trim().toLowerCase();
  return (
    contexts.length > 0 ||
    normalized.includes("timeline") ||
    normalized.includes("fixture") ||
    normalized.includes("trace")
  );
}

function webPreviewFixtureMode() {
  if (typeof window === "undefined") {
    return false;
  }

  const params = new URLSearchParams(window.location.search);
  return (
    params.has("previewFixture") ||
    window.localStorage.getItem("priority-agent.previewFixture") === "1"
  );
}

function emitWebPreviewUnavailableResponse(message: string, contexts: DesktopRunContext[]) {
  const contextLine = contexts.length
    ? `\n\n已附加上下文：${contexts.map((context) => context.label).join(", ")}。`
    : "";
  const promptLine = message.trim() ? `\n\n你的消息还在输入框历史里：${message.trim()}` : "";
  emitWebPreview({
    type: "assistant_delta",
    text: `当前打开的是浏览器预览（web-preview），这条消息没有发送给 LLM，也不能访问桌面或运行工具。请在 Tauri 桌面应用窗口里使用真实 agent。${contextLine}${promptLine}`,
  });
  emitWebPreview({ type: "run_completed" });
}

export function compactContext(): Promise<DesktopCompactionAttempt | null> {
  if (!isTauriRuntime()) {
    return Promise.resolve({
      trigger: "manual compact",
      strategy: "session_memory_compact",
      decision: "compacted",
      before_tokens: 3400,
      after_tokens: 2100,
      messages_before: 5,
      messages_after: 4,
      reason: "web preview manual compact",
      attempt_index: 1,
      consecutive_no_gain: 0,
      consecutive_failures: 0,
      circuit_open: false,
      boundary_id: "web-preview-boundary",
    });
  }

  return invoke("compact_context");
}

export function answerPermission(approved: boolean): Promise<boolean> {
  if (!isTauriRuntime()) {
    emitWebPreview({
      type: "tool_completed",
      id: `preview-permission-${approved ? "approved" : "rejected"}`,
      result_preview: approved ? "Permission approved" : "Permission rejected",
    });
    return Promise.resolve(true);
  }

  return invoke("answer_permission", { approved });
}

export function onDesktopRunEvent(callback: (event: DesktopRunEvent) => void) {
  if (!isTauriRuntime()) {
    webPreviewListeners.add(callback);
    return Promise.resolve(() => webPreviewListeners.delete(callback));
  }

  return listen<DesktopRunEvent>("desktop-run-event", (event) => {
    callback(event.payload);
  });
}

function emitWebPreview(event: DesktopRunEvent) {
  for (const listener of webPreviewListeners) {
    listener(event);
  }
}

function isTauriRuntime() {
  if (typeof window === "undefined" || !window.__TAURI_INTERNALS__) {
    return false;
  }

  const internals = window.__TAURI_INTERNALS__ as {
    invoke?: unknown;
    transformCallback?: unknown;
  };
  return typeof internals.invoke === "function" && typeof internals.transformCallback === "function";
}

function basename(path: string) {
  return path.split(/[\\/]/).filter(Boolean).at(-1) || path;
}
