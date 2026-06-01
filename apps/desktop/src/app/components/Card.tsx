import { type ReactNode, useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";

export type CardTone = "default" | "success" | "warning" | "danger" | "accent" | "violet";

type CardProps = {
  tone?: CardTone;
  icon: ReactNode;
  kind: string;
  name?: ReactNode;
  meta?: ReactNode;
  defaultOpen?: boolean;
  compact?: boolean;
  children: ReactNode;
};

export function Card({
  tone = "default",
  icon,
  kind,
  name,
  meta,
  defaultOpen = true,
  compact = false,
  children,
}: CardProps) {
  const [open, setOpen] = useState(defaultOpen);

  return (
    <div className={`card ${compact ? "card-compact" : ""}`} data-tone={tone} data-open={open}>
      <button
        type="button"
        className="card-head"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
      >
        <span className="card-head-icon">{icon}</span>
        <span className="card-head-kind">{kind}</span>
        {name ? <span className="card-head-name">{name}</span> : null}
        {meta ? <span className="card-head-meta">{meta}</span> : null}
        <span className="card-head-chevron">
          {open ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        </span>
      </button>
      {open ? <div className="card-body">{children}</div> : null}
    </div>
  );
}

// ── Specialized cards ────────────────────────────────────────

type ShellCardProps = {
  command: string;
  output: string;
  exitCode?: number;
  duration?: string;
  defaultOpen?: boolean;
};

export function ShellCard({ command, output, exitCode, duration, defaultOpen }: ShellCardProps) {
  const tone: CardTone = exitCode !== undefined && exitCode !== 0 ? "danger" : "default";
  const meta = [duration, exitCode !== undefined ? `exit ${exitCode}` : null]
    .filter(Boolean)
    .join(" · ");

  return (
    <Card tone={tone} icon={<span>$</span>} kind="Shell" name={command} meta={meta || undefined} defaultOpen={defaultOpen ?? false}>
      <pre className="card-shell-output">{output}</pre>
    </Card>
  );
}

type DiffCardProps = {
  path: string;
  diffPreview: string;
  additions?: number;
  deletions?: number;
  defaultOpen?: boolean;
};

export function DiffCard({ path, diffPreview, additions, deletions, defaultOpen }: DiffCardProps) {
  const meta = [
    additions ? `+${additions}` : null,
    deletions ? `-${deletions}` : null,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <Card tone="accent" icon={<span>±</span>} kind="Edit" name={path} meta={meta || undefined} defaultOpen={defaultOpen ?? true}>
      <pre className="card-diff-preview">{diffPreview}</pre>
    </Card>
  );
}

type ToolCardProps = {
  toolName: string;
  title: string;
  detail?: string;
  status: "running" | "completed" | "failed";
  defaultOpen?: boolean;
  children: ReactNode;
};

export function ToolCard({ toolName, title, detail, status, defaultOpen, children }: ToolCardProps) {
  const tone: CardTone = status === "failed" ? "danger" : status === "running" ? "accent" : "default";
  return (
    <Card
      tone={tone}
      icon={status === "running" ? <span className="pulse-dot" /> : <span>◆</span>}
      kind={toolName}
      name={title}
      meta={detail}
      defaultOpen={defaultOpen ?? status === "failed"}
    >
      {children}
    </Card>
  );
}

type ReasonCardProps = {
  content: string;
  defaultOpen?: boolean;
};

export function ReasonCard({ content, defaultOpen }: ReasonCardProps) {
  if (!content.trim()) return null;
  return (
    <Card tone="violet" icon={<span>💭</span>} kind="Thinking" defaultOpen={defaultOpen ?? false} compact>
      <div className="card-reason-content">{content}</div>
    </Card>
  );
}

type ErrorCardProps = {
  message: string;
};

export function ErrorCard({ message }: ErrorCardProps) {
  return (
    <Card tone="danger" icon={<span>⚠</span>} kind="Error" defaultOpen>
      <div className="card-error-content">{message}</div>
    </Card>
  );
}
