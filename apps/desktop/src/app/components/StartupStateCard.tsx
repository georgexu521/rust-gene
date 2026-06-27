import { DesktopSettings, RecentSession } from "../../runtime/desktopApi";

type StartupStateCardProps = {
  dismissedLabRecoveryId: string | null;
  projectPath: string;
  selectedSession: RecentSession | null;
  settings: DesktopSettings | null;
  onDismissLabRecovery: (labRunId: string) => void;
  onOpenLabDashboard: () => void;
  onResumeLab: () => void;
  onExportDiagnostics?: () => void;
  onForceResetRun?: () => void;
};

export function StartupStateCard({
  dismissedLabRecoveryId,
  projectPath,
  selectedSession,
  settings,
  onDismissLabRecovery,
  onOpenLabDashboard,
  onResumeLab,
  onExportDiagnostics,
  onForceResetRun,
}: StartupStateCardProps) {
  if (!settings || settings.startup_state.status === "new_conversation") {
    return null;
  }
  if (
    settings.startup_state.status === "lab_recovery" &&
    settings.startup_state.lab_run_id === dismissedLabRecoveryId
  ) {
    return null;
  }

  return (
    <div className={`startup-state-card ${settings.startup_state.status}`} role="status">
      <span>{startupStateLabel(settings.startup_state.status)}</span>
      <strong>{startupStateDetail(settings, selectedSession, projectPath)}</strong>
      {settings.startup_state.status === "lab_recovery" ? (
        <div className="startup-state-actions" aria-label="Lab recovery actions">
          <button type="button" onClick={onResumeLab}>
            Resume
          </button>
          <button type="button" onClick={onOpenLabDashboard}>
            Dashboard
          </button>
          <button
            type="button"
            onClick={() => onDismissLabRecovery(settings.startup_state.lab_run_id || "dismissed")}
          >
            Keep paused
          </button>
        </div>
      ) : null}
      {settings.startup_state.status === "desktop_run_recovery" ? (
        <div className="startup-state-actions" aria-label="Desktop run recovery actions">
          <button type="button" onClick={onExportDiagnostics}>
            Export diagnostics
          </button>
          <button type="button" onClick={onForceResetRun}>
            Force reset
          </button>
        </div>
      ) : null}
    </div>
  );
}

function startupStateLabel(status: string) {
  if (status === "lab_recovery") {
    return "Lab recovery";
  }
  if (status === "restored_session") {
    return "Restored session";
  }
  if (status === "desktop_run_recovery") {
    return "Interrupted run";
  }
  if (status === "new_conversation") {
    return "New conversation";
  }
  return "Startup state";
}

function startupStateDetail(
  settings: DesktopSettings,
  selectedSession: RecentSession | null,
  projectPath: string,
) {
  if (settings.startup_state.status === "lab_recovery") {
    const lab = settings.startup_state;
    return `Recover ${lab.lab_run_id || "LabRun"} at ${lab.lab_stage || "unknown stage"} with ${lab.lab_owner || "unknown owner"}: ${lab.lab_pause_reason || lab.detail}`;
  }
  if (settings.startup_state.status === "restored_session" && selectedSession) {
    return `Continuing ${selectedSession.title} in ${basename(projectPath)}`;
  }
  if (settings.startup_state.status === "desktop_run_recovery") {
    const run = settings.startup_state.desktop_run;
    if (run) {
      const provider = run.provider_name || "unknown provider";
      const session = run.session_id || "unknown session";
      return `${run.run_id} in ${session} via ${provider}: ${run.status}`;
    }
  }
  return settings.startup_state.detail;
}

function basename(path: string) {
  const segments = path.split("/").filter(Boolean);
  return segments[segments.length - 1] || path;
}
