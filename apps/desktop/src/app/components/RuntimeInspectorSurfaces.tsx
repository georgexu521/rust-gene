import {
  DesktopContextSnapshot,
  DesktopDiagnostic,
  DesktopWorkbenchSnapshot,
} from "../../runtime/desktopApi";
import { type ProviderUsageSnapshot, TraceItem } from "../types";
import { InspectorPanel, type InspectorTab } from "./InspectorPanel";

type RuntimeInspectorSurfacesProps = {
  activeTab: InspectorTab;
  contextSnapshot: DesktopContextSnapshot | null;
  diagnostics: DesktopDiagnostic[];
  isDrawerOpen: boolean;
  isRunning: boolean;
  latestUsage: ProviderUsageSnapshot | null;
  pendingPermission: boolean;
  sessionId: string | null;
  snapshot: DesktopWorkbenchSnapshot | null;
  traceItems: TraceItem[];
  onCloseDrawer: () => void;
  onOpenLabReport: (path: string) => void;
  onOpenOutput: () => void;
  onOpenTrace: () => void;
  onRefreshDiagnostics: () => void;
  onRefreshWorkbench: () => void;
  onStageLabCommand: (command: string) => void;
  onSuperviseLabDaemon: () => void;
  onTabChange: (tab: InspectorTab) => void;
};

export function RuntimeInspectorSurfaces({
  activeTab,
  contextSnapshot,
  diagnostics,
  isDrawerOpen,
  isRunning,
  latestUsage,
  pendingPermission,
  sessionId,
  snapshot,
  traceItems,
  onCloseDrawer,
  onOpenLabReport,
  onOpenOutput,
  onOpenTrace,
  onRefreshDiagnostics,
  onRefreshWorkbench,
  onStageLabCommand,
  onSuperviseLabDaemon,
  onTabChange,
}: RuntimeInspectorSurfacesProps) {
  const sharedProps = {
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
  };

  return (
    <>
      <InspectorPanel {...sharedProps} />
      {isDrawerOpen ? (
        <InspectorPanel
          {...sharedProps}
          idPrefix="inspector-drawer"
          isDrawer
          onClose={onCloseDrawer}
        />
      ) : null}
    </>
  );
}
