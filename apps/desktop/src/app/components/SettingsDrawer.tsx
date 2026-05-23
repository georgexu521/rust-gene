import { useState, type ReactNode } from "react";
import {
  Activity,
  Code2,
  Folder,
  FolderOpen,
  KeyRound,
  MessageCircle,
  RefreshCw,
  Settings,
  Terminal,
} from "lucide-react";
import {
  DetailLevelId,
  DesktopDiagnostic,
  DesktopSettings,
  PermissionModeId,
  PermissionModeOption,
  ProviderSetupInfo,
} from "../../runtime/desktopApi";

type SettingsDrawerProps = {
  isOpen: boolean;
  projectPath: string;
  selectedSessionTitle: string | null;
  activeSessionId: string | null;
  settings: DesktopSettings | null;
  diagnostics: DesktopDiagnostic[];
  providerSetup: ProviderSetupInfo | null;
  permissionOptions: PermissionModeOption[];
  onClose: () => void;
  onSelectRecentProject: (path: string) => void;
  onRefresh: () => void;
  onDetailLevelChange: (level: DetailLevelId) => void;
  onPermissionModeChange: (mode: PermissionModeId) => void;
  onOpenDiagnosticsFolder: () => void;
  onOpenSettingsFolder: () => void;
  onOpenShellProfile: () => void;
};

type SettingsCategory = "general" | "provider" | "permissions" | "diagnostics";

export function SettingsDrawer({
  isOpen,
  projectPath,
  selectedSessionTitle,
  activeSessionId,
  settings,
  diagnostics,
  providerSetup,
  permissionOptions,
  onClose,
  onSelectRecentProject,
  onRefresh,
  onDetailLevelChange,
  onPermissionModeChange,
  onOpenDiagnosticsFolder,
  onOpenSettingsFolder,
  onOpenShellProfile,
}: SettingsDrawerProps) {
  const [activeCategory, setActiveCategory] = useState<SettingsCategory>("general");

  if (!isOpen) {
    return null;
  }

  const providerDiagnostic = diagnostics.find((item) => item.id === "provider_keys");
  const needsProviderSetup = providerDiagnostic?.status !== "ok";

  return (
    <aside className="settings-drawer" aria-label="Settings">
      <nav className="settings-nav" aria-label="Settings categories">
        <button className="settings-back" type="button" onClick={onClose}>
          <span aria-hidden="true">←</span>
          <span>Back to app</span>
        </button>
        <SettingsNavButton
          active={activeCategory === "general"}
          icon={<Settings aria-hidden="true" size={17} />}
          label="General"
          onClick={() => setActiveCategory("general")}
        />
        <SettingsNavButton
          active={activeCategory === "provider"}
          icon={<Terminal aria-hidden="true" size={17} />}
          label="Provider"
          onClick={() => setActiveCategory("provider")}
        />
        <SettingsNavButton
          active={activeCategory === "permissions"}
          icon={<KeyRound aria-hidden="true" size={17} />}
          label="Permissions"
          onClick={() => setActiveCategory("permissions")}
        />
        <SettingsNavButton
          active={activeCategory === "diagnostics"}
          icon={<Activity aria-hidden="true" size={17} />}
          label="Diagnostics"
          onClick={() => setActiveCategory("diagnostics")}
        />
      </nav>

      <div className="settings-page">
        <header className="settings-page-header">
          <div>
            <div className="settings-eyebrow">Settings</div>
            <h2>{settingsCategoryTitle(activeCategory)}</h2>
          </div>
        </header>

        <div className="settings-content">
          {activeCategory === "general" ? (
            <>
              <section className="settings-section">
                <h3>Session</h3>
                <dl className="settings-kv">
                  <div>
                    <dt>Project</dt>
                    <dd>{projectPath || settings?.selected_project || "Not selected"}</dd>
                  </div>
                  <div>
                    <dt>Active session</dt>
                    <dd>{selectedSessionTitle || activeSessionId || settings?.active_session_id || "None"}</dd>
                  </div>
                  <div>
                    <dt>Startup</dt>
                    <dd>{settings?.startup_state.detail || "Not loaded"}</dd>
                  </div>
                  <div>
                    <dt>Recent projects</dt>
                    <dd>{settings?.recent_projects.length || 0}</dd>
                  </div>
                  <div>
                    <dt>Archived sessions</dt>
                    <dd>{settings?.archived_session_ids.length || 0}</dd>
                  </div>
                  <div>
                    <dt>Settings file</dt>
                    <dd>{settings?.settings_path || "Not loaded"}</dd>
                  </div>
                  <div>
                    <dt>Diagnostic log</dt>
                    <dd>{settings?.diagnostic_logs_path || "Not loaded"}</dd>
                  </div>
                </dl>
                <div className="settings-actions">
                  <button type="button" onClick={onRefresh}>
                    <RefreshCw aria-hidden="true" size={14} />
                    <span>Refresh</span>
                  </button>
                  <button type="button" onClick={onOpenSettingsFolder}>
                    <FolderOpen aria-hidden="true" size={14} />
                    <span>Open settings folder</span>
                  </button>
                  <button type="button" onClick={onOpenDiagnosticsFolder}>
                    <FolderOpen aria-hidden="true" size={14} />
                    <span>Open diagnostics folder</span>
                  </button>
                </div>
                {settings?.recent_projects.length ? (
                  <div className="settings-project-list" aria-label="Recent projects">
                    {settings.recent_projects.map((path) => (
                      <button
                        className={path === projectPath ? "active" : ""}
                        key={path}
                        title={path}
                        type="button"
                        onClick={() => onSelectRecentProject(path)}
                      >
                        <Folder aria-hidden="true" size={14} />
                        <span>
                          <strong>{basename(path)}</strong>
                          <small>{path}</small>
                        </span>
                      </button>
                    ))}
                  </div>
                ) : null}
              </section>

              <section className="settings-section">
                <h3>Work mode</h3>
                <p className="settings-copy">Choose how much technical detail Liz shows while working.</p>
                <div className="work-mode-options">
                  <button
                    className={settings?.detail_level === "coding" ? "active" : ""}
                    type="button"
                    onClick={() => onDetailLevelChange("coding")}
                  >
                    <Code2 aria-hidden="true" size={18} />
                    <span>
                      <strong>Coding</strong>
                      <small>Show commands, tool activity, file changes, and validation details.</small>
                    </span>
                    <i aria-hidden="true" />
                  </button>
                  <button
                    className={settings?.detail_level === "daily" ? "active" : ""}
                    type="button"
                    onClick={() => onDetailLevelChange("daily")}
                  >
                    <MessageCircle aria-hidden="true" size={18} />
                    <span>
                      <strong>Daily work</strong>
                      <small>Keep the transcript quieter and emphasize outcomes.</small>
                    </span>
                    <i aria-hidden="true" />
                  </button>
                </div>
              </section>
            </>
          ) : null}

          {activeCategory === "provider" ? (
            <section className="settings-section">
              <h3>Provider setup</h3>
              <ProviderSetupGuide
                needsSetup={needsProviderSetup}
                diagnostic={providerDiagnostic}
                providerSetup={providerSetup}
                onOpenShellProfile={onOpenShellProfile}
                onRefresh={onRefresh}
              />
              <dl className="settings-kv">
                <div>
                  <dt>Shell profile</dt>
                  <dd>{providerSetup?.shell_profile_path || "Not loaded"}</dd>
                </div>
                <div>
                  <dt>Accepted keys</dt>
                  <dd>{providerSetup?.provider_env_vars.join(", ") || "Not loaded"}</dd>
                </div>
                <div>
                  <dt>Example</dt>
                  <dd>{providerSetup?.example || "Not loaded"}</dd>
                </div>
              </dl>
            </section>
          ) : null}

          {activeCategory === "permissions" ? (
            <section className="settings-section permission-section">
              <h3>Permission defaults</h3>
              <p className="settings-copy">
                Choose the default approval behavior for new and resumed desktop
                runtime sessions.
              </p>
              <div className="permission-options">
                {permissionOptions.map((option) => (
                  <button
                    className={`permission-option ${
                      settings?.permission_mode === option.id ? "active" : ""
                    }`}
                    key={option.id}
                    type="button"
                    onClick={() => onPermissionModeChange(option.id)}
                  >
                    <span>
                      <strong>{option.label}</strong>
                      <small>{option.description}</small>
                    </span>
                    <i aria-hidden="true" />
                  </button>
                ))}
              </div>
            </section>
          ) : null}

          {activeCategory === "diagnostics" ? (
            <section className="settings-section">
              <h3>Diagnostics</h3>
              <div className="settings-actions">
                <button type="button" onClick={onOpenDiagnosticsFolder}>
                  <FolderOpen aria-hidden="true" size={14} />
                  <span>Open diagnostics folder</span>
                </button>
              </div>
              <div className="settings-diagnostics">
                {diagnostics.map((item) => (
                  <article className={`settings-diagnostic ${item.status}`} key={item.id}>
                    <div className="settings-diagnostic-row">
                      <strong>{item.label}</strong>
                      <span>{item.status}</span>
                    </div>
                    <p>{item.detail}</p>
                  </article>
                ))}
              </div>
            </section>
          ) : null}
        </div>
      </div>
    </aside>
  );
}

function SettingsNavButton({
  active,
  icon,
  label,
  onClick,
}: {
  active: boolean;
  icon: ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <button className={active ? "active" : ""} type="button" onClick={onClick}>
      {icon}
      <span>{label}</span>
    </button>
  );
}

function settingsCategoryTitle(category: SettingsCategory) {
  switch (category) {
    case "provider":
      return "Provider";
    case "permissions":
      return "Permissions";
    case "diagnostics":
      return "Diagnostics";
    default:
      return "General";
  }
}

function basename(path: string) {
  return path.split(/[\\/]/).filter(Boolean).at(-1) || path;
}

type ProviderSetupGuideProps = {
  needsSetup: boolean;
  diagnostic?: DesktopDiagnostic;
  providerSetup: ProviderSetupInfo | null;
  onOpenShellProfile: () => void;
  onRefresh: () => void;
};

function ProviderSetupGuide({
  needsSetup,
  diagnostic,
  providerSetup,
  onOpenShellProfile,
  onRefresh,
}: ProviderSetupGuideProps) {
  if (!needsSetup) {
    return (
      <div className="provider-guide ok">
        <div className="provider-guide-title">Provider is configured</div>
        <p>{diagnostic?.detail || "At least one provider key is available."}</p>
      </div>
    );
  }

  return (
    <div className="provider-guide warning">
      <div className="provider-guide-title">Provider key required</div>
      <p>
        Add one provider key to your shell profile, then restart the desktop app
        or reload the environment before refreshing diagnostics.
      </p>
      <ol>
        <li>Open the shell profile file.</li>
        <li>
          Add an export line such as{" "}
          <code>{providerSetup?.example || 'export MOONSHOT_API_KEY="your-key-here"'}</code>.
        </li>
        <li>Save the file, restart the app, then refresh diagnostics.</li>
      </ol>
      <div className="settings-actions">
        <button type="button" onClick={onOpenShellProfile}>
          <Terminal aria-hidden="true" size={14} />
          <span>Open shell profile</span>
        </button>
        <button type="button" onClick={onRefresh}>
          <RefreshCw aria-hidden="true" size={14} />
          <span>Refresh diagnostics</span>
        </button>
      </div>
    </div>
  );
}
