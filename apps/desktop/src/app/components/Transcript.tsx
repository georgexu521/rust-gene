import {
  CheckCircle2,
  CircleAlert,
  CircleDotDashed,
  Clock3,
  Bug,
  FilePenLine,
  KeyRound,
  TerminalSquare,
} from "lucide-react";
import { DesktopDiagnostic, ProviderModelStatus } from "../../runtime/desktopApi";
import { TimelineKind, TimelineStatus, TimelineSummary, TranscriptItem } from "../types";

type TranscriptProps = {
  items: TranscriptItem[];
  diagnostics: DesktopDiagnostic[];
  onPermissionAnswer?: (approved: boolean) => void;
  onOpenTrace?: (traceId: string) => void;
  projectPath: string;
  providerStatus: ProviderModelStatus | null;
};

export function Transcript({
  items,
  diagnostics,
  onPermissionAnswer,
  onOpenTrace,
  projectPath,
  providerStatus,
}: TranscriptProps) {
  return (
    <section className="transcript" aria-live="polite">
      {items.length === 0 ? (
        <EmptyState
          diagnostics={diagnostics}
          projectPath={projectPath}
          providerStatus={providerStatus}
        />
      ) : (
        items.map((item) =>
          item.role === "timeline" ? (
            <TimelineEvent
              item={item}
              key={item.id}
              onPermissionAnswer={onPermissionAnswer}
              onOpenTrace={onOpenTrace}
            />
          ) : (
            <article className={`message ${item.role}`} key={item.id}>
              <div className="message-label">{formatRole(item.role)}</div>
              <div className="message-body">{item.text}</div>
            </article>
          ),
        )
      )}
    </section>
  );
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
      <div className="empty-kicker">Priority Agent desktop</div>
      <h2>Start a focused run in {projectName}</h2>
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
  item,
  onPermissionAnswer,
  onOpenTrace,
}: {
  item: TimelineEventItem;
  onPermissionAnswer?: (approved: boolean) => void;
  onOpenTrace?: (traceId: string) => void;
}) {
  return (
    <article className={`timeline-event ${item.kind} ${item.status || "info"}`}>
      <div className="timeline-icon" aria-hidden="true">
        {iconForTimeline(item.kind, item.status)}
      </div>
      <div className="timeline-content">
        <div className="timeline-row">
          <div className="timeline-title">{item.title}</div>
          <div className="timeline-status">{labelForStatus(item.status)}</div>
        </div>
        {item.summary ? <TimelineSummaryView summary={item.summary} /> : null}
        {!item.summary && item.detail ? (
          <div className="timeline-detail">{item.detail}</div>
        ) : null}
        {item.facts && item.facts.length > 0 ? (
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
            className="timeline-debug-link"
            type="button"
            onClick={() => onOpenTrace(item.traceId!)}
          >
            <Bug aria-hidden="true" size={13} />
            <span>Debug</span>
          </button>
        ) : null}
      </div>
    </article>
  );
}

function TimelineSummaryView({ summary }: { summary: TimelineSummary }) {
  if (summary.kind === "shell") {
    return (
      <div className="timeline-summary shell">
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
