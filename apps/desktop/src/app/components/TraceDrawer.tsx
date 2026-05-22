import { TraceItem } from "../types";

type TraceDrawerProps = {
  isOpen: boolean;
  items: TraceItem[];
  onClose: () => void;
};

export function TraceDrawer({ isOpen, items, onClose }: TraceDrawerProps) {
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

      {items.length === 0 ? (
        <div className="trace-empty">No trace events yet</div>
      ) : (
        <div className="trace-list">
          {items.map((item) => (
            <article className={`trace-item ${item.kind}`} key={item.id}>
              <div className="trace-kind">{item.kind}</div>
              <div className="trace-title">{item.title}</div>
              {item.detail ? <div className="trace-detail">{item.detail}</div> : null}
            </article>
          ))}
        </div>
      )}
    </aside>
  );
}

