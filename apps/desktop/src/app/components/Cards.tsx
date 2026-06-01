import { type ReactNode, useState } from "react";
import {
  Check,
  ChevronDown,
  ChevronRight,
  Terminal,
  FileEdit,
  Brain,
  AlertTriangle,
} from "lucide-react";

// ══════════════════════════════════════════════════════════
// Card — base collapsible card (matches Reasonix's Card)
// ══════════════════════════════════════════════════════════

type Tone = "default" | "success" | "warning" | "danger" | "accent" | "violet";

function Card({
  tone = "default",
  icon,
  kind,
  name,
  meta,
  defaultOpen = true,
  compact = false,
  children,
}: {
  tone?: Tone;
  icon: ReactNode;
  kind: string;
  name?: ReactNode;
  meta?: ReactNode;
  defaultOpen?: boolean;
  compact?: boolean;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);

  return (
    <div className={`rx-card ${compact ? "rx-card-compact" : ""}`} data-tone={tone} data-open={open}>
      <button
        type="button"
        className="rx-card-head"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
      >
        <span className="rx-card-icon">{icon}</span>
        <span className="rx-card-kind">{kind}</span>
        {name ? <span className="rx-card-name">{name}</span> : null}
        <span className="rx-card-grow" />
        {meta ? <span className="rx-card-meta">{meta}</span> : null}
        <span className="rx-card-chevron">
          {open ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        </span>
      </button>
      {open ? <div className="rx-card-body">{children}</div> : null}
    </div>
  );
}

// ══════════════════════════════════════════════════════════
// ReasoningCard — thinking process (matches Reasonix)
// ══════════════════════════════════════════════════════════

function StatusIcon({ state, label }: { state: "running" | "done" | "failed" | "waiting"; label: string }) {
  if (state === "running") return <span className="rx-status-dot running" title={label} />;
  if (state === "done") return <Check size={10} style={{ color: "var(--rx-success)" }} />;
  if (state === "failed") return <AlertTriangle size={10} style={{ color: "var(--rx-danger)" }} />;
  return <span className="rx-status-dot warn" title={label} />;
}

export function ReasoningCard({
  text,
  streaming,
  elapsed,
}: {
  text: string;
  streaming: boolean;
  elapsed?: string;
}) {
  return (
    <Card
      tone="violet"
      icon={<Brain size={12} />}
      kind="Thinking"
      meta={
        <>
          {elapsed ? <span>{elapsed}</span> : null}
          <StatusIcon state={streaming ? "running" : "done"} label={streaming ? "thinking" : "done"} />
        </>
      }
      defaultOpen={streaming}
      compact
    >
      <div className="rx-reasoning-text">
        {text.split(/\n\n+/).map((para, i) => (
          <p key={i}>{para}</p>
        ))}
      </div>
    </Card>
  );
}

// ══════════════════════════════════════════════════════════
// ShellCard — shell command + output (matches Reasonix)
// ══════════════════════════════════════════════════════════

export function ShellCard({
  command,
  output,
  state,
  durationMs,
}: {
  command: string;
  output?: string;
  state: "await" | "running" | "done" | "failed";
  durationMs?: number;
}) {
  const tone: Tone = state === "failed" ? "danger" : state === "done" ? "success" : "warning";
  return (
    <Card
      tone={tone}
      icon={<Terminal size={12} />}
      kind="Shell"
      name={<code className="rx-shell-cmd-text">{command}</code>}
      compact
      defaultOpen={state === "failed"}
      meta={
        <>
          <StatusIcon state={state === "await" ? "waiting" : state === "running" ? "running" : state === "failed" ? "failed" : "done"} label={state} />
          {durationMs ? <span>{(durationMs / 1000).toFixed(1)}s</span> : null}
        </>
      }
    >
      {output ? (
        <pre className="rx-shell-output">
          {output.split("\n").map((ln, i) => {
            if (ln.startsWith("✓")) return <div key={i} className="rx-shell-ok">{ln}</div>;
            if (ln.startsWith("✗") || /error/i.test(ln)) return <div key={i} className="rx-shell-err">{ln}</div>;
            return <div key={i}>{ln}</div>;
          })}
        </pre>
      ) : null}
    </Card>
  );
}

// ══════════════════════════════════════════════════════════
// DiffCard — file edit preview (matches Reasonix)
// ══════════════════════════════════════════════════════════

export function DiffCard({
  filename,
  additions,
  deletions,
  diffPreview,
  applied,
}: {
  filename: string;
  additions?: number;
  deletions?: number;
  diffPreview?: string;
  applied: boolean;
}) {
  return (
    <Card
      tone={applied ? "accent" : "danger"}
      icon={<FileEdit size={12} />}
      kind="Edit"
      name={filename}
      compact
      defaultOpen={!applied}
      meta={
        <>
          {additions ? <span className="rx-diff-add">+{additions}</span> : null}
          {deletions ? <span className="rx-diff-del">-{deletions}</span> : null}
          <StatusIcon state={applied ? "done" : "failed"} label={applied ? "applied" : "failed"} />
        </>
      }
    >
      {diffPreview ? (
        <pre className="rx-diff-preview">{diffPreview}</pre>
      ) : null}
    </Card>
  );
}

// ══════════════════════════════════════════════════════════
// ToolCard — generic tool result (matches Reasonix)
// ══════════════════════════════════════════════════════════

export function ToolCard({
  name,
  detail,
  result,
  ok,
  durationMs,
}: {
  name: string;
  detail?: string;
  result?: string;
  ok: boolean;
  durationMs?: number;
}) {
  const tone: Tone = ok ? "default" : "danger";
  return (
    <Card
      tone={tone}
      icon={ok ? <Check size={12} /> : <AlertTriangle size={12} />}
      kind={name}
      name={detail}
      compact
      defaultOpen={!ok}
      meta={
        <>
          <StatusIcon state={ok ? "done" : "failed"} label={ok ? "done" : "failed"} />
          {durationMs ? <span>{(durationMs / 1000).toFixed(1)}s</span> : null}
        </>
      }
    >
      {result ? (
        <pre className="rx-tool-output">{result.slice(0, 2000)}</pre>
      ) : null}
    </Card>
  );
}

// ══════════════════════════════════════════════════════════
// ToolGroup — collapsible group of consecutive tool calls
// ══════════════════════════════════════════════════════════

export function ToolGroupShell({ count, children }: { count: number; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="rx-tool-group">
      <button className="rx-tool-group-header" onClick={() => setOpen((o) => !o)}>
        <span>{count} tool call{count === 1 ? "" : "s"}</span>
        <span className={`rx-tool-group-chevron ${open ? "open" : ""}`}>
          {open ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        </span>
      </button>
      {open ? <div className="rx-tool-group-body">{children}</div> : null}
    </div>
  );
}
