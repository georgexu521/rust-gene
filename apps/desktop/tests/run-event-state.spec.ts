import { expect, test } from "@playwright/test";
import {
  appendPermissionAnswer,
  applyRunEvent,
  initialRunViewState,
  loadSessionTranscript,
  submitUserMessage,
  withRunIdleWarning,
} from "../src/app/runEventState";

function ids(...values: string[]) {
  let index = 0;
  return () => values[index++] || `id-${index}`;
}

test.describe("run event state", () => {
  test("submits a user message and completes the active run", () => {
    const submitted = submitUserMessage(initialRunViewState, "Inspect UI", [], ids("user-1"));

    expect(submitted.isRunning).toBe(true);
    expect(submitted.items).toEqual([{ id: "user-1", role: "user", text: "Inspect UI" }]);
    expect(submitted.traceItems).toEqual([]);

    const started = applyRunEvent(submitted, {
      type: "run_started",
      run_id: "run-1",
      session_id: "session-1",
    }).state;
    const completed = applyRunEvent(started, { type: "run_completed" }, ids("done-1"));

    expect(completed.shouldRefreshSessions).toBe(true);
    expect(completed.state.isRunning).toBe(false);
    expect(completed.state.selectedSessionId).toBe("session-1");
    expect(completed.state.items).toContainEqual(
      expect.objectContaining({
        id: "run-1",
        role: "timeline",
        kind: "run",
        status: "completed",
        summary: expect.objectContaining({
          kind: "run",
          stage: "completed",
          headline: "Run completed",
          sessionId: "session-1",
          stats: [],
        }),
      }),
    );
  });

  test("treats idle watchdog messages as recoverable run warnings", () => {
    const submitted = submitUserMessage(initialRunViewState, "Inspect UI", [], ids("user-1"));
    const warned = withRunIdleWarning(submitted, "runtime idle");

    expect(warned.isRunning).toBe(true);
    expect(warned.error).toBe("runtime idle");

    const started = applyRunEvent(warned, {
      type: "run_started",
      run_id: "run-1",
      session_id: "session-1",
    }).state;
    const completed = applyRunEvent(started, { type: "run_completed" }, ids("done-1")).state;

    expect(started.error).toBeNull();
    expect(completed.error).toBeNull();
    expect(completed.isRunning).toBe(false);
  });

  test("keeps simple replies out of the tool timeline", () => {
    const submitted = submitUserMessage(initialRunViewState, "你好", [], ids("user-1"));
    const answered = applyRunEvent(
      submitted,
      { type: "assistant_delta", text: "你好，我在。" },
      ids("assistant-1"),
    ).state;
    const completed = applyRunEvent(answered, { type: "run_completed" }, ids("done-1")).state;

    expect(completed.isRunning).toBe(false);
    expect(completed.items).toEqual([
      { id: "user-1", role: "user", text: "你好" },
      { id: "assistant-1", role: "assistant", text: "你好，我在。", variant: undefined },
    ]);
  });

  test("ignores duplicate assistant delta events", () => {
    const submitted = submitUserMessage(initialRunViewState, "你好", [], ids("user-1"));
    const answered = applyRunEvent(
      submitted,
      { type: "assistant_delta", text: "你好！继续吧，有什么需要帮忙的？" },
      ids("assistant-1"),
    ).state;
    const duplicated = applyRunEvent(
      answered,
      { type: "assistant_delta", text: "你好！继续吧，有什么需要帮忙的？" },
      ids("assistant-2"),
    ).state;

    expect(duplicated.items).toEqual([
      { id: "user-1", role: "user", text: "你好" },
      {
        id: "assistant-1",
        role: "assistant",
        text: "你好！继续吧，有什么需要帮忙的？",
        variant: undefined,
      },
    ]);
  });

  test("carries attached contexts into the run summary", () => {
    const submitted = submitUserMessage(
      initialRunViewState,
      "Review diff",
      [{ type: "current_diff", label: "Current diff" }],
      ids("user-1"),
    );
    const started = applyRunEvent(submitted, {
      type: "run_started",
      run_id: "run-1",
      session_id: null,
    }).state;

    expect(started.items).toContainEqual(
      expect.objectContaining({
        id: "run-1",
        summary: expect.objectContaining({
          contexts: [{ type: "current_diff", label: "Current diff" }],
        }),
      }),
    );
    expect(started.traceItems).toContainEqual(
      expect.objectContaining({
        id: "run-1",
        contexts: [{ type: "current_diff", label: "Current diff" }],
      }),
    );

    const completed = applyRunEvent(started, { type: "run_completed" }, ids("done-1")).state;
    expect(completed.items).toContainEqual(
      expect.objectContaining({
        id: "run-1",
        summary: expect.objectContaining({
          contexts: [{ type: "current_diff", label: "Current diff" }],
        }),
      }),
    );
  });

  test("keeps the run card aligned with tool and permission progress", () => {
    const started = applyRunEvent(initialRunViewState, {
      type: "run_started",
      run_id: "run-1",
      session_id: "session-1",
    }).state;
    const toolRunning = applyRunEvent(started, {
      type: "tool_started",
      id: "tool-1",
      name: "bash",
    }).state;
    const permissionWaiting = applyRunEvent(toolRunning, {
      type: "permission_request",
      id: "permission-1",
      tool_name: "bash",
      arguments: { command: "git push" },
      prompt: "Allow git push?",
    }).state;
    const approved = appendPermissionAnswer(permissionWaiting, true, true, ids("answer-1"));

    expect(toolRunning.items).toContainEqual(
      expect.objectContaining({
        id: "run-1",
        summary: expect.objectContaining({
          kind: "run",
          stage: "running",
          headline: "Running tool",
          detail: "bash",
          stats: ["1 tool", "1 running", "bash"],
        }),
      }),
    );
    expect(permissionWaiting.items).toContainEqual(
      expect.objectContaining({
        id: "run-1",
        summary: expect.objectContaining({
          kind: "run",
          stage: "waiting",
          headline: "Waiting for permission",
          detail: "Allow git push?",
          stats: ["1 tool", "1 running", "bash"],
        }),
      }),
    );
    expect(approved.items).toContainEqual(
      expect.objectContaining({
        id: "run-1",
        summary: expect.objectContaining({
          kind: "run",
          stage: "running",
          headline: "Permission approved",
          sessionId: "session-1",
          stats: ["1 tool", "1 running", "bash"],
        }),
      }),
    );
  });

  test("surfaces action review facts on permission requests", () => {
    const permissionWaiting = applyRunEvent(initialRunViewState, {
      type: "permission_request",
      id: "permission-1",
      tool_name: "bash",
      arguments: { command: "git push" },
      prompt: "Allow git push?",
      metadata: {
        permission_evidence: {
          permission_family: "shell",
          request_kind: "runtime_rule",
          risk_level: "high",
          decision: "Ask",
          allowed_always_rules: ["bash:git push*"],
        },
        action_review: {
          decision: "ask_user",
          primary_reason: "network_requires_confirmation",
          model_recovery: "Wait for approval before continuing.",
          permission: {
            decision: "Ask",
            risk_level: "High",
          },
          side_effects: {
            external_side_effect: "git_remote_publication",
            network: {
              class: "remote_service",
            },
          },
          checkpoint: {
            status: "unavailable",
            requires_user_approval: true,
          },
          scope: {
            allowed: true,
          },
          budget: {
            allowed: true,
          },
        },
      },
    }).state;

    expect(permissionWaiting.items).toContainEqual(
      expect.objectContaining({
        id: "permission-1",
        role: "timeline",
        summary: expect.objectContaining({
          kind: "permission",
          actionDecision: "ask_user",
          actionReason: "network_requires_confirmation",
          sideEffect: "git_remote_publication",
          network: "remote_service",
          checkpoint: "unavailable",
          checkpointApproval: true,
          allowedRule: "bash:git push*",
          recovery: "Wait for approval before continuing.",
        }),
      }),
    );
    expect(permissionWaiting.traceItems).toContainEqual(
      expect.objectContaining({
        kind: "permission",
        detail: expect.stringContaining("review ask_user"),
      }),
    );
  });

  test("surfaces runtime diagnostics in run summary and trace", () => {
    const started = applyRunEvent(initialRunViewState, {
      type: "run_started",
      run_id: "run-1",
      session_id: "session-1",
    }).state;
    const diagnosed = applyRunEvent(
      started,
      {
        type: "runtime_diagnostic",
        diagnostic: {
          schema: "desktop_runtime_diagnostic.v1",
          task_state: {
            goal: "Wire diagnostics",
            stage: "closeout",
            verification: { status: "verified" },
            active_files: ["apps/desktop/src/app/runEventState.ts"],
          },
          verification_proof: {
            status: "verified",
            summary: "validation passed 1/1 current checks",
          },
          control_loop: {
            coverage: "7/7",
            phases: [{ phase: "closeout", events: 2, latest_label: "assistant" }],
          },
        },
      },
      ids("runtime-1"),
    ).state;
    const completed = applyRunEvent(diagnosed, { type: "run_completed" }).state;

    expect(diagnosed.items).toContainEqual(
      expect.objectContaining({
        id: "run-1",
        summary: expect.objectContaining({
          kind: "run",
          stage: "running",
          headline: "Runtime diagnostic",
          stats: [
            "stage closeout",
            "verification verified",
            "proof verified",
            "spine 7/7",
            "files 1",
          ],
        }),
      }),
    );
    expect(diagnosed.traceItems).toContainEqual(
      expect.objectContaining({
        id: "runtime-1",
        kind: "runtime",
        runtime: expect.objectContaining({
          schema: "desktop_runtime_diagnostic.v1",
        }),
      }),
    );
    expect(completed.items).toContainEqual(
      expect.objectContaining({
        id: "run-1",
        summary: expect.objectContaining({
          kind: "run",
          stage: "completed",
          stats: [
            "stage closeout",
            "verification verified",
            "proof verified",
            "spine 7/7",
          ],
        }),
      }),
    );
  });

  test("summarizes multi-tool runs with failures and recovery guidance", () => {
    const started = applyRunEvent(initialRunViewState, {
      type: "run_started",
      run_id: "run-1",
    }).state;
    const shellStarted = applyRunEvent(started, {
      type: "tool_started",
      id: "shell-1",
      name: "bash",
    }).state;
    const shellCompleted = applyRunEvent(shellStarted, {
      type: "tool_completed",
      id: "shell-1",
      result_preview: "ok",
      metadata: {
        tool: "bash",
        success: true,
        command: "corepack pnpm --dir apps/desktop test:ui-smoke",
        validation_family: "pnpm_test",
        terminal_task: { status: "completed", exit_code: 0 },
      },
    }).state;
    const fileStarted = applyRunEvent(shellCompleted, {
      type: "tool_started",
      id: "file-1",
      name: "file_edit",
    }).state;
    const fileCompleted = applyRunEvent(fileStarted, {
      type: "tool_completed",
      id: "file-1",
      result_preview: "Edited file",
      metadata: {
        tool: "file_edit",
        success: true,
        path: "apps/desktop/src/app/runEventState.ts",
        replacements: 2,
      },
    }).state;
    const failedStarted = applyRunEvent(fileCompleted, {
      type: "tool_started",
      id: "failed-1",
      name: "bash",
    }).state;
    const failed = applyRunEvent(failedStarted, {
      type: "tool_completed",
      id: "failed-1",
      result_preview: "cargo test failed",
      metadata: {
        tool: "bash",
        success: false,
        error_preview: "cargo test failed",
        user_note: "Fix the failing test and rerun it.",
      },
    }).state;
    const completed = applyRunEvent(failed, { type: "run_completed" }).state;

    expect(failed.items).toContainEqual(
      expect.objectContaining({
        id: "run-1",
        summary: expect.objectContaining({
          kind: "run",
          stage: "failed",
          headline: "Tool failed",
          detail: "Shell command",
          recovery: "Fix the failing test and rerun it.",
          stats: [
            "3 tools",
            "2 done",
            "1 failed",
            "bash x2",
            "file_edit",
            "1 file changed",
            "1 validation",
          ],
        }),
      }),
    );
    expect(completed.items).toContainEqual(
      expect.objectContaining({
        id: "run-1",
        status: "completed",
        summary: expect.objectContaining({
          kind: "run",
          stage: "completed",
          detail: "1 tool needs attention",
          stats: [
            "3 tools",
            "2 done",
            "1 failed",
            "bash x2",
            "file_edit",
            "1 file changed",
            "1 validation",
          ],
        }),
      }),
    );
  });

  test("coalesces assistant deltas into one transcript message", () => {
    const first = applyRunEvent(
      initialRunViewState,
      { type: "assistant_delta", text: "Hello" },
      ids("assistant-1"),
    ).state;
    const second = applyRunEvent(first, { type: "assistant_delta", text: " world" }).state;

    expect(second.items).toEqual([
      { id: "assistant-1", role: "assistant", text: "Hello world" },
    ]);
  });

  test("marks assistant text after tool activity as the final answer", () => {
    const started = applyRunEvent(initialRunViewState, {
      type: "run_started",
      run_id: "run-1",
    }).state;
    const toolStarted = applyRunEvent(started, {
      type: "tool_started",
      id: "tool-1",
      name: "bash",
    }).state;
    const toolCompleted = applyRunEvent(toolStarted, {
      type: "tool_completed",
      id: "tool-1",
      result_preview: "ok",
      metadata: {
        tool: "bash",
        success: true,
      },
    }).state;
    const firstDelta = applyRunEvent(
      toolCompleted,
      { type: "assistant_delta", text: "Done" },
      ids("assistant-1"),
    ).state;
    const secondDelta = applyRunEvent(firstDelta, {
      type: "assistant_delta",
      text: ".",
    }).state;

    expect(secondDelta.items).toContainEqual({
      id: "assistant-1",
      role: "assistant",
      text: "Done.",
      variant: "final",
    });
  });

  test("marks an existing assistant answer as final when the run completes", () => {
    const started = applyRunEvent(initialRunViewState, {
      type: "run_started",
      run_id: "run-1",
    }).state;
    const answered = applyRunEvent(
      started,
      { type: "assistant_delta", text: "Summary" },
      ids("assistant-1"),
    ).state;
    const completed = applyRunEvent(answered, { type: "run_completed" }).state;

    expect(completed.items).toContainEqual({
      id: "assistant-1",
      role: "assistant",
      text: "Summary",
      variant: "final",
    });
  });

  test("renders shell validation metadata as a specialized timeline summary", () => {
    const started = applyRunEvent(initialRunViewState, {
      type: "tool_started",
      id: "tool-1",
      name: "bash",
    }).state;
    const completed = applyRunEvent(started, {
      type: "tool_completed",
      id: "tool-1",
      result_preview: "ok",
      metadata: {
        tool: "bash",
        success: true,
        command: "corepack pnpm --dir apps/desktop test:ui-smoke",
        validation_family: "pnpm_test",
        command_category: "validation",
        command_kind: "package_script",
        duration_ms: 1200,
        terminal_task: { status: "completed", exit_code: 0 },
      },
    }).state;

    expect(completed.items).toContainEqual(
      expect.objectContaining({
        id: "tool-1",
        role: "timeline",
        title: "Pnpm Test",
        status: "completed",
        summary: expect.objectContaining({
          kind: "shell",
          command: "corepack pnpm --dir apps/desktop test:ui-smoke",
          validation: "Pnpm Test",
          exitCode: 0,
          duration: "1.2s",
        }),
      }),
    );
  });

  test("renders file edit metadata with diff preview", () => {
    const started = applyRunEvent(initialRunViewState, {
      type: "tool_started",
      id: "file-1",
      name: "file_edit",
    }).state;
    const completed = applyRunEvent(started, {
      type: "tool_completed",
      id: "file-1",
      result_preview: "Edited apps/desktop/src/app/App.tsx",
      metadata: {
        tool: "file_edit",
        success: true,
        path: "apps/desktop/src/app/App.tsx",
        replacements: 2,
        additions: 4,
        deletions: 1,
        diff_preview: "@@ -1 +1 @@\n-old\n+new",
        rollback_id: "cp_123",
        diagnostics_delta_status: "new_errors",
        diagnostics_delta_error_count: 1,
        diagnostics_delta_warning_count: 0,
      },
    }).state;

    expect(completed.items).toContainEqual(
      expect.objectContaining({
        id: "file-1",
        role: "timeline",
        title: "Edited file",
        summary: expect.objectContaining({
          kind: "file",
          action: "edit",
          path: "apps/desktop/src/app/App.tsx",
          replacements: 2,
          additions: 4,
          deletions: 1,
          diffPreview: "@@ -1 +1 @@\n-old\n+new",
          rollbackId: "cp_123",
          diagnosticsDelta: "new_errors",
          diagnosticsErrorDelta: 1,
          diagnosticsWarningDelta: 0,
        }),
      }),
    );
  });

  test("coalesces exact repeated file reads without hiding tool usage counts", () => {
    let state = applyRunEvent(initialRunViewState, {
      type: "run_started",
      run_id: "run-1",
    }).state;
    state = applyRunEvent(state, {
      type: "tool_started",
      id: "read-1",
      name: "file_read",
    }).state;
    state = applyRunEvent(state, {
      type: "tool_completed",
      id: "read-1",
      result_preview: "README",
      metadata: {
        tool: "file_read",
        success: true,
        path: "/Users/georgexu/Desktop/phageGPT/README.md",
        line_start: 1,
        line_end: 82,
        read_coverage: "full",
      },
    }).state;
    state = applyRunEvent(state, {
      type: "tool_started",
      id: "read-2",
      name: "file_read",
    }).state;
    state = applyRunEvent(state, {
      type: "tool_completed",
      id: "read-2",
      result_preview: "README again",
      metadata: {
        tool: "file_read",
        success: true,
        path: "/Users/georgexu/Desktop/phageGPT/README.md",
        line_start: 1,
        line_end: 82,
        read_coverage: "full",
      },
    }).state;

    const readItems = state.items.filter(
      (item) =>
        item.role === "timeline" &&
        item.kind === "tool" &&
        item.summary?.kind === "file" &&
        item.summary.action === "read",
    );
    expect(readItems).toHaveLength(1);
    expect(readItems[0]).toEqual(
      expect.objectContaining({
        summary: expect.objectContaining({
          repeatCount: 2,
          path: "/Users/georgexu/Desktop/phageGPT/README.md",
        }),
      }),
    );
    expect(state.items).toContainEqual(
      expect.objectContaining({
        id: "run-1",
        summary: expect.objectContaining({
          stats: ["2 tools", "2 done", "file_read x2"],
        }),
      }),
    );
  });

  test("renders file guardrail failures with recovery metadata", () => {
    const started = applyRunEvent(initialRunViewState, {
      type: "tool_started",
      id: "file-guardrail-1",
      name: "file_edit",
    }).state;
    const completed = applyRunEvent(started, {
      type: "tool_completed",
      id: "file-guardrail-1",
      result_preview: "Refusing file_edit for '.env'",
      metadata: {
        tool: "file_edit",
        success: false,
        path: ".env",
        failure_kind: "secret_or_credential_target",
        guardrail_reason: "target looks like an environment, credential, certificate, or SSH key file",
        recovery_action: "ask_user_for_explicit_secret_file_plan",
      },
    }).state;

    expect(completed.items).toContainEqual(
      expect.objectContaining({
        id: "file-guardrail-1",
        role: "timeline",
        title: "Edited file",
        status: "failed",
        summary: expect.objectContaining({
          kind: "failure",
          reason: expect.stringContaining("credential"),
          recovery: "ask_user_for_explicit_secret_file_plan",
        }),
      }),
    );
  });

  test("updates permission requests after approval", () => {
    const requested = applyRunEvent(initialRunViewState, {
      type: "permission_request",
      id: "permission-1",
      tool_name: "bash",
      arguments: { command: "git push" },
      prompt: "Allow git push?",
    }).state;
    const answered = appendPermissionAnswer(requested, true, true, ids("answer-1"));

    expect(answered.pendingPermission).toBeNull();
    expect(answered.items).toContainEqual(
      expect.objectContaining({
        id: "permission-1",
        role: "timeline",
        kind: "permission",
        title: "Permission approved",
        status: "completed",
        traceId: "answer-1-trace",
      }),
    );
    expect(answered.traceItems).toContainEqual(
      expect.objectContaining({
        id: "answer-1-trace",
        kind: "permission",
        title: "Permission approved",
      }),
    );
  });

  test("renders permission evidence in the timeline and trace", () => {
    const started = applyRunEvent(initialRunViewState, {
      type: "run_started",
      run_id: "run-1",
    }).state;
    const requested = applyRunEvent(started, {
      type: "permission_request",
      id: "permission-1",
      tool_name: "bash",
      arguments: { command: "git push" },
      prompt: "Allow git push?",
      metadata: {
        permission_evidence: {
          schema: "permission_decision_evidence.v1",
          request_kind: "runtime_rule",
          permission_family: "shell",
          decision: "ask",
          risk_level: "medium",
          reasons: ["command matched git remote mutation policy"],
          recovery: {
            recommended_action: "Approve once if this push is expected.",
          },
          command_classification: {
            parser_status: "simple",
            category: "git_remote_mutation",
            mutation: true,
          },
        },
      },
    }).state;

    expect(requested.items).toContainEqual(
      expect.objectContaining({
        id: "permission-1",
        role: "timeline",
        kind: "permission",
        summary: expect.objectContaining({
          kind: "permission",
          family: "shell",
          requestKind: "runtime_rule",
          risk: "medium",
          decision: "ask",
          reason: "command matched git remote mutation policy",
          recovery: "Approve once if this push is expected.",
          commandCategory: "git_remote_mutation",
          parserStatus: "simple",
          mutation: true,
        }),
      }),
    );
    expect(requested.traceItems).toContainEqual(
      expect.objectContaining({
        id: "permission-1-permission",
        detail: expect.stringContaining("risk medium"),
      }),
    );
  });

  test("keeps late permission requests visible after final text and completion", () => {
    const submitted = submitUserMessage(initialRunViewState, "Inspect timeline", [], ids("user-1"));
    let state = applyRunEvent(submitted, {
      type: "run_started",
      run_id: "run-1",
      session_id: "session-1",
    }).state;
    state = applyRunEvent(state, {
      type: "tool_started",
      id: "tool-1",
      name: "bash",
    }).state;
    state = applyRunEvent(state, {
      type: "tool_completed",
      id: "tool-1",
      result_preview: "ok",
      metadata: { tool: "bash", success: true, command: "cargo test -q" },
    }).state;
    state = applyRunEvent(state, { type: "assistant_delta", text: "Done" }, ids("assistant-1"))
      .state;
    state = applyRunEvent(state, {
      type: "permission_request",
      id: "permission-late",
      tool_name: "bash",
      arguments: { command: "git push" },
      prompt: "Allow git push to update the remote branch?",
      metadata: {
        permission_evidence: {
          request_kind: "runtime_rule",
          permission_family: "shell",
          risk_level: "medium",
        },
      },
    }).state;
    state = applyRunEvent(state, { type: "run_completed" }).state;

    expect(state.items).toContainEqual(
      expect.objectContaining({
        id: "permission-late",
        role: "timeline",
        kind: "permission",
        detail: "Allow git push to update the remote branch?",
        status: "waiting",
      }),
    );
  });

  test("does not rewrite a completed run card after a late permission answer", () => {
    const started = applyRunEvent(initialRunViewState, {
      type: "run_started",
      run_id: "run-1",
    }).state;
    const requested = applyRunEvent(started, {
      type: "permission_request",
      id: "permission-1",
      tool_name: "bash",
      arguments: { command: "git push" },
      prompt: "Allow git push?",
    }).state;
    const completed = applyRunEvent(requested, { type: "run_completed" }).state;
    const approved = appendPermissionAnswer(completed, true, true, ids("answer-1"));

    expect(completed.items).toContainEqual(
      expect.objectContaining({
        id: "run-1",
        status: "completed",
        summary: expect.objectContaining({
          kind: "run",
          stage: "completed",
          headline: "Run completed",
        }),
      }),
    );
    expect(approved.items).toContainEqual(
      expect.objectContaining({
        id: "run-1",
        status: "completed",
        summary: expect.objectContaining({
          kind: "run",
          stage: "completed",
          headline: "Run completed",
        }),
      }),
    );
  });

  test("does not rewrite a completed run card after a late tool event", () => {
    const started = applyRunEvent(initialRunViewState, {
      type: "run_started",
      run_id: "run-1",
    }).state;
    const completed = applyRunEvent(started, { type: "run_completed" }).state;
    const lateTool = applyRunEvent(completed, {
      type: "tool_completed",
      id: "late-tool",
      result_preview: "Permission approved",
      metadata: {
        tool: "bash",
        success: true,
      },
    }).state;

    expect(lateTool.items).toContainEqual(
      expect.objectContaining({
        id: "run-1",
        status: "completed",
        summary: expect.objectContaining({
          kind: "run",
          stage: "completed",
          headline: "Run completed",
        }),
      }),
    );
  });

  test("loads stored session messages and normalizes unknown roles as tool rows", () => {
    const runningState = submitUserMessage(initialRunViewState, "stuck run", [], ids("user-1"));
    const loaded = loadSessionTranscript(runningState, "session-1", [
      { id: 1, role: "user", content: "hi", created_at: "preview" },
      { id: 2, role: "assistant", content: "hello", created_at: "preview" },
      { id: 3, role: "tool", content: "ran command", created_at: "preview" },
    ]);

    expect(loaded.selectedSessionId).toBe("session-1");
    expect(loaded.isRunning).toBe(false);
    expect(loaded.pendingPermission).toBeNull();
    expect(loaded.error).toBeNull();
    expect(loaded.items).toEqual([
      { id: "message-1", role: "user", text: "hi" },
      { id: "message-2", role: "assistant", text: "hello" },
      { id: "message-3", role: "tool", text: "ran command" },
    ]);
    expect(loaded.traceItems).toEqual([
      {
        id: "loaded-session-1",
        kind: "run",
        title: "Session loaded",
        detail: "3 messages",
      },
    ]);
  });

  test("loads compact boundaries as transcript cards", () => {
    const loaded = loadSessionTranscript(
      initialRunViewState,
      "session-compact",
      [{ id: 1, role: "assistant", content: "summary", created_at: "preview" }],
      [
        {
          boundary_id: "boundary-1",
          strategy: "session_memory_compact",
          trigger: "manual compact",
          before_tokens: 10000,
          after_tokens: 4200,
          messages_before: 24,
          messages_after: 8,
          summary: "Previous coding work summarized.",
          created_at: "preview",
        },
      ],
    );

    expect(loaded.items[0]).toMatchObject({
      id: "compact-boundary-1",
      role: "timeline",
      kind: "compact",
      title: "Context compacted",
      detail: "Previous coding work summarized.",
      status: "completed",
    });
  });

  test("marks projected parts after revert target as reverted", () => {
    const loaded = loadSessionTranscript(
      initialRunViewState,
      "session-revert",
      [{ id: 1, role: "user", content: "change file", created_at: "preview" }],
      [],
      [
        {
          id: 1,
          part_index: 0,
          part_id: "tool_c1",
          kind: "tool",
          tool_call_id: "c1",
          tool_name: "file_edit",
          status: "completed",
          payload: { result_preview: "edited src/lib.rs" },
          projected_to_seq: 1,
          updated_at: "preview",
        },
        {
          id: 2,
          part_index: 1,
          part_id: "text_2",
          kind: "assistant_text",
          tool_call_id: null,
          tool_name: null,
          status: null,
          payload: { content: "Done." },
          projected_to_seq: 2,
          updated_at: "preview",
        },
        {
          id: 3,
          part_index: 2,
          part_id: "revert_3",
          kind: "revert",
          tool_call_id: null,
          tool_name: "revert",
          status: "completed",
          payload: {
            status: "completed",
            reverted_after: "tool_c1",
            restored_files: ["src/lib.rs"],
            removed_files: [],
            errors: [],
          },
          projected_to_seq: 3,
          updated_at: "preview",
        },
      ],
    );

    expect(loaded.items).toContainEqual(
      expect.objectContaining({
        id: "c1",
        role: "timeline",
        reverted: true,
        revertLabel: "Reverted",
      }),
    );
    expect(loaded.items).toContainEqual(
      expect.objectContaining({
        id: "part-text_2",
        role: "assistant",
        reverted: true,
      }),
    );
  });

  test("unrevert clears projected reverted marker", () => {
    const loaded = loadSessionTranscript(
      initialRunViewState,
      "session-unrevert",
      [],
      [],
      [
        {
          id: 1,
          part_index: 0,
          part_id: "tool_c1",
          kind: "tool",
          tool_call_id: "c1",
          tool_name: "file_edit",
          status: "completed",
          payload: { result_preview: "edited src/lib.rs" },
          projected_to_seq: 1,
          updated_at: "preview",
        },
        {
          id: 2,
          part_index: 1,
          part_id: "revert_2",
          kind: "revert",
          tool_call_id: null,
          tool_name: "revert",
          status: "completed",
          payload: { status: "completed", reverted_after: "tool_c1" },
          projected_to_seq: 2,
          updated_at: "preview",
        },
        {
          id: 3,
          part_index: 2,
          part_id: "unrevert_3",
          kind: "revert",
          tool_call_id: null,
          tool_name: "revert",
          status: "unreverted",
          payload: { status: "unreverted", reverted_after: "tool_c1" },
          projected_to_seq: 3,
          updated_at: "preview",
        },
      ],
    );

    const toolItem = loaded.items.find((item) => item.id === "c1" && item.role === "timeline");
    expect(toolItem).toBeTruthy();
    expect(toolItem?.reverted).toBeUndefined();
  });

  test("keeps ledger reuse details out of the main transcript", () => {
    let state = submitUserMessage(initialRunViewState, "再看一下 README", [], () => "user-1");
    state = applyRunEvent(
      state,
      { type: "run_started", run_id: "run-1", session_id: "session-1" },
      () => "id-1",
    ).state;
    state = applyRunEvent(
      state,
      {
        type: "assistant_delta",
        text: "这次重复读取被已有会话上下文接住了。\n\n复用依据：ledger: file `README.md` was read previously",
      },
      () => "assistant-1",
    ).state;
    state = applyRunEvent(state, { type: "run_completed" }, () => "done-1").state;

    expect(state.items).not.toContainEqual(
      expect.objectContaining({
        role: "timeline",
        kind: "compact",
        title: "Reused session context",
      }),
    );
  });
});
