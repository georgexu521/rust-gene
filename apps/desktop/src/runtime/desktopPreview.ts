import type {
  DesktopCompactionAttempt,
  DesktopFilePreview,
  DesktopLabArtifactBody,
  DesktopLabReportPage,
  DesktopRunContext,
  DesktopRunEvent,
} from "./desktopTypes";

const webPreviewListeners = new Set<(event: DesktopRunEvent) => void>();

export function sendWebPreviewMessage(
  message: string,
  contexts: DesktopRunContext[] = [],
): Promise<void> {
  if (!shouldUseWebPreviewFixtureRun(message, contexts)) {
    emitWebPreviewUnavailableResponse(message, contexts);
    return Promise.resolve();
  }

  if (shouldEmitWebPreviewRunError(message)) {
    const runId = crypto.randomUUID();
    emitWebPreview({ type: "run_started", run_id: runId, session_id: "web-preview" });
    emitWebPreview({
      type: "run_error",
      message: "Simulated desktop runtime error for web preview validation.",
    });
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
    cache_write_tokens: 12,
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

export function loadWebPreviewLabReportPage(
  path: string,
  offset = 0,
  limit = 32 * 1024,
): DesktopLabReportPage {
  const content = webPreviewLabReportContent(path);
  const safeOffset = Math.max(0, Math.min(offset, content.length));
  const safeLimit = Math.max(1, Math.min(limit, 128 * 1024));
  const end = Math.min(content.length, safeOffset + safeLimit);
  return {
    path,
    content: content.slice(safeOffset, end),
    offset: safeOffset,
    limit: safeLimit,
    total_bytes: content.length,
    has_more: end < content.length,
  };
}

export function loadWebPreviewLabArtifactBody(artifactId: string): DesktopLabArtifactBody {
  const isGraduate = artifactId.includes("graduate");
  return {
    artifact_id: artifactId,
    artifact_type: isGraduate ? "GraduateResult" : "ProfessorReview",
    title: isGraduate ? "Graduate implementation result" : "Professor review",
    stage: isGraduate ? "graduate_work" : "professor_review",
    owner: isGraduate ? "Graduate" : "Professor",
    status: "ReadyForHandoff",
    validation_status: isGraduate ? "needs_revision" : "not_verified",
    content: JSON.stringify(
      isGraduate
        ? {
            task_summary: "Updated the LabRun desktop status surface.",
            changed_files: ["apps/desktop/src/app/components/InspectorPanel.tsx"],
            validation_attempts: ["Playwright panel action check failed during LabRun desktop validation."],
            blockers: ["Needs a narrower follow-up task."],
            handoff_to_postdoc: "Review the panel action flow and assign a targeted repair.",
          }
        : {
            review_summary: "Professor reviewed the graduate result.",
            strategic_assessment: "Keep the work focused on the LabRun desktop status surface.",
            accepted: false,
            required_revisions: ["Narrow the Playwright panel action blocker."],
            user_report: "Decision: revise graduate implementation.",
          },
      null,
      2,
    ),
  };
}

export function loadWebPreviewFilePreview(
  path: string,
  limit = 32 * 1024,
): DesktopFilePreview {
  const content = webPreviewFileContent(path);
  const safeLimit = Math.max(1, Math.min(limit, 64 * 1024));
  const preview = content.slice(0, safeLimit);
  return {
    path,
    content: preview,
    line_count: preview.split(/\r?\n/).filter((line, index, lines) => line.length > 0 || index < lines.length - 1).length,
    total_bytes: content.length,
    truncated: preview.length < content.length,
  };
}

export function compactWebPreviewContext(): DesktopCompactionAttempt {
  return {
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
  };
}

export function answerWebPreviewPermission(approved: boolean): boolean {
  emitWebPreview({
    type: "tool_completed",
    id: `preview-permission-${approved ? "approved" : "rejected"}`,
    result_preview: approved ? "Permission approved" : "Permission rejected",
  });
  return true;
}

export function onWebPreviewRunEvent(callback: (event: DesktopRunEvent) => void) {
  webPreviewListeners.add(callback);
  return Promise.resolve(() => webPreviewListeners.delete(callback));
}

function shouldUseWebPreviewFixtureRun(message: string, contexts: DesktopRunContext[]) {
  if (!webPreviewFixtureMode()) {
    return false;
  }

  const normalized = message.trim().toLowerCase();
  return (
    contexts.length > 0 ||
    normalized.includes("error") ||
    normalized.includes("timeline") ||
    normalized.includes("fixture") ||
    normalized.includes("trace")
  );
}

function shouldEmitWebPreviewRunError(message: string) {
  const normalized = message.trim().toLowerCase();
  return normalized.includes("run error") || normalized.includes("fixture error");
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

function webPreviewLabReportContent(path: string) {
  if (path.includes("artifact_graduate_result")) {
    return [
      "# Graduate Result",
      "",
      "Changed files: apps/desktop/src/app/components/InspectorPanel.tsx.",
      "",
      "Validation: needs revision because the LabRun panel action check failed.",
      "",
      "Evidence: Playwright panel action check failed during LabRun desktop validation.",
      "",
      "Next action: return a smaller UI task to the graduate and require a smoke-test proof.",
    ].join("\n");
  }

  return [
    "# Professor Review",
    "",
    "Decision: revise graduate implementation.",
    "",
    "Evidence: Playwright panel action check failed during LabRun desktop validation.",
    "",
    "Next action: ask the postdoc to narrow the blocker and assign a targeted graduate repair.",
    "",
    "Professor steering: keep the work focused on the LabRun desktop status surface, not unrelated runtime policy.",
  ].join("\n");
}

function webPreviewFileContent(path: string) {
  if (path.endsWith("request_preparation_controller.rs")) {
    return [
      "pub struct RequestPreparationController {",
      "    context_budget: usize,",
      "}",
      "",
      "impl RequestPreparationController {",
      "    pub fn inject_project_map_zone(&self) {",
      "        // web preview fixture for Files inspector",
      "    }",
      "}",
    ].join("\n");
  }

  return [
    `// Web preview file fixture: ${path}`,
    "export function previewFile() {",
    "  return 'desktop file preview';",
    "}",
  ].join("\n");
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

function emitWebPreview(event: DesktopRunEvent) {
  for (const listener of webPreviewListeners) {
    listener(event);
  }
}
