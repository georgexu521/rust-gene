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
import { Fragment } from "react";
import { DesktopDiagnostic, DesktopRunContext, ProviderModelStatus } from "../../runtime/desktopApi";
import { TimelineKind, TimelineStatus, TimelineSummary, TranscriptItem } from "../types";

type TranscriptProps = {
  items: TranscriptItem[];
  diagnostics: DesktopDiagnostic[];
  onPermissionAnswer?: (approved: boolean) => void;
  onOpenContext?: (context: DesktopRunContext) => void;
  onOpenTrace?: (traceId: string) => void;
  projectPath: string;
  providerStatus: ProviderModelStatus | null;
};

export function Transcript({
  items,
  diagnostics,
  onPermissionAnswer,
  onOpenContext,
  onOpenTrace,
  projectPath,
  providerStatus,
}: TranscriptProps) {
  const renderedItems = annotateTranscriptItems(items);

  return (
    <section className="transcript" aria-live="polite">
      {items.length === 0 ? (
        <EmptyState
          diagnostics={diagnostics}
          projectPath={projectPath}
          providerStatus={providerStatus}
        />
      ) : (
        renderedItems.map(({ className, item }) =>
          item.role === "timeline" ? (
            <Fragment key={item.id}>
              {className?.includes("run-group-start") ? (
                <div className="timeline-section-label">Process</div>
              ) : null}
              <TimelineEvent
                className={className}
                item={item}
                onOpenContext={onOpenContext}
                onPermissionAnswer={onPermissionAnswer}
                onOpenTrace={onOpenTrace}
              />
            </Fragment>
          ) : (
            <article
              className={`message ${item.role}${item.role === "assistant" && item.variant === "final" ? " final" : ""}${className ? ` ${className}` : ""}`}
              key={item.id}
            >
              {className?.includes("run-group-final") ? (
                <div className="message-section-label">Conclusion</div>
              ) : null}
              <div className="message-label">
                <span>{formatRole(item.role)}</span>
                {item.role === "assistant" && item.variant === "final" ? (
                  <span className="message-badge">Final answer</span>
                ) : null}
              </div>
              <div className="message-body">{item.text}</div>
            </article>
          ),
        )
      )}
    </section>
  );
}

type AnnotatedTranscriptItem = {
  className?: string;
  item: TranscriptItem;
};

function annotateTranscriptItems(items: TranscriptItem[]): AnnotatedTranscriptItem[] {
  let inRunGroup = false;
  return items.map((item) => {
    if (item.role === "timeline" && item.kind === "run") {
      inRunGroup = true;
      return { className: "run-boundary run-group-start", item };
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
      return { className: "run-group-step", item };
    }
    return { item };
  });
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

type TimelineEventItem = Extract<TranscriptItem, { role: "timeline" }>;

function TimelineEvent({
  className,
  item,
  onPermissionAnswer,
  onOpenContext,
  onOpenTrace,
}: {
  className?: string;
  item: TimelineEventItem;
  onOpenContext?: (context: DesktopRunContext) => void;
  onPermissionAnswer?: (approved: boolean) => void;
  onOpenTrace?: (traceId: string) => void;
}) {
  const isCompact = isCompactToolEvent(item);

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
      className={`timeline-event ${item.kind} ${item.status || "info"}${isCompact ? " compact-shell" : ""}${className ? ` ${className}` : ""}`}
    >
      <div className="timeline-icon" aria-hidden="true">
        {iconForTimeline(item.kind, item.status)}
      </div>
      <div className="timeline-content">
        <div className="timeline-row">
          <div className="timeline-title">{item.title}</div>
          <div className="timeline-status">{labelForStatus(item.status)}</div>
        </div>
        {item.summary ? (
          <TimelineSummaryView
            compact={isCompact}
            summary={item.summary}
            onOpenContext={onOpenContext}
          />
        ) : null}
        {!item.summary && item.detail ? (
          <div className="timeline-detail">{item.detail}</div>
        ) : null}
        {!isCompact && item.facts && item.facts.length > 0 ? (
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
      <div className="timeline-summary file">
        <FilePenLine aria-hidden="true" size={15} />
        <div>
          <strong>{fileActionLabel(summary.action)}</strong>
          <div className="timeline-summary-meta">
            {compactSummaryMeta([
              summary.path,
              summary.replacements !== undefined ? `${summary.replacements} replacements` : null,
              summary.operations !== undefined ? `${summary.operations} operations` : null,
              summary.additions !== undefined ? `+${summary.additions}` : null,
              summary.deletions !== undefined ? `-${summary.deletions}` : null,
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

function isCompactToolEvent(item: TimelineEventItem) {
  return item.kind === "tool" && item.status === "completed" && item.summary?.kind === "shell";
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
