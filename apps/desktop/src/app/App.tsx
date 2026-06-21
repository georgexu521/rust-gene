import { FormEvent, useCallback, useEffect, useState } from "react";
import {
  Activity,
  AlertCircle,
  Database,
  Folder,
  Gauge,
  LayoutDashboard,
  FileText,
  ListChecks,
  Map,
  PanelRight,
  Plus,
  RotateCcw,
  Settings,
  UsersRound,
} from "lucide-react";
import {
  DetailLevelId,
  AgentModeId,
  DesktopRunContext,
  PermissionModeId,
  RecentSession,
  DesktopGoalStatus,
  archiveSession,
  compactContext,
  deleteSession,
  desktopRunContextDetail,
  desktopSettings,
  exportSession,
  openFilePath,
  newConversation,
  openDiagnosticsFolder,
  openSettingsFolder,
  openShellProfile,
  setAgentMode,
  pickProjectDirectory,
  pickProjectFile,
  renameSession,
  restoreArchivedSession,
  revertLastTurn,
  resumeSession,
  selectProject,
  setProviderModel,
  setDetailLevel,
  setPermissionMode,
  superviseLabDaemon,
  goalStatus,
  goalStart,
  goalPause,
  goalResume,
  goalClear,
  goalEdit,
} from "../runtime/desktopApi";
import { Composer } from "./components/Composer";
import { GoalProgressRow } from "./components/GoalProgressRow";
import { CommandPalette, useCommandPalette, type Command } from "./components/CommandPalette";
import { ContextDetailDrawer } from "./components/ContextDetailDrawer";
import { DeleteSessionDialog } from "./components/DeleteSessionDialog";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { ExportNoticeBanner } from "./components/ExportNoticeBanner";
import { type InspectorTab } from "./components/InspectorPanel";
import { JumpBar } from "./components/JumpBar";
import { RuntimeInspectorSurfaces } from "./components/RuntimeInspectorSurfaces";
import { Splash, shouldShowSplash } from "./components/Splash";
import { SessionHeader, type WorkspaceMode } from "./components/SessionHeader";
import { StatusBar } from "./components/StatusBar";
import { StartupStateCard } from "./components/StartupStateCard";
import { PermissionCard } from "./components/PermissionCard";
import { SettingsDrawer } from "./components/SettingsDrawer";
import { Sidebar } from "./components/Sidebar";
import { Transcript } from "./components/Transcript";
import { TraceDrawer } from "./components/TraceDrawer";
import { ToolOutputDrawer } from "./components/ToolOutputDrawer";
import { WorkbenchDrawer } from "./components/WorkbenchDrawer";
import { WorkspaceTopbar } from "./components/WorkspaceTopbar";
import {
  initialRunViewState,
  loadSessionTranscript,
  withError,
} from "./runEventState";
import { useDesktopBootstrap } from "./state/useDesktopBootstrap";
import { useRunEvents } from "./state/useRunEvents";
import { useWorkbenchSnapshots } from "./state/useWorkbenchSnapshots";
import { useTheme } from "./theme";

export function App() {
  const { theme, toggle: toggleTheme } = useTheme();
  const { open: paletteOpen, setOpen: setPaletteOpen } = useCommandPalette();
  const [showSplash, setShowSplash] = useState(() => shouldShowSplash());
  const [runState, setRunState] = useState(initialRunViewState);
  const reportRuntimeError = useCallback((error: unknown) => {
    setRunState((current) => withError(current, error));
  }, []);
  const {
    agentModeOpts,
    diagnostics,
    handleSearchChange,
    health,
    loadDesktopBootstrap,
    permissionOptions,
    projectPath,
    providerSetup,
    providerStatus,
    refreshDiagnostics,
    refreshSessions,
    selectedSessionSummary,
    sessionSearch,
    sessions,
    settings,
    setProjectPath,
    setProviderStatus,
    setSelectedSessionSummary,
    setSessions,
    setSettings,
  } = useDesktopBootstrap({ onError: reportRuntimeError });
  const {
    contextSnapshot,
    refreshContextSnapshot,
    refreshStartupSnapshots,
    refreshWorkbenchSnapshot,
    workbenchSnapshot,
  } = useWorkbenchSnapshots({ onError: reportRuntimeError });
  const [workspaceMode, setWorkspaceMode] = useState<WorkspaceMode>("direct");
  const [activeInspectorTab, setActiveInspectorTab] = useState<InspectorTab>("context");
  const [composer, setComposer] = useState("");
  const [composerFocusRequest, setComposerFocusRequest] = useState(0);
  const [runContexts, setRunContexts] = useState<DesktopRunContext[]>([]);
  const [activeContextDetail, setActiveContextDetail] = useState<DesktopRunContext | null>(null);
  const [isTraceOpen, setIsTraceOpen] = useState(false);
  const [isWorkbenchOpen, setIsWorkbenchOpen] = useState(false);
  const [isInspectorDrawerOpen, setIsInspectorDrawerOpen] = useState(false);
  const [isToolOutputOpen, setIsToolOutputOpen] = useState(false);
  const [isRevertingTurn, setIsRevertingTurn] = useState(false);
  const [activeTraceId, setActiveTraceId] = useState<string | null>(null);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [isEnvironmentOpen, setIsEnvironmentOpen] = useState(false);
  const [exportNotice, setExportNotice] = useState<string | null>(null);
  const [exportPath, setExportPath] = useState<string | null>(null);
  const [lastArchivedSession, setLastArchivedSession] = useState<RecentSession | null>(null);
  const [pendingDeleteSession, setPendingDeleteSession] = useState<RecentSession | null>(null);
  const [currentGoal, setCurrentGoal] = useState<DesktopGoalStatus | null>(null);
  const [dismissedStartupLabRecoveryId, setDismissedStartupLabRecoveryId] = useState<string | null>(null);
  const {
    clearRunWatchdog,
    handlePermission,
    permissionRecoveryRef,
    submitRuntimeMessage,
  } = useRunEvents({
    providerStatus,
    refreshContextSnapshot,
    refreshSessions,
    refreshWorkbenchSnapshot,
    runState,
    setRunState,
    setSelectedSessionSummary,
    setSettings,
    settings,
  });

  useEffect(() => {
    void initialize();
  }, []);

  useEffect(() => {
    const fetchGoal = () => { void goalStatus().then(setCurrentGoal).catch(() => {}); };
    fetchGoal();
    const interval = setInterval(fetchGoal, 5000);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (
        event.key !== "/" ||
        event.metaKey ||
        event.ctrlKey ||
        event.altKey ||
        isEditableTarget(event.target) ||
        paletteOpen ||
        isSettingsOpen ||
        pendingDeleteSession
      ) {
        return;
      }
      event.preventDefault();
      if (!composer.trim()) {
        setComposer("/");
      }
      setComposerFocusRequest((request) => request + 1);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [composer, isSettingsOpen, paletteOpen, pendingDeleteSession]);

  useEffect(() => {
    let busy = false;
    const supervise = async () => {
      if (busy) {
        return;
      }
      busy = true;
      try {
        await handleSuperviseLabDaemon();
      } finally {
        busy = false;
      }
    };
    void supervise();
    const interval = setInterval(() => {
      void supervise();
    }, 120_000);
    return () => clearInterval(interval);
  }, []);

  async function initialize() {
    const { settings: nextSettings, sessions: nextSessions } = await loadDesktopBootstrap();
    await refreshStartupSnapshots();

    const startupSessionExists = Boolean(
      nextSettings?.active_session_id &&
      nextSessions.some((session) => session.id === nextSettings.active_session_id),
    );
    if (nextSettings?.active_session_id && !startupSessionExists) {
      setSettings((current) => current ? { ...current, active_session_id: null } : current);
      setSelectedSessionSummary(null);
    }
    if (nextSettings?.active_session_id && startupSessionExists) {
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

  async function handleSuperviseLabDaemon() {
    try {
      await superviseLabDaemon();
      await refreshWorkbenchSnapshot();
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

  async function handleGoalCommand(command: string) {
    const goalArgs = command.slice("/goal".length).trim();
    if (!goalArgs || goalArgs === "status" || goalArgs === "show") {
      await goalStatus().then(setCurrentGoal);
      return;
    }
    if (goalArgs === "pause") {
      await goalPause();
      await goalStatus().then(setCurrentGoal);
      return;
    }
    if (goalArgs === "resume") {
      const result = await goalResume();
      setCurrentGoal(result.status);
      if (result.next_prompt) {
        await submitRuntimeMessage(result.next_prompt, [], "after goal resume");
      }
      return;
    }
    if (goalArgs === "clear" || goalArgs === "reset") {
      await goalClear();
      await goalStatus().then(setCurrentGoal);
      return;
    }
    if (goalArgs.startsWith("edit ")) {
      const status = await goalEdit(goalArgs.slice("edit ".length).trim());
      setCurrentGoal(status);
      return;
    }

    const result = await goalStart(goalArgs);
    setCurrentGoal(result.status);
    if (result.next_prompt) {
      await submitRuntimeMessage(result.next_prompt, [], "after goal start");
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const message = composer.trim();
    if (!message || runState.isRunning) {
      return;
    }

    const contexts = runContexts;
    setComposer("");
    setRunContexts([]);

    if (message === "/goal" || message.startsWith("/goal ")) {
      try {
        await handleGoalCommand(message);
      } catch (err) {
        setRunState((current) => withError(current, err));
        void refreshSessions();
      }
      return;
    }

    await submitRuntimeMessage(message, contexts, "after submit");
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

  function resetConversationView() {
    clearRunWatchdog();
    setRunState({ ...initialRunViewState, items: [], traceItems: [] });
    setComposer("");
    setRunContexts([]);
    setActiveTraceId(null);
    setIsTraceOpen(false);
  }

  const isEmptyConversation = runState.items.length === 0;
  const conversationTitle =
    selectedSessionSummary?.title || (isEmptyConversation ? "New Chat" : "Priority Agent");

  function stageSlashCommand(command: string) {
    setComposer(command);
  }

  function closePrimaryDrawers(except?: "settings" | "workbench" | "trace" | "output" | "inspector") {
    setIsEnvironmentOpen(false);
    setActiveContextDetail(null);
    if (except !== "settings") {
      setIsSettingsOpen(false);
    }
    if (except !== "workbench") {
      setIsWorkbenchOpen(false);
    }
    if (except !== "trace") {
      setIsTraceOpen(false);
    }
    if (except !== "output") {
      setIsToolOutputOpen(false);
    }
    if (except !== "inspector") {
      setIsInspectorDrawerOpen(false);
    }
  }

  function openSettingsDrawer() {
    closePrimaryDrawers("settings");
    setIsSettingsOpen(true);
  }

  function toggleWorkbenchDrawer() {
    if (isWorkbenchOpen) {
      setIsWorkbenchOpen(false);
      return;
    }
    closePrimaryDrawers("workbench");
    setIsWorkbenchOpen(true);
  }

  function openWorkbenchDrawer() {
    closePrimaryDrawers("workbench");
    setIsWorkbenchOpen(true);
  }

  function toggleTraceDrawer() {
    if (isTraceOpen) {
      setIsTraceOpen(false);
      return;
    }
    closePrimaryDrawers("trace");
    setIsTraceOpen(true);
  }

  function openTraceDrawer(traceId?: string | null) {
    closePrimaryDrawers("trace");
    if (traceId !== undefined) {
      setActiveTraceId(traceId);
    }
    setIsTraceOpen(true);
  }

  function toggleToolOutputDrawer() {
    if (isToolOutputOpen) {
      setIsToolOutputOpen(false);
      return;
    }
    closePrimaryDrawers("output");
    setIsToolOutputOpen(true);
  }

  function openToolOutputDrawer() {
    closePrimaryDrawers("output");
    setIsToolOutputOpen(true);
  }

  function openInspectorTab(tab: InspectorTab) {
    setActiveInspectorTab(tab);
    if (usesDrawerInspectorFallback()) {
      closePrimaryDrawers("inspector");
      setIsInspectorDrawerOpen(true);
    } else {
      setIsInspectorDrawerOpen(false);
    }
  }

  const commands: Command[] = [
    { id: "new-chat", label: "New Chat", icon: <Plus size={14} />, group: "action", run: () => void handleNewChat() },
    { id: "settings", label: "Open Settings", icon: <Settings size={14} />, group: "settings", run: () => openSettingsDrawer() },
    {
      id: "workbench",
      label: "Open Workbench",
      hint: "runtime overview",
      icon: <LayoutDashboard size={14} />,
      group: "nav",
      run: () => openWorkbenchDrawer(),
    },
    {
      id: "trace",
      label: "Open Trace",
      hint: "run events",
      icon: <PanelRight size={14} />,
      group: "nav",
      run: () => openTraceDrawer(),
    },
    {
      id: "output",
      label: "Open Tool Output",
      hint: "stored tool pages",
      icon: <FileText size={14} />,
      group: "nav",
      run: () => openToolOutputDrawer(),
    },
    {
      id: "context-inspector",
      label: "Show Context",
      hint: "tokens and cache",
      icon: <Gauge size={14} />,
      group: "nav",
      run: () => openInspectorTab("context"),
    },
    {
      id: "files-inspector",
      label: "Show Files",
      hint: "project map",
      icon: <Map size={14} />,
      group: "nav",
      run: () => openInspectorTab("files"),
    },
    {
      id: "execution-inspector",
      label: "Show Execution",
      hint: "timeline evidence",
      icon: <ListChecks size={14} />,
      group: "nav",
      run: () => openInspectorTab("execution"),
    },
    {
      id: "subagents-inspector",
      label: "Show Subagents",
      hint: "durable tasks",
      icon: <UsersRound size={14} />,
      group: "nav",
      run: () => openInspectorTab("subagents"),
    },
    {
      id: "labrun-inspector",
      label: "Show LabRun",
      hint: "project loop",
      icon: <LayoutDashboard size={14} />,
      group: "nav",
      run: () => openInspectorTab("labrun"),
    },
    {
      id: "diagnostics-inspector",
      label: "Show Diagnostics",
      hint: "environment checks",
      icon: <Database size={14} />,
      group: "nav",
      run: () => openInspectorTab("diagnostics"),
    },
    { id: "browse-project", label: "Switch Project", icon: <Folder size={14} />, group: "workspace", run: () => void handleBrowseProject() },
    {
      id: "lab-dashboard",
      label: "Lab Dashboard",
      hint: "/lab dashboard",
      icon: <LayoutDashboard size={14} />,
      group: "workspace",
      run: () => stageSlashCommand("/lab dashboard"),
    },
    {
      id: "lab-meeting",
      label: "Lab Meeting",
      hint: "/lab meeting open",
      icon: <FileText size={14} />,
      group: "workspace",
      run: () => stageSlashCommand("/lab meeting open"),
    },
    {
      id: "lab-recovery",
      label: "Lab Recovery",
      hint: "/lab recovery",
      icon: <RotateCcw size={14} />,
      group: "workspace",
      run: () => stageSlashCommand("/lab recovery"),
    },
    {
      id: "lab-daemon-health",
      label: "Lab Daemon Health",
      hint: "/lab daemon health",
      icon: <Activity size={14} />,
      group: "workspace",
      run: () => stageSlashCommand("/lab daemon health"),
    },
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
        onOpenSettings={openSettingsDrawer}
      />

      <section className={`workspace${isEmptyConversation ? " empty-workspace" : ""}`}>
        <WorkspaceTopbar
          contextSnapshot={contextSnapshot}
          conversationTitle={conversationTitle}
          diagnostics={diagnostics}
          health={health}
          isEnvironmentOpen={isEnvironmentOpen}
          isRevertingTurn={isRevertingTurn}
          isRunning={runState.isRunning}
          isToolOutputOpen={isToolOutputOpen}
          isTraceOpen={isTraceOpen}
          isWorkbenchOpen={isWorkbenchOpen}
          paletteOpen={paletteOpen}
          projectPath={projectPath}
          providerStatus={providerStatus}
          selectedSessionId={runState.selectedSessionId}
          settings={settings}
          workbenchSnapshot={workbenchSnapshot}
          onCompactContext={() => void handleCompactContext()}
          onCloseEnvironment={() => setIsEnvironmentOpen(false)}
          onExportSession={() => void handleExportSession()}
          onOpenPalette={() => setPaletteOpen(true)}
          onOpenSettings={openSettingsDrawer}
          onRevertLastTurn={() => void handleRevertLastTurn()}
          onToggleEnvironment={() => setIsEnvironmentOpen((open) => !open)}
          onToggleToolOutput={toggleToolOutputDrawer}
          onToggleTrace={toggleTraceDrawer}
          onToggleWorkbench={toggleWorkbenchDrawer}
        />

        <SessionHeader
          conversationTitle={conversationTitle}
          isRunning={runState.isRunning}
          mode={workspaceMode}
          projectPath={projectPath}
          providerStatus={providerStatus}
          selectedSessionSummary={selectedSessionSummary}
          settings={settings}
          workbenchSnapshot={workbenchSnapshot}
          onModeChange={setWorkspaceMode}
          onOpenLabRun={() => {
            openInspectorTab("labrun");
          }}
        />

        <StartupStateCard
          dismissedLabRecoveryId={dismissedStartupLabRecoveryId}
          projectPath={projectPath}
          selectedSession={selectedSessionSummary}
          settings={settings}
          onDismissLabRecovery={setDismissedStartupLabRecoveryId}
          onOpenLabDashboard={() => {
            stageSlashCommand("/lab dashboard");
            openWorkbenchDrawer();
          }}
          onResumeLab={() => stageSlashCommand("/lab resume")}
        />

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
          onOpenTrace={(traceId) => openTraceDrawer(traceId)}
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
          onOpenLabReport={(path) => {
            openFilePath(path).catch(console.error);
          }}
          onRefreshDiagnostics={() => void refreshDiagnostics()}
          onRefreshWorkbench={() => void refreshWorkbenchSnapshot()}
          onStageLabCommand={stageSlashCommand}
          onSuperviseLabDaemon={() => void handleSuperviseLabDaemon()}
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
          providerStatus={providerStatus}
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

        <GoalProgressRow
          goal={currentGoal}
          onPause={() => { void goalPause().then(() => goalStatus().then(setCurrentGoal)); }}
          onResume={() => {
            void goalResume()
              .then(async (result) => {
                setCurrentGoal(result.status);
                if (result.next_prompt) {
                  await submitRuntimeMessage(result.next_prompt, [], "after goal resume");
                }
              })
              .catch((err) => setRunState((current) => withError(current, err)));
          }}
          onClear={() => { void goalClear().then(() => goalStatus().then(setCurrentGoal)); }}
          onEdit={(obj) => { void goalEdit(obj).then(setCurrentGoal); }}
        />

        <Composer
          composer={composer}
          contexts={runContexts}
          projectPath={projectPath}
          recentProjects={settings?.recent_projects || []}
          providerStatus={providerStatus}
          providerSetup={providerSetup}
          detailLevel={settings?.detail_level}
          permissionMode={settings?.permission_mode}
          agentMode={settings?.agent_mode}
          agentModeOptions={agentModeOpts}
          permissionOptions={permissionOptions}
          isEmptyState={isEmptyConversation}
          isRunning={runState.isRunning}
          focusRequest={composerFocusRequest}
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
          onProviderCredentialSaved={() => void refreshDiagnostics()}
          onSubmit={handleSubmit}
        />

        {runState.error ? (
          <div className="error-banner runtime-alert" role="alert" aria-label="Runtime issue">
            <AlertCircle aria-hidden="true" size={17} />
            <div className="runtime-alert-copy">
              <strong>{runState.isRunning ? "Runtime warning" : "Runtime issue"}</strong>
              <span>{runState.error}</span>
            </div>
            <div className="runtime-alert-actions">
              {runState.traceItems.length > 0 ? (
                <button
                  type="button"
                  onClick={() => {
                    openTraceDrawer(runState.traceItems.at(-1)?.id || null);
                  }}
                >
                  Open trace
                </button>
              ) : null}
              <button type="button" onClick={() => openInspectorTab("diagnostics")}>
                Diagnostics
              </button>
              <button type="button" onClick={() => setRunState((current) => ({ ...current, error: null }))}>
                Dismiss
              </button>
            </div>
          </div>
        ) : null}
        {exportNotice ? (
          <ExportNoticeBanner
            notice={exportNotice}
            path={exportPath}
            onDismiss={() => {
              setExportNotice(null);
              setExportPath(null);
            }}
            onOpen={(path) => {
              openFilePath(path).catch(console.error);
            }}
          />
        ) : null}
      </section>

      {pendingDeleteSession ? (
        <DeleteSessionDialog
          session={pendingDeleteSession}
          onCancel={() => setPendingDeleteSession(null)}
          onConfirm={() => void handleConfirmDeleteSession()}
        />
      ) : null}

      <RuntimeInspectorSurfaces
        activeTab={activeInspectorTab}
        contextSnapshot={contextSnapshot}
        diagnostics={diagnostics}
        isDrawerOpen={isInspectorDrawerOpen}
        isRunning={runState.isRunning}
        latestUsage={runState.latestUsage}
        pendingPermission={Boolean(runState.pendingPermission)}
        sessionId={runState.selectedSessionId}
        snapshot={workbenchSnapshot}
        traceItems={runState.traceItems}
        onCloseDrawer={() => setIsInspectorDrawerOpen(false)}
        onOpenLabReport={(path) => {
          openFilePath(path).catch(console.error);
        }}
        onOpenOutput={openToolOutputDrawer}
        onOpenTrace={() => openTraceDrawer()}
        onRefreshDiagnostics={() => void refreshDiagnostics()}
        onRefreshWorkbench={() => void refreshWorkbenchSnapshot()}
        onStageLabCommand={stageSlashCommand}
        onSuperviseLabDaemon={() => void handleSuperviseLabDaemon()}
        onTabChange={setActiveInspectorTab}
      />

      <StatusBar
        health={health}
        providerStatus={providerStatus}
        contextSnapshot={contextSnapshot}
        projectPath={projectPath}
        isRunning={runState.isRunning}
        onOpenContext={() => openInspectorTab("context")}
        onOpenFiles={() => openInspectorTab("files")}
        onOpenSettings={openSettingsDrawer}
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

function isEditableTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  const tag = target.tagName.toLowerCase();
  return (
    target.isContentEditable ||
    tag === "input" ||
    tag === "textarea" ||
    tag === "select"
  );
}

function usesDrawerInspectorFallback() {
  return typeof window !== "undefined" && window.matchMedia("(max-width: 760px)").matches;
}
