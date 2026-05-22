import { TranscriptItem } from "../types";

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
        items.map((item) => (
          <article className={`message ${item.role}`} key={item.id}>
            <div className="message-label">{formatRole(item.role)}</div>
            <div className="message-body">{item.text}</div>
          </article>
        ))
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
