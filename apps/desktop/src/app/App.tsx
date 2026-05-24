import { FormEvent, useEffect, useRef, useState, type ReactNode } from "react";
import { Activity, Folder, Gauge, GitBranch, Globe, Info, MoreHorizontal, PanelRight, Settings } from "lucide-react";
import {
  DesktopDiagnostic,
  DesktopContextSnapshot,
  DiagnosticStatus,
  DetailLevelId,
  DesktopHealth,
  DesktopRunContext,
  ProviderModelStatus,
  DesktopRunEvent,
  DesktopSettings,
  PermissionModeId,
  PermissionModeOption,
  ProviderSetupInfo,
  RecentSession,
  answerPermission,
  archiveSession,
  compactContext,
  deleteSession,
  desktopContextSnapshot,
  desktopDiagnostics,
  desktopHealth,
  desktopRunContextDetail,
  desktopSettings,
  listRecentSessions,
  newConversation,
  onDesktopRunEvent,
  openDiagnosticsFolder,
  openSettingsFolder,
  openShellProfile,
  permissionModeOptions,
  pickProjectDirectory,
  pickProjectFile,
  providerModelStatus,
  providerSetupInfo,
  renameSession,
  restoreArchivedSession,
  resumeSession,
  searchSessions,
  selectProject,
  sendMessage,
  setProviderModel,
  setDetailLevel,
  setPermissionMode,
} from "../runtime/desktopApi";
import { Composer } from "./components/Composer";
import { ContextDetailDrawer } from "./components/ContextDetailDrawer";
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
  const [selectedSessionSummary, setSelectedSessionSummary] = useState<RecentSession | null>(null);
  const [sessionSearch, setSessionSearch] = useState("");
  const [diagnostics, setDiagnostics] = useState<DesktopDiagnostic[]>([]);
  const [contextSnapshot, setContextSnapshot] = useState<DesktopContextSnapshot | null>(null);
  const [composer, setComposer] = useState("");
  const [runContexts, setRunContexts] = useState<DesktopRunContext[]>([]);
  const [activeContextDetail, setActiveContextDetail] = useState<DesktopRunContext | null>(null);
  const [isTraceOpen, setIsTraceOpen] = useState(false);
  const [activeTraceId, setActiveTraceId] = useState<string | null>(null);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [isEnvironmentOpen, setIsEnvironmentOpen] = useState(false);
  const [lastArchivedSession, setLastArchivedSession] = useState<RecentSession | null>(null);
  const [pendingDeleteSession, setPendingDeleteSession] = useState<RecentSession | null>(null);
  const [runState, setRunState] = useState(initialRunViewState);
  const permissionRecoveryRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    void initialize();

    let cleanup = () => {};
    onDesktopRunEvent(handleRunEvent).then((unlisten) => {
      cleanup = unlisten;
    });

    return () => cleanup();
  }, []);

  useEffect(() => {
    if (!runState.pendingPermission) {
      return;
    }
    permissionRecoveryRef.current?.scrollIntoView({
      block: "nearest",
      behavior: "smooth",
    });
  }, [runState.pendingPermission]);

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
        nextContextSnapshot,
      ] =
        await Promise.all([
          desktopHealth(),
          desktopSettings(),
          listRecentSessions(),
          desktopDiagnostics(),
          providerSetupInfo(),
          permissionModeOptions(),
          providerModelStatus(),
          desktopContextSnapshot(),
        ]);
      setHealth(nextHealth);
      setSettings(nextSettings);
      setPermissionOptions(nextPermissionOptions);
      setProviderSetup(nextProviderSetup);
      setProviderStatus(nextProviderStatus);
      setContextSnapshot(nextContextSnapshot);
      setProjectPath(nextSettings.selected_project || nextHealth.cwd);
      setSessions(nextSessions);
      setSelectedSessionSummary(
        nextSessions.find((session) => session.id === nextSettings.active_session_id) || null,
      );
      setDiagnostics(nextDiagnostics.items);
      if (nextSettings.active_session_id) {
        const resumed = await resumeSession(nextSettings.active_session_id);
        setRunState((current) =>
          loadSessionTranscript(
            current,
            resumed.session_id,
            resumed.messages,
            resumed.compact_boundaries,
          ),
        );
        setContextSnapshot(await desktopContextSnapshot());
      }
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  async function refreshSessions() {
    try {
      const query = sessionSearch.trim();
      setSessions(query ? await searchSessions(query) : await listRecentSessions());
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  async function handleSearchChange(query: string) {
    setSessionSearch(query);
    try {
      setSessions(query.trim() ? await searchSessions(query) : await listRecentSessions());
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

  async function refreshContextSnapshot() {
    try {
      setContextSnapshot(await desktopContextSnapshot());
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  async function handleCompactContext() {
    try {
      await compactContext();
      await refreshContextSnapshot();
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

  async function handleDetailLevelChange(level: DetailLevelId) {
    try {
      setSettings(await setDetailLevel(level));
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  async function handleSelectProject() {
    try {
      const selected = await selectProject(projectPath);
      setProjectPath(selected.path);
      setSelectedSessionSummary(null);
      setSettings((current) =>
        current ? { ...current, selected_project: selected.path, active_session_id: null } : current,
      );
      resetConversationView();
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
      setSelectedSessionSummary(null);
      setSettings((current) =>
        current ? { ...current, selected_project: selected.path, active_session_id: null } : current,
      );
      resetConversationView();
      void refreshDiagnostics();
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  async function handleSelectRecentProject(path: string) {
    try {
      const selected = await selectProject(path);
      setProjectPath(selected.path);
      setSelectedSessionSummary(null);
      setSettings((current) =>
        current ? { ...current, selected_project: selected.path, active_session_id: null } : current,
      );
      resetConversationView();
      await refreshDiagnostics();
      await refreshSessions();
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  async function handleLoadSession(session: RecentSession) {
    try {
      const resumed = await resumeSession(session.id);
      setSelectedSessionSummary(session);
      setSettings((current) =>
        current ? { ...current, active_session_id: resumed.session_id } : current,
      );
      setRunState((current) =>
        loadSessionTranscript(
          current,
          resumed.session_id,
          resumed.messages,
          resumed.compact_boundaries,
        ),
      );
      await refreshContextSnapshot();
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  async function handleNewChat() {
    try {
      setSettings(await newConversation());
      setSelectedSessionSummary(null);
      resetConversationView();
      void refreshSessions();
      void refreshDiagnostics();
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  async function handleRenameSession(session: RecentSession, title: string) {
    try {
      const renamed = await renameSession(session.id, title);
      setSelectedSessionSummary((current) =>
        current?.id === renamed.id ? { ...current, ...renamed } : current,
      );
      setSessions((current) =>
        current.map((item) => (item.id === renamed.id ? { ...item, ...renamed } : item)),
      );
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  async function handleArchiveSession(session: RecentSession) {
    try {
      setSettings(await archiveSession(session.id));
      setLastArchivedSession(session);
      if (runState.selectedSessionId === session.id) {
        setSelectedSessionSummary(null);
        resetConversationView();
      } else {
        setRunState((current) => ({ ...current, error: null }));
      }
      await refreshSessions();
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  async function handleRestoreArchivedSession() {
    if (!lastArchivedSession) {
      return;
    }

    try {
      setSettings(await restoreArchivedSession(lastArchivedSession.id));
      setLastArchivedSession(null);
      await refreshSessions();
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  async function handleConfirmDeleteSession() {
    if (!pendingDeleteSession) {
      return;
    }
    const session = pendingDeleteSession;
    try {
      setSettings(await deleteSession(session.id));
      setPendingDeleteSession(null);
      if (lastArchivedSession?.id === session.id) {
        setLastArchivedSession(null);
      }
      if (runState.selectedSessionId === session.id) {
        setSelectedSessionSummary(null);
        resetConversationView();
      }
      await refreshSessions();
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
    setRunContexts([]);
    setRunState((current) => submitUserMessage(current, message, runContexts));

    try {
      await sendMessage(message, runContexts);
    } catch (err) {
      setRunState((current) => withError(current, err));
      void refreshSessions();
    }
  }

  async function handleAddContext(context: DesktopRunContext) {
    try {
      const detail = await desktopRunContextDetail(context);
      const enrichedContext = { ...context, detail };
      setRunContexts((current) =>
        current.some((existing) => existing.type === context.type)
          ? current.map((existing) => (existing.type === context.type ? enrichedContext : existing))
          : [...current, enrichedContext],
      );
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  async function handleAddFileContext() {
    const selectedPath = await pickProjectFile();
    if (!selectedPath) {
      return;
    }

    const label = selectedPath.split(/[\\/]/).filter(Boolean).pop() || "File";
    await handleAddContext({
      type: "file",
      label,
      path: selectedPath,
    });
  }

  function handleRemoveContext(type: DesktopRunContext["type"]) {
    setRunContexts((current) => current.filter((context) => context.type !== type));
    setActiveContextDetail((current) => (current?.type === type ? null : current));
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
      void refreshContextSnapshot();
    }
    if (event.type === "run_started" && event.session_id) {
      setSelectedSessionSummary((current) =>
        current?.id === event.session_id
          ? current
          : {
              id: event.session_id || "",
              title: "Current run",
              updated_at: "now",
              model: providerStatus?.active_model || settings?.model || "active model",
              message_count: 0,
            },
      );
      setSettings((current) =>
        current ? { ...current, active_session_id: event.session_id || null } : current,
      );
    }
  }

  function resetConversationView() {
    setRunState({ ...initialRunViewState, items: [], traceItems: [] });
    setComposer("");
    setActiveTraceId(null);
    setIsTraceOpen(false);
  }

  const isEmptyConversation = runState.items.length === 0;

  return (
    <main className="app-shell">
      <Sidebar
        projectPath={projectPath}
        recentProjects={settings?.recent_projects || []}
        sessions={sessions}
        sessionSearch={sessionSearch}
        selectedSessionId={runState.selectedSessionId}
        selectedSessionSummary={selectedSessionSummary}
        onArchiveSession={(session) => void handleArchiveSession(session)}
        onBrowseProject={() => void handleBrowseProject()}
        onDeleteSession={setPendingDeleteSession}
        onNewChat={() => void handleNewChat()}
        onRenameSession={(session, title) => void handleRenameSession(session, title)}
        onSearchChange={(query) => void handleSearchChange(query)}
        onSelectRecentProject={(path) => void handleSelectRecentProject(path)}
        onLoadSession={(session) => void handleLoadSession(session)}
        onOpenSettings={() => setIsSettingsOpen(true)}
      />

      <section className={`workspace${isEmptyConversation ? " empty-workspace" : ""}`}>
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
            <ContextMeter snapshot={contextSnapshot} onCompact={() => void handleCompactContext()} />
            <span className="health-status">
              <Activity aria-hidden="true" size={14} />
              {health ? `${health.status} · ${health.version}` : "Starting..."}
            </span>
            <button
              aria-expanded={isEnvironmentOpen}
              aria-label="Environment information"
              className="topbar-icon-button"
              type="button"
              onClick={() => setIsEnvironmentOpen((open) => !open)}
            >
              <Info aria-hidden="true" size={16} />
            </button>
            {isEnvironmentOpen ? (
              <EnvironmentPopover
                diagnostics={diagnostics}
                health={health}
                contextSnapshot={contextSnapshot}
                projectPath={projectPath}
                providerStatus={providerStatus}
                settings={settings}
              />
            ) : null}
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

        {settings && settings.startup_state.status !== "new_conversation" ? (
          <div className={`startup-state-card ${settings.startup_state.status}`} role="status">
            <span>{startupStateLabel(settings.startup_state.status)}</span>
            <strong>{startupStateDetail(settings, selectedSessionSummary, projectPath)}</strong>
          </div>
        ) : null}

        {lastArchivedSession ? (
          <div className="session-undo-banner" role="status">
            <span>Archived {lastArchivedSession.title}</span>
            <button type="button" onClick={() => void handleRestoreArchivedSession()}>
              Undo
            </button>
          </div>
        ) : null}

        <Transcript
          diagnostics={diagnostics}
          isRunning={runState.isRunning}
          items={runState.items}
          onOpenContext={setActiveContextDetail}
          onOpenTrace={(traceId) => {
            setActiveTraceId(traceId);
            setIsTraceOpen(true);
          }}
          onPermissionAnswer={(approved) => void handlePermission(approved)}
          projectPath={projectPath}
          providerStatus={providerStatus}
        />

        <TraceDrawer
          activeItemId={activeTraceId}
          contextSnapshot={contextSnapshot}
          isOpen={isTraceOpen}
          items={runState.traceItems}
          onOpenContext={setActiveContextDetail}
          onClose={() => setIsTraceOpen(false)}
        />

        <ContextDetailDrawer
          context={activeContextDetail}
          onClose={() => setActiveContextDetail(null)}
          onRemove={(type) => handleRemoveContext(type)}
        />

        <SettingsDrawer
          isOpen={isSettingsOpen}
          projectPath={projectPath}
          selectedSessionTitle={selectedSessionSummary?.title || null}
          activeSessionId={runState.selectedSessionId}
          settings={settings}
          diagnostics={diagnostics}
          providerSetup={providerSetup}
          permissionOptions={permissionOptions}
          onClose={() => setIsSettingsOpen(false)}
          onSelectRecentProject={(path) => void handleSelectRecentProject(path)}
          onRefresh={() => void refreshDiagnostics()}
          onDetailLevelChange={(level) => void handleDetailLevelChange(level)}
          onPermissionModeChange={(mode) => void handlePermissionModeChange(mode)}
          onOpenDiagnosticsFolder={() => void openDiagnosticsFolder()}
          onOpenSettingsFolder={() => void openSettingsFolder()}
          onOpenShellProfile={() => void openShellProfile()}
        />

        <div className="permission-recovery-slot" ref={permissionRecoveryRef}>
          <PermissionCard
            request={runState.pendingPermission}
            onAnswer={(approved) => void handlePermission(approved)}
          />
        </div>

        <Composer
          composer={composer}
          contexts={runContexts}
          projectPath={projectPath}
          recentProjects={settings?.recent_projects || []}
          providerStatus={providerStatus}
          detailLevel={settings?.detail_level}
          permissionMode={settings?.permission_mode}
          permissionOptions={permissionOptions}
          isEmptyState={isEmptyConversation}
          isRunning={runState.isRunning}
          onComposerChange={setComposer}
          onAddContext={(context) => void handleAddContext(context)}
          onAddFileContext={() => void handleAddFileContext()}
          onOpenContext={setActiveContextDetail}
          onRemoveContext={(type) => handleRemoveContext(type)}
          onProjectPathChange={setProjectPath}
          onBrowseProject={() => void handleBrowseProject()}
          onSelectProject={() => void handleSelectProject()}
          onSelectRecentProject={(path) => void handleSelectRecentProject(path)}
          onDetailLevelChange={(level) => void handleDetailLevelChange(level)}
          onPermissionModeChange={(mode) => void handlePermissionModeChange(mode)}
          onProviderModelChange={(providerId, model) =>
            void handleProviderModelChange(providerId, model)
          }
          onSubmit={handleSubmit}
        />

        {runState.error ? <div className="error-banner">{runState.error}</div> : null}
      </section>

      {pendingDeleteSession ? (
        <div className="confirm-backdrop" role="presentation">
          <section
            aria-labelledby="delete-session-title"
            aria-modal="true"
            className="confirm-dialog"
            role="dialog"
          >
            <div>
              <h2 id="delete-session-title">Delete session?</h2>
              <p>
                {pendingDeleteSession.title} will be removed from this desktop app. This
                cannot be undone.
              </p>
            </div>
            <div className="confirm-dialog-meta">
              <span>{pendingDeleteSession.model}</span>
              <span>{pendingDeleteSession.message_count} messages</span>
            </div>
            <div className="confirm-dialog-actions">
              <button type="button" onClick={() => setPendingDeleteSession(null)}>
                Cancel
              </button>
              <button
                className="danger"
                type="button"
                onClick={() => void handleConfirmDeleteSession()}
              >
                Delete
              </button>
            </div>
          </section>
        </div>
      ) : null}
    </main>
  );
}

function startupStateLabel(status: string) {
  if (status === "restored_session") {
    return "Restored session";
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
  if (settings.startup_state.status === "restored_session" && selectedSession) {
    return `Continuing ${selectedSession.title} in ${basename(projectPath)}`;
  }
  return settings.startup_state.detail;
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
