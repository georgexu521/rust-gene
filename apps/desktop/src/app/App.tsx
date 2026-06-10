import { FormEvent, useEffect, useRef, useState, type ReactNode } from "react";
import {
  Activity,
  Download,
  Folder,
  Gauge,
  GitBranch,
  Globe,
  Info,
  LayoutDashboard,
  FileText,
  Moon,
  MoreHorizontal,
  PanelRight,
  Plus,
  RotateCcw,
  Settings,
  Sun,
} from "lucide-react";
import {
  DesktopDiagnostic,
  DesktopContextSnapshot,
  DesktopWorkbenchSnapshot,
  DiagnosticStatus,
  DetailLevelId,
  AgentModeId,
  DesktopHealth,
  DesktopRunContext,
  ProviderModelStatus,
  DesktopRunEvent,
  DesktopSettings,
  PermissionModeId,
  PermissionModeOption,
  AgentModeOption,
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
  desktopWorkbenchSnapshot,
  exportSession,
  openFilePath,
  listRecentSessions,
  newConversation,
  onDesktopRunEvent,
  openDiagnosticsFolder,
  openSettingsFolder,
  openShellProfile,
  permissionModeOptions,
  setAgentMode,
  agentModeOptions,
  pickProjectDirectory,
  pickProjectFile,
  providerModelStatus,
  providerSetupInfo,
  renameSession,
  restoreArchivedSession,
  revertLastTurn,
  resumeSession,
  searchSessions,
  selectProject,
  sendMessage,
  setProviderModel,
  setDetailLevel,
  setPermissionMode,
} from "../runtime/desktopApi";
import { Composer } from "./components/Composer";
import { CommandPalette, useCommandPalette, type Command } from "./components/CommandPalette";
import { ContextDetailDrawer } from "./components/ContextDetailDrawer";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { JumpBar } from "./components/JumpBar";
import { Splash, shouldShowSplash } from "./components/Splash";
import { StatusBar } from "./components/StatusBar";
import { PermissionCard } from "./components/PermissionCard";
import { SettingsDrawer } from "./components/SettingsDrawer";
import { Sidebar } from "./components/Sidebar";
import { Transcript } from "./components/Transcript";
import { TraceDrawer } from "./components/TraceDrawer";
import { ToolOutputDrawer } from "./components/ToolOutputDrawer";
import { WorkbenchDrawer } from "./components/WorkbenchDrawer";
import { useTheme } from "./theme";
import {
  applyRunEvent,
  appendPermissionAnswer,
  initialRunViewState,
  loadSessionTranscript,
  submitUserMessage,
  withError,
  withRunIdleWarning,
} from "./runEventState";

const RUN_EVENT_IDLE_TIMEOUT_MS = 660_000;

export function App() {
  const { theme, toggle: toggleTheme } = useTheme();
  const { open: paletteOpen, setOpen: setPaletteOpen } = useCommandPalette();
  const [showSplash, setShowSplash] = useState(() => shouldShowSplash());
  const [health, setHealth] = useState<DesktopHealth | null>(null);
  const [settings, setSettings] = useState<DesktopSettings | null>(null);
  const [permissionOptions, setPermissionOptions] = useState<PermissionModeOption[]>([]);
  const [agentModeOpts, setAgentModeOpts] = useState<AgentModeOption[]>([]);
  const [providerSetup, setProviderSetup] = useState<ProviderSetupInfo | null>(null);
  const [providerStatus, setProviderStatus] = useState<ProviderModelStatus | null>(null);
  const [projectPath, setProjectPath] = useState("");
  const [sessions, setSessions] = useState<RecentSession[]>([]);
  const [selectedSessionSummary, setSelectedSessionSummary] = useState<RecentSession | null>(null);
  const [sessionSearch, setSessionSearch] = useState("");
  const [diagnostics, setDiagnostics] = useState<DesktopDiagnostic[]>([]);
  const [contextSnapshot, setContextSnapshot] = useState<DesktopContextSnapshot | null>(null);
  const [workbenchSnapshot, setWorkbenchSnapshot] = useState<DesktopWorkbenchSnapshot | null>(null);
  const [composer, setComposer] = useState("");
  const [runContexts, setRunContexts] = useState<DesktopRunContext[]>([]);
  const [activeContextDetail, setActiveContextDetail] = useState<DesktopRunContext | null>(null);
  const [isTraceOpen, setIsTraceOpen] = useState(false);
  const [isWorkbenchOpen, setIsWorkbenchOpen] = useState(false);
  const [isToolOutputOpen, setIsToolOutputOpen] = useState(false);
  const [isRevertingTurn, setIsRevertingTurn] = useState(false);
  const [activeTraceId, setActiveTraceId] = useState<string | null>(null);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [isEnvironmentOpen, setIsEnvironmentOpen] = useState(false);
  const [exportNotice, setExportNotice] = useState<string | null>(null);
  const [exportPath, setExportPath] = useState<string | null>(null);
  const [lastArchivedSession, setLastArchivedSession] = useState<RecentSession | null>(null);
  const [pendingDeleteSession, setPendingDeleteSession] = useState<RecentSession | null>(null);
  const [runState, setRunState] = useState(initialRunViewState);
  const permissionRecoveryRef = useRef<HTMLDivElement | null>(null);
  const activeRunSessionIdRef = useRef<string | null>(null);
  const runWatchdogRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    void initialize();

    let disposed = false;
    let cleanup: (() => void) | null = null;
    void onDesktopRunEvent(handleRunEvent).then((unlisten) => {
      if (disposed) {
        unlisten();
        return;
      }
      cleanup = unlisten;
    });

    return () => {
      disposed = true;
      clearRunWatchdog();
      cleanup?.();
    };
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
    const [
      healthResult,
      settingsResult,
      sessionsResult,
      diagnosticsResult,
      providerSetupResult,
      permissionOptionsResult,
      agentModeOptionsResult,
      providerStatusResult,
    ] = await Promise.allSettled([
      desktopHealth(),
      desktopSettings(),
      listRecentSessions(),
      desktopDiagnostics(),
      providerSetupInfo(),
      permissionModeOptions(),
      agentModeOptions(),
      providerModelStatus(),
    ]);

    const startupErrors = [
      healthResult,
      settingsResult,
      sessionsResult,
      diagnosticsResult,
      providerSetupResult,
      permissionOptionsResult,
      agentModeOptionsResult,
      providerStatusResult,
    ]
      .filter((result): result is PromiseRejectedResult => result.status === "rejected")
      .map((result) => result.reason);

    const nextHealth = healthResult.status === "fulfilled" ? healthResult.value : null;
    const nextSettings = settingsResult.status === "fulfilled" ? settingsResult.value : null;
    const nextSessions = sessionsResult.status === "fulfilled" ? sessionsResult.value : [];

    if (nextHealth) {
      setHealth(nextHealth);
    }
    if (nextSettings) {
      setSettings(nextSettings);
    }
    if (permissionOptionsResult.status === "fulfilled") {
      setPermissionOptions(permissionOptionsResult.value);
    }
    if (agentModeOptionsResult.status === "fulfilled") {
      setAgentModeOpts(agentModeOptionsResult.value);
    }
    if (providerSetupResult.status === "fulfilled") {
      setProviderSetup(providerSetupResult.value);
    }
    if (providerStatusResult.status === "fulfilled") {
      setProviderStatus(providerStatusResult.value);
    }
    setSessions(nextSessions);
    if (nextSettings || nextHealth) {
      setProjectPath(nextSettings?.selected_project || nextHealth?.cwd || "");
    }
    if (nextSettings) {
      setSelectedSessionSummary(
        nextSessions.find((session) => session.id === nextSettings.active_session_id) || null,
      );
    }
    if (diagnosticsResult.status === "fulfilled") {
      setDiagnostics(diagnosticsResult.value.items);
    }
    if (startupErrors.length > 0) {
      setRunState((current) => withError(current, startupErrors[0]));
    }

    await refreshStartupSnapshots();

    if (nextSettings?.active_session_id) {
      try {
        const resumed = await resumeSession(nextSettings.active_session_id);
        setRunState((current) =>
          loadSessionTranscript(
            current,
            resumed.session_id,
            resumed.messages,
            resumed.compact_boundaries,
            resumed.session_parts,
          ),
        );
        await refreshContextSnapshot();
      } catch (err) {
        setRunState((current) => withError(current, err));
      }
    }
  }

  async function refreshStartupSnapshots() {
    const [contextResult, workbenchResult] = await Promise.allSettled([
      desktopContextSnapshot(),
      desktopWorkbenchSnapshot(),
    ]);

    if (contextResult.status === "fulfilled") {
      setContextSnapshot(contextResult.value);
    }
    if (workbenchResult.status === "fulfilled") {
      setWorkbenchSnapshot(workbenchResult.value);
      if (workbenchResult.value.runtime_context) {
        setContextSnapshot(workbenchResult.value.runtime_context);
      }
    }

    const snapshotError =
      contextResult.status === "rejected"
        ? contextResult.reason
        : workbenchResult.status === "rejected"
          ? workbenchResult.reason
          : null;
    if (snapshotError) {
      setRunState((current) => withError(current, snapshotError));
    }
  }

  async function refreshSessions(options: { syncSelectedSessionId?: string | null } = {}) {
    try {
      const query = sessionSearch.trim();
      const nextSessions = query ? await searchSessions(query) : await listRecentSessions();
      setSessions(nextSessions);
      if (options.syncSelectedSessionId) {
        const selected = nextSessions.find((session) => session.id === options.syncSelectedSessionId);
        if (selected) {
          setSelectedSessionSummary(selected);
        }
      }
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

  async function refreshWorkbenchSnapshot() {
    try {
      const snapshot = await desktopWorkbenchSnapshot();
      setWorkbenchSnapshot(snapshot);
      if (snapshot.runtime_context) {
        setContextSnapshot(snapshot.runtime_context);
      }
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

  async function handleExportSession() {
    try {
      const result = await exportSession(runState.selectedSessionId, "markdown", "redacted");
      setExportNotice(`Exported ${result.privacy} ${result.format}: ${result.path}`);
      setExportPath(result.path);
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

  async function handleAgentModeChange(mode: AgentModeId) {
    try {
      setSettings(await setAgentMode(mode));
    } catch (err) {
      setRunState((current) => withError(current, err));
    }
  }

  async function handleSelectProject(path = projectPath) {
    try {
      const selected = await selectProject(path);
      setProjectPath(selected.path);
      setSelectedSessionSummary(null);
      setSettings((current) =>
        current ? { ...current, selected_project: selected.path, active_session_id: null } : current,
      );
      resetConversationView();
      void refreshDiagnostics();
      void refreshWorkbenchSnapshot();
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
      void refreshWorkbenchSnapshot();
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
      await refreshWorkbenchSnapshot();
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
          resumed.session_parts,
        ),
      );
      await refreshContextSnapshot();
      await refreshWorkbenchSnapshot();
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
      void refreshWorkbenchSnapshot();
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

  async function handleRevertLastTurn() {
    const sessionId = runState.selectedSessionId;
    if (!sessionId || isRevertingTurn || runState.isRunning) {
      return;
    }

    setIsRevertingTurn(true);
    try {
      await revertLastTurn(sessionId);
      const resumed = await resumeSession(sessionId);
      setRunState((current) =>
        loadSessionTranscript(
          current,
          resumed.session_id,
          resumed.messages,
          resumed.compact_boundaries,
          resumed.session_parts,
        ),
      );
      await Promise.allSettled([
        refreshSessions(),
        refreshDiagnostics(),
        refreshContextSnapshot(),
        refreshWorkbenchSnapshot(),
      ]);
    } catch (err) {
      setRunState((current) => withError(current, err));
    } finally {
      setIsRevertingTurn(false);
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
    armRunWatchdog("after submit");

    try {
      await sendMessage(message, runContexts);
    } catch (err) {
      clearRunWatchdog();
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
    if (event.type === "run_completed" || event.type === "run_error") {
      clearRunWatchdog();
    } else {
      armRunWatchdog(`after ${event.type}`);
    }

    setRunState((current) => {
      const result = applyRunEvent(current, event);
      return result.state;
    });

    if (event.type === "run_completed" || event.type === "run_error") {
      const completedSessionId = activeRunSessionIdRef.current;
      activeRunSessionIdRef.current = null;
      void refreshSessions({ syncSelectedSessionId: completedSessionId });
      void refreshContextSnapshot();
      void refreshWorkbenchSnapshot();
    }
    if (event.type === "run_started" && event.session_id) {
      activeRunSessionIdRef.current = event.session_id;
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
    clearRunWatchdog();
    setRunState({ ...initialRunViewState, items: [], traceItems: [] });
    setComposer("");
    setRunContexts([]);
    setActiveTraceId(null);
    setIsTraceOpen(false);
  }

  function armRunWatchdog(reason: string) {
    clearRunWatchdog();
    runWatchdogRef.current = setTimeout(() => {
      runWatchdogRef.current = null;
      setRunState((current) =>
        current.isRunning
          ? withRunIdleWarning(
              current,
              `Desktop runtime stopped sending events for ${Math.round(
                RUN_EVENT_IDLE_TIMEOUT_MS / 1000,
              )} seconds (${reason}). The provider request may still be active; check diagnostics or retry.`,
            )
          : current,
      );
      void refreshSessions();
      void refreshContextSnapshot();
      void refreshWorkbenchSnapshot();
    }, RUN_EVENT_IDLE_TIMEOUT_MS);
  }

  function clearRunWatchdog() {
    if (!runWatchdogRef.current) {
      return;
    }
    clearTimeout(runWatchdogRef.current);
    runWatchdogRef.current = null;
  }

  const isEmptyConversation = runState.items.length === 0;
  const workbenchBadge = workbenchStatusBadge(diagnostics, workbenchSnapshot);
  const conversationTitle =
    selectedSessionSummary?.title || (isEmptyConversation ? "New Chat" : "Priority Agent");

  const commands: Command[] = [
    { id: "new-chat", label: "New Chat", icon: <Plus size={14} />, group: "action", run: () => void handleNewChat() },
    { id: "settings", label: "Open Settings", icon: <Settings size={14} />, group: "settings", run: () => setIsSettingsOpen(true) },
    { id: "browse-project", label: "Switch Project", icon: <Folder size={14} />, group: "workspace", run: () => void handleBrowseProject() },
  ];

  if (showSplash) {
    return <Splash onDone={() => setShowSplash(false)} />;
  }

  return (
    <ErrorBoundary label="App">
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
              <h1>{conversationTitle}</h1>
              <button
                aria-label="Export current session"
                className="title-icon-button"
                type="button"
                disabled={!runState.selectedSessionId}
                onClick={() => void handleExportSession()}
              >
                <Download aria-hidden="true" size={17} />
              </button>
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
              aria-expanded={isWorkbenchOpen}
              className={`trace-toggle workbench-toggle${workbenchBadge.tone ? ` ${workbenchBadge.tone}` : ""}`}
              type="button"
              onClick={() => {
                setIsEnvironmentOpen(false);
                setIsWorkbenchOpen((open) => !open);
              }}
            >
              <LayoutDashboard aria-hidden="true" size={15} />
              <span>Workbench</span>
              <small>{workbenchBadge.label}</small>
            </button>
            <button
              className="trace-toggle"
              type="button"
              disabled={!runState.selectedSessionId || runState.isRunning || isRevertingTurn}
              onClick={() => void handleRevertLastTurn()}
            >
              <RotateCcw aria-hidden="true" size={15} />
              <span>{isRevertingTurn ? "Reverting" : "Revert"}</span>
            </button>
            <button
              className="trace-toggle"
              type="button"
              aria-expanded={isToolOutputOpen}
              onClick={() => setIsToolOutputOpen((open) => !open)}
            >
              <FileText aria-hidden="true" size={15} />
              <span>Output</span>
            </button>
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
        <JumpBar items={runState.items} />

        <TraceDrawer
          activeItemId={activeTraceId}
          contextSnapshot={contextSnapshot}
          isOpen={isTraceOpen}
          items={runState.traceItems}
          onOpenContext={setActiveContextDetail}
          onClose={() => setIsTraceOpen(false)}
        />

        <ToolOutputDrawer
          isOpen={isToolOutputOpen}
          sessionId={runState.selectedSessionId}
          onClose={() => setIsToolOutputOpen(false)}
        />

        <WorkbenchDrawer
          diagnostics={diagnostics}
          isOpen={isWorkbenchOpen}
          snapshot={workbenchSnapshot}
          onClose={() => setIsWorkbenchOpen(false)}
          onRefreshDiagnostics={() => void refreshDiagnostics()}
          onRefreshWorkbench={() => void refreshWorkbenchSnapshot()}
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
          agentMode={settings?.agent_mode}
          agentModeOptions={agentModeOpts}
          permissionOptions={permissionOptions}
          isEmptyState={isEmptyConversation}
          isRunning={runState.isRunning}
          onComposerChange={setComposer}
          onAddContext={(context) => void handleAddContext(context)}
          onAddFileContext={() => void handleAddFileContext()}
          onOpenContext={setActiveContextDetail}
          onRemoveContext={(type) => handleRemoveContext(type)}
          onBrowseProject={() => void handleBrowseProject()}
          onSelectProject={(path) => void handleSelectProject(path)}
          onSelectRecentProject={(path) => void handleSelectRecentProject(path)}
          onDetailLevelChange={(level) => void handleDetailLevelChange(level)}
          onPermissionModeChange={(mode) => void handlePermissionModeChange(mode)}
          onAgentModeChange={(mode) => void handleAgentModeChange(mode)}
          onProviderModelChange={(providerId, model) =>
            void handleProviderModelChange(providerId, model)
          }
          onSubmit={handleSubmit}
        />

        {runState.error ? <div className="error-banner">{runState.error}</div> : null}
        {exportNotice ? (
          <div className="export-banner">
            <span>{exportNotice}</span>
            {exportPath ? (
              <button
                type="button"
                onClick={() => {
                  openFilePath(exportPath).catch(console.error);
                }}
                style={{ marginLeft: "0.75rem", fontSize: "0.8rem" }}
              >
                Open folder
              </button>
            ) : null}
          </div>
        ) : null}
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

      <StatusBar
        health={health}
        providerStatus={providerStatus}
        contextSnapshot={contextSnapshot}
        projectPath={projectPath}
        isRunning={runState.isRunning}
      />
    </main>
      <CommandPalette
        open={paletteOpen}
        onClose={() => setPaletteOpen(false)}
        commands={commands}
      />
    </ErrorBoundary>
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
