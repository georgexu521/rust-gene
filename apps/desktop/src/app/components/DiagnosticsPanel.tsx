import { DesktopDiagnostic } from "../../runtime/desktopApi";

type DiagnosticsPanelProps = {
  diagnostics: DesktopDiagnostic[];
  onRefresh: () => void;
};

export function DiagnosticsPanel({ diagnostics, onRefresh }: DiagnosticsPanelProps) {
  if (diagnostics.length === 0) {
    return null;
  }

  const summary = summarizeDiagnostics(diagnostics);

  return (
    <section className={`diagnostics-panel ${summary.status}`}>
      <div className="diagnostics-heading">
        <div>
          <div className="diagnostics-title">Environment diagnostics</div>
          <div className="diagnostics-summary">{summary.text}</div>
        </div>
        <button type="button" onClick={onRefresh}>
          Refresh
        </button>
      </div>
      <div className="diagnostics-list">
        {diagnostics.map((item) => (
          <article className={`diagnostic-item ${item.status}`} key={item.id}>
            <div className="diagnostic-status">{item.status}</div>
            <div>
              <div className="diagnostic-label">{item.label}</div>
              <div className="diagnostic-detail">{item.detail}</div>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}

function summarizeDiagnostics(diagnostics: DesktopDiagnostic[]) {
  const errors = diagnostics.filter((item) => item.status === "error").length;
  const warnings = diagnostics.filter((item) => item.status === "warning").length;

  if (errors > 0) {
    return {
      status: "error",
      text: `${errors} blocking issue${errors === 1 ? "" : "s"} found`,
    };
  }

  if (warnings > 0) {
    return {
      status: "warning",
      text: `${warnings} warning${warnings === 1 ? "" : "s"} found`,
    };
  }

  return {
    status: "ok",
    text: "Ready for local agent runs",
  };
}
