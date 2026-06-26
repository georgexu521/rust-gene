import {
  Activity,
  CheckCircle2,
  CircleAlert,
  CircleDotDashed,
  Clock3,
  Bug,
  FilePenLine,
  KeyRound,
  TerminalSquare,
} from "lucide-react";
import { useEffect, useRef } from "react";
import { DesktopDiagnostic, DesktopRunContext, ProviderModelStatus } from "../../runtime/desktopApi";
import { TimelineKind, TimelineStatus, TimelineSummary, TranscriptItem } from "../types";
import {
  ReasoningCard,
  ShellCard,
  DiffCard,
  ToolCard,
  ToolGroupShell,
} from "./Cards";

type TranscriptProps = {
  items: TranscriptItem[];
  isRunning: boolean;
  diagnostics: DesktopDiagnostic[];
  dismissedRunReviewIds?: Set<string>;
  onContinueFromRunReview?: (prompt: string) => void;
  onDismissRunReview?: (runId: string) => void;
  onPermissionAnswer?: (approved: boolean) => void;
  onOpenContext?: (context: DesktopRunContext) => void;
  onOpenTrace?: (traceId: string) => void;
  onOpenToolOutput?: () => void;
  onRevertLastTurn?: () => void;
  projectPath: string;
  providerStatus: ProviderModelStatus | null;
};

export function Transcript({
  items,
  isRunning,
  diagnostics,
  dismissedRunReviewIds,
  onContinueFromRunReview,
  onDismissRunReview,
  onPermissionAnswer,
  onOpenContext,
  onOpenTrace,
  onOpenToolOutput,
  onRevertLastTurn,
  projectPath,
  providerStatus,
}: TranscriptProps) {
  const transcriptRef = useRef<HTMLElement | null>(null);
  const scrollAnchorRef = useRef<HTMLDivElement | null>(null);
  const pinnedToBottomRef = useRef(true);
  const previousItemCountRef = useRef(items.length);
  const renderedItems = annotateTranscriptItems(items);
  const latestItem = items.at(-1);
  const scrollSignature = `${items.length}:${latestItem?.id || ""}:${
    latestItem?.role === "assistant" ? latestItem.text.length : ""
  }:${isRunning}`;

  useEffect(() => {
    const itemCountIncreased = items.length > previousItemCountRef.current;
    previousItemCountRef.current = items.length;
    const shouldForceScroll = latestItem?.role === "user" || itemCountIncreased;
    if (!pinnedToBottomRef.current && !shouldForceScroll) {
      return;
    }
    scrollAnchorRef.current?.scrollIntoView({ block: "end" });
  }, [items.length, latestItem?.role, scrollSignature]);

  function handleScroll() {
    const element = transcriptRef.current;
    if (!element) {
      return;
    }
    const distanceFromBottom = element.scrollHeight - element.scrollTop - element.clientHeight;
    pinnedToBottomRef.current = distanceFromBottom < 140;
  }

  return (
    <section className="transcript" aria-live="polite" ref={transcriptRef} onScroll={handleScroll}>
      {items.length === 0 ? (
        <EmptyState
          diagnostics={diagnostics}
          projectPath={projectPath}
          providerStatus={providerStatus}
        />
      ) : (
        renderedItems.map(({ className, item, runGroup }) =>
          className?.includes("transcript-hidden") ? null : item.role === "reasoning" ? (
            <article className="message reasoning" key={item.id}>
              <ReasoningCard text={item.text} streaming={item.streaming ?? false} />
            </article>
          ) : item.role === "timeline" ? (
            <TimelineEvent
              className={className}
              item={item}
              runGroup={runGroup}
              dismissedRunReviewIds={dismissedRunReviewIds}
              onContinueFromRunReview={onContinueFromRunReview}
              onOpenContext={onOpenContext}
              onPermissionAnswer={onPermissionAnswer}
              onOpenTrace={onOpenTrace}
              onOpenToolOutput={onOpenToolOutput}
              onDismissRunReview={onDismissRunReview}
              onRevertLastTurn={onRevertLastTurn}
            />
          ) : (
            <article
              className={`message ${item.role}${item.role === "assistant" && item.variant === "final" ? " final" : ""}${item.reverted ? " reverted" : ""}${className ? ` ${className}` : ""}`}
              key={item.id}
            >
              {item.role === "tool" ? <div className="message-label">{formatRole(item.role)}</div> : null}
              {item.reverted ? <div className="message-revert-badge">{item.revertLabel || "Reverted"}</div> : null}
              <div className="message-body">
                {item.text}
                {isStreamingAssistant(item, isRunning, renderedItems) ? (
                  <span className="streaming-caret" aria-hidden="true" />
                ) : null}
              </div>
            </article>
          ),
        )
      )}
      {isRunning && !hasAssistantAfterLastUser(items) ? (
        <div className="assistant-typing" role="status">
          Liz is thinking
          <span aria-hidden="true">...</span>
        </div>
      ) : null}
      <div className="transcript-scroll-anchor" ref={scrollAnchorRef} />
    </section>
  );
}

type AnnotatedTranscriptItem = {
  className?: string;
  item: TranscriptItem;
  runGroup?: RunGroupPreview;
};

function annotateTranscriptItems(items: TranscriptItem[]): AnnotatedTranscriptItem[] {
  let inRunGroup = false;
  return items.map((item, index) => {
    if (item.role === "timeline" && item.kind === "run") {
      inRunGroup = true;
      return {
        className: timelineItemClass(item, "run-boundary run-group-start"),
        item,
        runGroup: buildRunGroupPreview(items, index),
      };
    }
    if (item.role === "assistant" && item.variant === "final") {
      const className = inRunGroup ? "run-group-final" : undefined;
      inRunGroup = false;
      return { className, item };
    }
    if (item.role === "user") {
      inRunGroup = false;
      return { item };
    }
    if (item.role === "timeline" && inRunGroup) {
      return { className: timelineItemClass(item, "run-group-step"), item };
    }
    if (item.role === "timeline") {
      return { className: timelineItemClass(item), item };
    }
    return { item };
  });
}

function timelineItemClass(
  item: Extract<TranscriptItem, { role: "timeline" }>,
  baseClass = "",
) {
  const classes = [baseClass];
  if (item.reverted) {
    classes.push("reverted");
  }
  if (shouldHideTimelineItem(item)) {
    classes.push("transcript-hidden");
  }
  return classes.filter(Boolean).join(" ") || undefined;
}

function shouldHideTimelineItem(item: Extract<TranscriptItem, { role: "timeline" }>) {
  if (item.kind === "compact") {
    return false;
  }
  if (item.kind === "run") {
    const hasStats = item.summary?.kind === "run" && Boolean(item.summary.stats?.length);
    const hasContexts = item.summary?.kind === "run" && Boolean(item.summary.contexts?.length);
    return item.status === "completed" && !hasStats && !hasContexts;
  }
  if (item.kind === "permission") {
    return item.status !== "waiting";
  }
  if (item.kind === "tool") {
    return false;
  }

  return true;
}

function EmptyState({
  diagnostics,
  projectPath,
  providerStatus,
}: {
  diagnostics: DesktopDiagnostic[];
  projectPath: string;
  providerStatus: ProviderModelStatus | null;
}) {
  const projectName = basename(projectPath) || "selected project";
  const errors = diagnostics.filter((item) => item.status === "error").length;
  const warnings = diagnostics.filter((item) => item.status === "warning").length;
  const readiness =
    errors > 0
      ? `${errors} setup issue${errors === 1 ? "" : "s"} to resolve`
      : warnings > 0
        ? `${warnings} warning${warnings === 1 ? "" : "s"} before long runs`
        : "Ready for local agent runs";

  return (
    <div className="empty-state">
      <h2>What should we build in {projectName}?</h2>
      <p>Ask Liz to inspect code, make an edit, review a diff, or verify behavior.</p>
      <div className="empty-state-grid">
        <div>
          <span>Project</span>
          <strong title={projectPath}>{projectName}</strong>
        </div>
        <div>
          <span>Runtime</span>
          <strong>{providerStatus?.active_model || "Checking provider"}</strong>
        </div>
        <div>
          <span>Diagnostics</span>
          <strong>{readiness}</strong>
        </div>
      </div>
    </div>
  );
}

function formatRole(role: TranscriptItem["role"]) {
  if (role === "tool") {
    return "Tool";
  }
  return role === "user" ? "You" : "Liz";
}

function hasAssistantAfterLastUser(items: TranscriptItem[]) {
  let lastUserIndex = -1;
  for (let index = items.length - 1; index >= 0; index -= 1) {
    if (items[index]?.role === "user") {
      lastUserIndex = index;
      break;
    }
  }
  return items.slice(lastUserIndex + 1).some((item) => item.role === "assistant");
}

function isStreamingAssistant(
  item: TranscriptItem,
  isRunning: boolean,
  renderedItems: AnnotatedTranscriptItem[],
) {
  if (!isRunning || item.role !== "assistant") {
    return false;
  }
  for (let index = renderedItems.length - 1; index >= 0; index -= 1) {
    const rendered = renderedItems[index]?.item;
    if (rendered?.role === "assistant") {
      return rendered.id === item.id && item.variant !== "final";
    }
  }
  return false;
}

type TimelineEventItem = Extract<TranscriptItem, { role: "timeline" }>;

type RunGroupPreviewStep = {
  id: string;
  title: string;
  detail?: string;
  status?: TimelineStatus;
  traceId?: string;
};

type RunGroupPreview = {
  tools: RunGroupPreviewStep[];
  validations: RunGroupPreviewStep[];
  files: RunGroupPreviewStep[];
  permissions: RunGroupPreviewStep[];
  failures: RunGroupPreviewStep[];
  finalText?: string;
};

function TimelineEvent({
  className,
  item,
  runGroup,
  onPermissionAnswer,
  onOpenContext,
  onOpenTrace,
  onOpenToolOutput,
  onDismissRunReview,
  onContinueFromRunReview,
  onRevertLastTurn,
  dismissedRunReviewIds,
}: {
  className?: string;
  item: TimelineEventItem;
  runGroup?: RunGroupPreview;
  dismissedRunReviewIds?: Set<string>;
  onContinueFromRunReview?: (prompt: string) => void;
  onOpenContext?: (context: DesktopRunContext) => void;
  onPermissionAnswer?: (approved: boolean) => void;
  onOpenTrace?: (traceId: string) => void;
  onOpenToolOutput?: () => void;
  onDismissRunReview?: (runId: string) => void;
  onRevertLastTurn?: () => void;
}) {
  const isCompact = isCompactToolEvent(item);
  const isCompactPermission = item.kind === "permission" && item.status === "waiting";

  if (item.kind === "run") {
    return (
      <article className={`timeline-run-row ${item.status || "info"}${className ? ` ${className}` : ""}`}>
        <span>{runStatusText(item)}</span>
        {item.summary?.kind === "run" && item.summary.stats && item.summary.stats.length > 0 ? (
          <div className="timeline-run-stats compact">
            {item.summary.stats.map((stat) => (
              <span key={stat}>{stat}</span>
            ))}
          </div>
        ) : null}
        {item.summary?.kind === "run" && item.summary.contexts && item.summary.contexts.length > 0 ? (
          <div className="timeline-run-contexts" aria-label="Run attached context">
            {item.summary.contexts.map((context) => (
              <button
                aria-label={`Open run context ${context.label}`}
                disabled={!onOpenContext}
                key={context.type}
                type="button"
                onClick={() => onOpenContext?.(context)}
              >
                {context.label}
              </button>
            ))}
          </div>
        ) : null}
        {item.traceId && onOpenTrace ? (
          <button
            aria-label="Open trace for current run"
            className="timeline-run-trace"
            title="Open trace"
            type="button"
            onClick={() => onOpenTrace(item.traceId!)}
          >
            Trace
          </button>
        ) : null}
        {runGroup && hasRunGroupPreview(runGroup) && !dismissedRunReviewIds?.has(item.id) ? (
          <RunGroupPanel
            runGroup={runGroup}
            runId={item.id}
            runStatus={item.status}
            onContinueFromRunReview={onContinueFromRunReview}
            onDismissRunReview={onDismissRunReview}
            onOpenToolOutput={onOpenToolOutput}
            onOpenTrace={onOpenTrace}
            onRevertLastTurn={onRevertLastTurn}
          />
        ) : null}
      </article>
    );
  }

  if (item.kind === "usage") {
    return (
      <article className={`timeline-event usage${className ? ` ${className}` : ""}`}>
        <Clock3 aria-hidden="true" size={13} />
        <div className="timeline-usage-content">
          <span>{item.title}</span>
          {item.detail ? <span>{item.detail}</span> : null}
        </div>
        {item.traceId && onOpenTrace ? (
          <button
            aria-label={`Open trace for ${item.title}`}
            className="timeline-debug-link"
            title="Open trace"
            type="button"
            onClick={() => onOpenTrace(item.traceId!)}
          >
            Trace
          </button>
        ) : null}
      </article>
    );
  }

  return (
    <article
      className={`timeline-event ${item.kind} ${item.status || "info"}${isCompact ? " compact-shell" : ""}${isCompactPermission ? " compact-permission" : ""}${className ? ` ${className}` : ""}`}
    >
      <div className="timeline-icon" aria-hidden="true">
        {iconForTimeline(item.kind, item.status)}
      </div>
      <div className="timeline-content">
        <div className="timeline-row">
          <div className="timeline-title">{item.title}</div>
          <div className="timeline-status">
            {item.reverted ? <span className="timeline-revert-badge">{item.revertLabel || "Reverted"}</span> : null}
            {labelForStatus(item.status)}
          </div>
        </div>
        {item.summary && !isCompactPermission ? (
          <TimelineSummaryView
            compact={isCompact}
            summary={item.summary}
            onOpenContext={onOpenContext}
          />
        ) : null}
        {(!item.summary || item.summary.kind === "permission" || isCompactPermission) && item.detail ? (
          <div className="timeline-detail">{item.detail}</div>
        ) : null}
        {!isCompact && !isCompactPermission && item.facts && item.facts.length > 0 ? (
          <div className="timeline-facts">
            {item.facts.map((fact) => (
              <span key={fact}>{fact}</span>
            ))}
          </div>
        ) : null}
        {item.kind === "permission" && item.status === "waiting" && onPermissionAnswer ? (
          <div className="timeline-actions">
            <button type="button" onClick={() => onPermissionAnswer(false)}>
              Reject
            </button>
            <button type="button" onClick={() => onPermissionAnswer(true)}>
              Approve
            </button>
          </div>
        ) : null}
        {item.traceId && onOpenTrace ? (
          <button
            aria-label={`Open trace for ${item.title}`}
            className="timeline-debug-link"
            title="Open trace"
            type="button"
            onClick={() => onOpenTrace(item.traceId!)}
          >
            <Bug aria-hidden="true" size={12} />
            <span>Trace</span>
          </button>
        ) : null}
      </div>
    </article>
  );
}

function runStatusText(item: TimelineEventItem) {
  if (item.status === "completed") {
    return "Done";
  }
  if (item.status === "failed") {
    return "Needs attention";
  }
  if (item.status === "waiting") {
    return "Waiting for approval";
  }
  return "Working";
}

function RunGroupPanel({
  runGroup,
  runId,
  runStatus,
  onContinueFromRunReview,
  onDismissRunReview,
  onOpenToolOutput,
  onOpenTrace,
  onRevertLastTurn,
}: {
  runGroup: RunGroupPreview;
  runId: string;
  runStatus?: TimelineStatus;
  onContinueFromRunReview?: (prompt: string) => void;
  onDismissRunReview?: (runId: string) => void;
  onOpenToolOutput?: () => void;
  onOpenTrace?: (traceId: string) => void;
  onRevertLastTurn?: () => void;
}) {
  const groups = [
    { key: "validations", label: "Validation", items: runGroup.validations },
    { key: "files", label: "Changed files / Diff", items: runGroup.files },
    { key: "permissions", label: "Permission", items: runGroup.permissions },
    { key: "failures", label: "Residual risks / Needs attention", items: runGroup.failures },
    { key: "tools", label: "Tools", items: runGroup.tools },
  ].filter((group) => group.items.length > 0);
  const repairPrompt = buildRunReviewRepairPrompt(runGroup);
  const latestTraceId = latestTraceIdFromRunGroup(runGroup);

  return (
    <div className="run-group-panel" aria-label="Run summary panel">
      <div className="run-group-panel-header">
        <span>Run review</span>
        <span>
          {runStatus === "failed"
            ? "Needs attention"
            : runGroup.finalText || "Review changes, validation, permissions, and residual risk"}
        </span>
      </div>
      <div className="run-group-panel-grid">
        {groups.map((group) => (
          <section className={`run-group-card ${group.key}`} key={group.key}>
            <div className="run-group-card-title">{group.label}</div>
            <div className="run-group-card-list">
              {group.items.slice(0, 4).map((step) => (
                <div className={`run-group-step ${step.status || "info"}`} key={step.id}>
                  <span className="run-group-step-dot" aria-hidden="true" />
                  <div>
                    <strong>{step.title}</strong>
                    {step.detail ? <span>{step.detail}</span> : null}
                  </div>
                  {step.traceId && onOpenTrace ? (
                    <button
                      aria-label={`Open trace for ${step.title}`}
                      type="button"
                      onClick={() => onOpenTrace(step.traceId!)}
                    >
                      Trace
                    </button>
                  ) : null}
                </div>
              ))}
              {group.items.length > 4 ? (
                <div className="run-group-more">+{group.items.length - 4} more</div>
              ) : null}
            </div>
          </section>
        ))}
      </div>
      <div className="run-review-actions" aria-label="Run review actions">
        <button type="button" onClick={() => onDismissRunReview?.(runId)}>
          Accept
        </button>
        <button type="button" onClick={() => onDismissRunReview?.(runId)}>
          Dismiss review
        </button>
        <button type="button" disabled={!onRevertLastTurn} onClick={onRevertLastTurn}>
          Revert last turn
        </button>
        <button
          type="button"
          disabled={!onContinueFromRunReview}
          onClick={() => onContinueFromRunReview?.(repairPrompt)}
        >
          Continue with fix
        </button>
        <button
          type="button"
          disabled={!latestTraceId || !onOpenTrace}
          onClick={() => latestTraceId && onOpenTrace?.(latestTraceId)}
        >
          Open trace
        </button>
        <button type="button" disabled={!onOpenToolOutput} onClick={onOpenToolOutput}>
          Open tool output
        </button>
      </div>
    </div>
  );
}

function latestTraceIdFromRunGroup(runGroup: RunGroupPreview) {
  return [
    ...runGroup.failures,
    ...runGroup.validations,
    ...runGroup.files,
    ...runGroup.permissions,
    ...runGroup.tools,
  ]
    .map((step) => step.traceId)
    .filter(Boolean)
    .at(-1);
}

function buildRunReviewRepairPrompt(runGroup: RunGroupPreview) {
  const failures = runGroup.failures.map((item) => `${item.title}: ${item.detail || "failed"}`);
  const failedValidation = runGroup.validations
    .filter((item) => item.status === "failed")
    .map((item) => `${item.title}: ${item.detail || "failed"}`);
  const missingValidation =
    runGroup.validations.length === 0 ? ["No validation evidence was visible in the run review."] : [];
  const lines = [...failures, ...failedValidation, ...missingValidation].slice(0, 5);
  return [
    "Please continue from the run review and fix the remaining issue.",
    ...lines.map((line) => `- ${line}`),
    "After the fix, rerun the relevant validation and report the evidence.",
  ].join("\n");
}

function buildRunGroupPreview(items: TranscriptItem[], runIndex: number): RunGroupPreview | undefined {
  const runGroup: RunGroupPreview = {
    tools: [],
    validations: [],
    files: [],
    permissions: [],
    failures: [],
  };

  for (let index = runIndex + 1; index < items.length; index += 1) {
    const item = items[index];
    if (!item || item.role === "user") {
      break;
    }
    if (item.role === "timeline" && item.kind === "run") {
      break;
    }
    if (item.role === "assistant" && item.variant === "final") {
      runGroup.finalText = summarizeFinalText(item.text);
      continue;
    }
    if (item.role !== "timeline") {
      continue;
    }

    addTimelineItemToRunGroup(runGroup, item);
  }

  return hasRunGroupPreview(runGroup) ? runGroup : undefined;
}

function addTimelineItemToRunGroup(runGroup: RunGroupPreview, item: TimelineEventItem) {
  const failed = item.status === "failed" || item.kind === "error" || item.summary?.kind === "failure";
  if (failed) {
    runGroup.failures.push(runGroupStepFromTimeline(item));
  }

  if (item.kind === "permission") {
    runGroup.permissions.push(runGroupStepFromTimeline(item));
    return;
  }

  if (item.kind !== "tool") {
    return;
  }

  if (item.summary?.kind === "shell" && item.summary.validation) {
    runGroup.validations.push(runGroupStepFromTimeline(item, item.summary.validation));
    return;
  }

  if (item.summary?.kind === "file" && item.summary.action !== "read") {
    runGroup.files.push(runGroupStepFromTimeline(item, fileChangeDetail(item.summary)));
    return;
  }

  if (failed) {
    return;
  }

  runGroup.tools.push(runGroupStepFromTimeline(item));
}

function runGroupStepFromTimeline(item: TimelineEventItem, detailOverride?: string): RunGroupPreviewStep {
  return {
    id: item.id,
    title: item.title,
    detail: detailOverride || runGroupDetail(item),
    status: item.status,
    traceId: item.traceId,
  };
}

function runGroupDetail(item: TimelineEventItem) {
  if (item.summary?.kind === "shell") {
    return compactSummaryMeta([
      item.summary.command,
      item.summary.exitCode !== undefined ? `exit ${item.summary.exitCode}` : null,
      item.summary.duration,
    ]).join(" · ");
  }
  if (item.summary?.kind === "file") {
    return compactSummaryMeta([
      item.summary.path,
      fileChangeDetail(item.summary),
    ]).join(" · ");
  }
  if (item.summary?.kind === "permission") {
    return compactSummaryMeta([
      item.summary.risk ? `risk ${item.summary.risk}` : null,
      item.summary.actionDecision ? `review ${item.summary.actionDecision}` : null,
      item.summary.reason,
    ]).join(" · ");
  }
  if (item.summary?.kind === "failure") {
    return item.summary.reason;
  }
  return item.detail;
}

function fileChangeDetail(summary: Extract<TimelineSummary, { kind: "file" }>) {
  return compactSummaryMeta([
    summary.additions !== undefined ? `+${summary.additions}` : null,
    summary.deletions !== undefined ? `-${summary.deletions}` : null,
    summary.replacements !== undefined ? `${summary.replacements} replacements` : null,
    summary.operations !== undefined ? `${summary.operations} operations` : null,
  ]).join(" · ");
}

function summarizeFinalText(text: string) {
  const firstLine = text.trim().split(/\r?\n/).find(Boolean);
  if (!firstLine) {
    return undefined;
  }
  return firstLine.length > 88 ? `${firstLine.slice(0, 85)}...` : firstLine;
}

function hasRunGroupPreview(runGroup: RunGroupPreview) {
  return Boolean(
    runGroup.finalText ||
      runGroup.tools.length ||
      runGroup.validations.length ||
      runGroup.files.length ||
      runGroup.permissions.length ||
      runGroup.failures.length,
  );
}

function TimelineSummaryView({
  compact = false,
  onOpenContext,
  summary,
}: {
  compact?: boolean;
  onOpenContext?: (context: DesktopRunContext) => void;
  summary: TimelineSummary;
}) {
  if (summary.kind === "run") {
    return (
      <div className={`timeline-summary run ${summary.stage}`}>
        <Activity aria-hidden="true" size={15} />
        <div>
          <strong>{summary.headline}</strong>
          <div className="timeline-summary-meta">
            {compactSummaryMeta([summary.detail, summary.sessionId]).join(" · ")}
          </div>
          {summary.stats && summary.stats.length > 0 ? (
            <div className="timeline-run-stats">
              {summary.stats.map((stat) => (
                <span key={stat}>{stat}</span>
              ))}
            </div>
          ) : null}
          {summary.contexts && summary.contexts.length > 0 ? (
            <div className="timeline-attached-contexts" aria-label="Run attached context">
              <span>Attached context</span>
              {summary.contexts.map((context) => (
                <button
                  aria-label={`Open run context ${context.label}`}
                  disabled={!onOpenContext}
                  key={context.type}
                  type="button"
                  onClick={() => onOpenContext?.(context)}
                >
                  {context.label}
                </button>
              ))}
            </div>
          ) : null}
          {summary.recovery ? (
            <div className="timeline-recovery">{summary.recovery}</div>
          ) : null}
        </div>
      </div>
    );
  }

  if (summary.kind === "shell") {
    return (
      <div className={`timeline-summary shell${compact ? " compact" : ""}`}>
        <TerminalSquare aria-hidden="true" size={15} />
        <div>
          <code>{summary.command}</code>
          <div className="timeline-summary-meta">
            {compactSummaryMeta([
              summary.validation,
              summary.exitCode !== undefined ? `exit ${summary.exitCode}` : null,
              summary.duration,
            ]).join(" · ")}
          </div>
        </div>
      </div>
    );
  }

  if (summary.kind === "file") {
    return (
      <div className={`timeline-summary file${compact ? " compact" : ""}`}>
        <FilePenLine aria-hidden="true" size={15} />
        <div>
          <strong>{fileActionLabel(summary.action)}</strong>
          <div className="timeline-summary-meta">
            {compactSummaryMeta([
              summary.path,
              summary.repeatCount && summary.repeatCount > 1 ? `repeated ${summary.repeatCount}x` : null,
              readRangeLabel(summary),
              summary.replacements !== undefined ? `${summary.replacements} replacements` : null,
              summary.operations !== undefined ? `${summary.operations} operations` : null,
              summary.additions !== undefined ? `+${summary.additions}` : null,
              summary.deletions !== undefined ? `-${summary.deletions}` : null,
              summary.rollbackId ? `rollback ${summary.rollbackId}` : null,
              diagnosticsDeltaLabel(summary),
            ]).join(" · ")}
          </div>
          {summary.diffPreview ? (
            <ExpandablePreview
              className="timeline-diff-preview"
              label={summary.diffTruncated ? "Diff preview truncated" : "Diff preview"}
              text={summary.diffPreview}
            />
          ) : null}
        </div>
      </div>
    );
  }

  if (summary.kind === "permission") {
    return (
      <div className="timeline-summary permission-review">
        <KeyRound aria-hidden="true" size={15} />
        <div>
          <strong>{permissionHeadline(summary)}</strong>
          <div className="timeline-summary-meta">
            {compactSummaryMeta([
              summary.actionDecision ? `review ${summary.actionDecision}` : null,
              summary.actionReason ? summary.actionReason.replaceAll("_", " ") : null,
              summary.risk ? `risk ${summary.risk}` : null,
              summary.requestKind ? summary.requestKind.replaceAll("_", " ") : null,
              summary.sideEffect ? `effect ${summary.sideEffect.replaceAll("_", " ")}` : null,
              summary.network ? `network ${summary.network.replaceAll("_", " ")}` : null,
              summary.checkpoint ? `checkpoint ${summary.checkpoint.replaceAll("_", " ")}` : null,
              summary.checkpointApproval ? "approval required" : null,
              summary.scopeAllowed === false ? "scope blocked" : null,
              summary.budgetAllowed === false ? "budget blocked" : null,
              summary.commandCategory ? summary.commandCategory.replaceAll("_", " ") : null,
              summary.parserStatus ? `parser ${summary.parserStatus}` : null,
              summary.mutation ? "mutates workspace" : null,
              summary.allowedRule ? `allow rule ${summary.allowedRule}` : null,
            ]).join(" · ")}
          </div>
          {summary.reason ? <div className="timeline-detail">{summary.reason}</div> : null}
          {summary.recovery ? <div className="timeline-recovery">{summary.recovery}</div> : null}
        </div>
      </div>
    );
  }

  return (
    <div className="timeline-summary failure">
      <CircleAlert aria-hidden="true" size={15} />
      <div>
        <strong>{summary.reason}</strong>
        {summary.recovery ? (
          <div className="timeline-summary-meta">{summary.recovery}</div>
        ) : null}
        {summary.outputPreview ? (
          <ExpandablePreview
            className="timeline-output-preview"
            label={summary.outputTruncated ? "Output preview truncated" : "Output preview"}
            text={summary.outputPreview}
          />
        ) : null}
      </div>
    </div>
  );
}

function permissionHeadline(summary: Extract<TimelineSummary, { kind: "permission" }>) {
  const family = summary.family ? summary.family.replaceAll("_", " ") : "tool";
  return `Review ${family} permission`;
}

function readRangeLabel(summary: Extract<TimelineSummary, { kind: "file" }>) {
  if (summary.action !== "read" || !summary.lineStart || !summary.lineEnd) {
    return null;
  }
  if (summary.readCoverage === "full") {
    return `${summary.lineEnd} lines`;
  }
  return `lines ${summary.lineStart}-${summary.lineEnd}`;
}

function isCompactToolEvent(item: TimelineEventItem) {
  return item.kind === "tool" && item.status !== "failed";
}

function diagnosticsDeltaLabel(summary: Extract<TimelineSummary, { kind: "file" }>) {
  if (!summary.diagnosticsDelta || summary.diagnosticsDelta === "not_checked") {
    return null;
  }
  if (summary.diagnosticsDelta === "unchanged") {
    return "diagnostics unchanged";
  }
  if (summary.diagnosticsDelta === "improved") {
    return "diagnostics improved";
  }
  const parts = compactSummaryMeta([
    summary.diagnosticsErrorDelta ? `${signed(summary.diagnosticsErrorDelta)} errors` : null,
    summary.diagnosticsWarningDelta ? `${signed(summary.diagnosticsWarningDelta)} warnings` : null,
  ]);
  return parts.length > 0 ? parts.join(", ") : summary.diagnosticsDelta.replaceAll("_", " ");
}

function signed(value: number) {
  return value > 0 ? `+${value}` : `${value}`;
}

function ExpandablePreview({
  className,
  label,
  text,
}: {
  className: string;
  label: string;
  text: string;
}) {
  const shouldCollapse = text.length > 360 || text.split("\n").length > 8;
  if (!shouldCollapse) {
    return (
      <pre className={className} aria-label={label}>
        {text}
      </pre>
    );
  }

  return (
    <details className="timeline-expandable-preview">
      <summary>{label}</summary>
      <pre className={className}>{text}</pre>
    </details>
  );
}

function iconForTimeline(kind: TimelineKind, status?: TimelineStatus) {
  if (status === "completed") {
    return <CheckCircle2 size={15} />;
  }
  if (status === "failed" || kind === "error") {
    return <CircleAlert size={15} />;
  }
  if (kind === "permission") {
    return <KeyRound size={15} />;
  }
  if (kind === "tool") {
    return <TerminalSquare size={15} />;
  }
  if (kind === "usage") {
    return <Clock3 size={15} />;
  }
  return <CircleDotDashed size={15} />;
}

function labelForStatus(status?: TimelineStatus) {
  switch (status) {
    case "running":
      return "Running";
    case "waiting":
      return "Waiting";
    case "completed":
      return "Done";
    case "failed":
      return "Failed";
    default:
      return "Info";
  }
}

function fileActionLabel(action: Extract<TimelineSummary, { kind: "file" }>["action"]) {
  switch (action) {
    case "read":
      return "Read file";
    case "write":
      return "Wrote file";
    case "edit":
      return "Edited file";
    case "patch":
      return "Patched files";
  }
}

function compactSummaryMeta(values: Array<string | null | undefined>) {
  return values.filter((value): value is string => Boolean(value && value.trim()));
}

function basename(path: string) {
  return path.split(/[\\/]/).filter(Boolean).at(-1) || path;
}
