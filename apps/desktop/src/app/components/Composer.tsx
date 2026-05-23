import { FormEvent, useEffect, useRef, useState } from "react";
import {
  ArrowUp,
  Check,
  ChevronDown,
  FolderOpen,
  GitBranch,
  Laptop,
  Plus,
  RotateCcw,
} from "lucide-react";
import {
  DetailLevelId,
  PermissionModeId,
  PermissionModeOption,
  ProviderModelStatus,
} from "../../runtime/desktopApi";

type ComposerProps = {
  composer: string;
  projectPath: string;
  providerStatus: ProviderModelStatus | null;
  detailLevel?: DetailLevelId | null;
  permissionMode?: PermissionModeId | null;
  permissionOptions: PermissionModeOption[];
  isEmptyState?: boolean;
  isRunning: boolean;
  onComposerChange: (value: string) => void;
  onProjectPathChange: (value: string) => void;
  onBrowseProject: () => void;
  onSelectProject: () => void;
  onDetailLevelChange: (level: DetailLevelId) => void;
  onPermissionModeChange: (mode: PermissionModeId) => void;
  onProviderModelChange: (providerId: string, model: string) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
};

export function Composer({
  composer,
  projectPath,
  providerStatus,
  detailLevel,
  permissionMode,
  permissionOptions,
  isEmptyState = false,
  isRunning,
  onComposerChange,
  onProjectPathChange,
  onBrowseProject,
  onSelectProject,
  onDetailLevelChange,
  onPermissionModeChange,
  onProviderModelChange,
  onSubmit,
}: ComposerProps) {
  const composerRef = useRef<HTMLFormElement | null>(null);
  const [openMenu, setOpenMenu] = useState<"project" | "mode" | "provider" | null>(null);
  const activeProvider = providerStatus?.active_provider || "";
  const activeModel = providerStatus?.active_model || "";
  const projectSegments = projectPath.split("/").filter(Boolean);
  const activeProviderLabel =
    providerStatus?.providers.find((provider) => provider.id === activeProvider)?.label ||
    activeProvider ||
    "No provider";
  const activeModelLabel =
    providerStatus?.models.find((model) => model.id === activeModel)?.label ||
    activeModel ||
    "No model";
  const projectName = projectSegments[projectSegments.length - 1] || projectPath || "Project";
  const modeLabel = detailLevel === "daily" ? "Daily work" : "Coding";
  const permissionLabel = formatPermissionMode(permissionMode);

  function toggleMenu(menu: "project" | "mode" | "provider") {
    setOpenMenu((current) => (current === menu ? null : menu));
  }

  useEffect(() => {
    if (!openMenu) {
      return;
    }

    function closeOnOutsideClick(event: MouseEvent) {
      if (!composerRef.current?.contains(event.target as Node)) {
        setOpenMenu(null);
      }
    }

    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setOpenMenu(null);
      }
    }

    document.addEventListener("mousedown", closeOnOutsideClick);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("mousedown", closeOnOutsideClick);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [openMenu]);

  return (
    <form
      className={`composer${isEmptyState ? " empty-composer" : ""}${
        openMenu ? " composer-menu-open" : ""
      }`}
      ref={composerRef}
      onSubmit={onSubmit}
    >
      <textarea
        aria-label="Message"
        value={composer}
        onChange={(event) => onComposerChange(event.target.value)}
        placeholder={
          isEmptyState ? "Ask anything" : "Ask Liz to inspect, edit, or verify this project..."
        }
      />
      <div className="composer-toolbar">
        <button
          aria-label="Add context"
          className="composer-add-button"
          title="Add context"
          type="button"
        >
          <Plus aria-hidden="true" size={18} />
        </button>
        <div className="composer-context-controls" aria-label="Composer context">
          <div className="composer-context-menu">
            <button
              aria-expanded={openMenu === "project"}
              aria-label="Project"
              className="composer-context-pill"
              type="button"
              onClick={() => toggleMenu("project")}
            >
              <FolderOpen aria-hidden="true" size={16} />
              <span>{projectName}</span>
              <ChevronDown aria-hidden="true" size={14} />
            </button>
            {openMenu === "project" ? (
              <div
                aria-label="Project controls"
                className="composer-popover project-popover"
                role="dialog"
              >
                <div className="composer-popover-title">Project</div>
                <input
                  aria-label="Project path"
                  value={projectPath}
                  onChange={(event) => onProjectPathChange(event.target.value)}
                />
                <div className="composer-popover-actions">
                  <button
                    aria-label="Apply project path"
                    title="Apply project path"
                    type="button"
                    onClick={() => {
                      setOpenMenu(null);
                      onSelectProject();
                    }}
                  >
                    <RotateCcw aria-hidden="true" size={16} />
                    <span>Apply</span>
                  </button>
                  <button
                    aria-label="Browse project"
                    title="Browse project"
                    type="button"
                    onClick={() => {
                      setOpenMenu(null);
                      onBrowseProject();
                    }}
                  >
                    <FolderOpen aria-hidden="true" size={16} />
                    <span>Browse</span>
                  </button>
                </div>
              </div>
            ) : null}
          </div>

          <div className="composer-context-menu">
            <button
              aria-expanded={openMenu === "mode"}
              aria-label="Mode"
              className="composer-context-pill"
              type="button"
              onClick={() => toggleMenu("mode")}
            >
              <Laptop aria-hidden="true" size={16} />
              <span>{modeLabel}</span>
              <ChevronDown aria-hidden="true" size={14} />
            </button>
            {openMenu === "mode" ? (
              <div
                aria-label="Mode details"
                className="composer-popover mode-popover"
                role="dialog"
              >
                <div className="composer-popover-title">Mode</div>
                <div className="composer-option-list">
                  {detailLevelOptions.map((option) => (
                    <button
                      aria-label={`Use mode ${option.label}`}
                      className={option.id === detailLevel ? "active" : ""}
                      key={option.id}
                      type="button"
                      onClick={() => onDetailLevelChange(option.id)}
                    >
                      <span>
                        <strong>{option.label}</strong>
                        <small>{option.description}</small>
                      </span>
                      {option.id === detailLevel ? <Check aria-hidden="true" size={15} /> : null}
                    </button>
                  ))}
                </div>
                <div className="composer-popover-title secondary">Permission</div>
                <div className="composer-option-list compact">
                  {permissionOptions.map((option) => (
                    <button
                      aria-label={`Use permission ${option.label}`}
                      className={option.id === permissionMode ? "active" : ""}
                      key={option.id}
                      type="button"
                      onClick={() => onPermissionModeChange(option.id)}
                    >
                      <span>
                        <strong>{option.label}</strong>
                        <small>{option.description}</small>
                      </span>
                      {option.id === permissionMode ? (
                        <Check aria-hidden="true" size={15} />
                      ) : null}
                    </button>
                  ))}
                </div>
                <p>Current mode: {modeLabel}. Current permission: {permissionLabel}.</p>
              </div>
            ) : null}
          </div>

          <div className="composer-context-menu">
            <button
              aria-expanded={openMenu === "provider"}
              aria-label="Provider"
              className="composer-context-pill"
              type="button"
              onClick={() => toggleMenu("provider")}
            >
              <GitBranch aria-hidden="true" size={16} />
              <span>{activeProviderLabel}</span>
              <ChevronDown aria-hidden="true" size={14} />
            </button>
            {openMenu === "provider" ? (
              <div
                aria-label="Provider controls"
                className="composer-popover provider-popover"
                role="dialog"
              >
                <div className="composer-popover-header">
                  <div>
                    <div className="composer-popover-title">Provider</div>
                    <p>
                      {providerStatus
                        ? `${providerStatus.configured_count} configured`
                        : "Checking provider"}
                    </p>
                  </div>
                  <strong>{activeModelLabel}</strong>
                </div>
                <div className="composer-option-list">
                  {providerStatus?.providers.map((provider) => (
                    <button
                      aria-label={`Use provider ${provider.label}`}
                      className={provider.id === activeProvider ? "active" : ""}
                      disabled={!provider.configured}
                      key={provider.id}
                      type="button"
                      onClick={() => onProviderModelChange(provider.id, provider.model)}
                    >
                      <span>
                        <strong>{provider.label}</strong>
                        <small>{provider.configured ? provider.model : provider.note}</small>
                      </span>
                      {provider.id === activeProvider ? (
                        <Check aria-hidden="true" size={15} />
                      ) : null}
                    </button>
                  ))}
                </div>
                <div className="composer-popover-title secondary">Model</div>
                <div className="composer-option-list compact">
                  {providerStatus?.models.map((model) => (
                    <button
                      aria-label={`Use model ${model.label}`}
                      className={model.id === activeModel ? "active" : ""}
                      disabled={!activeProvider}
                      key={model.id}
                      type="button"
                      onClick={() => onProviderModelChange(activeProvider, model.id)}
                    >
                      <span>{model.label}</span>
                      {model.id === activeModel ? <Check aria-hidden="true" size={15} /> : null}
                    </button>
                  ))}
                </div>
              </div>
            ) : null}
          </div>
        </div>
        <div className="composer-runtime-summary" aria-label="Runtime selectors">
          <span aria-label="Model">{activeModelLabel}</span>
        </div>
        <button
          aria-label="Send message"
          className="send-button"
          disabled={isRunning || composer.trim().length === 0}
          title="Send message"
          type="submit"
        >
          <ArrowUp aria-hidden="true" size={18} />
        </button>
      </div>
    </form>
  );
}

const detailLevelOptions: Array<{
  id: DetailLevelId;
  label: string;
  description: string;
}> = [
  {
    id: "coding",
    label: "Coding",
    description: "More technical detail and controls",
  },
  {
    id: "daily",
    label: "Daily work",
    description: "Less technical detail",
  },
];

function formatPermissionMode(mode?: string | null) {
  switch (mode) {
    case "auto":
      return "Full access";
    case "auto_low_risk":
      return "Auto low risk";
    case "read_only":
      return "Read only";
    case "default":
      return "Ask by default";
    default:
      return "Checking";
  }
}
