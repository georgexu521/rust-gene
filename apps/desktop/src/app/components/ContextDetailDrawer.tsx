import { X } from "lucide-react";
import { DesktopRunContext } from "../../runtime/desktopApi";

type ContextDetailDrawerProps = {
  context: DesktopRunContext | null;
  onClose: () => void;
  onRemove: (type: DesktopRunContext["type"]) => void;
};

export function ContextDetailDrawer({ context, onClose, onRemove }: ContextDetailDrawerProps) {
  if (!context) {
    return null;
  }

  const detail = context.detail;

  return (
    <aside className="context-detail-drawer" aria-label="Context details">
      <div className="context-detail-header">
        <div>
          <div className="context-detail-eyebrow">Context</div>
          <h2>{context.label}</h2>
        </div>
        <button aria-label="Close context details" type="button" onClick={onClose}>
          <X aria-hidden="true" size={16} />
        </button>
      </div>

      {!detail ? (
        <div className="context-detail-empty">No detail has been resolved for this context.</div>
      ) : detail.type === "file" ? (
        <div className="context-detail-body">
          <section>
            <h3>File</h3>
            <pre>{detail.relative_path}</pre>
          </section>
          <section>
            <h3>Size</h3>
            <p>
              {detail.size_bytes.toLocaleString()} bytes · {detail.line_count.toLocaleString()} lines
            </p>
          </section>
          {detail.line_start ? (
            <section>
              <h3>Selected range</h3>
              <p>
                {detail.line_start}
                {detail.line_end && detail.line_end !== detail.line_start ? `-${detail.line_end}` : ""}
              </p>
            </section>
          ) : null}
          <section>
            <h3>File preview{detail.truncated ? " (truncated)" : ""}</h3>
            <pre className="context-detail-diff">{detail.preview || "No file preview available."}</pre>
          </section>
        </div>
      ) : (
        <div className="context-detail-body">
          <section>
            <h3>Summary</h3>
            <pre>{detail.shortstat}</pre>
          </section>
          <section>
            <h3>Changed files</h3>
            {detail.files.length ? (
              <ul>
                {detail.files.map((file) => (
                  <li key={file}>{file}</li>
                ))}
              </ul>
            ) : (
              <p>No changed files detected.</p>
            )}
          </section>
          <section>
            <h3>Stat</h3>
            <pre>{detail.stat}</pre>
          </section>
          <section>
            <h3>Patch preview{detail.truncated ? " (truncated)" : ""}</h3>
            <pre className="context-detail-diff">{detail.patch_preview || "No diff preview available."}</pre>
          </section>
        </div>
      )}

      <div className="context-detail-actions">
        <button type="button" onClick={() => onRemove(context.type)}>
          Remove context
        </button>
      </div>
    </aside>
  );
}
