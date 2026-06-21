import { useEffect, useRef, type KeyboardEvent as ReactKeyboardEvent, type ReactNode } from "react";
import {
  Activity,
  Download,
  FileText,
  Folder,
  Gauge,
  GitBranch,
  Globe,
  Info,
  LayoutDashboard,
  MoreHorizontal,
  PanelRight,
  RotateCcw,
  Settings,
} from "lucide-react";
import {
  DesktopContextSnapshot,
  DesktopDiagnostic,
  DesktopHealth,
  DesktopSettings,
  DesktopWorkbenchSnapshot,
  DiagnosticStatus,
  ProviderModelStatus,
} from "../../runtime/desktopApi";

type WorkspaceTopbarProps = {
  contextSnapshot: DesktopContextSnapshot | null;
  conversationTitle: string;
  diagnostics: DesktopDiagnostic[];
  health: DesktopHealth | null;
  isEnvironmentOpen: boolean;
  isRevertingTurn: boolean;
  isRunning: boolean;
  isToolOutputOpen: boolean;
  isTraceOpen: boolean;
  isWorkbenchOpen: boolean;
  paletteOpen: boolean;
  projectPath: string;
  providerStatus: ProviderModelStatus | null;
  selectedSessionId: string | null;
  settings: DesktopSettings | null;
  workbenchSnapshot: DesktopWorkbenchSnapshot | null;
  onCompactContext: () => void;
  onCloseEnvironment: () => void;
  onExportSession: () => void;
  onOpenPalette: () => void;
  onOpenSettings: () => void;
  onRevertLastTurn: () => void;
  onToggleEnvironment: () => void;
  onToggleToolOutput: () => void;
  onToggleTrace: () => void;
  onToggleWorkbench: () => void;
};

export function WorkspaceTopbar({
  contextSnapshot,
  conversationTitle,
  diagnostics,
  health,
  isEnvironmentOpen,
  isRevertingTurn,
  isRunning,
  isToolOutputOpen,
  isTraceOpen,
  isWorkbenchOpen,
  paletteOpen,
  projectPath,
  providerStatus,
  selectedSessionId,
  settings,
  workbenchSnapshot,
  onCompactContext,
  onCloseEnvironment,
  onExportSession,
  onOpenPalette,
  onOpenSettings,
  onRevertLastTurn,
  onToggleEnvironment,
  onToggleToolOutput,
  onToggleTrace,
  onToggleWorkbench,
}: WorkspaceTopbarProps) {
  const workbenchBadge = workbenchStatusBadge(diagnostics, workbenchSnapshot);
  const environmentRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!isEnvironmentOpen) {
      return;
    }

    const onPointerDown = (event: PointerEvent) => {
      if (environmentRef.current?.contains(event.target as Node)) {
        return;
      }
      onCloseEnvironment();
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      event.preventDefault();
      onCloseEnvironment();
    };

    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [isEnvironmentOpen, onCloseEnvironment]);

  function closeEnvironmentOnEscape(event: ReactKeyboardEvent) {
    if (!isEnvironmentOpen || event.key !== "Escape") {
      return;
    }
    event.preventDefault();
    onCloseEnvironment();
  }

  return (
    <header className="topbar">
      <div className="title-cluster">
        <div className="topbar-title-row">
          <h1>{conversationTitle}</h1>
          <button
            aria-label="Export current session"
            className="title-icon-button"
            disabled={!selectedSessionId}
            type="button"
            onClick={onExportSession}
          >
            <Download aria-hidden="true" size={17} />
          </button>
          <button
            aria-expanded={paletteOpen}
            aria-label="More conversation actions"
            className="title-icon-button"
            type="button"
            onClick={onOpenPalette}
          >
            <MoreHorizontal aria-hidden="true" size={18} />
          </button>
        </div>
        <div className="eyebrow">Priority Agent</div>
      </div>
      <div className="health" ref={environmentRef} onKeyDown={closeEnvironmentOnEscape}>
        <ContextMeter snapshot={contextSnapshot} onCompact={onCompactContext} />
        <span className="health-status">
          <Activity aria-hidden="true" size={14} />
          {health ? `${health.status} · ${health.version}` : "Starting..."}
        </span>
        <button
          aria-expanded={isEnvironmentOpen}
          aria-label="Environment information"
          className="topbar-icon-button"
          type="button"
          onClick={onToggleEnvironment}
        >
          <Info aria-hidden="true" size={16} />
        </button>
        {isEnvironmentOpen ? (
          <EnvironmentPopover
            contextSnapshot={contextSnapshot}
            diagnostics={diagnostics}
            health={health}
            projectPath={projectPath}
            providerStatus={providerStatus}
            settings={settings}
          />
        ) : null}
        <button
          aria-label="Open settings"
          className="topbar-icon-button"
          type="button"
          onClick={onOpenSettings}
        >
          <Settings aria-hidden="true" size={16} />
        </button>
        <button
          aria-expanded={isWorkbenchOpen}
          className={`trace-toggle workbench-toggle${workbenchBadge.tone ? ` ${workbenchBadge.tone}` : ""}`}
          type="button"
          onClick={onToggleWorkbench}
        >
          <LayoutDashboard aria-hidden="true" size={15} />
          <span>Workbench</span>
          <small>{workbenchBadge.label}</small>
        </button>
        <button
          className="trace-toggle"
          disabled={!selectedSessionId || isRunning || isRevertingTurn}
          type="button"
          onClick={onRevertLastTurn}
        >
          <RotateCcw aria-hidden="true" size={15} />
          <span>{isRevertingTurn ? "Reverting" : "Revert"}</span>
        </button>
        <button
          aria-expanded={isToolOutputOpen}
          className="trace-toggle"
          type="button"
          onClick={onToggleToolOutput}
        >
          <FileText aria-hidden="true" size={15} />
          <span>Output</span>
        </button>
        <button
          aria-expanded={isTraceOpen}
          className="trace-toggle"
          type="button"
          onClick={onToggleTrace}
        >
          <PanelRight aria-hidden="true" size={15} />
          <span>Trace</span>
        </button>
      </div>
    </header>
  );
}

function workbenchStatusBadge(
  diagnostics: DesktopDiagnostic[],
  snapshot: DesktopWorkbenchSnapshot | null,
) {
  const blocking = diagnostics.filter((item) => item.status === "error").length;
  if (blocking > 0) {
    return {
      label: `${blocking} issue${blocking === 1 ? "" : "s"}`,
      tone: "error",
    };
  }

  const symbols = snapshot?.symbol_index?.total_symbols;
  if (symbols && symbols > 0) {
    return {
      label: `${symbols} symbols`,
      tone: "",
    };
  }

  return {
    label: "Ready",
    tone: "",
  };
}

function basename(path: string) {
  return path.split(/[\\/]/).filter(Boolean).at(-1) || path;
}

function formatTokenCount(tokens: number) {
  if (tokens >= 1000) {
    return `${Math.round(tokens / 100) / 10}k`;
  }
  return `${tokens}`;
}

function ContextMeter({
  snapshot,
  onCompact,
}: {
  snapshot: DesktopContextSnapshot | null;
  onCompact: () => void;
}) {
  const percent = Math.min(100, Math.max(0, snapshot?.usage_percent || 0));
  const decision = snapshot?.compact.latest_attempt_decision;
  const label = snapshot ? `Context ${percent}%` : "Context";
  const detail = snapshot
    ? `${formatTokenCount(snapshot.total_estimated_tokens)} / ${formatTokenCount(snapshot.max_context_tokens)}`
    : "Checking";

  return (
    <button
      aria-label="Compact conversation context"
      className={`context-meter${snapshot?.compact.circuit_open ? " warning" : ""}`}
      title={decision ? `Last compact: ${decision}` : "Compact conversation context"}
      type="button"
      onClick={onCompact}
    >
      <Gauge aria-hidden="true" size={14} />
      <span>{label}</span>
      <small>{detail}</small>
    </button>
  );
}

function EnvironmentPopover({
  contextSnapshot,
  diagnostics,
  health,
  projectPath,
  providerStatus,
  settings,
}: {
  contextSnapshot: DesktopContextSnapshot | null;
  diagnostics: DesktopDiagnostic[];
  health: DesktopHealth | null;
  projectPath: string;
  providerStatus: ProviderModelStatus | null;
  settings: DesktopSettings | null;
}) {
  const providerLabel = providerStatus?.active_provider || settings?.provider_name || "Not configured";
  const modelLabel = providerStatus?.active_model || settings?.model || "Checking model";
  const blockingDiagnostics = diagnostics.filter((item) => item.status === "error").length;

  return (
    <aside className="environment-popover" aria-label="Environment details">
      <div className="environment-popover-header">
        <span>Environment</span>
        <Settings aria-hidden="true" size={15} />
      </div>

      <div className="environment-section">
        <EnvironmentRow
          detail={projectPath}
          icon={<Folder aria-hidden="true" size={15} />}
          label={basename(projectPath) || "Project"}
        />
        <EnvironmentRow
          detail={modelLabel}
          icon={<GitBranch aria-hidden="true" size={15} />}
          label={providerLabel}
        />
        <EnvironmentRow
          detail={health ? `${health.status} ${health.version}` : "Starting"}
          icon={<Activity aria-hidden="true" size={15} />}
          label="Runtime"
        />
        <EnvironmentRow
          detail={
            contextSnapshot
              ? `${contextSnapshot.usage_percent}% · ${contextSnapshot.history_messages} messages`
              : "Checking"
          }
          icon={<Gauge aria-hidden="true" size={15} />}
          label="Context"
        />
      </div>

      <div className="environment-section">
        <EnvironmentRow
          detail={settings?.permission_mode || "Not loaded"}
          icon={<Info aria-hidden="true" size={15} />}
          label="Permission mode"
        />
        <EnvironmentRow
          detail={
            blockingDiagnostics > 0
              ? `${blockingDiagnostics} issue${blockingDiagnostics === 1 ? "" : "s"}`
              : "Ready"
          }
          icon={<Globe aria-hidden="true" size={15} />}
          label="Diagnostics"
        />
      </div>

      <div className="environment-section">
        <div className="environment-section-title">Sources</div>
        {diagnostics.slice(0, 3).map((item) => (
          <EnvironmentRow
            detail={item.detail}
            icon={<Info aria-hidden="true" size={15} />}
            key={item.id}
            label={item.label}
            tone={item.status}
          />
        ))}
      </div>
    </aside>
  );
}

function EnvironmentRow({
  detail,
  icon,
  label,
  tone,
}: {
  detail: string;
  icon: ReactNode;
  label: string;
  tone?: DiagnosticStatus;
}) {
  return (
    <div className={`environment-row${tone ? ` ${tone}` : ""}`}>
      {icon}
      <span>{label}</span>
      <small title={detail}>{detail}</small>
    </div>
  );
}
