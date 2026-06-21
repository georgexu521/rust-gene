import {
  Activity,
  Beaker,
  Bot,
  CircleDot,
  Folder,
  GitBranch,
  Server,
} from "lucide-react";
import {
  DesktopSettings,
  DesktopWorkbenchSnapshot,
  ProviderModelStatus,
  RecentSession,
} from "../../runtime/desktopApi";

export type WorkspaceMode = "direct" | "labrun";

type SessionHeaderProps = {
  mode: WorkspaceMode;
  conversationTitle: string;
  isRunning: boolean;
  projectPath: string;
  providerStatus: ProviderModelStatus | null;
  selectedSessionSummary: RecentSession | null;
  settings: DesktopSettings | null;
  workbenchSnapshot: DesktopWorkbenchSnapshot | null;
  onModeChange: (mode: WorkspaceMode) => void;
  onOpenLabRun: () => void;
};

export function SessionHeader({
  mode,
  conversationTitle,
  isRunning,
  projectPath,
  providerStatus,
  selectedSessionSummary,
  settings,
  workbenchSnapshot,
  onModeChange,
  onOpenLabRun,
}: SessionHeaderProps) {
  const projectName = basename(projectPath) || "No project";
  const providerLabel = providerStatus?.active_provider || "provider";
  const modelLabel = providerStatus?.active_model || "model";
  const lab = workbenchSnapshot?.lab_status;
  const labActive = Boolean(lab?.lab_run_id || lab?.proposal_id || lab?.available);
  const labState = lab?.run_status || lab?.proposal_status || lab?.state || "not started";
  const runLabel = isRunning ? "running" : "idle";
  const sessionLabel = selectedSessionSummary
    ? `${selectedSessionSummary.message_count} messages`
    : settings?.active_session_id
      ? "session restored"
      : "new session";

  return (
    <section className="session-header" aria-label="Session header">
      <div className="session-header-main">
        <div className="session-header-title-row">
          <div className="session-header-title" title={conversationTitle}>{conversationTitle}</div>
          <span className={`session-run-pill ${isRunning ? "running" : ""}`}>
            <CircleDot aria-hidden="true" size={12} />
            {runLabel}
          </span>
        </div>
        <div className="session-header-meta" aria-label="Session metadata">
          <span title={projectPath}>
            <Folder aria-hidden="true" size={13} />
            {projectName}
          </span>
          <span title={`${providerLabel}/${modelLabel}`}>
            <Server aria-hidden="true" size={13} />
            {providerLabel} / {modelLabel}
          </span>
          <span>
            <GitBranch aria-hidden="true" size={13} />
            {sessionLabel}
          </span>
          <span title={lab?.detail || labState}>
            <Activity aria-hidden="true" size={13} />
            Lab {labState}
          </span>
        </div>
      </div>

      <div className="mode-switcher" aria-label="Agent workspace mode">
        <button
          aria-pressed={mode === "direct"}
          className={mode === "direct" ? "active" : ""}
          type="button"
          onClick={() => onModeChange("direct")}
        >
          <Bot aria-hidden="true" size={15} />
          <span>Direct Agent</span>
        </button>
        <button
          aria-pressed={mode === "labrun"}
          className={mode === "labrun" ? "active" : ""}
          type="button"
          onClick={() => {
            onModeChange("labrun");
            onOpenLabRun();
          }}
        >
          <Beaker aria-hidden="true" size={15} />
          <span>LabRun</span>
          {labActive ? <small>{labState}</small> : null}
        </button>
      </div>
    </section>
  );
}

function basename(path: string) {
  const segments = path.split("/").filter(Boolean);
  return segments[segments.length - 1] || path;
}
