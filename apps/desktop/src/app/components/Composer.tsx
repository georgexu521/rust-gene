import { FormEvent, KeyboardEvent as ReactKeyboardEvent, useEffect, useRef, useState } from "react";
import {
  ArrowUp,
  Check,
  ChevronDown,
  FileText,
  FolderOpen,
  GitBranch,
  GitCompare,
  Image,
  Laptop,
  Plus,
  RotateCcw,
} from "lucide-react";
import {
  DetailLevelId,
  PermissionModeId,
  PermissionModeOption,
  AgentModeId,
  AgentModeOption,
  DesktopRunContext,
  ProviderModelStatus,
} from "../../runtime/desktopApi";

type ComposerProps = {
  composer: string;
  contexts: DesktopRunContext[];
  projectPath: string;
  recentProjects: string[];
  providerStatus: ProviderModelStatus | null;
  detailLevel?: DetailLevelId | null;
  permissionMode?: PermissionModeId | null;
  agentMode?: AgentModeId | null;
  agentModeOptions?: AgentModeOption[] | null;
  permissionOptions?: PermissionModeOption[] | null;
  isEmptyState?: boolean;
  isRunning?: boolean;
  onComposerChange: (value: string) => void;
  onOpenContext: (context: DesktopRunContext) => void;
  onBrowseProject: () => void;
  onSelectProject: (path: string) => void;
  onSelectRecentProject: (path: string) => void;
  onDetailLevelChange?: (level: DetailLevelId) => void;
  onPermissionModeChange?: (mode: PermissionModeId) => void;
  onAgentModeChange?: (mode: AgentModeId) => void;
  onProviderModelChange: (providerId: string, model: string) => void;
  onAddContext: (context: DesktopRunContext) => void;
  onAddFileContext: () => void;
  onRemoveContext: (id: DesktopRunContext["type"]) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
};

export function Composer({
  composer,
  contexts,
  projectPath,
  recentProjects,
  providerStatus,
  detailLevel,
  permissionMode,
  permissionOptions = [],
  agentMode,
  agentModeOptions,
  isEmptyState = false,
  isRunning,
  onComposerChange,
  onOpenContext,
  onBrowseProject,
  onSelectProject,
  onSelectRecentProject,
  onDetailLevelChange,
  onPermissionModeChange,
  onAgentModeChange,
  onProviderModelChange,
  onAddContext,
  onAddFileContext,
  onRemoveContext,
  onSubmit,
}: ComposerProps) {
  const composerRef = useRef<HTMLFormElement | null>(null);
  const [openMenu, setOpenMenu] = useState<"context" | "project" | "mode" | "provider" | null>(
    null,
  );
  const [draftProjectPath, setDraftProjectPath] = useState(projectPath);
  const activeProvider = providerStatus?.active_provider || "";
  const activeModel = providerStatus?.active_model || "";
  const permissionOptionList = permissionOptions || [];
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

  function toggleMenu(menu: "context" | "project" | "mode" | "provider") {
    setOpenMenu((current) => {
      const next = current === menu ? null : menu;
      if (next === "project") {
        setDraftProjectPath(projectPath);
      }
      return next;
    });
  }

  function addCurrentDiffContext() {
    onAddContext({
      type: "current_diff",
      label: "Current diff",
    });
    setOpenMenu(null);
  }

  function addFileContext() {
    setOpenMenu(null);
    onAddFileContext();
  }

  function submitOnEnter(event: ReactKeyboardEvent<HTMLTextAreaElement>) {
    if (event.key !== "Enter" || event.shiftKey || event.nativeEvent.isComposing) {
      return;
    }
    event.preventDefault();
    event.currentTarget.form?.requestSubmit();
  }

  useEffect(() => {
    if (!openMenu) {
      setDraftProjectPath(projectPath);
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
  }, [openMenu, projectPath]);

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
        onKeyDown={submitOnEnter}
        placeholder={
          isEmptyState ? "Ask anything" : "Ask Liz to inspect, edit, or verify this project..."
        }
      />
      {contexts.length ? (
        <div className="composer-context-chips" aria-label="Attached context">
          {contexts.map((context) => (
            <button
              aria-label={`Open context ${context.label}`}
              key={context.type}
              type="button"
              onClick={() => onOpenContext(context)}
            >
              {context.type === "file" ? (
                <FileText aria-hidden="true" size={14} />
              ) : (
                <GitCompare aria-hidden="true" size={14} />
              )}
              <span>{context.label}</span>
            </button>
          ))}
        </div>
      ) : null}
      <div className="composer-toolbar">
        <button
          aria-expanded={openMenu === "context"}
          aria-label="Add context"
          className="composer-add-button"
          title="Add context"
          type="button"
          onClick={() => toggleMenu("context")}
        >
          <Plus aria-hidden="true" size={18} />
        </button>
        {openMenu === "context" ? (
          <div
            aria-label="Add context options"
            className="composer-popover context-popover"
            role="dialog"
          >
            <div className="composer-popover-title">Add context</div>
            <div className="composer-option-list">
              <button
                aria-label="Reference current diff"
                type="button"
                onClick={addCurrentDiffContext}
              >
                <span>
                  <strong>
                    <GitCompare aria-hidden="true" size={15} />
                    Current diff
                  </strong>
                  <small>Ask Liz to inspect unstaged and staged changes.</small>
                </span>
              </button>
              <button aria-label="Attach file" type="button" onClick={addFileContext}>
                <span>
                  <strong>
                    <FileText aria-hidden="true" size={15} />
                    File
                  </strong>
                  <small>Attach a specific project file with a readable preview.</small>
                </span>
              </button>
              <button aria-label="Add screenshot" disabled type="button">
                <span>
                  <strong>
                    <Image aria-hidden="true" size={15} />
                    Screenshot
                  </strong>
                  <small>Add a screen capture after native capture support lands.</small>
                </span>
              </button>
            </div>
          </div>
        ) : null}
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
                  value={draftProjectPath}
                  onChange={(event) => setDraftProjectPath(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key !== "Enter") {
                      return;
                    }
                    event.preventDefault();
                    const nextPath = draftProjectPath.trim();
                    if (nextPath) {
                      setOpenMenu(null);
                      onSelectProject(nextPath);
                    }
                  }}
                />
                {recentProjects.length ? (
                  <>
                    <div className="composer-popover-title secondary">Recent projects</div>
                    <div className="composer-option-list compact project-recent-list">
                      {recentProjects.map((path) => (
                        <button
                          aria-label={`Use recent project ${projectLabel(path)}`}
                          className={path === projectPath ? "active" : ""}
                          key={path}
                          type="button"
                          onClick={() => {
                            setOpenMenu(null);
                            onSelectRecentProject(path);
                          }}
                        >
                          <span>
                            <strong>{projectLabel(path)}</strong>
                            <small>{path}</small>
                          </span>
                          {path === projectPath ? <Check aria-hidden="true" size={15} /> : null}
                        </button>
                      ))}
                    </div>
                  </>
                ) : null}
                <div className="composer-popover-actions">
                  <button
                    aria-label="Apply project path"
                    disabled={!draftProjectPath.trim() || isRunning}
                    title="Apply project path"
                    type="button"
                    onClick={() => {
                      setOpenMenu(null);
                      onSelectProject(draftProjectPath.trim());
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
                      onClick={() => onDetailLevelChange?.(option.id)}
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
                  {permissionOptionList.map((option) => (
                    <button
                      aria-label={`Use permission ${option.label}`}
                      className={option.id === permissionMode ? "active" : ""}
                      key={option.id}
                      type="button"
                      onClick={() => onPermissionModeChange?.(option.id)}
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
                {agentModeOptions && agentModeOptions.length > 0 ? (
                  <>
                    <div className="composer-popover-title secondary">Agent</div>
                    <div className="composer-option-list compact">
                      {agentModeOptions.map((option) => (
                        <button
                          aria-label={`Use agent ${option.label}`}
                          className={option.id === agentMode ? "active" : ""}
                          key={option.id}
                          type="button"
                          onClick={() => onAgentModeChange?.(option.id)}
                        >
                          <span>
                            <strong>{option.label}</strong>
                            <small>{option.description}</small>
                          </span>
                          {option.id === agentMode ? <Check aria-hidden="true" size={15} /> : null}
                        </button>
                      ))}
                    </div>
                  </>
                ) : null}
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
                      onClick={() => {
                        setOpenMenu(null);
                        onProviderModelChange(provider.id, provider.model);
                      }}
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
                      onClick={() => {
                        setOpenMenu(null);
                        onProviderModelChange(activeProvider, model.id);
                      }}
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

function projectLabel(path: string) {
  const segments = path.split("/").filter(Boolean);
  return segments[segments.length - 1] || path || "Project";
}

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
