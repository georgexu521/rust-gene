import { useEffect, useRef, useState } from "react";
import {
  DesktopToolOutputMeta,
  DesktopToolOutputPage,
  loadDesktopToolOutputIndex,
  loadDesktopToolOutputPage,
} from "../../runtime/desktopApi";
import { useDrawerKeyboard } from "./useDrawerKeyboard";

const PAGE_LIMIT = 64 * 1024;

type ToolOutputDrawerProps = {
  isOpen: boolean;
  sessionId: string | null;
  onClose: () => void;
};

export function ToolOutputDrawer({ isOpen, sessionId, onClose }: ToolOutputDrawerProps) {
  const [items, setItems] = useState<DesktopToolOutputMeta[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [page, setPage] = useState<DesktopToolOutputPage | null>(null);
  const [offset, setOffset] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const drawerRef = useDrawerKeyboard<HTMLElement>({
    initialFocusRef: closeButtonRef,
    isOpen,
    onClose,
  });

  useEffect(() => {
    if (!isOpen || !sessionId) {
      return;
    }

    let cancelled = false;
    setLoading(true);
    setError(null);
    void loadDesktopToolOutputIndex(sessionId)
      .then((nextItems) => {
        if (cancelled) {
          return;
        }
        setItems(nextItems);
        setSelectedId((current) =>
          current && nextItems.some((item) => item.id === current)
            ? current
            : nextItems[0]?.id || null,
        );
        setOffset(0);
      })
      .catch((err) => {
        if (!cancelled) {
          setError(String(err));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [isOpen, sessionId]);

  useEffect(() => {
    if (!isOpen || !sessionId || !selectedId) {
      setPage(null);
      return;
    }

    let cancelled = false;
    setLoading(true);
    setError(null);
    void loadDesktopToolOutputPage(sessionId, selectedId, offset, PAGE_LIMIT)
      .then((nextPage) => {
        if (!cancelled) {
          setPage(nextPage);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(String(err));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [isOpen, sessionId, selectedId, offset]);

  if (!isOpen) {
    return null;
  }

  return (
    <aside ref={drawerRef} className="trace-drawer tool-output-drawer" aria-label="Tool output">
      <div className="trace-header">
        <div>
          <div className="trace-eyebrow">Output</div>
          <h2>Tool output</h2>
        </div>
        <button ref={closeButtonRef} type="button" onClick={onClose}>
          Close
        </button>
      </div>

      {!sessionId ? (
        <div className="trace-empty">No active session</div>
      ) : (
        <div className="tool-output-layout">
          <div className="tool-output-list" aria-label="Stored output">
            {items.length === 0 && !loading ? (
              <div className="trace-empty">No stored output</div>
            ) : (
              items.map((item) => (
                <button
                  className={`tool-output-item${item.id === selectedId ? " active" : ""}`}
                  key={item.id}
                  type="button"
                  onClick={() => {
                    setSelectedId(item.id);
                    setOffset(0);
                  }}
                >
                  <span>{item.tool_name}</span>
                  <small>{formatBytes(item.original_bytes)}</small>
                </button>
              ))
            )}
          </div>

          <section className="tool-output-page" aria-label="Output page">
            {error ? <div className="error-banner">{error}</div> : null}
            {page ? (
              <>
                <div className="tool-output-page-meta">
                  <span>{page.uri}</span>
                  <strong>
                    {formatBytes(page.offset)} -{" "}
                    {formatBytes(Math.min(page.offset + page.content.length, page.total_bytes))} /{" "}
                    {formatBytes(page.total_bytes)}
                  </strong>
                </div>
                <pre>{page.content}</pre>
                <div className="tool-output-page-actions">
                  <button
                    type="button"
                    disabled={page.offset === 0 || loading}
                    onClick={() => setOffset(Math.max(0, page.offset - PAGE_LIMIT))}
                  >
                    Prev
                  </button>
                  <button
                    type="button"
                    disabled={!page.has_more || loading}
                    onClick={() => setOffset(page.offset + page.limit)}
                  >
                    Next
                  </button>
                </div>
              </>
            ) : (
              <div className="trace-empty">{loading ? "Loading output" : "Select output"}</div>
            )}
          </section>
        </div>
      )}
    </aside>
  );
}

function formatBytes(bytes: number) {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
  if (bytes >= 1024) {
    return `${Math.round(bytes / 1024)} KB`;
  }
  return `${bytes} B`;
}
