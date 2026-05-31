import { useEffect } from "react";
import {
  DesktopContextSnapshot,
  DesktopRunContext,
  DesktopRuntimeDiagnostic,
} from "../../runtime/desktopApi";
import { TimelineSummary, TraceItem } from "../types";

type TraceDrawerProps = {
  activeItemId: string | null;
  contextSnapshot: DesktopContextSnapshot | null;
  isOpen: boolean;
  items: TraceItem[];
  onOpenContext?: (context: DesktopRunContext) => void;
  onClose: () => void;
};

export function TraceDrawer({
  activeItemId,
  contextSnapshot,
  isOpen,
  items,
  onClose,
  onOpenContext,
}: TraceDrawerProps) {
  useEffect(() => {
    if (!isOpen || !activeItemId) {
      return;
    }
    const frame = requestAnimationFrame(() => {
      document
        .querySelector(`[data-trace-id="${CSS.escape(activeItemId)}"]`)
        ?.scrollIntoView({ block: "center" });
    });
    return () => cancelAnimationFrame(frame);
  }, [activeItemId, isOpen, items.length]);

  if (!isOpen) {
    return null;
  }

  return (
    <aside className="trace-drawer" aria-label="Run trace">
      <div className="trace-header">
        <div>
          <div className="trace-eyebrow">Trace</div>
          <h2>Run events</h2>
        </div>
        <button type="button" onClick={onClose}>
          Close
        </button>
      </div>

      {contextSnapshot ? (
        <section className="trace-context-state" aria-label="Trace context state">
          <div>
            <span>Context</span>
            <strong>{contextSnapshot.usage_percent}%</strong>
          </div>
          <div>
            <span>History</span>
            <strong>{contextSnapshot.history_messages} messages</strong>
          </div>
          <div>
            <span>Compact</span>
            <strong>
              {contextSnapshot.compact.latest_attempt_decision ||
                `${contextSnapshot.compact.compression_count} runs`}
            </strong>
          </div>
        </section>
      ) : null}

      {items.length === 0 ? (
        <div className="trace-empty">No trace events yet</div>
      ) : (
        <div className="trace-list">
          {items.map((item) => (
            <article
              className={`trace-item ${item.kind} ${item.status || ""} ${item.id === activeItemId ? "active" : ""}`}
              data-trace-id={item.id}
              key={item.id}
            >
              <div className="trace-kind">{item.kind}</div>
              <div className="trace-title">{item.title}</div>
              {item.detail ? <div className="trace-detail">{item.detail}</div> : null}
              {item.summary ? <TraceSummary summary={item.summary} /> : null}
              {item.facts && item.facts.length > 0 ? (
                <div className="trace-facts">
                  {item.facts.map((fact) => (
                    <span key={fact}>{fact}</span>
                  ))}
                </div>
              ) : null}
              {item.runtime ? <RuntimeDiagnosticView diagnostic={item.runtime} /> : null}
              {item.contexts && item.contexts.length > 0 ? (
                <div className="trace-contexts" aria-label="Trace attached context">
                  <span>Attached context</span>
                  {item.contexts.map((context) => (
                    <button
                      aria-label={`Open trace context ${context.label}`}
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
            </article>
          ))}
        </div>
      )}
    </aside>
  );
}

function TraceSummary({ summary }: { summary: TimelineSummary }) {
  if (summary.kind === "shell") {
    return (
      <div className="trace-summary">
        <span>Command</span>
        <code>{summary.command}</code>
        <small>
          {[summary.validation, summary.exitCode !== undefined ? `exit ${summary.exitCode}` : null, summary.duration]
            .filter(Boolean)
            .join(" · ")}
        </small>
      </div>
    );
  }

  if (summary.kind === "file") {
    return (
      <div className="trace-summary">
        <span>File change</span>
        {summary.path ? <code>{summary.path}</code> : null}
        <small>
          {[
            summary.replacements !== undefined ? `${summary.replacements} replacements` : null,
            summary.operations !== undefined ? `${summary.operations} operations` : null,
            summary.additions !== undefined ? `+${summary.additions}` : null,
            summary.deletions !== undefined ? `-${summary.deletions}` : null,
          ]
            .filter(Boolean)
            .join(" · ")}
        </small>
        {summary.diffPreview ? <pre>{summary.diffPreview}</pre> : null}
      </div>
    );
  }

  if (summary.kind === "failure") {
    return (
      <div className="trace-summary failure">
        <span>Failure</span>
        <strong>{summary.reason}</strong>
        {summary.recovery ? <small>{summary.recovery}</small> : null}
        {summary.outputPreview ? <pre>{summary.outputPreview}</pre> : null}
      </div>
    );
  }

  if (summary.kind === "permission") {
    return (
      <div className="trace-summary permission">
        <span>Permission review</span>
        <small>
          {[
            summary.actionDecision ? `review ${summary.actionDecision}` : null,
            summary.risk ? `risk ${summary.risk}` : null,
            summary.requestKind ? summary.requestKind.replaceAll("_", " ") : null,
            summary.checkpoint ? `checkpoint ${summary.checkpoint.replaceAll("_", " ")}` : null,
            summary.parserStatus ? `parser ${summary.parserStatus}` : null,
          ]
            .filter(Boolean)
            .join(" · ")}
        </small>
        {summary.reason ? <strong>{summary.reason}</strong> : null}
        {summary.recovery ? <small>{summary.recovery}</small> : null}
      </div>
    );
  }

  return null;
}

function RuntimeDiagnosticView({ diagnostic }: { diagnostic: DesktopRuntimeDiagnostic }) {
  const taskState = recordField(diagnostic, "task_state");
  const verification = recordField(taskState, "verification");
  const done = recordField(taskState, "done");
  const modeScore = recordField(taskState, "mode_score");
  const lightweightPlan = recordField(taskState, "lightweight_plan");
  const proof = recordField(diagnostic, "verification_proof");
  const controlLoop = recordField(diagnostic, "control_loop");
  const stopCheck = recordField(taskState, "stop_check");
  const activeFiles = stringArrayField(taskState, "active_files").slice(0, 6);
  const phases = recordArrayField(controlLoop, "phases").slice(0, 7);

  return (
    <div className="trace-runtime">
      <div className="trace-runtime-grid">
        <RuntimeMetric label="Stage" value={stringField(taskState, "stage")} />
        <RuntimeMetric label="Verification" value={stringField(verification, "status")} />
        <RuntimeMetric label="Proof" value={stringField(proof, "status")} />
        <RuntimeMetric label="Spine" value={stringField(controlLoop, "coverage")} />
        <RuntimeMetric label="Mode confidence" value={numberField(modeScore, "confidence")} />
        <RuntimeMetric label="Done" value={booleanField(done, "satisfied")} />
        <RuntimeMetric label="Stop" value={stringField(stopCheck, "reason")} />
      </div>

      {stringField(taskState, "goal") ? (
        <div className="trace-runtime-line">
          <span>Goal</span>
          <strong>{stringField(taskState, "goal")}</strong>
        </div>
      ) : null}

      {stringField(proof, "summary") ? (
        <div className="trace-runtime-line">
          <span>Proof summary</span>
          <strong>{stringField(proof, "summary")}</strong>
        </div>
      ) : null}

      {stringField(lightweightPlan, "objective") ? (
        <div className="trace-runtime-line">
          <span>Light plan</span>
          <strong>{stringField(lightweightPlan, "objective")}</strong>
        </div>
      ) : null}

      {activeFiles.length > 0 ? (
        <div className="trace-runtime-files">
          {activeFiles.map((file) => (
            <code key={file}>{file}</code>
          ))}
        </div>
      ) : null}

      {phases.length > 0 ? (
        <div className="trace-phase-list">
          {phases.map((phase) => (
            <div key={stringField(phase, "phase") || JSON.stringify(phase)}>
              <span>{stringField(phase, "phase") || "phase"}</span>
              <strong>
                {numberField(phase, "events") ?? 0}
                {stringField(phase, "latest_label")
                  ? ` · ${stringField(phase, "latest_label")}`
                  : ""}
              </strong>
            </div>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function RuntimeMetric({
  label,
  value,
}: {
  label: string;
  value: boolean | number | string | undefined;
}) {
  return (
    <div>
      <span>{label}</span>
      <strong>{value === undefined ? "none" : String(value)}</strong>
    </div>
  );
}

function recordField(value: Record<string, unknown> | null, key: string): Record<string, unknown> | null {
  const field = value?.[key];
  return isRecord(field) ? field : null;
}

function stringArrayField(value: Record<string, unknown> | null, key: string): string[] {
  const field = value?.[key];
  if (!Array.isArray(field)) {
    return [];
  }
  return field.filter((item): item is string => typeof item === "string");
}

function recordArrayField(value: Record<string, unknown> | null, key: string): Record<string, unknown>[] {
  const field = value?.[key];
  if (!Array.isArray(field)) {
    return [];
  }
  return field.filter(isRecord);
}

function stringField(value: Record<string, unknown> | null, key: string) {
  const field = value?.[key];
  return typeof field === "string" ? field : undefined;
}

function numberField(value: Record<string, unknown> | null, key: string) {
  const field = value?.[key];
  return typeof field === "number" ? field : undefined;
}

function booleanField(value: Record<string, unknown> | null, key: string) {
  const field = value?.[key];
  return typeof field === "boolean" ? field : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
