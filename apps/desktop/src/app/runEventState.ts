import { DesktopMessage, DesktopRunEvent } from "../runtime/desktopApi";
import {
  PermissionRequest,
  TimelineStatus,
  TimelineSummary,
  TraceItem,
  TranscriptItem,
} from "./types";

export type RunViewState = {
  items: TranscriptItem[];
  traceItems: TraceItem[];
  pendingPermission: PermissionRequest | null;
  isRunning: boolean;
  error: string | null;
  selectedSessionId: string | null;
};

export type RunEventResult = {
  state: RunViewState;
  shouldRefreshSessions: boolean;
};

export const initialRunViewState: RunViewState = {
  items: [],
  traceItems: [],
  pendingPermission: null,
  isRunning: false,
  error: null,
  selectedSessionId: null,
};

export function applyRunEvent(
  state: RunViewState,
  event: DesktopRunEvent,
  createId: () => string = () => crypto.randomUUID(),
): RunEventResult {
  switch (event.type) {
    case "run_started":
      return {
        state: {
          ...state,
          selectedSessionId: event.session_id || state.selectedSessionId,
          items: [
            ...state.items,
            timelineEvent({
              id: event.run_id,
              kind: "run",
              title: "Agent run",
              detail: event.session_id ? `Session ${event.session_id}` : undefined,
              status: "running",
              traceId: event.run_id,
            }),
          ],
          traceItems: [
            ...state.traceItems,
            {
              id: event.run_id,
              kind: "run",
              title: "Run started",
              detail: event.session_id ? `session ${event.session_id}` : undefined,
            },
          ],
        },
        shouldRefreshSessions: false,
      };
    case "assistant_delta":
      return {
        state: {
          ...state,
          items: appendAssistantDelta(state.items, event.text, createId),
        },
        shouldRefreshSessions: false,
      };
    case "thinking_started":
      return appendTraceOnly(state, {
        id: createId(),
        kind: "run",
        title: "Thinking started",
      });
    case "thinking_delta":
      return appendTraceOnly(state, {
        id: createId(),
        kind: "run",
        title: "Thinking",
        detail: event.text,
      });
    case "thinking_completed":
      return appendTraceOnly(state, {
        id: createId(),
        kind: "run",
        title: "Thinking completed",
      });
    case "tool_started":
      return appendToolNote(
        state,
        {
          id: event.id,
          kind: "tool",
          title: event.name,
          detail: "Tool started",
          status: "running",
          traceId: event.id,
        },
        () => event.id,
        traceTool(event.id, `Tool started: ${event.name}`),
      );
    case "tool_args_delta":
      return appendTraceOnly(
        state,
        traceTool(`${event.id}-args-${createId()}`, "Tool args", event.delta),
      );
    case "tool_call_completed":
      return appendTraceOnly(state, traceTool(`${event.id}-call`, "Tool call prepared"));
    case "tool_execution_progress":
      return updateToolNote(
        state,
        {
          id: event.id,
          kind: "tool",
          detail: event.progress,
          status: "running",
        },
        traceTool(`${event.id}-progress`, "Tool progress", event.progress),
      );
    case "tool_completed":
      const toolPresentation = presentToolCompletion(event.result_preview, event.metadata);
      return updateToolNote(
        state,
        {
          id: event.id,
          kind: "tool",
          title: toolPresentation.title,
          detail: toolPresentation.detail,
          facts: toolPresentation.facts,
          summary: toolPresentation.summary,
          status: toolPresentation.status,
          traceId: `${event.id}-done`,
        },
        traceTool(`${event.id}-done`, "Tool completed", event.result_preview),
      );
    case "permission_request":
      return {
        state: {
          ...state,
          pendingPermission: event,
          traceItems: [
            ...state.traceItems,
            {
              id: `${event.id}-permission`,
              kind: "permission",
              title: `Permission requested: ${event.tool_name}`,
              detail: event.prompt,
            },
          ],
          items: [
            ...state.items,
            timelineEvent({
              id: event.id,
              kind: "permission",
              title: `Permission needed: ${event.tool_name}`,
              detail: event.prompt,
              status: "waiting",
              traceId: `${event.id}-permission`,
            }),
          ],
        },
        shouldRefreshSessions: false,
      };
    case "usage":
      const usageDetail = [
        `prompt ${event.prompt_tokens}`,
        `completion ${event.completion_tokens}`,
        event.reasoning_tokens ? `reasoning ${event.reasoning_tokens}` : null,
        event.cached_tokens ? `cached ${event.cached_tokens}` : null,
      ]
        .filter(Boolean)
        .join(" · ");
      const usageId = createId();
      return {
        state: {
          ...state,
          items: [
            ...state.items,
            timelineEvent({
              id: usageId,
              kind: "usage",
              title: "Token usage",
              detail: usageDetail,
              status: "info",
              traceId: usageId,
            }),
          ],
          traceItems: [
            ...state.traceItems,
            {
              id: usageId,
              kind: "usage",
              title: "Usage",
              detail: usageDetail,
            },
          ],
        },
        shouldRefreshSessions: false,
      };
    case "output_truncated": {
      const traceId = createId();
      return appendTimelineAndTrace(state, timelineEvent({
        id: traceId,
        kind: "run",
        title: "Output truncated",
        detail: "Open trace for the complete runtime details.",
        status: "info",
        traceId,
      }), {
        id: traceId,
        kind: "run",
        title: "Output truncated",
      });
    }
    case "run_error": {
      const traceId = createId();
      return {
        state: {
          ...state,
          error: event.message,
          isRunning: false,
          pendingPermission: null,
          items: [
            ...state.items,
            timelineEvent({
              id: traceId,
              kind: "error",
              title: "Run error",
              detail: event.message,
              status: "failed",
              traceId,
            }),
          ],
          traceItems: [
            ...state.traceItems,
            {
              id: traceId,
              kind: "error",
              title: "Run error",
              detail: event.message,
            },
          ],
        },
        shouldRefreshSessions: true,
      };
    }
    case "run_completed":
      return {
        state: {
          ...state,
          isRunning: false,
          pendingPermission: null,
          items: completeLatestRun(state.items),
          traceItems: [
            ...state.traceItems,
            {
              id: createId(),
              kind: "run",
              title: "Run completed",
            },
          ],
        },
        shouldRefreshSessions: true,
      };
  }
}

export function submitUserMessage(
  state: RunViewState,
  text: string,
  createId: () => string = () => crypto.randomUUID(),
): RunViewState {
  return {
    ...state,
    error: null,
    isRunning: true,
    items: [...state.items, { id: createId(), role: "user", text }],
    traceItems: [],
  };
}

export function loadSessionTranscript(
  state: RunViewState,
  sessionId: string,
  messages: DesktopMessage[],
): RunViewState {
  return {
    ...state,
    selectedSessionId: sessionId,
    items: messages.map(messageToTranscriptItem),
    traceItems: [
      {
        id: `loaded-${sessionId}`,
        kind: "run",
        title: "Session loaded",
        detail: `${messages.length} messages`,
      },
    ],
    pendingPermission: null,
    error: null,
  };
}

export function appendPermissionAnswer(
  state: RunViewState,
  approved: boolean,
  answered: boolean,
  createId: () => string = () => `permission-${Date.now()}`,
): RunViewState {
  const permissionId = state.pendingPermission?.id;
  const answerTitle = answered
    ? approved
      ? "Permission approved"
      : "Permission rejected"
    : "No pending permission request was available";
  const answerStatus: TimelineStatus = approved ? "completed" : "failed";
  const answerTraceId = `${createId()}-trace`;
  const updatedItems =
    answered && permissionId
      ? state.items.map((item) => {
          if (item.role !== "timeline" || item.id !== permissionId) {
            return item;
          }
          return {
            ...item,
            title: answerTitle,
            status: answerStatus,
            traceId: answerTraceId,
          };
        })
      : updateLatestWaitingPermission(
          state.items,
          answerTitle,
          answerStatus,
          answerTraceId,
          createId,
        );

  return {
    ...state,
    pendingPermission: null,
    traceItems: [
      ...state.traceItems,
      {
        id: answerTraceId,
        kind: "permission",
        title: approved ? "Permission approved" : "Permission rejected",
        detail: answered ? undefined : "No pending permission request was available",
      },
    ],
    items: updatedItems,
  };
}

export function withError(state: RunViewState, error: unknown): RunViewState {
  return {
    ...state,
    error: String(error),
    isRunning: false,
  };
}

function messageToTranscriptItem(message: DesktopMessage): TranscriptItem {
  const role = normalizeRole(message.role);
  return {
    id: `message-${message.id}`,
    role,
    text: message.content,
  };
}

type MessageTranscriptRole = Exclude<TranscriptItem["role"], "timeline">;

function normalizeRole(role: string): MessageTranscriptRole {
  if (role === "user" || role === "assistant") {
    return role;
  }

  return "tool";
}

function appendAssistantDelta(
  items: TranscriptItem[],
  text: string,
  createId: () => string,
): TranscriptItem[] {
  const last = items[items.length - 1];
  if (last?.role === "assistant") {
    return [...items.slice(0, -1), { ...last, text: last.text + text }];
  }

  return [...items, { id: createId(), role: "assistant", text }];
}

function appendToolNote(
  state: RunViewState,
  event: Omit<Extract<TranscriptItem, { role: "timeline" }>, "role"> & { title: string },
  createId: () => string,
  traceItem?: TraceItem,
): RunEventResult {
  return {
    state: {
      ...state,
      traceItems: traceItem ? [...state.traceItems, traceItem] : state.traceItems,
      items: [...state.items, timelineEvent({ ...event, id: event.id || createId() })],
    },
    shouldRefreshSessions: false,
  };
}

function updateToolNote(
  state: RunViewState,
  patch: Partial<Omit<Extract<TranscriptItem, { role: "timeline" }>, "role">> & {
    id: string;
    kind: "tool";
  },
  traceItem?: TraceItem,
): RunEventResult {
  return {
    state: {
      ...state,
      items: state.items.map((item) => {
        if (item.role !== "timeline" || item.id !== patch.id) {
          return item;
        }
        return {
          ...item,
          ...patch,
          title: patch.title || item.title,
        };
      }),
      traceItems: traceItem ? [...state.traceItems, traceItem] : state.traceItems,
    },
    shouldRefreshSessions: false,
  };
}

function appendTimelineAndTrace(
  state: RunViewState,
  item: Extract<TranscriptItem, { role: "timeline" }>,
  traceItem: TraceItem,
): RunEventResult {
  return {
    state: {
      ...state,
      items: [...state.items, item],
      traceItems: [...state.traceItems, traceItem],
    },
    shouldRefreshSessions: false,
  };
}

function appendTraceOnly(state: RunViewState, traceItem: TraceItem): RunEventResult {
  return {
    state: {
      ...state,
      traceItems: [...state.traceItems, traceItem],
    },
    shouldRefreshSessions: false,
  };
}

function timelineEvent(
  item: Omit<Extract<TranscriptItem, { role: "timeline" }>, "role">,
): Extract<TranscriptItem, { role: "timeline" }> {
  return {
    role: "timeline",
    ...item,
  };
}

function completeLatestRun(items: TranscriptItem[]): TranscriptItem[] {
  let index = -1;
  for (let itemIndex = items.length - 1; itemIndex >= 0; itemIndex -= 1) {
    const item = items[itemIndex];
    if (item.role === "timeline" && item.kind === "run" && item.status === "running") {
      index = itemIndex;
      break;
    }
  }
  if (index < 0) {
    return [
      ...items,
      timelineEvent({
        id: `run-completed-${Date.now()}`,
        kind: "run",
        title: "Agent run",
        status: "completed",
      }),
    ];
  }

  const nextItems = [...items];
  const item = nextItems[index];
  if (item.role === "timeline") {
    nextItems[index] = {
      ...item,
      detail: item.detail || "Completed",
      status: "completed",
    };
  }
  return nextItems;
}

function updateLatestWaitingPermission(
  items: TranscriptItem[],
  title: string,
  status: "completed" | "failed",
  traceId: string,
  createId: () => string,
): TranscriptItem[] {
  let index = -1;
  for (let itemIndex = items.length - 1; itemIndex >= 0; itemIndex -= 1) {
    const item = items[itemIndex];
    if (item.role === "timeline" && item.kind === "permission" && item.status === "waiting") {
      index = itemIndex;
      break;
    }
  }
  if (index < 0) {
    return [
      ...items,
      timelineEvent({
        id: createId(),
        kind: "permission",
        title,
        status,
        traceId,
      }),
    ];
  }

  const nextItems = [...items];
  const item = nextItems[index];
  if (item.role === "timeline") {
    nextItems[index] = {
      ...item,
      title,
      status,
      traceId,
    };
  }
  return nextItems;
}

function traceTool(id: string, title: string, detail?: string): TraceItem {
  return {
    id,
    kind: "tool",
    title,
    detail,
  };
}

type ToolPresentation = {
  title: string;
  detail?: string;
  facts?: string[];
  summary?: TimelineSummary;
  status: "completed" | "failed";
};

type ToolSummary = {
  tool?: string;
  success?: boolean;
  duration_ms?: number;
  command?: string;
  command_category?: string;
  validation_family?: string;
  command_kind?: string;
  path?: string;
  pattern?: string;
  action?: string;
  replacements?: number;
  operations?: number;
  additions?: number;
  deletions?: number;
  diff_preview?: string;
  diff_preview_truncated?: boolean;
  output_chars?: number;
  terminal_task?: Record<string, unknown>;
  terminal_tasks_count?: number;
  error_preview?: string;
  recovery_action?: string;
  user_note?: string;
};

function presentToolCompletion(resultPreview: string, metadata: unknown): ToolPresentation {
  const summary = toolSummary(metadata);
  if (!summary) {
    return {
      title: "Tool completed",
      detail: resultPreview,
      status: "completed",
    };
  }

  const status = summary.success === false ? "failed" : "completed";
  const title = toolTitle(summary);
  const detail = toolDetail(summary, resultPreview);
  const facts = toolFacts(summary);
  const specialSummary = timelineSummary(summary, resultPreview);

  return {
    title,
    detail,
    facts,
    summary: specialSummary,
    status,
  };
}

function toolSummary(metadata: unknown): ToolSummary | null {
  if (!isRecord(metadata)) {
    return null;
  }
  return metadata as ToolSummary;
}

function toolTitle(summary: ToolSummary): string {
  switch (summary.tool) {
    case "bash":
      return summary.validation_family
        ? validationLabel(summary.validation_family)
        : "Shell command";
    case "file_edit":
      return "Edited file";
    case "file_write":
      return "Wrote file";
    case "file_read":
      return "Read file";
    case "file_patch":
      return "Patched files";
    case "grep":
      return "Searched project";
    case "git":
      return summary.action ? `Git ${summary.action}` : "Git";
    default:
      return summary.tool || "Tool completed";
  }
}

function toolDetail(summary: ToolSummary, resultPreview: string): string | undefined {
  if (summary.error_preview) {
    return summary.error_preview;
  }
  if (summary.command) {
    return summary.command;
  }
  if (summary.path) {
    return summary.path;
  }
  if (summary.pattern) {
    return summary.pattern;
  }
  return resultPreview || undefined;
}

function toolFacts(summary: ToolSummary): string[] {
  const facts = compactFacts([
    summary.tool ? `tool ${summary.tool}` : null,
    summary.validation_family ? `validation ${summary.validation_family}` : null,
    summary.command_category ? `category ${summary.command_category}` : null,
    summary.command_kind ? `kind ${summary.command_kind}` : null,
    summary.path ? `path ${summary.path}` : null,
    summary.replacements !== undefined ? `${summary.replacements} replacements` : null,
    summary.operations !== undefined ? `${summary.operations} operations` : null,
    summary.action ? `action ${summary.action}` : null,
    durationFact(summary.duration_ms),
    terminalFact(summary.terminal_task),
    summary.terminal_tasks_count ? `${summary.terminal_tasks_count} terminal tasks` : null,
    summary.output_chars !== undefined ? `${summary.output_chars} chars` : null,
  ]);

  return facts.slice(0, 6);
}

function timelineSummary(summary: ToolSummary, resultPreview: string): TimelineSummary | undefined {
  if (summary.success === false) {
    return {
      kind: "failure",
      reason: summary.error_preview || resultPreview || "Tool failed",
      recovery: summary.user_note || summary.recovery_action,
      outputPreview: resultPreview,
      outputTruncated: resultPreview.length >= 2000,
    };
  }

  if (summary.tool === "bash" && summary.command) {
    return {
      kind: "shell",
      command: summary.command,
      validation: summary.validation_family
        ? validationLabel(summary.validation_family)
        : summary.command_category,
      exitCode: terminalExitCode(summary.terminal_task),
      duration: durationFact(summary.duration_ms) || undefined,
    };
  }

  if (isFileTool(summary.tool)) {
    return {
      kind: "file",
      action: fileAction(summary.tool),
      path: summary.path,
      operations: summary.operations,
      replacements: summary.replacements,
      additions: summary.additions,
      deletions: summary.deletions,
      diffPreview: summary.diff_preview,
      diffTruncated: summary.diff_preview_truncated,
    };
  }

  return undefined;
}

function isFileTool(tool: string | undefined): tool is "file_read" | "file_write" | "file_edit" | "file_patch" {
  return tool === "file_read" || tool === "file_write" || tool === "file_edit" || tool === "file_patch";
}

function fileAction(tool: "file_read" | "file_write" | "file_edit" | "file_patch") {
  switch (tool) {
    case "file_read":
      return "read";
    case "file_write":
      return "write";
    case "file_edit":
      return "edit";
    case "file_patch":
      return "patch";
  }
}

function terminalFact(task: Record<string, unknown> | undefined) {
  if (!task) {
    return null;
  }
  const exitCode = typeof task.exit_code === "number" ? task.exit_code : null;
  const status = typeof task.status === "string" ? task.status : null;
  if (exitCode !== null) {
    return `exit ${exitCode}`;
  }
  return status ? `terminal ${status}` : null;
}

function terminalExitCode(task: Record<string, unknown> | undefined) {
  if (!task) {
    return undefined;
  }
  return typeof task.exit_code === "number" ? task.exit_code : undefined;
}

function durationFact(durationMs: number | undefined) {
  if (typeof durationMs !== "number") {
    return null;
  }
  if (durationMs >= 1000) {
    return `${(durationMs / 1000).toFixed(1)}s`;
  }
  return `${durationMs}ms`;
}

function validationLabel(value: string) {
  return value
    .split("_")
    .filter(Boolean)
    .map((part) => part[0].toUpperCase() + part.slice(1))
    .join(" ");
}

function compactFacts(values: Array<string | null | undefined>) {
  return values.filter((value): value is string => Boolean(value && value.trim()));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
