import { TranscriptItem } from "../types";

type TranscriptProps = {
  items: TranscriptItem[];
};

export function Transcript({ items }: TranscriptProps) {
  return (
    <section className="transcript" aria-live="polite">
      {items.length === 0 ? (
        <div className="empty-state">
          <h2>Start a local coding session</h2>
          <p>
            Pick a project, describe the change, and Priority Agent will stream
            progress from the Rust runtime.
          </p>
        </div>
      ) : (
        items.map((item) => (
          <article className={`message ${item.role}`} key={item.id}>
            <div className="message-label">{item.role}</div>
            <div className="message-body">{item.text}</div>
          </article>
        ))
      )}
    </section>
  );
}

