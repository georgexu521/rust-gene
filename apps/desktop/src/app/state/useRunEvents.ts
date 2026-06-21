import { Dispatch, RefObject, SetStateAction, useCallback, useEffect, useRef } from "react";
import {
  DesktopRunContext,
  DesktopSettings,
  ProviderModelStatus,
  RecentSession,
  answerPermission,
  onDesktopRunEvent,
  sendMessage,
} from "../../runtime/desktopApi";
import {
  RunViewState,
  appendPermissionAnswer,
  applyRunEvent,
  submitUserMessage,
  withError,
  withRunIdleWarning,
} from "../runEventState";

const RUN_EVENT_IDLE_TIMEOUT_MS = 660_000;

type UseRunEventsOptions = {
  providerStatus: ProviderModelStatus | null;
  refreshContextSnapshot: () => Promise<void>;
  refreshSessions: (options?: { syncSelectedSessionId?: string | null }) => Promise<void>;
  refreshWorkbenchSnapshot: () => Promise<void>;
  runState: RunViewState;
  setRunState: Dispatch<SetStateAction<RunViewState>>;
  setSelectedSessionSummary: Dispatch<SetStateAction<RecentSession | null>>;
  setSettings: Dispatch<SetStateAction<DesktopSettings | null>>;
  settings: DesktopSettings | null;
};

export function useRunEvents({
  providerStatus,
  refreshContextSnapshot,
  refreshSessions,
  refreshWorkbenchSnapshot,
  runState,
  setRunState,
  setSelectedSessionSummary,
  setSettings,
  settings,
}: UseRunEventsOptions) {
  const permissionRecoveryRef = useRef<HTMLDivElement | null>(null);
  const activeRunSessionIdRef = useRef<string | null>(null);
  const runWatchdogRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const latestRef = useRef({
    providerStatus,
    refreshContextSnapshot,
    refreshSessions,
    refreshWorkbenchSnapshot,
    setSelectedSessionSummary,
    setSettings,
    settings,
  });

  latestRef.current = {
    providerStatus,
    refreshContextSnapshot,
    refreshSessions,
    refreshWorkbenchSnapshot,
    setSelectedSessionSummary,
    setSettings,
    settings,
  };

  const clearRunWatchdog = useCallback(() => {
    if (!runWatchdogRef.current) {
      return;
    }
    clearTimeout(runWatchdogRef.current);
    runWatchdogRef.current = null;
  }, []);

  const armRunWatchdog = useCallback(
    (reason: string) => {
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
        void latestRef.current.refreshSessions();
        void latestRef.current.refreshContextSnapshot();
        void latestRef.current.refreshWorkbenchSnapshot();
      }, RUN_EVENT_IDLE_TIMEOUT_MS);
    },
    [clearRunWatchdog, setRunState],
  );

  const handleRunEvent = useCallback(
    (event: Parameters<Parameters<typeof onDesktopRunEvent>[0]>[0]) => {
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
        void latestRef.current.refreshSessions({ syncSelectedSessionId: completedSessionId });
        void latestRef.current.refreshContextSnapshot();
        void latestRef.current.refreshWorkbenchSnapshot();
      }
      if (event.type === "run_started" && event.session_id) {
        activeRunSessionIdRef.current = event.session_id;
        latestRef.current.setSelectedSessionSummary((current) =>
          current?.id === event.session_id
            ? current
            : {
                id: event.session_id || "",
                title: "Current run",
                updated_at: "now",
                model:
                  latestRef.current.providerStatus?.active_model ||
                  latestRef.current.settings?.model ||
                  "active model",
                message_count: 0,
              },
        );
        latestRef.current.setSettings((current) =>
          current ? { ...current, active_session_id: event.session_id || null } : current,
        );
      }
    },
    [armRunWatchdog, clearRunWatchdog, setRunState],
  );

  useEffect(() => {
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
  }, [clearRunWatchdog, handleRunEvent]);

  useEffect(() => {
    if (!runState.pendingPermission) {
      return;
    }
    permissionRecoveryRef.current?.scrollIntoView({
      block: "nearest",
      behavior: "smooth",
    });
  }, [runState.pendingPermission]);

  const submitRuntimeMessage = useCallback(
    async (message: string, contexts: DesktopRunContext[], watchdogReason: string) => {
      setRunState((current) => submitUserMessage(current, message, contexts));
      armRunWatchdog(watchdogReason);

      try {
        await sendMessage(message, contexts);
      } catch (error) {
        clearRunWatchdog();
        setRunState((current) => withError(current, error));
        void latestRef.current.refreshSessions();
      }
    },
    [armRunWatchdog, clearRunWatchdog, setRunState],
  );

  const handlePermission = useCallback(
    async (approved: boolean) => {
      try {
        const answered = await answerPermission(approved);
        setRunState((current) => appendPermissionAnswer(current, approved, answered));
      } catch (error) {
        setRunState((current) => withError(current, error));
      }
    },
    [setRunState],
  );

  return {
    clearRunWatchdog,
    handlePermission,
    permissionRecoveryRef: permissionRecoveryRef as RefObject<HTMLDivElement>,
    submitRuntimeMessage,
  };
}
