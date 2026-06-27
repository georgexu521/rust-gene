import { FormEvent, KeyboardEvent as ReactKeyboardEvent, useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowUp,
  Check,
  ChevronDown,
  FileText,
  FolderOpen,
  GitBranch,
  GitCompare,
  Image,
  KeyRound,
  Laptop,
  ListChecks,
  Paperclip,
  Plus,
  RotateCcw,
  Search,
  X,
} from "lucide-react";
import {
  DetailLevelId,
  DesktopIndexedFile,
  PermissionModeId,
  PermissionModeOption,
  AgentModeId,
  AgentModeOption,
  DesktopRunContext,
  ProviderModelStatus,
  ProviderSetupInfo,
  saveProviderCredential,
} from "../../runtime/desktopApi";

type ComposerProps = {
  composer: string;
  contexts: DesktopRunContext[];
  projectPath: string;
  recentProjects: string[];
  providerStatus: ProviderModelStatus | null;
  providerSetup: ProviderSetupInfo | null;
  symbolFiles?: DesktopIndexedFile[] | null;
  detailLevel?: DetailLevelId | null;
  permissionMode?: PermissionModeId | null;
  agentMode?: AgentModeId | null;
  agentModeOptions?: AgentModeOption[] | null;
  permissionOptions?: PermissionModeOption[] | null;
  isEmptyState?: boolean;
  isRunning?: boolean;
  focusRequest?: number;
  promptHistory?: string[];
  onComposerChange: (value: string) => void;
  onOpenContext: (context: DesktopRunContext) => void;
  onBrowseProject: () => void;
  onSelectProject: (path: string) => void;
  onSelectRecentProject: (path: string) => void;
  onDetailLevelChange?: (level: DetailLevelId) => void;
  onPermissionModeChange?: (mode: PermissionModeId) => void;
  onAgentModeChange?: (mode: AgentModeId) => void;
  onProviderModelChange: (providerId: string, model: string) => void;
  onProviderCredentialSaved?: () => void;
  onAddContext: (context: DesktopRunContext) => void;
  onAddFileContext: () => void;
  onRemoveContext: (id: DesktopRunContext["type"]) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
};

type FileMentionSuggestion = {
  id: string;
  kind: "file" | "symbol";
  label: string;
  detail: string;
  path: string;
  line?: number;
};

export function Composer({
  composer,
  contexts,
  projectPath,
  recentProjects,
  providerStatus,
  providerSetup,
  symbolFiles = [],
  detailLevel,
  permissionMode,
  permissionOptions = [],
  agentMode,
  agentModeOptions,
  isEmptyState = false,
  isRunning,
  focusRequest = 0,
  promptHistory = [],
  onComposerChange,
  onOpenContext,
  onBrowseProject,
  onSelectProject,
  onSelectRecentProject,
  onDetailLevelChange,
  onPermissionModeChange,
  onAgentModeChange,
  onProviderModelChange,
  onProviderCredentialSaved,
  onAddContext,
  onAddFileContext,
  onRemoveContext,
  onSubmit,
}: ComposerProps) {
  const composerRef = useRef<HTMLFormElement | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const localPromptHistoryRef = useRef<string[]>([]);
  const [openMenu, setOpenMenu] = useState<"context" | "project" | "mode" | "provider" | null>(
    null,
  );
  const [draftProjectPath, setDraftProjectPath] = useState(projectPath);
  const [selectedSlashIndex, setSelectedSlashIndex] = useState(0);
  const [localPromptHistory, setLocalPromptHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState<number | null>(null);
  const [historyDraft, setHistoryDraft] = useState("");
  const [repairProviderId, setRepairProviderId] = useState("");
  const [providerKey, setProviderKey] = useState("");
  const [providerSaving, setProviderSaving] = useState(false);
  const [providerSaveMessage, setProviderSaveMessage] = useState("");
  const activeProvider = providerStatus?.active_provider || "";
  const activeModel = providerStatus?.active_model || "";
  const permissionOptionList = permissionOptions || [];
  const providerOptions = providerStatus?.providers || [];
  const missingProviderOptions = providerOptions.filter((provider) => !provider.configured);
  const repairProvider =
    providerOptions.find((provider) => provider.id === repairProviderId) ||
    missingProviderOptions[0] ||
    providerOptions[0];
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
  const modeLabel =
    detailLevel === "labrun"
      ? "LabRun"
      : detailLevel === "engineering" || detailLevel === "coding"
        ? "Engineering"
        : "Daily work";
  const permissionLabel = formatPermissionMode(permissionMode);
  const slashQuery = slashCommandQuery(composer);
  const slashCommandOpen = openMenu === null && slashQuery !== null && !isExactSlashCommand(composer);
  const fileMentionQueryValue = fileMentionQuery(composer);
  const fileMentionOpen = openMenu === null && !slashCommandOpen && fileMentionQueryValue !== null;
  const slashMatches = useMemo(
    () => slashCommands.filter((command) => matchesSlashCommand(command, slashQuery || "")).slice(0, 8),
    [slashQuery],
  );
  const fileMentionSuggestions = useMemo(
    () => fileMentionMatches(symbolFiles || [], fileMentionQueryValue || "").slice(0, 8),
    [symbolFiles, fileMentionQueryValue],
  );
  function currentPromptHistory() {
    return promptHistory.length ? promptHistory : localPromptHistoryRef.current;
  }

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

  function attachFileMention(suggestion: FileMentionSuggestion) {
    onAddContext({
      type: "file",
      label: suggestion.label,
      path: suggestion.path,
      line_start: suggestion.line,
      line_end: suggestion.line,
    });
    updateComposerDraft(removeTrailingFileMention(composer));
    requestAnimationFrame(() => textareaRef.current?.focus());
  }

  function attachFileMentionPicker() {
    updateComposerDraft(removeTrailingFileMention(composer));
    addFileContext();
  }

  function attachFileMentionDiff() {
    updateComposerDraft(removeTrailingFileMention(composer));
    addCurrentDiffContext();
  }

  function applySlashCommand(command: SlashCommand) {
    onComposerChange(command.value);
    setHistoryIndex(null);
    setHistoryDraft("");
    setSelectedSlashIndex(0);
    requestAnimationFrame(() => textareaRef.current?.focus());
  }

  function updateComposerDraft(value: string) {
    setHistoryIndex(null);
    setHistoryDraft("");
    onComposerChange(value);
  }

  function handleFormSubmit(event: FormEvent<HTMLFormElement>) {
    const message = composer.trim();
    if (message && !isRunning) {
      setLocalPromptHistory((current) => {
        const next = current[current.length - 1] === message ? current : [...current, message];
        const bounded = next.slice(-50);
        localPromptHistoryRef.current = bounded;
        return bounded;
      });
    }
    setHistoryIndex(null);
    setHistoryDraft("");
    onSubmit(event);
  }

  function navigatePromptHistory(
    direction: -1 | 1,
    event: ReactKeyboardEvent<HTMLTextAreaElement>,
  ) {
    const historyEntries = currentPromptHistory();
    if (historyEntries.length === 0 || event.metaKey || event.ctrlKey || event.altKey) {
      return false;
    }

    const target = event.currentTarget;
    const selectionStart = target.selectionStart ?? composer.length;
    const selectionEnd = target.selectionEnd ?? composer.length;
    const atStart = selectionStart === 0 && selectionEnd === 0;
    const atEnd = selectionStart === composer.length && selectionEnd === composer.length;
    const emptyDraft = composer.trim().length === 0;

    if (direction === -1) {
      if (historyIndex === null && !emptyDraft && !atStart) {
        return false;
      }
      const nextIndex =
        historyIndex === null ? historyEntries.length - 1 : Math.max(0, historyIndex - 1);
      if (historyIndex === null) {
        setHistoryDraft(composer);
      }
      setHistoryIndex(nextIndex);
      onComposerChange(historyEntries[nextIndex]);
      requestAnimationFrame(() => {
        const textarea = textareaRef.current;
        if (textarea) {
          const end = textarea.value.length;
          textarea.setSelectionRange(end, end);
        }
      });
      return true;
    }

    if (historyIndex === null || !atEnd) {
      return false;
    }
    const nextIndex = historyIndex + 1;
    if (nextIndex >= historyEntries.length) {
      setHistoryIndex(null);
      onComposerChange(historyDraft);
    } else {
      setHistoryIndex(nextIndex);
      onComposerChange(historyEntries[nextIndex]);
    }
    requestAnimationFrame(() => {
      const textarea = textareaRef.current;
      if (textarea) {
        const end = textarea.value.length;
        textarea.setSelectionRange(end, end);
      }
    });
    return true;
  }

  async function saveProviderKey() {
    if (!repairProvider || !providerKey.trim()) {
      return;
    }
    setProviderSaving(true);
    setProviderSaveMessage("");
    try {
      const message = await saveProviderCredential(repairProvider.id, providerKey.trim());
      setProviderSaveMessage(message);
      setProviderKey("");
      onProviderCredentialSaved?.();
    } catch (err) {
      setProviderSaveMessage(String(err));
    } finally {
      setProviderSaving(false);
    }
  }

  function submitOnEnter(event: ReactKeyboardEvent<HTMLTextAreaElement>) {
    if (slashCommandOpen && slashMatches.length > 0) {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setSelectedSlashIndex((current) => (current + 1) % slashMatches.length);
        return;
      }
      if (event.key === "ArrowUp") {
        event.preventDefault();
        setSelectedSlashIndex((current) => (current - 1 + slashMatches.length) % slashMatches.length);
        return;
      }
      if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
        event.preventDefault();
        applySlashCommand(slashMatches[selectedSlashIndex] || slashMatches[0]);
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        updateComposerDraft("");
        setSelectedSlashIndex(0);
        return;
      }
    }

    if (event.key === "ArrowUp" && navigatePromptHistory(-1, event)) {
      event.preventDefault();
      return;
    }
    if (event.key === "ArrowDown" && navigatePromptHistory(1, event)) {
      event.preventDefault();
      return;
    }

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

  useEffect(() => {
    if (focusRequest <= 0) {
      return;
    }
    requestAnimationFrame(() => textareaRef.current?.focus());
  }, [focusRequest]);

  useEffect(() => {
    setSelectedSlashIndex(0);
  }, [slashQuery]);

  useEffect(() => {
    if (selectedSlashIndex < slashMatches.length) {
      return;
    }
    setSelectedSlashIndex(0);
  }, [selectedSlashIndex, slashMatches.length]);

  return (
    <form
      className={`composer${isEmptyState ? " empty-composer" : ""}${
        openMenu ? " composer-menu-open" : ""
      }`}
      ref={composerRef}
      onSubmit={handleFormSubmit}
    >
      <textarea
        aria-label="Message"
        aria-controls={slashCommandOpen ? "composer-slash-command-menu" : undefined}
        aria-expanded={slashCommandOpen}
        aria-haspopup="listbox"
        ref={textareaRef}
        value={composer}
        onChange={(event) => updateComposerDraft(event.target.value)}
        onKeyDown={submitOnEnter}
        placeholder={
          isEmptyState ? "Ask anything" : "Ask Liz to inspect, edit, or verify this project..."
        }
      />
      {slashCommandOpen ? (
        <div
          aria-label="Slash commands"
          className="slash-command-popover"
          id="composer-slash-command-menu"
          role="listbox"
        >
          <div className="slash-command-header">
            <Search aria-hidden="true" size={14} />
            <span>Slash commands</span>
            {slashQuery ? <small>/{slashQuery}</small> : <small>Type to filter</small>}
          </div>
          {slashMatches.length === 0 ? (
            <div className="slash-command-empty">No matching commands</div>
          ) : (
            <div className="slash-command-list">
              {slashMatches.map((command, index) => (
                <button
                  aria-label={`Use slash command ${command.command}`}
                  aria-selected={index === selectedSlashIndex}
                  className={index === selectedSlashIndex ? "active" : ""}
                  key={command.command}
                  role="option"
                  type="button"
                  onMouseEnter={() => setSelectedSlashIndex(index)}
                  onClick={() => applySlashCommand(command)}
                >
                  <span className="slash-command-icon">
                    <ListChecks aria-hidden="true" size={14} />
                  </span>
                  <span>
                    <strong>{command.command}</strong>
                    <small>{command.description}</small>
                  </span>
                  <code>{command.group}</code>
                </button>
              ))}
            </div>
          )}
        </div>
      ) : null}
      {fileMentionOpen ? (
        <div
          aria-label="File and symbol mentions"
          className="file-mention-popover"
          role="dialog"
        >
          <div className="slash-command-header">
            <FileText aria-hidden="true" size={14} />
            <span>@file context</span>
            {fileMentionQueryValue ? <small>@{fileMentionQueryValue}</small> : <small>Attach context</small>}
          </div>
          {fileMentionSuggestions.length > 0 ? (
            <div className="slash-command-list file-mention-list">
              {fileMentionSuggestions.map((suggestion) => (
                <button
                  aria-label={`Attach ${suggestion.label}`}
                  key={suggestion.id}
                  type="button"
                  onClick={() => attachFileMention(suggestion)}
                >
                  <span className="slash-command-icon">
                    <FileText aria-hidden="true" size={14} />
                  </span>
                  <span>
                    <strong>{suggestion.label}</strong>
                    <small>{suggestion.detail}</small>
                  </span>
                  <code>{suggestion.kind}</code>
                </button>
              ))}
            </div>
          ) : (
            <div className="slash-command-empty">No indexed file or symbol matches</div>
          )}
          <div className="file-mention-actions">
            <button type="button" onClick={attachFileMentionPicker}>
              <FileText aria-hidden="true" size={14} />
              Attach file
            </button>
            <button type="button" onClick={attachFileMentionDiff}>
              <GitCompare aria-hidden="true" size={14} />
              Current diff
            </button>
          </div>
        </div>
      ) : null}
      <div className="composer-context-chips" aria-label="Attached context">
        <div className="composer-project-context" title={projectPath}>
          <FolderOpen aria-hidden="true" size={14} />
          <span>Project</span>
          <strong>{projectName}</strong>
        </div>
        {contexts.length ? (
          <div className="composer-attachment-list">
            {contexts.map((context) => (
              <article className="composer-attachment" key={context.type}>
                <button
                  aria-label={`Open context ${context.label}`}
                  className="composer-attachment-main"
                  type="button"
                  onClick={() => onOpenContext(context)}
                >
                  <span className="composer-attachment-icon">
                    {context.type === "file" ? (
                      <FileText aria-hidden="true" size={14} />
                    ) : (
                      <GitCompare aria-hidden="true" size={14} />
                    )}
                  </span>
                  <span>
                    <strong>{context.label}</strong>
                    <small>{contextAttachmentDetail(context)}</small>
                  </span>
                </button>
                <button
                  aria-label={`Remove context ${context.label}`}
                  className="composer-attachment-remove"
                  title={`Remove ${context.label}`}
                  type="button"
                  onClick={() => onRemoveContext(context.type)}
                >
                  <X aria-hidden="true" size={13} />
                </button>
              </article>
            ))}
          </div>
        ) : (
          <div className="composer-context-empty">
            <Paperclip aria-hidden="true" size={14} />
            <span>Add files or current diff when the task needs sharper context.</span>
          </div>
        )}
      </div>
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
              <div
                aria-label="Screenshot context unavailable"
                className="composer-option-unavailable"
                role="note"
              >
                <span>
                  <strong>
                    <Image aria-hidden="true" size={15} />
                    Screenshot
                  </strong>
                  <small>Screen capture context is not connected yet.</small>
                </span>
              </div>
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
                  {providerOptions.map((provider) => (
                    <div
                      className={`provider-option-row ${provider.id === activeProvider ? "active" : ""} ${
                        provider.configured ? "configured" : "missing"
                      }`}
                      key={provider.id}
                    >
                      <button
                        aria-label={`Use provider ${provider.label}`}
                        disabled={!provider.configured}
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
                      {!provider.configured ? (
                        <button
                          aria-label={`Repair provider ${provider.label}`}
                          className="provider-repair-inline"
                          type="button"
                          onClick={() => {
                            setRepairProviderId(provider.id);
                            setProviderSaveMessage("");
                          }}
                        >
                          Setup
                        </button>
                      ) : null}
                    </div>
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
                {providerStatus && missingProviderOptions.length > 0 ? (
                  <div className="provider-repair-panel" aria-label="Provider setup repair">
                    <div className="provider-repair-title">
                      <KeyRound aria-hidden="true" size={14} />
                      <span>Provider setup repair</span>
                    </div>
                    <p>
                      Save a missing API key here, then refresh provider status. Runtime requests still use the configured Rust provider path.
                    </p>
                    <label>
                      <span>Provider</span>
                      <select
                        aria-label="Repair provider"
                        value={repairProvider?.id || ""}
                        onChange={(event) => {
                          setRepairProviderId(event.target.value);
                          setProviderSaveMessage("");
                        }}
                      >
                        {providerOptions.map((provider) => (
                          <option key={provider.id} value={provider.id}>
                            {provider.label}
                            {provider.configured ? " (configured)" : ""}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label>
                      <span>API key</span>
                      <input
                        aria-label="Provider API key"
                        placeholder={
                          repairProvider
                            ? `Paste ${repairProvider.label} API key`
                            : "Paste provider API key"
                        }
                        type="password"
                        value={providerKey}
                        onChange={(event) => setProviderKey(event.target.value)}
                      />
                    </label>
                    <div className="provider-repair-actions">
                      <button
                        disabled={!repairProvider || !providerKey.trim() || providerSaving}
                        type="button"
                        onClick={() => void saveProviderKey()}
                      >
                        {providerSaving ? "Saving..." : "Save key"}
                      </button>
                      <span>
                        {providerSetup?.provider_env_vars.length
                          ? providerSetup.provider_env_vars.slice(0, 3).join(", ")
                          : "Accepted provider env vars"}
                      </span>
                    </div>
                    {providerSaveMessage ? (
                      <p className="provider-repair-message">{providerSaveMessage}</p>
                    ) : null}
                  </div>
                ) : null}
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
    id: "daily",
    label: "Daily",
    description: "Quieter task view",
  },
  {
    id: "engineering",
    label: "Engineering",
    description: "More technical detail and controls",
  },
  {
    id: "labrun",
    label: "LabRun",
    description: "Project governance view",
  },
];

type SlashCommand = {
  command: string;
  description: string;
  group: "agent" | "lab" | "context" | "session";
  value: string;
};

const slashCommands: SlashCommand[] = [
  {
    command: "/help",
    description: "Show available commands and desktop workflow hints.",
    group: "agent",
    value: "/help",
  },
  {
    command: "/status",
    description: "Summarize current session, provider, context, and runtime state.",
    group: "agent",
    value: "/status",
  },
  {
    command: "/model",
    description: "Show or change the active provider model.",
    group: "agent",
    value: "/model",
  },
  {
    command: "/goal",
    description: "Start or inspect a durable goal for Direct Agent Mode.",
    group: "session",
    value: "/goal ",
  },
  {
    command: "/goal pause",
    description: "Pause the current Direct Agent goal.",
    group: "session",
    value: "/goal pause",
  },
  {
    command: "/goal resume",
    description: "Resume the current Direct Agent goal.",
    group: "session",
    value: "/goal resume",
  },
  {
    command: "/compact",
    description: "Request context compaction when the runtime supports it.",
    group: "context",
    value: "/compact",
  },
  {
    command: "/lab dashboard",
    description: "Open the LabRun dashboard and current project-loop status.",
    group: "lab",
    value: "/lab dashboard",
  },
  {
    command: "/lab proposal",
    description: "Inspect or prepare LabRun proposal state.",
    group: "lab",
    value: "/lab proposal",
  },
  {
    command: "/lab meeting open",
    description: "Stage a professor/postdoc meeting for LabRun steering.",
    group: "lab",
    value: "/lab meeting open",
  },
  {
    command: "/lab continue",
    description: "Continue the active LabRun loop with a short steering note.",
    group: "lab",
    value: "/lab continue ",
  },
  {
    command: "/lab intervene",
    description: "Send a user-to-professor intervention into LabRun.",
    group: "lab",
    value: "/lab intervene ",
  },
  {
    command: "/lab recovery",
    description: "Inspect paused, blocked, or recoverable LabRun state.",
    group: "lab",
    value: "/lab recovery",
  },
  {
    command: "/lab daemon health",
    description: "Check LabRun daemon supervision state.",
    group: "lab",
    value: "/lab daemon health",
  },
  {
    command: "/lab closeout auto",
    description: "Stage LabRun closeout report generation.",
    group: "lab",
    value: "/lab closeout auto",
  },
];

function slashCommandQuery(value: string) {
  if (value.includes("\n")) {
    return null;
  }
  const trimmedStart = value.trimStart();
  if (!trimmedStart.startsWith("/")) {
    return null;
  }
  return trimmedStart.slice(1);
}

function isExactSlashCommand(value: string) {
  const normalized = value.trimStart();
  return slashCommands.some((command) => command.value === normalized);
}

function matchesSlashCommand(command: SlashCommand, query: string) {
  const normalized = query.trim().toLowerCase();
  if (!normalized) {
    return true;
  }
  const haystack = `${command.command} ${command.description} ${command.group}`.toLowerCase();
  return haystack.includes(normalized);
}

function fileMentionQuery(value: string) {
  const match = value.match(/(?:^|\s)@([^\s@]*)$/);
  if (!match) {
    return null;
  }
  return match[1] || "";
}

function removeTrailingFileMention(value: string) {
  return value.replace(/(?:^|\s)@[^\s@]*$/, "").trimEnd();
}

function fileMentionMatches(
  files: DesktopIndexedFile[],
  query: string,
): FileMentionSuggestion[] {
  const normalizedQuery = query.trim().toLowerCase();
  const suggestions: FileMentionSuggestion[] = [];
  for (const file of files) {
    const path = file.path;
    const pathMatch = !normalizedQuery || path.toLowerCase().includes(normalizedQuery);
    if (pathMatch) {
      suggestions.push({
        id: `file:${path}`,
        kind: "file",
        label: path,
        detail: `${file.lines} lines · ${file.symbols.length} symbols`,
        path,
      });
    }
    for (const symbol of file.symbols.slice(0, 8)) {
      const symbolHaystack =
        `${symbol.name} ${symbol.kind} ${symbol.signature}`.toLowerCase();
      if (normalizedQuery && !pathMatch && !symbolHaystack.includes(normalizedQuery)) {
        continue;
      }
      suggestions.push({
        id: `symbol:${path}:${symbol.line}:${symbol.name}`,
        kind: "symbol",
        label: symbol.name,
        detail: `${path}:${symbol.line} · ${symbol.kind}`,
        path,
        line: symbol.line,
      });
    }
    if (suggestions.length >= 24) {
      break;
    }
  }
  return suggestions;
}

function contextAttachmentDetail(context: DesktopRunContext) {
  if (context.type === "file") {
    const detail = context.detail?.type === "file" ? context.detail : null;
    const path = detail?.relative_path || context.path;
    const lineLabel =
      context.line_start && context.line_end
        ? `:${context.line_start}-${context.line_end}`
        : context.line_start
          ? `:${context.line_start}`
          : "";
    const sizeLabel = detail ? ` · ${detail.line_count} lines` : "";
    return `${path}${lineLabel}${sizeLabel}`;
  }

  const detail = context.detail?.type === "current_diff" ? context.detail : null;
  if (detail?.shortstat) {
    return detail.shortstat;
  }
  if (detail?.files.length) {
    return `${detail.files.length} changed files`;
  }
  return "Current workspace diff";
}

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
