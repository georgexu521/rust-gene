import { type ReactNode } from "react";
import { Database, FileCode2, Gauge, Map, RefreshCw } from "lucide-react";
import { DesktopWorkbenchSnapshot } from "../../runtime/desktopApi";

type WorkbenchPanelProps = {
  snapshot: DesktopWorkbenchSnapshot | null;
  onRefresh: () => void;
};

export function WorkbenchPanel({ snapshot, onRefresh }: WorkbenchPanelProps) {
  const projectMap = snapshot?.project_map;
  const symbolIndex = snapshot?.symbol_index;
  const runtime = snapshot?.runtime_context;
  const topFiles = symbolIndex?.files.slice(0, 4) || [];

  return (
    <section className="workbench-panel" aria-label="Frontend workbench">
      <div className="workbench-header">
        <div>
          <span className="workbench-eyebrow">Workbench</span>
          <h2>Project intelligence</h2>
        </div>
        <button
          aria-label="Refresh workbench snapshot"
          className="workbench-refresh"
          type="button"
          onClick={onRefresh}
        >
          <RefreshCw aria-hidden="true" size={15} />
          <span>Refresh</span>
        </button>
      </div>

      <div className="workbench-metrics">
        <WorkbenchMetric
          detail={projectMap?.freshness || "loading"}
          icon={<Map aria-hidden="true" size={16} />}
          label="Project map"
          value={projectMap?.available ? "Available" : "Missing"}
        />
        <WorkbenchMetric
          detail={symbolIndex?.truncated ? "budgeted preview" : "complete preview"}
          icon={<FileCode2 aria-hidden="true" size={16} />}
          label="Symbol index"
          value={`${symbolIndex?.total_symbols ?? 0} symbols`}
        />
        <WorkbenchMetric
          detail={runtime ? `${runtime.history_messages} messages` : "no active runtime"}
          icon={<Gauge aria-hidden="true" size={16} />}
          label="Runtime context"
          value={runtime ? `${runtime.usage_percent}%` : "Idle"}
        />
        <WorkbenchMetric
          detail={runtime ? `${runtime.tool_schema_tokens} tool tokens` : "starts after run"}
          icon={<Database aria-hidden="true" size={16} />}
          label="Cache surface"
          value={runtime?.stable_prefix_fingerprint || "not ready"}
        />
      </div>

      <div className="workbench-body">
        <section className="workbench-map" aria-label="Project map preview">
          <div className="workbench-section-title">
            <Map aria-hidden="true" size={15} />
            <span>Map preview</span>
            {projectMap?.truncated ? <small>truncated</small> : null}
          </div>
          <pre>{projectMap?.content_preview || "Loading project map..."}</pre>
        </section>

        <section className="workbench-index" aria-label="Symbol index preview">
          <div className="workbench-section-title">
            <FileCode2 aria-hidden="true" size={15} />
            <span>Symbol index</span>
            <small>schema v{symbolIndex?.schema_version ?? 1}</small>
          </div>
          {topFiles.length === 0 ? (
            <div className="workbench-empty">No indexed symbols yet</div>
          ) : (
            <div className="workbench-file-list">
              {topFiles.map((file) => (
                <article className="workbench-file" key={file.path}>
                  <div>
                    <strong title={file.path}>{file.path}</strong>
                    <span>
                      {file.lines} lines · {file.hash.slice(0, 8)}
                    </span>
                  </div>
                  <p>{file.summary}</p>
                  {file.symbols.length > 0 ? (
                    <div className="workbench-symbols">
                      {file.symbols.slice(0, 6).map((symbol) => (
                        <code key={`${file.path}:${symbol.kind}:${symbol.name}:${symbol.line}`}>
                          {symbol.kind} {symbol.name}:{symbol.line}
                        </code>
                      ))}
                    </div>
                  ) : null}
                </article>
              ))}
            </div>
          )}
        </section>
      </div>
    </section>
  );
}

function WorkbenchMetric({
  detail,
  icon,
  label,
  value,
}: {
  detail: string;
  icon: ReactNode;
  label: string;
  value: string;
}) {
  return (
    <div className="workbench-metric">
      {icon}
      <span>{label}</span>
      <strong title={value}>{value}</strong>
      <small title={detail}>{detail}</small>
    </div>
  );
}
