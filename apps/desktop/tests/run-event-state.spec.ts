import { expect, test } from "@playwright/test";
import {
  appendPermissionAnswer,
  applyRunEvent,
  initialRunViewState,
  loadSessionTranscript,
  submitUserMessage,
} from "../src/app/runEventState";

function ids(...values: string[]) {
  let index = 0;
  return () => values[index++] || `id-${index}`;
}

test.describe("run event state", () => {
  test("submits a user message and completes the active run", () => {
    const submitted = submitUserMessage(initialRunViewState, "Inspect UI", ids("user-1"));

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

  test("loads stored session messages and normalizes unknown roles as tool rows", () => {
    const loaded = loadSessionTranscript(initialRunViewState, "session-1", [
      { id: 1, role: "user", content: "hi", created_at: "preview" },
      { id: 2, role: "assistant", content: "hello", created_at: "preview" },
      { id: 3, role: "tool", content: "ran command", created_at: "preview" },
    ]);

    expect(loaded.selectedSessionId).toBe("session-1");
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
});
