import { DesktopMessage, DesktopRunEvent } from "../runtime/desktopApi";
import { PermissionRequest, TraceItem, TranscriptItem } from "./types";

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
        `Running ${event.name}`,
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
      return appendToolNote(
        state,
        event.progress,
        () => `${event.id}-progress-${createId()}`,
        traceTool(`${event.id}-progress`, "Tool progress", event.progress),
      );
    case "tool_completed":
      return appendToolNote(
        state,
        event.result_preview,
        () => `${event.id}-done`,
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
            {
              id: event.id,
              role: "tool",
              text: `Permission needed: ${event.tool_name} - ${event.prompt}`,
            },
          ],
        },
        shouldRefreshSessions: false,
      };
    case "usage":
      return {
        state: {
          ...state,
          traceItems: [
            ...state.traceItems,
            {
              id: createId(),
              kind: "usage",
              title: "Usage",
              detail: [
                `prompt ${event.prompt_tokens}`,
                `completion ${event.completion_tokens}`,
                event.reasoning_tokens ? `reasoning ${event.reasoning_tokens}` : null,
                event.cached_tokens ? `cached ${event.cached_tokens}` : null,
              ]
                .filter(Boolean)
                .join(" · "),
            },
          ],
        },
        shouldRefreshSessions: false,
      };
    case "output_truncated":
      return appendTraceOnly(state, {
        id: createId(),
        kind: "run",
        title: "Output truncated",
      });
    case "run_error":
      return {
        state: {
          ...state,
          error: event.message,
          isRunning: false,
          pendingPermission: null,
          traceItems: [
            ...state.traceItems,
            {
              id: createId(),
              kind: "error",
              title: "Run error",
              detail: event.message,
            },
          ],
        },
        shouldRefreshSessions: true,
      };
    case "run_completed":
      return {
        state: {
          ...state,
          isRunning: false,
          pendingPermission: null,
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
  return {
    ...state,
    pendingPermission: null,
    traceItems: [
      ...state.traceItems,
      {
        id: `${createId()}-trace`,
        kind: "permission",
        title: approved ? "Permission approved" : "Permission rejected",
        detail: answered ? undefined : "No pending permission request was available",
      },
    ],
    items: [
      ...state.items,
      {
        id: createId(),
        role: "tool",
        text: answered
          ? approved
            ? "Permission approved"
            : "Permission rejected"
          : "No pending permission request was available",
      },
    ],
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

function normalizeRole(role: string): TranscriptItem["role"] {
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
  text: string,
  createId: () => string,
  traceItem?: TraceItem,
): RunEventResult {
  return {
    state: {
      ...state,
      traceItems: traceItem ? [...state.traceItems, traceItem] : state.traceItems,
      items: [...state.items, { id: createId(), role: "tool", text }],
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

function traceTool(id: string, title: string, detail?: string): TraceItem {
  return {
    id,
    kind: "tool",
    title,
    detail,
  };
}
