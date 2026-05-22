import { FormEvent, useEffect, useState } from "react";
import { Activity, MoreHorizontal, PanelRight } from "lucide-react";
import {
  DesktopDiagnostic,
  DesktopHealth,
  ProviderModelStatus,
  DesktopRunEvent,
  DesktopSettings,
  PermissionModeId,
  PermissionModeOption,
  ProviderSetupInfo,
  RecentSession,
  answerPermission,
  desktopDiagnostics,
  desktopHealth,
  desktopSettings,
  listRecentSessions,
  onDesktopRunEvent,
  openSettingsFolder,
  openShellProfile,
  permissionModeOptions,
  pickProjectDirectory,
  providerModelStatus,
  providerSetupInfo,
  resumeSession,
  selectProject,
  sendMessage,
  setProviderModel,
  setPermissionMode,
} from "../runtime/desktopApi";
import { Composer } from "./components/Composer";
import { DiagnosticsPanel } from "./components/DiagnosticsPanel";
import { PermissionCard } from "./components/PermissionCard";
import { SettingsDrawer } from "./components/SettingsDrawer";
import { Sidebar } from "./components/Sidebar";
import { Transcript } from "./components/Transcript";
import { TraceDrawer } from "./components/TraceDrawer";
import {
  applyRunEvent,
  appendPermissionAnswer,
  initialRunViewState,
  loadSessionTranscript,
  submitUserMessage,
  withError,
} from "./runEventState";

export function App() {
  const [health, setHealth] = useState<DesktopHealth | null>(null);
  const [settings, setSettings] = useState<DesktopSettings | null>(null);
  const [permissionOptions, setPermissionOptions] = useState<PermissionModeOption[]>([]);
  const [providerSetup, setProviderSetup] = useState<ProviderSetupInfo | null>(null);
  const [providerStatus, setProviderStatus] = useState<ProviderModelStatus | null>(null);
  const [projectPath, setProjectPath] = useState("");
  const [sessions, setSessions] = useState<RecentSession[]>([]);
  const [diagnostics, setDiagnostics] = useState<DesktopDiagnostic[]>([]);
  const [composer, setComposer] = useState("");
  const [isTraceOpen, setIsTraceOpen] = useState(false);
  const [activeTraceId, setActiveTraceId] = useState<string | null>(null);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [runState, setRunState] = useState(initialRunViewState);

  useEffect(() => {
    void initialize();

    let cleanup = () => {};
    onDesktopRunEvent(handleRunEvent).then((unlisten) => {
      cleanup = unlisten;
    });

    return () => cleanup();
  }, []);

  async function initialize() {
    try {
      const [
        nextHealth,
        nextSettings,
        nextSessions,
        nextDiagnostics,
        nextProviderSetup,
        nextPermissionOptions,
        nextProviderStatus,
      ] =
        await Promise.all([
          desktopHealth(),
          desktopSettings(),
          listRecentSessions(),
          desktopDiagnostics(),
          providerSetupInfo(),
          permissionModeOptions(),
          providerModelStatus(),
        ]);
      setHealth(nextHealth);
      setSettings(nextSettings);
      setPermissionOptions(nextPermissionOptions);
      setProviderSetup(nextProviderSetup);
      setProviderStatus(nextProviderStatus);
      setProjectPath(nextSettings.selected_project || nextHealth.cwd);
      setSessions(nextSessions);
      setDiagnostics(nextDiagnostics.items);
      if (nextSettings.active_session_id) {
        const resumed = await resumeSession(nextSettings.active_session_id);
        setRunState((current) =>
          loadSessionTranscript(current, resumed.session_id, resumed.messages),
        );
      }
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  async function refreshSessions() {
    try {
      setSessions(await listRecentSessions());
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  async function refreshDiagnostics() {
    try {
      const [nextSettings, nextDiagnostics, nextProviderSetup, nextProviderStatus] = await Promise.all([
        desktopSettings(),
        desktopDiagnostics(),
        providerSetupInfo(),
        providerModelStatus(),
      ]);
      setSettings(nextSettings);
      setDiagnostics(nextDiagnostics.items);
      setProviderSetup(nextProviderSetup);
      setProviderStatus(nextProviderStatus);
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  async function handleProviderModelChange(providerId: string, model: string) {
    try {
      const nextProviderStatus = await setProviderModel(providerId, model);
      setProviderStatus(nextProviderStatus);
      setSettings(await desktopSettings());
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  async function handlePermissionModeChange(mode: PermissionModeId) {
    try {
      setSettings(await setPermissionMode(mode));
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  async function handleSelectProject() {
    try {
      const selected = await selectProject(projectPath);
      setProjectPath(selected.path);
      setSettings((current) =>
        current ? { ...current, selected_project: selected.path, active_session_id: null } : current,
      );
      void refreshDiagnostics();
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  async function handleBrowseProject() {
    try {
      const selectedPath = await pickProjectDirectory();
      if (!selectedPath) {
        return;
      }
      const selected = await selectProject(selectedPath);
      setProjectPath(selected.path);
      setSettings((current) =>
        current ? { ...current, selected_project: selected.path, active_session_id: null } : current,
      );
      void refreshDiagnostics();
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  async function handleLoadSession(session: RecentSession) {
    try {
      const resumed = await resumeSession(session.id);
      setSettings((current) =>
        current ? { ...current, active_session_id: resumed.session_id } : current,
      );
      setRunState((current) =>
        loadSessionTranscript(current, resumed.session_id, resumed.messages),
      );
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const message = composer.trim();
    if (!message || runState.isRunning) {
      return;
    }

    setComposer("");
    setRunState((current) => submitUserMessage(current, message));

    try {
      await sendMessage(message);
    } catch (err) {
      setRunState((current) => withError(current, err));
      void refreshSessions();
    }
  }

  async function handlePermission(approved: boolean) {
    try {
      const answered = await answerPermission(approved);
      setRunState((current) => appendPermissionAnswer(current, approved, answered));
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  function handleRunEvent(event: DesktopRunEvent) {
    setRunState((current) => {
      const result = applyRunEvent(current, event);
      return result.state;
    });

    if (event.type === "run_completed" || event.type === "run_error") {
      void refreshSessions();
    }
    if (event.type === "run_started" && event.session_id) {
      setSettings((current) =>
        current ? { ...current, active_session_id: event.session_id || null } : current,
      );
    }
  }

  return (
    <main className="app-shell">
      <Sidebar
        sessions={sessions}
        selectedSessionId={runState.selectedSessionId}
        onLoadSession={(session) => void handleLoadSession(session)}
        onOpenSettings={() => setIsSettingsOpen(true)}
      />

      <section className="workspace">
        <header className="topbar">
          <div className="title-cluster">
            <div className="topbar-title-row">
              <h1>What should we build in rust-agent?</h1>
              <button
                aria-label="More conversation actions"
                className="title-icon-button"
                type="button"
              >
                <MoreHorizontal aria-hidden="true" size={18} />
              </button>
            </div>
            <div className="eyebrow">Priority Agent</div>
          </div>
          <div className="health">
            <span className="health-status">
              <Activity aria-hidden="true" size={14} />
              {health ? `${health.status} · ${health.version}` : "Starting..."}
            </span>
            <button
              className="trace-toggle"
              type="button"
              onClick={() => setIsTraceOpen((open) => !open)}
            >
              <PanelRight aria-hidden="true" size={15} />
              <span>Trace</span>
            </button>
          </div>
        </header>

        <DiagnosticsPanel
          diagnostics={diagnostics}
          onRefresh={() => void refreshDiagnostics()}
        />

        <Transcript
          items={runState.items}
          onOpenTrace={(traceId) => {
            setActiveTraceId(traceId);
            setIsTraceOpen(true);
          }}
          onPermissionAnswer={(approved) => void handlePermission(approved)}
        />

        <TraceDrawer
          activeItemId={activeTraceId}
          isOpen={isTraceOpen}
          items={runState.traceItems}
          onClose={() => setIsTraceOpen(false)}
        />

        <SettingsDrawer
          isOpen={isSettingsOpen}
          projectPath={projectPath}
          activeSessionId={runState.selectedSessionId}
          settings={settings}
          diagnostics={diagnostics}
          providerSetup={providerSetup}
          permissionOptions={permissionOptions}
          onClose={() => setIsSettingsOpen(false)}
          onRefresh={() => void refreshDiagnostics()}
          onPermissionModeChange={(mode) => void handlePermissionModeChange(mode)}
          onOpenSettingsFolder={() => void openSettingsFolder()}
          onOpenShellProfile={() => void openShellProfile()}
        />

        <PermissionCard
          request={runState.pendingPermission}
          onAnswer={(approved) => void handlePermission(approved)}
        />

        <Composer
          composer={composer}
          projectPath={projectPath}
          providerStatus={providerStatus}
          isRunning={runState.isRunning}
          onComposerChange={setComposer}
          onProjectPathChange={setProjectPath}
          onBrowseProject={() => void handleBrowseProject()}
          onSelectProject={() => void handleSelectProject()}
          onProviderModelChange={(providerId, model) =>
            void handleProviderModelChange(providerId, model)
          }
          onSubmit={handleSubmit}
        />

        {runState.error ? <div className="error-banner">{runState.error}</div> : null}
      </section>
    </main>
  );
}
