import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  AgentModeId,
  AgentModeOption,
  DesktopCompactionAttempt,
  DesktopContextSnapshot,
  DesktopDiagnostic,
  DesktopDiagnosticsResponse,
  DesktopExportResult,
  DesktopFilePreview,
  DesktopHealth,
  DesktopLabArtifactBody,
  DesktopLabDaemonActionResult,
  DesktopLabReportPage,
  DesktopMessage,
  DesktopRevertResult,
  DesktopRunContext,
  DesktopRunContextDetail,
  DesktopRunEvent,
  DesktopSessionRevertRecord,
  DesktopSettings,
  DesktopToolOutputMeta,
  DesktopToolOutputPage,
  DesktopWorkbenchSnapshot,
  DetailLevelId,
  PermissionModeId,
  PermissionModeOption,
  ProviderModelStatus,
  ProviderSetupInfo,
  RecentSession,
  ResumedSession,
  SelectedProject,
} from "./desktopTypes";
import {
  answerWebPreviewPermission,
  compactWebPreviewContext,
  loadWebPreviewFilePreview,
  loadWebPreviewLabArtifactBody,
  loadWebPreviewLabReportPage,
  onWebPreviewRunEvent,
  sendWebPreviewMessage,
} from "./desktopPreview";
export {
  goalClear,
  goalEdit,
  goalLog,
  goalPause,
  goalResume,
  goalStart,
  goalStatus,
} from "./desktopGoalApi";
export type * from "./desktopTypes";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

let webPreviewSettings: DesktopSettings = {
  selected_project: "/Users/georgexu/Desktop/rust-agent",
  active_session_id: "web-preview",
  permission_mode: "auto",
  detail_level: "coding",
  agent_mode: "auto",
  provider_name: "deepseek",
  model: "deepseek-v4-flash",
  settings_path: "web-preview",
  diagnostic_logs_path: "web-preview/logs/desktop.log",
  recent_projects: ["/Users/georgexu/Desktop/rust-agent", "/Users/georgexu/Desktop/bioclaw"],
  archived_session_ids: [],
  startup_state: {
    status: "restored_session",
    detail: "Restored web-preview in rust-agent",
    lab_run_id: null,
    lab_stage: null,
    lab_owner: null,
    lab_pause_reason: null,
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
      lab_status: {
        available: true,
        state: "run",
        detail: "Active at graduate_work with Postdoc",
        lab_run_id: "labrun_preview",
        proposal_id: "labproposal_preview",
        proposal_status: null,
        run_status: "Active",
        stage: "graduate_work",
        owner: "Postdoc",
        needs_user: false,
        cycle_count: 2,
        artifact_count: 5,
        meeting_count: 1,
        task_total: 3,
        task_open: 1,
        task_blocked: 1,
        blockers: ["Wire status actions: Playwright panel action check failed"],
        validation_retry_count: 2,
        validation_retry_escalated_count: 1,
        latest_validation_retry: "gradtask_preview attempt 2: Playwright panel action check failed",
        meeting_recommended: true,
        meeting_topic: "resolve 1 blocked graduate task at stage graduate_work",
        latest_report_path: `${webPreviewSettings.selected_project}/.priority-agent/lab/runs/labrun_preview/reports/artifact_professor_review.md`,
        daemon_policy: {
          enabled: true,
          mode: "hybrid_cycles",
          max_steps: 4,
          max_steps_per_cycle: 6,
          interval_ms: 500,
          last_started_at: null,
          last_start_error: null,
        },
        artifacts: [
          {
            artifact_id: "artifact_professor_review",
            artifact_type: "ProfessorReview",
            stage: "professor_review",
            owner: "Professor",
            status: "ReadyForHandoff",
            validation_status: "not_verified",
            title: "Professor review",
            created_at: "2026-06-21T10:00:00Z",
            updated_at: "2026-06-21T10:00:00Z",
            report_path: `${webPreviewSettings.selected_project}/.priority-agent/lab/runs/labrun_preview/reports/artifact_professor_review.md`,
            report_preview: "# Professor Review\n\nDecision: revise graduate implementation.\n\nEvidence: Playwright panel action check failed during LabRun desktop validation.\n\nNext action: ask the postdoc to narrow the blocker and assign a targeted graduate repair.",
            report_preview_truncated: false,
            evidence_refs: ["labevidence_preview"],
          },
          {
            artifact_id: "artifact_graduate_result",
            artifact_type: "GraduateResult",
            stage: "graduate_work",
            owner: "Graduate",
            status: "NeedsRevision",
            validation_status: "needs_revision",
            title: "Graduate implementation result",
            created_at: "2026-06-21T09:30:00Z",
            updated_at: "2026-06-21T09:45:00Z",
            report_path: `${webPreviewSettings.selected_project}/.priority-agent/lab/runs/labrun_preview/reports/artifact_graduate_result.md`,
            report_preview: "# Graduate Result\n\nChanged files: apps/desktop/src/app/components/InspectorPanel.tsx.\n\nValidation: needs revision because the LabRun panel action check failed.",
            report_preview_truncated: false,
            evidence_refs: ["labevidence_preview"],
          },
        ],
        reports: [
          {
            artifact_id: "artifact_professor_review",
            path: `${webPreviewSettings.selected_project}/.priority-agent/lab/runs/labrun_preview/reports/artifact_professor_review.md`,
            preview: "# Professor Review\n\nDecision: revise graduate implementation.\n\nEvidence: Playwright panel action check failed during LabRun desktop validation.",
            truncated: false,
          },
          {
            artifact_id: "artifact_graduate_result",
            path: `${webPreviewSettings.selected_project}/.priority-agent/lab/runs/labrun_preview/reports/artifact_graduate_result.md`,
            preview: "# Graduate Result\n\nValidation: needs revision because the LabRun panel action check failed.",
            truncated: false,
          },
        ],
        evidence_refs: [
          {
            evidence_id: "labevidence_preview",
            kind: "File",
            role: "Postdoc",
            reference: "apps/desktop/tests/desktop-ui-smoke.spec.ts",
            summary: "Playwright panel action check failed during LabRun desktop validation.",
            artifact_id: "artifact_graduate_result",
            cycle_id: "cycle_preview",
            created_at: "2026-06-21T09:40:00Z",
            estimated_summary_tokens: 18,
          },
        ],
      },
      subagent_tasks: [
        {
          task_id: "provider-compare-background",
          agent_id: "agent_preview",
          profile: "implementer",
          role: "Specialist",
          status: "completed",
          description: "Provider comparison background implementer smoke",
          child_session_id: "preview-session:subagent:provider-compare-background",
          result_artifact_id: 1,
          artifact_status: "completed",
          result_preview: "background subagent tool smoke",
          tools_used: ["bash", "file_write", "file_read"],
          proof_kind: "subagent_claim_only",
          completion_sink: "agent_manager",
          recovery_status: null,
          recovery_action: null,
          updated_at: "preview",
        },
      ],
    });
  }

  return invoke("desktop_workbench_snapshot");
}

export async function superviseLabDaemon(): Promise<DesktopLabDaemonActionResult> {
  if (!isTauriRuntime()) {
    const snapshot = await desktopWorkbenchSnapshot();
    return {
      action: "supervise",
      output: "Lab daemon service supervision repaired missing service.\nPreview mode",
      lab_status: snapshot.lab_status,
    };
  }

  return invoke("lab_daemon_supervise");
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
        lab_run_id: null,
        lab_stage: null,
        lab_owner: null,
        lab_pause_reason: null,
      },
    };
    return Promise.resolve({ path });
  }

  return invoke("select_project", { path });
}

export function desktopSettings(): Promise<DesktopSettings> {
  if (!isTauriRuntime()) {
    if (webPreviewFixtureName() === "labRecovery") {
      return Promise.resolve({
        ...webPreviewSettings,
        startup_state: {
          status: "lab_recovery",
          detail: "LabRun labrun_preview is recoverable at graduate_work with Postdoc: app_shutdown",
          lab_run_id: "labrun_preview",
          lab_stage: "graduate_work",
          lab_owner: "Postdoc",
          lab_pause_reason: "app_shutdown",
        },
      });
    }
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
        lab_run_id: null,
        lab_stage: null,
        lab_owner: null,
        lab_pause_reason: null,
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
        "DEEPSEEK_API_KEY",
        "MINIMAX_API_KEY",
        "KIMI_CODE_API_KEY",
        "GLM_API_KEY",
        "ZAI_API_KEY",
        "ZHIPUAI_API_KEY",
        "BIGMODEL_API_KEY",
        "MOONSHOT_API_KEY",
        "OPENAI_API_KEY",
      ],
      example: 'export DEEPSEEK_API_KEY="your-key-here"',
    });
  }

  return invoke("provider_setup_info");
}

export function providerModelStatus(): Promise<ProviderModelStatus> {
  if (!isTauriRuntime()) {
    return Promise.resolve({
      active_provider: "deepseek",
      active_provider_label: "DeepSeek",
      active_model: "deepseek-v4-flash",
      active_base_url: "https://api.deepseek.com",
      runtime_model: "deepseek-v4-flash",
      runtime_provider_ready: true,
      selection_source: "preview",
      configured_count: 1,
      providers: [
        {
          id: "deepseek",
          label: "DeepSeek",
          provider_type: "DeepSeek",
          model: "deepseek-v4-flash",
          base_url: "https://api.deepseek.com",
          configured: true,
          active: true,
          note: "current",
        },
        {
          id: "minimax",
          label: "MiniMax",
          provider_type: "Minimax",
          model: "MiniMax-M3",
          base_url: "",
          configured: false,
          active: false,
          note: "missing MINIMAX_API_KEY",
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
          id: "deepseek-v4-flash",
          label: "deepseek-v4-flash",
          provider_id: "deepseek",
          active: true,
          note: "current",
        },
        {
          id: "deepseek-chat",
          label: "deepseek-chat",
          provider_id: "deepseek",
          active: false,
          note: "takes effect next request",
        },
        {
          id: "deepseek-reasoner",
          label: "deepseek-reasoner",
          provider_id: "deepseek",
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
  if (!isTauriRuntime()) {
    const redacted = key.length > 8 ? `${key.slice(0, 4)}...${key.slice(-4)}` : "redacted";
    return Promise.resolve(`Saved preview credential for ${providerId} (${redacted}).`);
  }

  return invoke("save_provider_credential", { providerId, key });
}

export function openFilePath(path: string): Promise<void> {
  if (!isTauriRuntime()) {
    return Promise.resolve();
  }

  return invoke("open_file_path", { path });
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

export function loadDesktopLabReportPage(
  path: string,
  offset = 0,
  limit = 32 * 1024,
): Promise<DesktopLabReportPage> {
  if (!isTauriRuntime()) {
    return Promise.resolve(loadWebPreviewLabReportPage(path, offset, limit));
  }

  return invoke("desktop_lab_report_page", { path, offset, limit });
}

export function loadDesktopLabArtifactBody(artifactId: string): Promise<DesktopLabArtifactBody> {
  if (!isTauriRuntime()) {
    return Promise.resolve(loadWebPreviewLabArtifactBody(artifactId));
  }

  return invoke("desktop_lab_artifact_body", { artifactId });
}

export function loadDesktopFilePreview(
  path: string,
  limit = 32 * 1024,
): Promise<DesktopFilePreview> {
  if (!isTauriRuntime()) {
    return Promise.resolve(loadWebPreviewFilePreview(path, limit));
  }

  return invoke("desktop_file_preview", { path, limit });
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
    return sendWebPreviewMessage(message, contexts);
  }

  return invoke("send_message", { contexts, message });
}

function webPreviewFixtureName() {
  if (typeof window === "undefined") {
    return "";
  }
  return new URLSearchParams(window.location.search).get("previewFixture") || "";
}

export function compactContext(): Promise<DesktopCompactionAttempt | null> {
  if (!isTauriRuntime()) {
    return Promise.resolve(compactWebPreviewContext());
  }

  return invoke("compact_context");
}

export function answerPermission(approved: boolean): Promise<boolean> {
  if (!isTauriRuntime()) {
    return Promise.resolve(answerWebPreviewPermission(approved));
  }

  return invoke("answer_permission", { approved });
}

export function onDesktopRunEvent(callback: (event: DesktopRunEvent) => void) {
  if (!isTauriRuntime()) {
    return onWebPreviewRunEvent(callback);
  }

  return listen<DesktopRunEvent>("desktop-run-event", (event) => {
    callback(event.payload);
  });
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
