import { X } from "lucide-react";
import { DesktopDiagnostic, DesktopWorkbenchSnapshot } from "../../runtime/desktopApi";
import { DiagnosticsPanel } from "./DiagnosticsPanel";
import { WorkbenchPanel } from "./WorkbenchPanel";

type WorkbenchDrawerProps = {
  diagnostics: DesktopDiagnostic[];
  isOpen: boolean;
  snapshot: DesktopWorkbenchSnapshot | null;
  onClose: () => void;
  onRefreshDiagnostics: () => void;
  onRefreshWorkbench: () => void;
};

export function WorkbenchDrawer({
  diagnostics,
  isOpen,
  snapshot,
  onClose,
  onRefreshDiagnostics,
  onRefreshWorkbench,
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
        <WorkbenchPanel snapshot={snapshot} onRefresh={onRefreshWorkbench} />
      </div>
    </aside>
  );
}
