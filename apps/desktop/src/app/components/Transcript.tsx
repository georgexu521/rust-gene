import {
  CheckCircle2,
  CircleAlert,
  CircleDotDashed,
  Clock3,
  KeyRound,
  TerminalSquare,
} from "lucide-react";
import { TimelineKind, TimelineStatus, TranscriptItem } from "../types";

type TranscriptProps = {
  items: TranscriptItem[];
};

export function Transcript({ items }: TranscriptProps) {
  return (
    <section className="transcript" aria-live="polite">
      {items.length === 0 ? (
        <div className="empty-state">
          <div className="empty-kicker">Local runtime ready</div>
          <h2>What should we build in rust-agent?</h2>
          <p>Ask for an edit, review, diagnosis, or verification pass.</p>
        </div>
      ) : (
        items.map((item) =>
          item.role === "timeline" ? (
            <TimelineEvent item={item} key={item.id} />
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

function formatRole(role: TranscriptItem["role"]) {
  if (role === "tool") {
    return "Tool";
  }
  return role === "user" ? "You" : "Liz";
}

type TimelineEventItem = Extract<TranscriptItem, { role: "timeline" }>;

function TimelineEvent({ item }: { item: TimelineEventItem }) {
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
        {item.detail ? <div className="timeline-detail">{item.detail}</div> : null}
        {item.facts && item.facts.length > 0 ? (
          <div className="timeline-facts">
            {item.facts.map((fact) => (
              <span key={fact}>{fact}</span>
            ))}
          </div>
        ) : null}
      </div>
    </article>
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
