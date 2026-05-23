import { useEffect } from "react";
import { DesktopRunContext } from "../../runtime/desktopApi";
import { TraceItem } from "../types";

type TraceDrawerProps = {
  activeItemId: string | null;
  isOpen: boolean;
  items: TraceItem[];
  onOpenContext?: (context: DesktopRunContext) => void;
  onClose: () => void;
};

export function TraceDrawer({
  activeItemId,
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

      {items.length === 0 ? (
        <div className="trace-empty">No trace events yet</div>
      ) : (
        <div className="trace-list">
          {items.map((item) => (
            <article
              className={`trace-item ${item.kind} ${item.id === activeItemId ? "active" : ""}`}
              data-trace-id={item.id}
              key={item.id}
            >
              <div className="trace-kind">{item.kind}</div>
              <div className="trace-title">{item.title}</div>
              {item.detail ? <div className="trace-detail">{item.detail}</div> : null}
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
