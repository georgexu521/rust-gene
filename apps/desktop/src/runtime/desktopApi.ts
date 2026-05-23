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

export type ResumedSession = {
  session_id: string;
  messages: DesktopMessage[];
};

export type DesktopSettings = {
  selected_project: string;
  active_session_id?: string | null;
  permission_mode: PermissionModeId;
  detail_level: DetailLevelId;
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

export type ProviderSetupInfo = {
  shell_profile_path: string;
  provider_env_vars: string[];
  example: string;
};

export type ProviderModelStatus = {
  active_provider?: string | null;
  active_model: string;
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
  | { type: "run_completed" }
  | { type: "output_truncated" }
  | { type: "run_error"; message: string };

const webPreviewListeners = new Set<(event: DesktopRunEvent) => void>();
let webPreviewSettings: DesktopSettings = {
  selected_project: "/Users/georgexu/Desktop/rust-agent",
  active_session_id: "web-preview",
  permission_mode: "auto",
  detail_level: "coding",
  provider_name: "kimi",
  model: "kimi-k2.5",
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
      provider_env_vars: ["MINIMAX_API_KEY", "OPENAI_API_KEY", "MOONSHOT_API_KEY"],
      example: 'export MOONSHOT_API_KEY="your-key-here"',
    });
  }

  return invoke("provider_setup_info");
}

export function providerModelStatus(): Promise<ProviderModelStatus> {
  if (!isTauriRuntime()) {
    return Promise.resolve({
      active_provider: "kimi",
      active_model: "kimi-k2.5",
      configured_count: 1,
      providers: [
        {
          id: "kimi",
          label: "Kimi",
          provider_type: "Kimi",
          model: "kimi-k2.5",
          base_url: "https://api.moonshot.cn/v1",
          configured: true,
          active: true,
          note: "current",
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
      ],
      models: [
        {
          id: "kimi-k2.5",
          label: "kimi-k2.5",
          provider_id: "kimi",
          active: true,
          note: "current",
        },
        {
          id: "kimi-k2.5-thinking",
          label: "kimi-k2.5-thinking",
          provider_id: "kimi",
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
      active_model: model,
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
    }));
  }

  return invoke("resume_session", { sessionId });
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
      },
    });
    emitWebPreview({ type: "run_completed" });
    return Promise.resolve();
  }

  return invoke("send_message", { contexts, message });
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
