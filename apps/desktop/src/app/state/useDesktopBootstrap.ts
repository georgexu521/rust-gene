import { useCallback, useState } from "react";
import {
  AgentModeOption,
  DesktopDiagnostic,
  DesktopHealth,
  DesktopSettings,
  PermissionModeOption,
  ProviderModelStatus,
  ProviderSetupInfo,
  RecentSession,
  agentModeOptions,
  desktopDiagnostics,
  desktopHealth,
  desktopSettings,
  listRecentSessions,
  permissionModeOptions,
  providerModelStatus,
  providerSetupInfo,
  searchSessions,
} from "../../runtime/desktopApi";

type UseDesktopBootstrapOptions = {
  onError: (error: unknown) => void;
};

type DesktopBootstrapResult = {
  health: DesktopHealth | null;
  settings: DesktopSettings | null;
  sessions: RecentSession[];
};

export function useDesktopBootstrap({ onError }: UseDesktopBootstrapOptions) {
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

  const loadDesktopBootstrap = useCallback(async (): Promise<DesktopBootstrapResult> => {
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
      onError(startupErrors[0]);
    }

    return {
      health: nextHealth,
      settings: nextSettings,
      sessions: nextSessions,
    };
  }, [onError]);

  const refreshSessions = useCallback(
    async (options: { syncSelectedSessionId?: string | null } = {}) => {
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
      } catch (error) {
        onError(error);
      }
    },
    [onError, sessionSearch],
  );

  const handleSearchChange = useCallback(
    async (query: string) => {
      setSessionSearch(query);
      try {
        setSessions(query.trim() ? await searchSessions(query) : await listRecentSessions());
      } catch (error) {
        onError(error);
      }
    },
    [onError],
  );

  const refreshDiagnostics = useCallback(async () => {
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
    } catch (error) {
      onError(error);
    }
  }, [onError]);

  return {
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
    setHealth,
    setProjectPath,
    setProviderStatus,
    setSelectedSessionSummary,
    setSessions,
    setSettings,
  };
}
