import { X } from "lucide-react";
import { DesktopDiagnostic, DesktopWorkbenchSnapshot } from "../../runtime/desktopApi";
import { DiagnosticsPanel } from "./DiagnosticsPanel";
import { WorkbenchPanel } from "./WorkbenchPanel";

type WorkbenchDrawerProps = {
  diagnostics: DesktopDiagnostic[];
  isOpen: boolean;
  snapshot: DesktopWorkbenchSnapshot | null;
  onClose: () => void;
  onOpenLabReport: (path: string) => void;
  onRefreshDiagnostics: () => void;
  onRefreshWorkbench: () => void;
  onStageLabCommand: (command: string) => void;
  onSuperviseLabDaemon: () => void;
};

export function WorkbenchDrawer({
  diagnostics,
  isOpen,
  snapshot,
  onClose,
  onOpenLabReport,
  onRefreshDiagnostics,
  onRefreshWorkbench,
  onStageLabCommand,
  onSuperviseLabDaemon,
}: WorkbenchDrawerProps) {
  if (!isOpen) {
    return null;
  }

  return (
    <aside className="workbench-drawer" aria-label="Workbench">
      <div className="workbench-drawer-header">
        <div>
          <div className="trace-eyebrow">Workbench</div>
          <h2>Project intelligence</h2>
        </div>
        <button aria-label="Close workbench" type="button" onClick={onClose}>
          <X aria-hidden="true" size={16} />
        </button>
      </div>

      <div className="workbench-drawer-body">
        <DiagnosticsPanel diagnostics={diagnostics} onRefresh={onRefreshDiagnostics} />
        <WorkbenchPanel
          snapshot={snapshot}
          onOpenLabReport={onOpenLabReport}
          onRefresh={onRefreshWorkbench}
          onStageLabCommand={onStageLabCommand}
          onSuperviseLabDaemon={onSuperviseLabDaemon}
        />
      </div>
    </aside>
  );
}
