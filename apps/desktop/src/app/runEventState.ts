import {
  DesktopCompactBoundary,
  DesktopMessage,
  DesktopRunContext,
  DesktopRunEvent,
} from "../runtime/desktopApi";
import {
  runtimeDiagnosticDetail,
  runtimeDiagnosticFacts,
  runtimeDiagnosticRunStage,
} from "./runtimeDiagnosticPresentation";
import {
  runtimeStatsFromRunSummary,
  timelineTools,
  toolExecutionCount,
  toolUsageStats,
} from "./toolTimelinePresentation";
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
  pendingRunContexts: DesktopRunContext[];
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
  pendingRunContexts: [],
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
          error: null,
          selectedSessionId: event.session_id || state.selectedSessionId,
          items: [
            ...state.items,
            timelineEvent({
              id: event.run_id,
              kind: "run",
              title: "Agent run",
              detail: event.session_id ? `Session ${event.session_id}` : undefined,
              summary: {
                kind: "run",
                stage: "running",
                headline: "Runtime connected",
                detail: "Preparing model and tool events",
                sessionId: event.session_id ?? undefined,
                contexts: state.pendingRunContexts,
              },
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
              contexts: state.pendingRunContexts,
            },
          ],
        },
        shouldRefreshSessions: false,
      };
    case "assistant_delta":
      return {
        state: {
          ...state,
          error: null,
          items: appendAssistantDelta(state.items, event.text, createId),
        },
        shouldRefreshSessions: false,
      };
    case "thinking_started":
      return {
        state: {
          ...state,
          error: null,
          items: appendReasoning(state.items, "", true, createId),
          traceItems: appendTrace(state.traceItems, {
            id: createId(),
            kind: "run",
            title: "Thinking started",
          }),
        },
        shouldRefreshSessions: false,
      };
    case "thinking_delta":
      return {
        state: {
          ...state,
          error: null,
          items: appendReasoning(state.items, event.text, true, createId),
        },
        shouldRefreshSessions: false,
      };
    case "thinking_completed":
      return {
        state: {
          ...state,
          items: finalizeReasoning(state.items),
          traceItems: appendTrace(state.traceItems, {
            id: createId(),
            kind: "run",
            title: "Thinking completed",
          }),
        },
        shouldRefreshSessions: false,
      };
    case "tool_started": {
      const result = appendToolNote(
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
      return {
        ...result,
        state: updateLatestOpenRunSummary(result.state, {
          stage: "running",
          headline: "Running tool",
          detail: event.name,
          stats: runStats(result.state.items),
        }),
      };
    }
    case "tool_args_delta":
      return appendTraceOnly(
        state,
        traceTool(`${event.id}-args-${createId()}`, "Tool args", event.delta),
      );
    case "tool_call_completed":
      return appendTraceOnly(state, traceTool(`${event.id}-call`, "Tool call prepared"));
    case "tool_execution_progress": {
      const result = updateToolNote(
        state,
        {
          id: event.id,
          kind: "tool",
          detail: event.progress,
          status: "running",
        },
        traceTool(`${event.id}-progress`, "Tool progress", event.progress),
      );
      return {
        ...result,
        state: updateLatestOpenRunSummary(result.state, {
          stage: "running",
          headline: "Tool in progress",
          detail: event.progress,
          stats: runStats(result.state.items),
        }),
      };
    }
    case "tool_completed": {
      const toolPresentation = presentToolCompletion(event.result_preview, event.metadata);
      const result = updateToolNote(
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
        traceTool(`${event.id}-done`, toolPresentation.title, event.result_preview, {
          facts: toolPresentation.facts,
          status: toolPresentation.status,
          summary: toolPresentation.summary,
        }),
      );
      const coalescedState = {
        ...result.state,
        items: coalesceRepeatedReadToolItems(result.state.items),
      };
      return {
        ...result,
        state: updateLatestOpenRunSummary(coalescedState, {
          stage: toolPresentation.status === "failed" ? "failed" : "running",
          headline: toolPresentation.status === "failed" ? "Tool failed" : "Tool completed",
          detail: toolPresentation.title,
          recovery: runRecovery(toolPresentation.summary),
          stats: runStats(coalescedState.items),
        }),
      };
    }
    case "permission_request": {
      const permissionSummary = permissionTimelineSummary(event);
      const permissionDetail = permissionSummary?.reason || event.prompt;
      const updatedRunState = updateLatestRunSummary(state, {
        stage: "waiting",
        headline: "Waiting for permission",
        detail: permissionDetail,
        stats: runStats(state.items),
      });
      return {
        state: {
          ...updatedRunState,
          pendingPermission: event,
          traceItems: [
            ...updatedRunState.traceItems,
            {
              id: `${event.id}-permission`,
              kind: "permission",
              title: `Permission requested: ${event.tool_name}`,
              detail: permissionTraceDetail(event, permissionSummary),
              facts: [event.prompt],
              status: "waiting",
              summary: permissionSummary,
            },
          ],
          items: [
            ...updatedRunState.items,
            timelineEvent({
              id: event.id,
              kind: "permission",
              title: `Permission needed: ${event.tool_name}`,
              detail: event.prompt,
              facts: [event.prompt],
              summary: permissionSummary,
              status: "waiting",
              traceId: `${event.id}-permission`,
            }),
          ],
        },
        shouldRefreshSessions: false,
      };
    }
    case "runtime_diagnostic": {
      const detail = runtimeDiagnosticDetail(event.diagnostic);
      const runtimeFacts = runtimeDiagnosticFacts(event.diagnostic);
      const updatedRunState = updateLatestOpenRunSummary(state, {
        stage: runtimeDiagnosticRunStage(event.diagnostic),
        headline: "Runtime diagnostic",
        detail,
        stats: compactFacts([...runStats(state.items), ...runtimeFacts]),
      });
      return appendTraceOnly(updatedRunState, {
        id: createId(),
        kind: "runtime",
        title: "Runtime diagnostic",
        detail,
        runtime: event.diagnostic,
      });
    }
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
    case "closeout": {
      const traceId = createId();
      const verified = event.status === "verified" || event.status === "not_applicable";
      const partial = event.status === "partial";
      const failed = !verified && !partial;
      return appendTimelineAndTrace(
        updateLatestOpenRunSummary(state, {
          stage: failed ? "failed" : "completed",
          headline: `Closeout ${event.status}`,
          detail: event.evidence_summary || "Final verification status recorded",
          stats: runStats(state.items),
        }),
        timelineEvent({
          id: traceId,
          kind: "run",
          title: `Closeout ${event.status}`,
          detail: event.evidence_summary || undefined,
          status: failed ? "failed" : "info",
          traceId,
        }),
        {
          id: traceId,
          kind: "runtime",
          title: `Closeout ${event.status}`,
          detail: event.evidence_summary || undefined,
        },
      );
    }
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
          ...updateLatestRunSummary(state, {
            stage: "failed",
            headline: "Run failed",
            detail: event.message,
            recovery: "Inspect the error details, fix the runtime issue, then rerun.",
            stats: runStats(state.items),
          }),
          error: event.message,
          isRunning: false,
          pendingPermission: null,
          pendingRunContexts: [],
          items: [
            ...updateLatestRunSummary(state, {
              stage: "failed",
              headline: "Run failed",
              detail: event.message,
              recovery: "Inspect the error details, fix the runtime issue, then rerun.",
              stats: runStats(state.items),
            }).items,
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
          error: null,
          isRunning: false,
          pendingPermission: null,
          pendingRunContexts: [],
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
  contexts: DesktopRunContext[] = [],
  createId: () => string = () => crypto.randomUUID(),
): RunViewState {
  return {
    ...state,
    error: null,
    isRunning: true,
    pendingRunContexts: contexts,
    items: [...state.items, { id: createId(), role: "user", text }],
    traceItems: [],
  };
}

export function loadSessionTranscript(
  state: RunViewState,
  sessionId: string,
  messages: DesktopMessage[],
  compactBoundaries: DesktopCompactBoundary[] = [],
): RunViewState {
  return {
    ...state,
    isRunning: false,
    selectedSessionId: sessionId,
    pendingRunContexts: [],
    items: [...compactBoundaries.map(compactBoundaryToTranscriptItem), ...messages.map(messageToTranscriptItem)],
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

function compactBoundaryToTranscriptItem(boundary: DesktopCompactBoundary): TranscriptItem {
  const savedTokens = Math.max(0, boundary.before_tokens - boundary.after_tokens);
  const stats = [
    boundary.strategy,
    savedTokens > 0 ? `saved ${formatTokenCount(savedTokens)} tokens` : undefined,
    boundary.messages_before > 0
      ? `${boundary.messages_before} -> ${boundary.messages_after} messages`
      : undefined,
  ].filter(Boolean) as string[];

  return {
    id: `compact-${boundary.boundary_id}`,
    role: "timeline",
    kind: "compact",
    title: "Context compacted",
    detail: boundary.summary || boundary.trigger || boundary.created_at,
    facts: stats,
    status: "completed",
    traceId: `compact-${boundary.boundary_id}`,
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
  const runSummary = {
    stage: approved ? "running" : "failed",
    headline: approved ? "Permission approved" : "Permission rejected",
    detail: answered ? undefined : "No pending permission request was available",
    stats: runStats(updatedItems),
  } satisfies Omit<Extract<TimelineSummary, { kind: "run" }>, "kind" | "sessionId">;
  const updatedRunState = updateLatestOpenRunSummary({ ...state, items: updatedItems }, runSummary);

  return {
    ...updatedRunState,
    pendingPermission: null,
    pendingRunContexts: approved ? updatedRunState.pendingRunContexts : [],
    traceItems: [
      ...state.traceItems,
      {
        id: answerTraceId,
        kind: "permission",
        title: approved ? "Permission approved" : "Permission rejected",
        detail: answered ? undefined : "No pending permission request was available",
        status: answerStatus,
      },
    ],
  };
}

export function withError(state: RunViewState, error: unknown): RunViewState {
  return {
    ...state,
    error: String(error),
    isRunning: false,
    pendingRunContexts: [],
  };
}

export function withRunIdleWarning(state: RunViewState, warning: unknown): RunViewState {
  if (!state.isRunning) {
    return state;
  }
  return {
    ...state,
    error: String(warning),
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

function appendReasoning(
  items: TranscriptItem[],
  text: string,
  streaming: boolean,
  createId: () => string,
): TranscriptItem[] {
  const last = items[items.length - 1];
  if (last?.role === "reasoning") {
    return [
      ...items.slice(0, -1),
      { ...last, text: last.text + text, streaming },
    ];
  }
  return [...items, { id: createId(), role: "reasoning", text, streaming }];
}

function finalizeReasoning(items: TranscriptItem[]): TranscriptItem[] {
  const last = items[items.length - 1];
  if (last?.role === "reasoning") {
    return [...items.slice(0, -1), { ...last, streaming: false }];
  }
  return items;
}

function appendTrace(traceItems: TraceItem[], item: TraceItem): TraceItem[] {
  return [...traceItems, item];
}

function appendAssistantDelta(
  items: TranscriptItem[],
  text: string,
  createId: () => string,
): TranscriptItem[] {
  const last = items[items.length - 1];
  if (last?.role === "assistant") {
    if (last.text === text) {
      return items;
    }

    return [
      ...items.slice(0, -1),
      {
        ...last,
        text: last.text + text,
        variant: last.variant || (hasRunActivitySinceLastUser(items) ? "final" : undefined),
      },
    ];
  }

  return [
    ...items,
    {
      id: createId(),
      role: "assistant",
      text,
      variant: hasRunActivitySinceLastUser(items) ? "final" : undefined,
    },
  ];
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

function updateLatestRunSummary(
  state: RunViewState,
  patch: Omit<Extract<TimelineSummary, { kind: "run" }>, "kind" | "sessionId">,
): RunViewState {
  return updateLatestRunSummaryMatching(state, patch, () => true);
}

function updateLatestOpenRunSummary(
  state: RunViewState,
  patch: Omit<Extract<TimelineSummary, { kind: "run" }>, "kind" | "sessionId">,
): RunViewState {
  return updateLatestRunSummaryMatching(state, patch, (item) => item.status !== "completed");
}

function updateLatestRunSummaryMatching(
  state: RunViewState,
  patch: Omit<Extract<TimelineSummary, { kind: "run" }>, "kind" | "sessionId">,
  matches: (item: Extract<TranscriptItem, { role: "timeline" }>) => boolean,
): RunViewState {
  let index = -1;
  for (let itemIndex = state.items.length - 1; itemIndex >= 0; itemIndex -= 1) {
    const item = state.items[itemIndex];
    if (item.role === "timeline" && item.kind === "run" && matches(item)) {
      index = itemIndex;
      break;
    }
  }
  if (index < 0) {
    return state;
  }

  const nextItems = [...state.items];
  const item = nextItems[index];
  if (item.role === "timeline") {
    const previousSummary =
      item.summary?.kind === "run"
        ? item.summary
        : {
            kind: "run" as const,
            stage: patch.stage,
            headline: patch.headline,
          };
    nextItems[index] = {
      ...item,
      summary: {
        ...previousSummary,
        ...patch,
        stats: patch.stats
          ? uniqueToolValues([...patch.stats, ...runtimeStatsFromRunSummary(previousSummary)])
          : previousSummary.stats,
      },
    };
  }
  return {
    ...state,
    items: nextItems,
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
    const completedItems = markLatestAssistantAsFinal(items);
    if (runStats(items).length === 0) {
      return completedItems;
    }
    return [
      ...completedItems,
      timelineEvent({
        id: `run-completed-${Date.now()}`,
        kind: "run",
        title: "Agent run",
        summary: {
          kind: "run",
          stage: "completed",
          headline: "Run completed",
          detail: "No active run was found in the transcript",
          stats: runStats(items),
        },
        status: "completed",
      }),
    ];
  }

  const nextItems = [...markLatestAssistantAsFinal(items)];
  const item = nextItems[index];
  if (item.role === "timeline") {
    if (item.summary?.kind === "run" && item.summary.headline.startsWith("Closeout ")) {
      nextItems[index] = {
        ...item,
        detail: item.detail || item.summary.detail || "Completed",
        status: item.summary.stage === "failed" ? "failed" : "completed",
      };
      return nextItems;
    }
    nextItems[index] = {
      ...item,
      detail: item.detail || "Completed",
      summary: {
        kind: "run",
        stage: "completed",
        headline: "Run completed",
        detail: runCompletionDetail(nextItems),
        sessionId: item.summary?.kind === "run" ? item.summary.sessionId : undefined,
        stats: compactFacts([
          ...runStats(nextItems),
          ...runtimeStatsFromRunSummary(item.summary),
        ]),
        contexts: item.summary?.kind === "run" ? item.summary.contexts : undefined,
      },
      status: "completed",
    };
  }
  return nextItems;
}

function markLatestAssistantAsFinal(items: TranscriptItem[]): TranscriptItem[] {
  const lastAssistantIndex = findLatestIndex(items, (item) => item.role === "assistant");
  if (lastAssistantIndex < 0 || !hasRunActivitySinceLastUser(items)) {
    return items;
  }

  const nextItems = [...items];
  const item = nextItems[lastAssistantIndex];
  if (item.role === "assistant") {
    nextItems[lastAssistantIndex] = {
      ...item,
      variant: "final",
    };
  }
  return nextItems;
}

function hasRunActivitySinceLastUser(items: TranscriptItem[]): boolean {
  for (let index = items.length - 1; index >= 0; index -= 1) {
    const item = items[index];
    if (item.role === "user") {
      return false;
    }
    if (item.role === "timeline" && (item.kind === "run" || item.kind === "tool")) {
      return true;
    }
  }
  return false;
}

function findLatestIndex(
  items: TranscriptItem[],
  predicate: (item: TranscriptItem) => boolean,
): number {
  for (let index = items.length - 1; index >= 0; index -= 1) {
    if (predicate(items[index])) {
      return index;
    }
  }
  return -1;
}

function runCompletionDetail(items: TranscriptItem[]): string {
  const failedTools = timelineTools(items).filter((item) => item.status === "failed").length;
  if (failedTools > 0) {
    return `${failedTools} tool${failedTools === 1 ? " needs" : "s need"} attention`;
  }
  return "Conversation and session state refreshed";
}

function runRecovery(summary: TimelineSummary | undefined): string | undefined {
  if (summary?.kind !== "failure") {
    return undefined;
  }
  return summary.recovery || "Inspect the failing output, fix the issue, then rerun.";
}

function runStats(items: TranscriptItem[]): string[] {
  const tools = timelineTools(items);
  const toolCount = tools.reduce((sum, item) => sum + toolExecutionCount(item), 0);
  const completed = tools
    .filter((item) => item.status === "completed")
    .reduce((sum, item) => sum + toolExecutionCount(item), 0);
  const failed = tools
    .filter((item) => item.status === "failed")
    .reduce((sum, item) => sum + toolExecutionCount(item), 0);
  const running = tools
    .filter((item) => item.status === "running")
    .reduce((sum, item) => sum + toolExecutionCount(item), 0);
  const fileChanges = uniqueToolValues(
    tools
      .map((item) => item.summary)
      .filter((summary): summary is Extract<TimelineSummary, { kind: "file" }> =>
        summary?.kind === "file" && summary.action !== "read",
      )
      .map((summary) => summary.path || summary.action),
  ).length;
  const validations = tools.filter((item) => item.summary?.kind === "shell" && item.summary.validation)
    .length;

  return compactFacts([
    toolCount > 0 ? `${toolCount} tool${toolCount === 1 ? "" : "s"}` : null,
    running > 0 ? `${running} running` : null,
    completed > 0 ? `${completed} done` : null,
    failed > 0 ? `${failed} failed` : null,
    ...toolUsageStats(tools),
    fileChanges > 0 ? `${fileChanges} file${fileChanges === 1 ? "" : "s"} changed` : null,
    validations > 0 ? `${validations} validation${validations === 1 ? "" : "s"}` : null,
  ]);
}

function coalesceRepeatedReadToolItems(items: TranscriptItem[]): TranscriptItem[] {
  const next: TranscriptItem[] = [];
  const readIndexes = new Map<string, number>();
  for (const item of items) {
    const key = repeatedReadKey(item);
    if (!key) {
      next.push(item);
      continue;
    }
    const existingIndex = readIndexes.get(key);
    if (existingIndex === undefined) {
      readIndexes.set(key, next.length);
      next.push(item);
      continue;
    }

    const existing = next[existingIndex];
    if (existing?.role !== "timeline" || existing.summary?.kind !== "file") {
      next.push(item);
      continue;
    }
    const repeated = existing as Extract<TranscriptItem, { role: "timeline" }> & {
      summary: Extract<TimelineSummary, { kind: "file" }>;
    };
    const current = item as Extract<TranscriptItem, { role: "timeline" }> & {
      summary: Extract<TimelineSummary, { kind: "file" }>;
    };
    const itemRepeat = current.summary.repeatCount || 1;
    const existingRepeat = repeated.summary.repeatCount || 1;
    next[existingIndex] = {
      ...repeated,
      detail: repeated.detail || current.detail,
      status: current.status || repeated.status,
      traceId: current.traceId || repeated.traceId,
      summary: {
        ...repeated.summary,
        repeatCount: existingRepeat + itemRepeat,
      },
    };
  }
  return next;
}

function repeatedReadKey(item: TranscriptItem): string | null {
  if (
    item.role !== "timeline" ||
    item.kind !== "tool" ||
    item.status !== "completed" ||
    item.summary?.kind !== "file" ||
    item.summary.action !== "read" ||
    !item.summary.path
  ) {
    return null;
  }
  return [
    "file_read",
    item.summary.path,
    item.summary.lineStart || "",
    item.summary.lineEnd || "",
    item.summary.readCoverage || "",
  ].join(":");
}

function uniqueToolValues(values: string[]): string[] {
  return Array.from(new Set(values.filter((value) => value.trim())));
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

function traceTool(
  id: string,
  title: string,
  detail?: string,
  extras: Partial<Pick<TraceItem, "facts" | "status" | "summary">> = {},
): TraceItem {
  return {
    id,
    kind: "tool",
    title,
    detail,
    ...extras,
  };
}

type ToolPresentation = {
  title: string;
  detail?: string;
  facts?: string[];
  summary?: TimelineSummary;
  status: "completed" | "failed";
};

type PermissionEvidenceSummary = {
  permission_family?: string;
  request_kind?: string;
  risk_level?: string;
  decision?: string;
  reasons?: unknown;
  reason?: string;
  recovery?: unknown;
  command_classification?: unknown;
  allowed_always_rules?: unknown;
};

type PermissionReviewSummary = {
  risk?: string;
  reason?: string;
  recovery_hint?: string;
  risk_facts?: unknown;
  matched_rules?: unknown;
  classifier_result?: unknown;
};

type ActionReviewDesktopSummary = {
  decision?: string;
  reason?: string;
  permission?: string;
  risk?: string;
  recovery?: string;
  sideEffect?: string;
  network?: string;
  checkpoint?: string;
  checkpointApproval?: boolean;
  scopeAllowed?: boolean;
  budgetAllowed?: boolean;
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
  checkpoint_id?: string;
  rollback_id?: string;
  failure_kind?: string;
  guardrail_reason?: string;
  diagnostics_delta_status?: string;
  diagnostics_delta_diagnostic_count?: number;
  diagnostics_delta_error_count?: number;
  diagnostics_delta_warning_count?: number;
  line_start?: number;
  line_end?: number;
  read_coverage?: string;
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

function permissionTimelineSummary(
  event: PermissionRequest,
): Extract<TimelineSummary, { kind: "permission" }> | undefined {
  const evidence = permissionEvidence(event.metadata);
  const review = permissionReview(event.review);
  const actionReview = actionReviewSummary(event.metadata);
  const command = commandClassification(evidence?.command_classification || review?.classifier_result);
  if (!evidence && !review && !actionReview && !command) {
    return undefined;
  }
  return {
    kind: "permission",
    family: evidence?.permission_family,
    requestKind: evidence?.request_kind,
    risk: actionReview?.risk || evidence?.risk_level || review?.risk,
    decision: evidence?.decision,
    reason:
      actionReview?.reason || firstString(evidence?.reasons) || evidence?.reason || review?.reason,
    recovery:
      actionReview?.recovery ||
      recoveryAction(evidence?.recovery) ||
      review?.recovery_hint ||
      "Approve only if the request matches the intended task.",
    commandCategory: stringField(command, "category"),
    parserStatus: stringField(command, "parser_status"),
    mutation: booleanField(command, "mutation"),
    actionDecision: actionReview?.decision,
    actionReason: actionReview?.reason,
    sideEffect: actionReview?.sideEffect,
    network: actionReview?.network,
    checkpoint: actionReview?.checkpoint,
    checkpointApproval: actionReview?.checkpointApproval,
    allowedRule: firstString(evidence?.allowed_always_rules),
    scopeAllowed: actionReview?.scopeAllowed,
    budgetAllowed: actionReview?.budgetAllowed,
  };
}

function permissionTraceDetail(
  event: PermissionRequest,
  summary: Extract<TimelineSummary, { kind: "permission" }> | undefined,
) {
  const parts = compactFacts([
    event.prompt,
    summary?.actionDecision ? `review ${summary.actionDecision}` : null,
    summary?.actionReason ? `reason ${summary.actionReason}` : null,
    summary?.risk ? `risk ${summary.risk}` : null,
    summary?.requestKind ? `kind ${summary.requestKind}` : null,
    summary?.sideEffect ? `effect ${summary.sideEffect}` : null,
    summary?.network ? `network ${summary.network}` : null,
    summary?.checkpoint ? `checkpoint ${summary.checkpoint}` : null,
    summary?.commandCategory ? `command ${summary.commandCategory}` : null,
    summary?.recovery,
  ]);
  return parts.join(" · ");
}

function permissionEvidence(metadata: unknown): PermissionEvidenceSummary | null {
  if (!isRecord(metadata)) {
    return null;
  }
  const evidence = metadata.permission_evidence;
  return isRecord(evidence) ? (evidence as PermissionEvidenceSummary) : null;
}

function permissionReview(review: unknown): PermissionReviewSummary | null {
  return isRecord(review) ? (review as PermissionReviewSummary) : null;
}

function actionReviewSummary(metadata: unknown): ActionReviewDesktopSummary | null {
  if (!isRecord(metadata)) {
    return null;
  }
  const review = recordField(metadata, "action_review");
  if (!review) {
    return null;
  }
  const permission = recordField(review, "permission");
  const sideEffects = recordField(review, "side_effects");
  const network = recordField(sideEffects, "network");
  const checkpoint = recordField(review, "checkpoint");
  const scope = recordField(review, "scope");
  const budget = recordField(review, "budget");
  return {
    decision: stringField(review, "decision"),
    reason: stringField(review, "primary_reason"),
    permission: stringField(permission, "decision"),
    risk: stringField(permission, "risk_level"),
    recovery: stringField(review, "model_recovery") || stringField(review, "user_reason"),
    sideEffect: stringField(sideEffects, "external_side_effect"),
    network: stringField(network, "class"),
    checkpoint: stringField(checkpoint, "status"),
    checkpointApproval: booleanField(checkpoint, "requires_user_approval"),
    scopeAllowed: booleanField(scope, "allowed"),
    budgetAllowed: booleanField(budget, "allowed"),
  };
}

function commandClassification(value: unknown): Record<string, unknown> | null {
  return isRecord(value) ? value : null;
}

function recoveryAction(value: unknown) {
  if (!isRecord(value)) {
    return undefined;
  }
  return stringField(value, "recommended_action") || stringField(value, "action");
}

function firstString(value: unknown) {
  if (typeof value === "string") {
    return value;
  }
  if (!Array.isArray(value)) {
    return undefined;
  }
  return value.find((item): item is string => typeof item === "string");
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
      reason: summary.guardrail_reason || summary.error_preview || resultPreview || "Tool failed",
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
      rollbackId: summary.rollback_id || summary.checkpoint_id,
      diagnosticsDelta: summary.diagnostics_delta_status,
      diagnosticsErrorDelta: summary.diagnostics_delta_error_count,
      diagnosticsWarningDelta: summary.diagnostics_delta_warning_count,
      lineStart: summary.line_start,
      lineEnd: summary.line_end,
      readCoverage: summary.read_coverage,
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

function formatTokenCount(tokens: number) {
  if (tokens >= 1000) {
    return `${Math.round(tokens / 100) / 10}k`;
  }
  return `${tokens}`;
}

function recordField(value: Record<string, unknown> | null, key: string): Record<string, unknown> | null {
  const field = value?.[key];
  return isRecord(field) ? field : null;
}

function arrayField(value: Record<string, unknown> | null, key: string): unknown[] {
  const field = value?.[key];
  return Array.isArray(field) ? field : [];
}

function stringField(value: Record<string, unknown> | null, key: string) {
  const field = value?.[key];
  return typeof field === "string" ? field : undefined;
}

function booleanField(value: Record<string, unknown> | null, key: string) {
  const field = value?.[key];
  return typeof field === "boolean" ? field : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
