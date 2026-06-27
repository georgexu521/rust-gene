import { type ReactNode, useEffect, useRef, useState } from "react";
import {
  Activity,
  AlertCircle,
  BarChart3,
  CheckCircle2,
  Database,
  ExternalLink,
  FileCode2,
  Gauge,
  ListChecks,
  Map,
  MessageSquare,
  PauseCircle,
  PanelRight,
  PlayCircle,
  RefreshCw,
  RotateCcw,
  Search,
  UsersRound,
  X,
} from "lucide-react";
import {
  DesktopContextSnapshot,
  DesktopDiagnostic,
  DesktopFilePreview,
  DesktopLabArtifactBody,
  DesktopLabReportPage,
  DesktopLabStatusSnapshot,
  DesktopToolOutputMeta,
  DesktopToolOutputPage,
  DesktopWorkbenchSnapshot,
  loadDesktopLabArtifactBody,
  loadDesktopFilePreview,
  loadDesktopLabReportPage,
  loadDesktopToolOutputIndex,
  loadDesktopToolOutputPage,
} from "../../runtime/desktopApi";
import { type ProviderUsageSnapshot, TraceItem } from "../types";
import { DiagnosticsPanel } from "./DiagnosticsPanel";
import {
  EmptyInspector,
  formatBytes,
  formatTokens,
  InspectorMetric,
  KeyValue,
  MetricGrid,
  nullableTokenCount,
} from "./InspectorPrimitives";
import { useDrawerKeyboard } from "./useDrawerKeyboard";

export type InspectorTab = "context" | "files" | "execution" | "subagents" | "labrun" | "diagnostics";

type InspectorPanelProps = {
  activeTab: InspectorTab;
  contextSnapshot: DesktopContextSnapshot | null;
  diagnostics: DesktopDiagnostic[];
  isRunning: boolean;
  latestUsage: ProviderUsageSnapshot | null;
  pendingPermission: boolean;
  sessionId: string | null;
  snapshot: DesktopWorkbenchSnapshot | null;
  traceItems: TraceItem[];
  onOpenLabReport: (path: string) => void;
  onOpenOutput: () => void;
  onOpenTrace: () => void;
  onRefreshDiagnostics: () => void;
  onRefreshWorkbench: () => void;
  onStageLabCommand: (command: string) => void;
  onSuperviseLabDaemon: () => void;
  onTabChange: (tab: InspectorTab) => void;
  idPrefix?: string;
  isDrawer?: boolean;
  onClose?: () => void;
};

const tabs: Array<{ id: InspectorTab; label: string; icon: ReactNode }> = [
  { id: "context", label: "Context", icon: <Gauge aria-hidden="true" size={14} /> },
  { id: "files", label: "Files", icon: <Map aria-hidden="true" size={14} /> },
  { id: "execution", label: "Execution", icon: <Activity aria-hidden="true" size={14} /> },
  { id: "subagents", label: "Subagents", icon: <UsersRound aria-hidden="true" size={14} /> },
  { id: "labrun", label: "LabRun", icon: <ListChecks aria-hidden="true" size={14} /> },
  { id: "diagnostics", label: "Diagnostics", icon: <AlertCircle aria-hidden="true" size={14} /> },
];

const LAB_REPORT_PAGE_LIMIT = 32 * 1024;

export function InspectorPanel({
  activeTab,
  contextSnapshot,
  diagnostics,
  isRunning,
  latestUsage,
  pendingPermission,
  sessionId,
  snapshot,
  traceItems,
  onOpenLabReport,
  onOpenOutput,
  onOpenTrace,
  onRefreshDiagnostics,
  onRefreshWorkbench,
  onStageLabCommand,
  onSuperviseLabDaemon,
  onTabChange,
  idPrefix = "inspector",
  isDrawer = false,
  onClose,
}: InspectorPanelProps) {
  const effectiveContext = contextSnapshot || snapshot?.runtime_context || null;
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const drawerRef = useDrawerKeyboard<HTMLElement>({
    initialFocusRef: closeButtonRef,
    isOpen: isDrawer,
    onClose: onClose || (() => {}),
  });
  const label = isDrawer ? "Runtime inspector drawer" : "Runtime inspector";

  return (
    <aside
      ref={drawerRef}
      className={`inspector-panel${isDrawer ? " inspector-drawer" : ""}`}
      aria-label={label}
      tabIndex={isDrawer ? -1 : undefined}
    >
      <div className="inspector-header">
        <div>
          <span>Workbench</span>
          <h2>{label}</h2>
        </div>
        <div className="inspector-header-actions">
          <button aria-label="Refresh workbench snapshot" type="button" onClick={onRefreshWorkbench}>
            <RefreshCw aria-hidden="true" size={15} />
          </button>
          {isDrawer && onClose ? (
            <button ref={closeButtonRef} aria-label="Close runtime inspector" type="button" onClick={onClose}>
              <X aria-hidden="true" size={15} />
            </button>
          ) : null}
        </div>
      </div>

      <div className="inspector-tabs" aria-label="Inspector tabs" role="tablist">
        {tabs.map((tab) => (
          <button
            aria-controls={`${idPrefix}-tab-${tab.id}`}
            aria-selected={activeTab === tab.id}
            className={activeTab === tab.id ? "active" : ""}
            id={`${idPrefix}-tab-button-${tab.id}`}
            key={tab.id}
            role="tab"
            type="button"
            onClick={() => onTabChange(tab.id)}
          >
            {tab.icon}
            <span>{tab.label}</span>
          </button>
        ))}
      </div>

      <div
        aria-labelledby={`${idPrefix}-tab-button-${activeTab}`}
        className="inspector-body"
        id={`${idPrefix}-tab-${activeTab}`}
        role="tabpanel"
      >
        {activeTab === "context" ? (
          <ContextInspector latestUsage={latestUsage} snapshot={effectiveContext} />
        ) : null}
        {activeTab === "files" ? <FilesInspector snapshot={snapshot} /> : null}
        {activeTab === "execution" ? (
          <ExecutionInspector
            isRunning={isRunning}
            pendingPermission={pendingPermission}
            sessionId={sessionId}
            traceItems={traceItems}
            onOpenOutput={onOpenOutput}
            onOpenTrace={onOpenTrace}
          />
        ) : null}
        {activeTab === "subagents" ? <SubagentsInspector snapshot={snapshot} /> : null}
        {activeTab === "labrun" ? (
          <LabRunInspector
            context={effectiveContext}
            lab={snapshot?.lab_status || null}
            onOpenLabReport={onOpenLabReport}
            onStageLabCommand={onStageLabCommand}
            onSuperviseLabDaemon={onSuperviseLabDaemon}
          />
        ) : null}
        {activeTab === "diagnostics" ? (
          <DiagnosticsInspector diagnostics={diagnostics} onRefreshDiagnostics={onRefreshDiagnostics} />
        ) : null}
      </div>
    </aside>
  );
}

function ContextInspector({
  latestUsage,
  snapshot,
}: {
  latestUsage: ProviderUsageSnapshot | null;
  snapshot: DesktopContextSnapshot | null;
}) {
  if (!snapshot) {
    return <EmptyInspector icon={<Gauge size={16} />} title="No context snapshot" detail="Context details appear after startup or a run." />;
  }

  const compact = snapshot.compact;
  const contextUsage = Math.max(0, Math.min(100, snapshot.usage_percent));
  const tokenBreakdown = contextTokenBreakdown(snapshot);
  const cacheTotal = snapshot.prompt_cache_cached_tokens + snapshot.prompt_cache_miss_tokens;
  const compactDelta =
    compact.latest_attempt_tokens_before !== undefined &&
    compact.latest_attempt_tokens_before !== null &&
    compact.latest_attempt_tokens_after !== undefined &&
    compact.latest_attempt_tokens_after !== null
      ? compact.latest_attempt_tokens_before - compact.latest_attempt_tokens_after
      : null;

  return (
    <div className="inspector-stack">
      <MetricGrid>
        <InspectorMetric label="Runtime estimate" value={`${snapshot.usage_percent}%`} detail={`${formatTokens(snapshot.total_estimated_tokens)} / ${formatTokens(snapshot.max_context_tokens)}`} />
        <InspectorMetric label="History" value={`${snapshot.history_messages} msg`} detail={`${formatTokens(snapshot.history_tokens)}`} />
        <InspectorMetric label="Cache hit" value={`${snapshot.prompt_cache_hit_rate_percent.toFixed(1)}%`} detail={`${formatTokens(snapshot.prompt_cache_cached_tokens)} cached`} />
        <InspectorMetric label="Compression" value={`${compact.compression_count}`} detail={compact.circuit_open ? "circuit open" : "circuit closed"} />
      </MetricGrid>

      <section className="inspector-card">
        <div className="inspector-section-title">
          <Gauge aria-hidden="true" size={14} />
          <span>Token budget</span>
        </div>
        <div className="inspector-budget-bar" aria-label="Context usage">
          <span style={{ width: `${contextUsage}%` }} />
        </div>
        <KeyValue label="Used" value={`${formatTokens(snapshot.total_estimated_tokens)} (${snapshot.usage_percent}%)`} />
        <KeyValue label="Window" value={formatTokens(snapshot.max_context_tokens)} />
        <KeyValue label="Stable prefix" value={snapshot.stable_prefix_fingerprint || "unavailable"} />
        <div className="inspector-token-breakdown" aria-label="Token breakdown">
          {tokenBreakdown.map((item) => (
            <div key={item.label}>
              <span>{item.label}</span>
              <strong>{formatTokens(item.tokens)}</strong>
              <small>{item.percent.toFixed(1)}%</small>
              <i style={{ width: `${Math.max(2, item.percent)}%` }} />
            </div>
          ))}
        </div>
      </section>

      <section className="inspector-card">
        <div className="inspector-section-title">
          <Database aria-hidden="true" size={14} />
          <span>Prompt cache</span>
        </div>
        <KeyValue label="Cached/read" value={formatTokens(snapshot.prompt_cache_cached_tokens)} />
        <KeyValue label="Miss" value={formatTokens(snapshot.prompt_cache_miss_tokens)} />
        <KeyValue label="Total seen" value={formatTokens(cacheTotal)} />
        <KeyValue label="Hit rate" value={`${snapshot.prompt_cache_hit_rate_percent.toFixed(1)}%`} />
        <KeyValue label="Diagnostics" value={`${snapshot.prompt_cache_diagnostic_count}`} />
        <KeyValue label="Reason" value={snapshot.prompt_cache_last_reason || "unavailable"} />
        <div className="inspector-note">
          Provider cache-write tokens are shown as unavailable unless the provider usage payload exposes them.
        </div>
      </section>

      <section className="inspector-card">
        <div className="inspector-section-title">
          <BarChart3 aria-hidden="true" size={14} />
          <span>Compression and provider usage</span>
        </div>
        <KeyValue label="Count" value={`${compact.compression_count}`} />
        <KeyValue label="Circuit" value={compact.circuit_open ? "open" : "closed"} />
        <KeyValue label="Strategy" value={compact.latest_strategy || "none"} />
        <KeyValue label="Last trigger" value={compact.latest_attempt_trigger || "none"} />
        <KeyValue label="Decision" value={compact.latest_attempt_decision || "none"} />
        <KeyValue label="Before" value={nullableTokenCount(compact.latest_attempt_tokens_before)} />
        <KeyValue label="After" value={nullableTokenCount(compact.latest_attempt_tokens_after)} />
        <KeyValue label="Saved" value={compactDelta === null ? "unavailable" : formatTokens(Math.max(0, compactDelta))} />
        <KeyValue label="Provider input" value={nullableTokenCount(latestUsage?.promptTokens)} />
        <KeyValue label="Provider output" value={nullableTokenCount(latestUsage?.completionTokens)} />
        <KeyValue label="Provider total" value={nullableTokenCount(latestUsage?.totalTokens)} />
        <KeyValue label="Provider reasoning" value={nullableTokenCount(latestUsage?.reasoningTokens)} />
        <KeyValue label="Cache write" value={nullableTokenCount(latestUsage?.cacheWriteTokens)} />
        <div className="inspector-note">
          Runtime estimate is for context planning. Provider usage shows the latest completed usage event; missing provider fields remain unavailable.
        </div>
      </section>
    </div>
  );
}

function FilesInspector({ snapshot }: { snapshot: DesktopWorkbenchSnapshot | null }) {
  const projectMap = snapshot?.project_map;
  const symbolIndex = snapshot?.symbol_index;
  const topFiles = symbolIndex?.files.slice(0, 5) || [];
  const [selectedFilePath, setSelectedFilePath] = useState<string | null>(null);
  const [filePreview, setFilePreview] = useState<DesktopFilePreview | null>(null);
  const [filePreviewLoading, setFilePreviewLoading] = useState(false);
  const [filePreviewError, setFilePreviewError] = useState<string | null>(null);

  useEffect(() => {
    if (!selectedFilePath) {
      setFilePreview(null);
      setFilePreviewError(null);
      return;
    }

    let cancelled = false;
    setFilePreviewLoading(true);
    setFilePreviewError(null);
    void loadDesktopFilePreview(selectedFilePath, 32 * 1024)
      .then((preview) => {
        if (!cancelled) {
          setFilePreview(preview);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setFilePreview(null);
          setFilePreviewError(String(err));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setFilePreviewLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [selectedFilePath]);

  return (
    <div className="inspector-stack">
      <MetricGrid>
        <InspectorMetric label="Project map" value={projectMap?.available ? "Available" : "Missing"} detail={projectMap?.freshness || "loading"} />
        <InspectorMetric label="Symbols" value={`${symbolIndex?.total_symbols ?? 0}`} detail={symbolIndex?.truncated ? "truncated" : "preview"} />
      </MetricGrid>
      <section className="inspector-card">
        <div className="inspector-section-title">
          <Map aria-hidden="true" size={14} />
          <span>Map preview</span>
        </div>
        <pre className="inspector-pre">{projectMap?.content_preview || "Loading project map..."}</pre>
      </section>
      <section className="inspector-card">
        <div className="inspector-section-title">
          <FileCode2 aria-hidden="true" size={14} />
          <span>Symbol index</span>
        </div>
        {topFiles.length === 0 ? (
          <div className="inspector-empty">No indexed symbols yet</div>
        ) : (
          <div className="inspector-list">
            {topFiles.map((file) => (
              <button
                className={`inspector-list-item action ${file.path === selectedFilePath ? "active" : ""}`}
                key={file.path}
                type="button"
                onClick={() => setSelectedFilePath(file.path)}
              >
                <strong title={file.path}>{file.path}</strong>
                <span>{file.lines} lines · {file.hash.slice(0, 8)}</span>
                <p title={file.summary}>{file.summary}</p>
              </button>
            ))}
          </div>
        )}
      </section>
      <section className="inspector-card">
        <div className="inspector-section-title">
          <FileCode2 aria-hidden="true" size={14} />
          <span>Selected file preview</span>
        </div>
        {!selectedFilePath ? (
          <div className="inspector-empty">Select an indexed file to preview it.</div>
        ) : filePreviewError ? (
          <div className="inspector-error">{filePreviewError}</div>
        ) : filePreview ? (
          <div className="inspector-file-preview" aria-label="Selected file preview">
            <div>
              <strong title={filePreview.path}>{filePreview.path}</strong>
              <span>
                {filePreview.line_count.toLocaleString()} lines · {formatBytes(filePreview.total_bytes)}
                {filePreview.truncated ? " · truncated" : ""}
              </span>
            </div>
            <pre>{filePreview.content || "File preview is empty."}</pre>
          </div>
        ) : (
          <div className="inspector-empty">{filePreviewLoading ? "Loading file preview" : "No file preview loaded"}</div>
        )}
      </section>
    </div>
  );
}

function ExecutionInspector({
  isRunning,
  pendingPermission,
  sessionId,
  traceItems,
  onOpenOutput,
  onOpenTrace,
}: {
  isRunning: boolean;
  pendingPermission: boolean;
  sessionId: string | null;
  traceItems: TraceItem[];
  onOpenOutput: () => void;
  onOpenTrace: () => void;
}) {
  const recent = traceItems.slice(-14).reverse();
  const [outputItems, setOutputItems] = useState<DesktopToolOutputMeta[]>([]);
  const [selectedOutputId, setSelectedOutputId] = useState<string | null>(null);
  const [outputPage, setOutputPage] = useState<DesktopToolOutputPage | null>(null);
  const [outputLoading, setOutputLoading] = useState(false);
  const [outputError, setOutputError] = useState<string | null>(null);

  useEffect(() => {
    if (!sessionId) {
      setOutputItems([]);
      setSelectedOutputId(null);
      setOutputPage(null);
      return;
    }

    let cancelled = false;
    setOutputLoading(true);
    setOutputError(null);
    void loadDesktopToolOutputIndex(sessionId)
      .then((items) => {
        if (cancelled) {
          return;
        }
        setOutputItems(items);
        setSelectedOutputId((current) =>
          current && items.some((item) => item.id === current) ? current : items[0]?.id || null,
        );
      })
      .catch((err) => {
        if (!cancelled) {
          setOutputError(String(err));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setOutputLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [sessionId]);

  useEffect(() => {
    if (!sessionId || !selectedOutputId) {
      setOutputPage(null);
      return;
    }

    let cancelled = false;
    setOutputLoading(true);
    setOutputError(null);
    void loadDesktopToolOutputPage(sessionId, selectedOutputId, 0, 8192)
      .then((page) => {
        if (!cancelled) {
          setOutputPage(page);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setOutputError(String(err));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setOutputLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [sessionId, selectedOutputId]);

  return (
    <div className="inspector-stack">
      <MetricGrid>
        <InspectorMetric label="Run" value={isRunning ? "Running" : "Idle"} detail={pendingPermission ? "waiting permission" : "ready"} />
        <InspectorMetric label="Trace" value={`${traceItems.length}`} detail="events" />
        <InspectorMetric label="Output" value={`${outputItems.length}`} detail={sessionId ? "stored pages" : "no session"} />
      </MetricGrid>
      <div className="inspector-actions">
        <button type="button" onClick={onOpenTrace}>
          <PanelRight aria-hidden="true" size={14} />
          Open trace
        </button>
        <button type="button" onClick={onOpenOutput}>
          <ExternalLink aria-hidden="true" size={14} />
          Tool output
        </button>
      </div>
      <section className="inspector-card">
        <div className="inspector-section-title">
          <Activity aria-hidden="true" size={14} />
          <span>Trace evidence</span>
        </div>
        {recent.length === 0 ? (
          <div className="inspector-empty">No runtime events yet</div>
        ) : (
          <div className="inspector-list">
            {recent.map((item) => (
              <TraceEvidenceItem item={item} key={item.id} />
            ))}
          </div>
        )}
      </section>
      <section className="inspector-card">
        <div className="inspector-section-title">
          <ExternalLink aria-hidden="true" size={14} />
          <span>Stored output</span>
        </div>
        {!sessionId ? (
          <div className="inspector-empty">No active session</div>
        ) : outputError ? (
          <div className="inspector-error">{outputError}</div>
        ) : outputItems.length === 0 && !outputLoading ? (
          <div className="inspector-empty">No stored output</div>
        ) : (
          <div className="inspector-output-browser">
            <div className="inspector-output-list" aria-label="Inspector stored output">
              {outputItems.map((item) => (
                <button
                  className={item.id === selectedOutputId ? "active" : ""}
                  key={item.id}
                  type="button"
                  onClick={() => setSelectedOutputId(item.id)}
                >
                  <span>{item.tool_name}</span>
                  <small>{formatBytes(item.original_bytes)}</small>
                </button>
              ))}
            </div>
            <div className="inspector-output-preview" aria-label="Inspector output preview">
              {outputPage ? (
                <>
                  <div>
                    <span title={outputPage.uri}>{outputPage.uri}</span>
                    <strong>{formatBytes(outputPage.total_bytes)}</strong>
                  </div>
                  <pre>{outputPage.content || "Output page is empty"}</pre>
                  {outputPage.has_more ? <small>Open output drawer for additional pages.</small> : null}
                </>
              ) : (
                <div className="inspector-empty">{outputLoading ? "Loading output" : "Select output"}</div>
              )}
            </div>
          </div>
        )}
      </section>
    </div>
  );
}

function TraceEvidenceItem({ item }: { item: TraceItem }) {
  return (
    <article className={`inspector-list-item ${item.status || item.kind}`}>
      <strong>{item.title}</strong>
      <span>{item.kind}{item.status ? ` · ${item.status}` : ""}</span>
      {item.detail ? <p title={item.detail}>{item.detail}</p> : null}
      {item.summary ? <InspectorTraceSummary summary={item.summary} /> : null}
      {item.facts && item.facts.length > 0 ? (
        <div className="inspector-chip-row">
          {item.facts.slice(0, 4).map((fact) => (
            <code key={fact} title={fact}>{fact}</code>
          ))}
        </div>
      ) : null}
    </article>
  );
}

function InspectorTraceSummary({ summary }: { summary: TraceItem["summary"] }) {
  if (!summary) {
    return null;
  }

  if (summary.kind === "shell") {
    return (
      <div className="inspector-evidence-summary">
        <span>Command</span>
        <code title={summary.command}>{summary.command}</code>
        <small>{[summary.validation, summary.exitCode !== undefined ? `exit ${summary.exitCode}` : null, summary.duration].filter(Boolean).join(" · ")}</small>
      </div>
    );
  }

  if (summary.kind === "file") {
    return (
      <div className="inspector-evidence-summary">
        <span>File</span>
        {summary.path ? <code title={summary.path}>{summary.path}</code> : null}
        <small>
          {[
            summary.action,
            summary.operations !== undefined ? `${summary.operations} ops` : null,
            summary.replacements !== undefined ? `${summary.replacements} replacements` : null,
            summary.additions !== undefined ? `+${summary.additions}` : null,
            summary.deletions !== undefined ? `-${summary.deletions}` : null,
          ].filter(Boolean).join(" · ")}
        </small>
      </div>
    );
  }

  if (summary.kind === "failure") {
    return (
      <div className="inspector-evidence-summary failed">
        <span>Failure</span>
        <strong>{summary.reason}</strong>
        {summary.recovery ? <small>{summary.recovery}</small> : null}
      </div>
    );
  }

  if (summary.kind === "permission") {
    return (
      <div className="inspector-evidence-summary waiting">
        <span>Permission</span>
        <small>
          {[
            summary.actionDecision ? `review ${summary.actionDecision}` : null,
            summary.risk ? `risk ${summary.risk}` : null,
            summary.checkpoint ? `checkpoint ${summary.checkpoint.replaceAll("_", " ")}` : null,
          ].filter(Boolean).join(" · ")}
        </small>
        {summary.reason ? <strong>{summary.reason}</strong> : null}
      </div>
    );
  }

  if (summary.kind === "run") {
    return (
      <div className="inspector-evidence-summary">
        <span>Run</span>
        <strong>{summary.headline}</strong>
        {summary.stats?.length ? <small>{summary.stats.join(" · ")}</small> : null}
      </div>
    );
  }

  return null;
}

function SubagentsInspector({ snapshot }: { snapshot: DesktopWorkbenchSnapshot | null }) {
  const tasks = snapshot?.subagent_tasks || [];

  return (
    <div className="inspector-stack">
      <MetricGrid>
        <InspectorMetric label="Tasks" value={`${tasks.length}`} detail="recent durable tasks" />
        <InspectorMetric label="Active" value={`${tasks.filter((task) => task.status !== "completed").length}`} detail="not completed" />
      </MetricGrid>
      {tasks.length === 0 ? (
        <EmptyInspector icon={<UsersRound size={16} />} title="No durable subagent tasks" detail="Subagent artifacts appear here after background or delegated work." />
      ) : (
        <div className="inspector-list">
          {tasks.slice(0, 8).map((task) => (
            <article className="inspector-list-item" key={`${task.task_id}:${task.agent_id}`}>
              <strong title={task.task_id}>{task.task_id}</strong>
              <span>{task.status} · {task.profile || task.role}</span>
              <p title={task.description}>{task.description}</p>
              <div className="inspector-chip-row">
                {task.tools_used.slice(0, 4).map((tool) => (
                  <code key={`${task.task_id}:${tool}`}>{tool}</code>
                ))}
                {task.result_artifact_id ? <code>artifact {task.result_artifact_id}</code> : null}
              </div>
            </article>
          ))}
        </div>
      )}
    </div>
  );
}

function LabRunInspector({
  context,
  lab,
  onOpenLabReport,
  onStageLabCommand,
  onSuperviseLabDaemon,
}: {
  context: DesktopContextSnapshot | null;
  lab: DesktopLabStatusSnapshot | null;
  onOpenLabReport: (path: string) => void;
  onStageLabCommand: (command: string) => void;
  onSuperviseLabDaemon: () => void;
}) {
  const [professorMessage, setProfessorMessage] = useState("");
  const [labSearch, setLabSearch] = useState("");
  const [selectedReportPath, setSelectedReportPath] = useState<string | null>(null);
  const [reportOffset, setReportOffset] = useState(0);
  const [reportPage, setReportPage] = useState<DesktopLabReportPage | null>(null);
  const [reportLoading, setReportLoading] = useState(false);
  const [reportError, setReportError] = useState<string | null>(null);
  const [selectedArtifactId, setSelectedArtifactId] = useState<string | null>(null);
  const [artifactBody, setArtifactBody] = useState<DesktopLabArtifactBody | null>(null);
  const [artifactBodyLoading, setArtifactBodyLoading] = useState(false);
  const [artifactBodyError, setArtifactBodyError] = useState<string | null>(null);

  useEffect(() => {
    if (!selectedReportPath) {
      setReportPage(null);
      setReportError(null);
      setReportLoading(false);
      return;
    }

    let cancelled = false;
    setReportLoading(true);
    setReportError(null);
    loadDesktopLabReportPage(selectedReportPath, reportOffset, LAB_REPORT_PAGE_LIMIT)
      .then((page) => {
        if (cancelled) {
          return;
        }
        setReportPage(page);
      })
      .catch((error: unknown) => {
        if (cancelled) {
          return;
        }
        setReportPage(null);
        setReportError(error instanceof Error ? error.message : String(error));
      })
      .finally(() => {
        if (!cancelled) {
          setReportLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [selectedReportPath, reportOffset]);

  useEffect(() => {
    if (!selectedArtifactId) {
      setArtifactBody(null);
      setArtifactBodyError(null);
      setArtifactBodyLoading(false);
      return;
    }

    let cancelled = false;
    setArtifactBodyLoading(true);
    setArtifactBodyError(null);
    loadDesktopLabArtifactBody(selectedArtifactId)
      .then((body) => {
        if (cancelled) {
          return;
        }
        setArtifactBody(body);
      })
      .catch((error: unknown) => {
        if (cancelled) {
          return;
        }
        setArtifactBody(null);
        setArtifactBodyError(error instanceof Error ? error.message : String(error));
      })
      .finally(() => {
        if (!cancelled) {
          setArtifactBodyLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [selectedArtifactId]);

  if (!lab) {
    return <EmptyInspector icon={<ListChecks size={16} />} title="LabRun loading" detail="LabRun status appears after the workbench snapshot loads." />;
  }

  const proposalCommand = lab.proposal_id ? `/lab approve ${lab.proposal_id}` : "/lab start ";
  const professorCommand = professorMessage.trim()
    ? `/lab professor ${professorMessage.trim()}`
    : "/lab professor ";
  const interventionCommand = professorMessage.trim()
    ? `/lab intervene ${professorMessage.trim()}`
    : "/lab intervene ";
  const taskProgress = lab.task_total > 0 ? Math.round((Math.max(0, lab.task_total - lab.task_open) / lab.task_total) * 100) : 0;
  const contextUsage = context ? Math.max(0, Math.min(100, context.usage_percent)) : 0;
  const cacheTotal = context ? context.prompt_cache_cached_tokens + context.prompt_cache_miss_tokens : 0;
  const labQuery = labSearch.trim().toLowerCase();
  const artifactRows = lab.artifacts.filter((artifact) => matchesLabArtifact(artifact, labQuery));
  const reportRows = lab.reports.filter((report) => matchesLabReport(report, labQuery));
  const evidenceRows = lab.evidence_refs.filter((evidence) => matchesLabEvidence(evidence, labQuery));
  const timelineNodes = labRunTimelineNodes(lab);
  const previewReport = (path: string) => {
    setSelectedReportPath(path);
    setReportOffset(0);
  };
  const previewArtifactBody = (artifactId: string) => {
    setSelectedArtifactId(artifactId);
  };
  const previousReportPage = () => {
    setReportOffset((current) => Math.max(0, current - LAB_REPORT_PAGE_LIMIT));
  };
  const nextReportPage = () => {
    if (!reportPage?.has_more) {
      return;
    }
    setReportOffset(reportPage.offset + reportPage.limit);
  };

  return (
    <div className="inspector-stack">
      <MetricGrid>
        <InspectorMetric label="State" value={lab.run_status || lab.proposal_status || lab.state} detail={lab.owner || "no owner"} />
        <InspectorMetric label="Tasks" value={`${lab.task_open}/${lab.task_total}`} detail={`${lab.task_blocked} blocked`} />
        <InspectorMetric label="Artifacts" value={`${lab.artifact_count}`} detail={`${lab.meeting_count} meetings`} />
        <InspectorMetric label="Retries" value={`${lab.validation_retry_count}`} detail={`${lab.validation_retry_escalated_count} escalated`} />
      </MetricGrid>
      <div className="inspector-actions">
        {lab.latest_report_path ? (
          <button type="button" onClick={() => onOpenLabReport(lab.latest_report_path!)}>
            <ExternalLink aria-hidden="true" size={14} />
            Open report
          </button>
        ) : null}
        <button type="button" onClick={onSuperviseLabDaemon}>
          <Activity aria-hidden="true" size={14} />
          Supervise
        </button>
        <button type="button" onClick={() => onStageLabCommand("/lab meeting open")}>
          <MessageSquare aria-hidden="true" size={14} />
          Meeting
        </button>
        <button type="button" onClick={() => onStageLabCommand("/lab dashboard")}>
          <RotateCcw aria-hidden="true" size={14} />
          Dashboard
        </button>
      </div>
      <section className="inspector-card labrun-proposal-card" aria-label="LabRun proposal intake">
        <div className="inspector-section-title">
          <ListChecks aria-hidden="true" size={14} />
          <span>Proposal intake</span>
        </div>
        <p>
          Discuss scope with the professor first, then stage formal approval when the proposal is ready.
        </p>
        <KeyValue label="Proposal" value={lab.proposal_id || "not drafted"} />
        <KeyValue label="Status" value={lab.proposal_status || lab.run_status || lab.state} />
        <div className="inspector-actions labrun-action-grid">
          <button
            type="button"
            onClick={() => onStageLabCommand(lab.proposal_id ? "/lab proposal" : "/lab start ")}
          >
            <ListChecks aria-hidden="true" size={14} />
            Proposal
          </button>
          <button type="button" onClick={() => onStageLabCommand(proposalCommand)}>
            <PlayCircle aria-hidden="true" size={14} />
            {lab.proposal_id ? "Approve proposal" : "Draft proposal"}
          </button>
        </div>
      </section>
      <section className="inspector-card labrun-control-card" aria-label="LabRun project controls">
        <div className="inspector-section-title">
          <Activity aria-hidden="true" size={14} />
          <span>Project controls</span>
        </div>
        <div className="inspector-actions labrun-action-grid">
          <button type="button" onClick={() => onStageLabCommand("/lab resume")}>
            <PlayCircle aria-hidden="true" size={14} />
            Resume LabRun
          </button>
          <button type="button" onClick={() => onStageLabCommand("/lab pause user_pause")}>
            <PauseCircle aria-hidden="true" size={14} />
            Pause LabRun
          </button>
          <button type="button" onClick={() => onStageLabCommand("/lab continue ")}>
            <RotateCcw aria-hidden="true" size={14} />
            Continue cycle
          </button>
          <button type="button" onClick={() => onStageLabCommand("/lab meeting open")}>
            <UsersRound aria-hidden="true" size={14} />
            Open meeting
          </button>
        </div>
      </section>
      <section className="inspector-card labrun-status-board" aria-label="LabRun status board">
        <div className="inspector-section-title">
          <ListChecks aria-hidden="true" size={14} />
          <span>Status board</span>
        </div>
        <div className="labrun-stage-row">
          <div>
            <span>Stage</span>
            <strong title={lab.stage || lab.proposal_status || lab.state}>{lab.stage || lab.proposal_status || lab.state}</strong>
          </div>
          <div>
            <span>Owner</span>
            <strong>{lab.owner || "unassigned"}</strong>
          </div>
          <div>
            <span>Cycle</span>
            <strong>{lab.cycle_count}</strong>
          </div>
        </div>
        <div className="labrun-progress-row">
          <div>
            <span>Tasks completed</span>
            <strong>{Math.max(0, lab.task_total - lab.task_open)} / {lab.task_total}</strong>
          </div>
          <div className="inspector-budget-bar" aria-label="LabRun task progress">
            <span style={{ width: `${taskProgress}%` }} />
          </div>
        </div>
        <KeyValue label="Run" value={lab.lab_run_id || lab.proposal_id || "none"} />
        <KeyValue label="Blocked" value={`${lab.task_blocked}`} />
        <KeyValue label="Needs user" value={lab.needs_user ? "yes" : "no"} />
        <KeyValue label="Meeting" value={lab.meeting_recommended ? "recommended" : "quiet"} />
        <KeyValue label="Topic" value={lab.meeting_topic || "none"} />
        <KeyValue label="Detail" value={lab.detail} />
      </section>
      <section className="inspector-card labrun-timeline-card" aria-label="LabRun role timeline graph">
        <div className="inspector-section-title">
          <UsersRound aria-hidden="true" size={14} />
          <span>Role timeline</span>
        </div>
        <div className="labrun-timeline-graph">
          {timelineNodes.map((node, index) => (
            <div className="labrun-timeline-step" key={node.id}>
              <div
                aria-label={`${node.label}: ${node.status}`}
                className={`labrun-timeline-node ${node.status}`}
              >
                <span>{index + 1}</span>
              </div>
              {index < timelineNodes.length - 1 ? (
                <div
                  aria-hidden="true"
                  className={`labrun-timeline-link ${node.status}`}
                />
              ) : null}
              <div className="labrun-timeline-copy">
                <strong>{node.label}</strong>
                <small>{node.detail}</small>
              </div>
            </div>
          ))}
        </div>
      </section>
      <section className="inspector-card labrun-side-channel" aria-label="Professor side-channel">
        <div className="inspector-section-title">
          <MessageSquare aria-hidden="true" size={14} />
          <span>Professor side-channel</span>
        </div>
        <p>
          User feedback enters LabRun through the professor, then the professor steers the postdoc/graduate loop.
        </p>
        <textarea
          aria-label="Professor message"
          placeholder="Tell the professor what changed, what is blocked, or what direction should be reconsidered."
          value={professorMessage}
          onChange={(event) => setProfessorMessage(event.target.value)}
        />
        <div className="inspector-actions labrun-action-grid">
          <button type="button" onClick={() => onStageLabCommand(professorCommand)}>
            <MessageSquare aria-hidden="true" size={14} />
            Message professor
          </button>
          <button type="button" onClick={() => onStageLabCommand(interventionCommand)}>
            <AlertCircle aria-hidden="true" size={14} />
            Urgent intervention
          </button>
        </div>
      </section>
      <section className="inspector-card labrun-report-card" aria-label="LabRun reports and artifacts">
        <div className="inspector-section-title">
          <ExternalLink aria-hidden="true" size={14} />
          <span>Reports and artifacts</span>
        </div>
        <KeyValue label="Artifacts" value={`${lab.artifact_count}`} />
        <KeyValue label="Meetings" value={`${lab.meeting_count}`} />
        <KeyValue label="Latest report" value={lab.latest_report_path || "none"} />
        <label className="labrun-search-box">
          <Search aria-hidden="true" size={14} />
          <input
            aria-label="Search LabRun artifacts"
            placeholder="Search artifacts, reports, or evidence"
            value={labSearch}
            onChange={(event) => setLabSearch(event.target.value)}
          />
        </label>
        {artifactRows.length > 0 ? (
          <div className="inspector-list labrun-data-list" aria-label="LabRun artifacts">
            {artifactRows.map((artifact) => (
              <article className="inspector-list-item" key={artifact.artifact_id}>
                <strong title={artifact.title}>{artifact.title}</strong>
                <span>
                  {artifact.artifact_type} · {artifact.stage} · {artifact.owner}
                </span>
                <p title={artifact.artifact_id}>
                  {artifact.status}{artifact.validation_status ? ` · ${artifact.validation_status}` : ""}
                </p>
                <div className="inspector-chip-row">
                  <code title={artifact.artifact_id}>{artifact.artifact_id}</code>
                  {artifact.evidence_refs.slice(0, 3).map((evidenceRef) => (
                    <code key={`${artifact.artifact_id}:${evidenceRef}`} title={evidenceRef}>{evidenceRef}</code>
                  ))}
                </div>
                {artifact.report_preview ? (
                  <pre className="labrun-report-preview">
                    {artifact.report_preview}{artifact.report_preview_truncated ? "\n..." : ""}
                  </pre>
                ) : null}
                {artifact.report_path ? (
                  <div className="inspector-actions labrun-row-actions">
                    <button type="button" onClick={() => onOpenLabReport(artifact.report_path!)}>
                      <ExternalLink aria-hidden="true" size={13} />
                      Open report
                    </button>
                    <button type="button" onClick={() => previewReport(artifact.report_path!)}>
                      <FileCode2 aria-hidden="true" size={13} />
                      Preview full report
                    </button>
                    <button type="button" onClick={() => previewArtifactBody(artifact.artifact_id)}>
                      <FileCode2 aria-hidden="true" size={13} />
                      Preview artifact body
                    </button>
                    <button type="button" onClick={() => onStageLabCommand(`/lab review artifact ${artifact.artifact_id} `)}>
                      <CheckCircle2 aria-hidden="true" size={13} />
                      Review artifact
                    </button>
                  </div>
                ) : (
                  <div className="inspector-actions labrun-row-actions">
                    <button type="button" onClick={() => previewArtifactBody(artifact.artifact_id)}>
                      <FileCode2 aria-hidden="true" size={13} />
                      Preview artifact body
                    </button>
                  </div>
                )}
              </article>
            ))}
          </div>
        ) : lab.artifacts.length > 0 ? (
          <div className="inspector-empty">No matching artifacts</div>
        ) : (
          <div className="inspector-empty">No structured artifacts yet</div>
        )}
        {reportRows.length > 0 ? (
          <div className="labrun-evidence-block" aria-label="LabRun report previews">
            <div className="inspector-section-title">
              <FileCode2 aria-hidden="true" size={14} />
              <span>Report previews</span>
            </div>
            <div className="inspector-list labrun-data-list">
              {reportRows.map((report) => (
                <article className="inspector-list-item" key={report.artifact_id}>
                  <strong title={report.artifact_id}>{report.artifact_id}</strong>
                  <span title={report.path}>{report.path}</span>
                  {report.preview ? (
                    <pre className="labrun-report-preview">
                      {report.preview}{report.truncated ? "\n..." : ""}
                    </pre>
                  ) : (
                    <p>No report preview available</p>
                  )}
                  <div className="inspector-actions labrun-row-actions">
                    <button type="button" onClick={() => onOpenLabReport(report.path)}>
                      <ExternalLink aria-hidden="true" size={13} />
                      Open report
                    </button>
                    <button type="button" onClick={() => previewReport(report.path)}>
                      <FileCode2 aria-hidden="true" size={13} />
                      Preview full report
                    </button>
                  </div>
                </article>
              ))}
            </div>
          </div>
        ) : lab.reports.length > 0 && labQuery ? (
          <div className="inspector-empty">No matching reports</div>
        ) : null}
        {(selectedArtifactId || artifactBodyLoading || artifactBodyError || artifactBody) ? (
          <div className="labrun-full-report" aria-label="LabRun artifact body viewer">
            <div className="inspector-section-title">
              <FileCode2 aria-hidden="true" size={14} />
              <span>Artifact body</span>
            </div>
            <div className="inspector-actions labrun-row-actions">
              <button type="button" onClick={() => setSelectedArtifactId(null)}>
                Close body
              </button>
            </div>
            {artifactBodyLoading ? (
              <div className="inspector-empty">Loading artifact body</div>
            ) : artifactBodyError ? (
              <div className="inspector-error">{artifactBodyError}</div>
            ) : artifactBody ? (
              <>
                <KeyValue label="Artifact" value={artifactBody.artifact_id} />
                <KeyValue label="Type" value={artifactBody.artifact_type} />
                <KeyValue label="Owner" value={`${artifactBody.owner} · ${artifactBody.status}`} />
                <KeyValue label="Validation" value={artifactBody.validation_status || "none"} />
                <pre className="labrun-report-preview">{artifactBody.content || "Artifact body is empty."}</pre>
              </>
            ) : null}
          </div>
        ) : null}
        {evidenceRows.length > 0 ? (
          <div className="labrun-evidence-block" aria-label="LabRun evidence refs">
            <div className="inspector-section-title">
              <CheckCircle2 aria-hidden="true" size={14} />
              <span>Evidence refs</span>
            </div>
            <div className="inspector-list labrun-data-list">
              {evidenceRows.map((evidence) => (
                <article className="inspector-list-item" key={evidence.evidence_id}>
                  <strong title={evidence.summary}>{evidence.summary}</strong>
                  <span>
                    {evidence.kind} · {evidence.role} · {evidence.estimated_summary_tokens} tokens
                  </span>
                  <p title={evidence.reference}>{evidence.reference}</p>
                  <div className="inspector-chip-row">
                    <code title={evidence.evidence_id}>{evidence.evidence_id}</code>
                    {evidence.artifact_id ? <code title={evidence.artifact_id}>{evidence.artifact_id}</code> : null}
                    {evidence.cycle_id ? <code title={evidence.cycle_id}>{evidence.cycle_id}</code> : null}
                  </div>
                </article>
              ))}
            </div>
          </div>
        ) : lab.evidence_refs.length > 0 && labQuery ? (
          <div className="inspector-empty">No matching evidence refs</div>
        ) : null}
        <div className="inspector-actions labrun-action-grid">
          {lab.latest_report_path ? (
            <button type="button" onClick={() => onOpenLabReport(lab.latest_report_path!)}>
              <ExternalLink aria-hidden="true" size={14} />
              Open latest
            </button>
          ) : null}
          <button type="button" onClick={() => onStageLabCommand("/lab report")}>
            <FileCode2 aria-hidden="true" size={14} />
            Latest report
          </button>
          <button type="button" onClick={() => onStageLabCommand("/lab report list")}>
            <ListChecks aria-hidden="true" size={14} />
            Report list
          </button>
          <button type="button" onClick={() => onStageLabCommand("/lab review")}>
            <CheckCircle2 aria-hidden="true" size={14} />
            Review state
          </button>
        </div>
        {selectedReportPath ? (
          <div className="labrun-full-report-viewer" aria-label="Full LabRun report viewer">
            <div className="inspector-section-title">
              <FileCode2 aria-hidden="true" size={14} />
              <span>Full report viewer</span>
            </div>
            <code title={selectedReportPath}>{selectedReportPath}</code>
            {reportLoading ? <div className="inspector-empty">Loading report page...</div> : null}
            {reportError ? <div className="inspector-error">{reportError}</div> : null}
            {reportPage ? (
              <>
                <div className="inspector-chip-row">
                  <code>{formatTokens(reportPage.offset)} offset</code>
                  <code>{formatTokens(reportPage.total_bytes)} bytes</code>
                  {reportPage.has_more ? <code>more pages</code> : <code>end</code>}
                </div>
                <pre className="labrun-full-report-pre">{reportPage.content}</pre>
                <div className="inspector-actions labrun-row-actions">
                  <button type="button" onClick={previousReportPage} disabled={reportPage.offset <= 0}>
                    Previous page
                  </button>
                  <button type="button" onClick={nextReportPage} disabled={!reportPage.has_more}>
                    Next page
                  </button>
                  <button type="button" onClick={() => setSelectedReportPath(null)}>
                    Close preview
                  </button>
                </div>
              </>
            ) : null}
          </div>
        ) : null}
      </section>
      <section className="inspector-card labrun-cost-card" aria-label="LabRun cost context and cache">
        <div className="inspector-section-title">
          <Database aria-hidden="true" size={14} />
          <span>Cost, context, and cache</span>
        </div>
        {context ? (
          <>
            <div className="inspector-budget-bar" aria-label="LabRun context usage">
              <span style={{ width: `${contextUsage}%` }} />
            </div>
            <KeyValue label="Context" value={`${formatTokens(context.total_estimated_tokens)} (${context.usage_percent}%)`} />
            <KeyValue label="Window" value={formatTokens(context.max_context_tokens)} />
            <KeyValue label="Cache read" value={formatTokens(context.prompt_cache_cached_tokens)} />
            <KeyValue label="Cache miss" value={formatTokens(context.prompt_cache_miss_tokens)} />
            <KeyValue label="Cache total" value={formatTokens(cacheTotal)} />
            <KeyValue label="Compression" value={`${context.compact.compression_count}`} />
            <KeyValue label="Strategy" value={context.compact.latest_strategy || "none"} />
          </>
        ) : (
          <div className="inspector-empty">No context snapshot available</div>
        )}
        <div className="inspector-note">
          LabRun cumulative provider cost is shown through runtime/API fields when available; the frontend does not estimate missing provider billing fields.
        </div>
      </section>
      {lab.blockers.length > 0 ? (
        <section className="inspector-card">
          <div className="inspector-section-title">
            <AlertCircle aria-hidden="true" size={14} />
            <span>Blockers</span>
          </div>
          <div className="inspector-chip-row">
            {lab.blockers.slice(0, 5).map((blocker) => (
              <code key={blocker} title={blocker}>{blocker}</code>
            ))}
          </div>
        </section>
      ) : null}
    </div>
  );
}

function DiagnosticsInspector({
  diagnostics,
  onRefreshDiagnostics,
}: {
  diagnostics: DesktopDiagnostic[];
  onRefreshDiagnostics: () => void;
}) {
  const blocking = diagnostics.filter((item) => item.status === "error").length;
  const warnings = diagnostics.filter((item) => item.status === "warning").length;

  return (
    <div className="inspector-stack">
      <MetricGrid>
        <InspectorMetric label="Errors" value={`${blocking}`} detail="blocking" />
        <InspectorMetric label="Warnings" value={`${warnings}`} detail="review" />
      </MetricGrid>
      <DiagnosticsPanel diagnostics={diagnostics} onRefresh={onRefreshDiagnostics} />
      {blocking === 0 && warnings === 0 ? (
        <EmptyInspector icon={<CheckCircle2 size={16} />} title="Environment ready" detail="No blocking diagnostics are currently reported." />
      ) : null}
    </div>
  );
}

function matchesLabArtifact(
  artifact: DesktopLabStatusSnapshot["artifacts"][number],
  query: string,
) {
  if (!query) {
    return true;
  }
  return [
    artifact.artifact_id,
    artifact.artifact_type,
    artifact.stage,
    artifact.owner,
    artifact.status,
    artifact.validation_status,
    artifact.title,
    artifact.report_path,
    artifact.report_preview,
    ...artifact.evidence_refs,
  ]
    .filter(Boolean)
    .some((value) => String(value).toLowerCase().includes(query));
}

function matchesLabReport(
  report: DesktopLabStatusSnapshot["reports"][number],
  query: string,
) {
  if (!query) {
    return true;
  }
  return [report.artifact_id, report.path, report.preview]
    .filter(Boolean)
    .some((value) => String(value).toLowerCase().includes(query));
}

function matchesLabEvidence(
  evidence: DesktopLabStatusSnapshot["evidence_refs"][number],
  query: string,
) {
  if (!query) {
    return true;
  }
  return [
    evidence.evidence_id,
    evidence.kind,
    evidence.role,
    evidence.reference,
    evidence.summary,
    evidence.artifact_id,
    evidence.cycle_id,
  ]
    .filter(Boolean)
    .some((value) => String(value).toLowerCase().includes(query));
}

type LabRunTimelineStatus = "done" | "active" | "blocked" | "pending";

function labRunTimelineNodes(lab: DesktopLabStatusSnapshot): Array<{
  id: string;
  label: string;
  detail: string;
  status: LabRunTimelineStatus;
}> {
  const artifactTypes = lab.artifacts.map((artifact) => artifact.artifact_type.toLowerCase());
  const artifactOwners = lab.artifacts.map((artifact) => artifact.owner.toLowerCase());
  const stage = (lab.stage || lab.proposal_status || lab.state || "").toLowerCase();
  const owner = (lab.owner || "").toLowerCase();
  const blocked = lab.task_blocked > 0 || lab.blockers.length > 0 || lab.needs_user;
  const hasPostdocPlan = artifactTypes.some((kind) => kind.includes("postdocplan"));
  const hasGraduateResult = artifactTypes.some((kind) => kind.includes("graduateresult"));
  const hasValidation =
    lab.validation_retry_count > 0 ||
    lab.artifacts.some((artifact) => Boolean(artifact.validation_status));
  const hasPostdocAudit = artifactTypes.some(
    (kind) => kind.includes("audit") || kind.includes("integration"),
  );
  const hasProfessorReview =
    artifactTypes.some((kind) => kind.includes("professorreview")) || Boolean(lab.latest_report_path);

  const statusFor = (step: string, done: boolean): LabRunTimelineStatus => {
    if (blocked && (owner.includes(step) || stage.includes(step))) {
      return "blocked";
    }
    if (owner.includes(step) || stage.includes(step)) {
      return "active";
    }
    return done ? "done" : "pending";
  };

  const nodes: Array<{
    id: string;
    label: string;
    detail: string;
    status: LabRunTimelineStatus;
  }> = [
    {
      id: "professor_intake",
      label: "Professor intake",
      detail: lab.proposal_id ? lab.proposal_status || "proposal tracked" : "proposal not drafted",
      status: statusFor("professor", Boolean(lab.proposal_id)),
    },
    {
      id: "postdoc_plan",
      label: "Postdoc plan",
      detail: hasPostdocPlan ? "plan artifact present" : "waiting for scoped plan",
      status: statusFor("postdoc", hasPostdocPlan),
    },
    {
      id: "graduate_task",
      label: "Graduate task",
      detail: lab.task_total > 0 ? `${Math.max(0, lab.task_total - lab.task_open)}/${lab.task_total} complete` : "no task dispatched",
      status: blocked && owner.includes("graduate") ? "blocked" : statusFor("graduate", hasGraduateResult),
    },
    {
      id: "validation",
      label: "Validation",
      detail: hasValidation ? `${lab.validation_retry_count} retries recorded` : "no validation evidence yet",
      status: blocked && stage.includes("validation") ? "blocked" : hasValidation ? "done" : "pending",
    },
    {
      id: "postdoc_audit",
      label: "Postdoc audit",
      detail: hasPostdocAudit ? "audit or integration artifact present" : "waiting for code-aware audit",
      status: statusFor("audit", hasPostdocAudit),
    },
    {
      id: "professor_review",
      label: "Professor review",
      detail: hasProfessorReview ? "review/report evidence present" : "waiting for final acceptance",
      status: statusFor("review", hasProfessorReview),
    },
  ];
  return nodes.map((node) => {
    if (node.status === "pending" && artifactOwners.some((artifactOwner) => artifactOwner.includes(node.id.split("_")[0]))) {
      return { ...node, status: "active" as LabRunTimelineStatus };
    }
    return node;
  });
}

function contextTokenBreakdown(snapshot: DesktopContextSnapshot) {
  const knownTotal =
    snapshot.history_tokens + snapshot.tool_schema_tokens + snapshot.memory_snapshot_tokens;
  const otherTokens = Math.max(0, snapshot.total_estimated_tokens - knownTotal);
  const denominator = Math.max(1, snapshot.total_estimated_tokens);
  return [
    { label: "History", tokens: snapshot.history_tokens },
    { label: "Tool schemas", tokens: snapshot.tool_schema_tokens },
    { label: "Memory", tokens: snapshot.memory_snapshot_tokens },
    { label: "Other/runtime", tokens: otherTokens },
  ]
    .filter((item) => item.tokens > 0 || item.label !== "Other/runtime")
    .map((item) => ({
      ...item,
      percent: (item.tokens / denominator) * 100,
    }));
}
