import { Folder, FolderOpen, RefreshCw, Terminal, X } from "lucide-react";
import {
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
  onPermissionModeChange: (mode: PermissionModeId) => void;
  onOpenSettingsFolder: () => void;
  onOpenShellProfile: () => void;
};

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
  onPermissionModeChange,
  onOpenSettingsFolder,
  onOpenShellProfile,
}: SettingsDrawerProps) {
  if (!isOpen) {
    return null;
  }

  const providerDiagnostic = diagnostics.find((item) => item.id === "provider_keys");
  const needsProviderSetup = providerDiagnostic?.status !== "ok";

  return (
    <aside className="settings-drawer" aria-label="Settings">
      <div className="settings-header">
        <div>
          <div className="settings-eyebrow">Settings</div>
          <h2>Desktop state</h2>
        </div>
        <button aria-label="Close settings" type="button" onClick={onClose}>
          <X aria-hidden="true" size={16} />
        </button>
      </div>

      <div className="settings-content">
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

        <section className="settings-section">
          <h3>Permission defaults</h3>
          <p className="settings-copy">
            Choose the default approval behavior for new and resumed desktop
            runtime sessions.
          </p>
          <div className="permission-options">
            {permissionOptions.map((option) => (
              <label
                className={`permission-option ${
                  settings?.permission_mode === option.id ? "active" : ""
                }`}
                key={option.id}
              >
                <input
                  checked={settings?.permission_mode === option.id}
                  name="permission-mode"
                  onChange={() => onPermissionModeChange(option.id)}
                  type="radio"
                />
                <span>
                  <strong>{option.label}</strong>
                  <small>{option.description}</small>
                </span>
              </label>
            ))}
          </div>
        </section>

        <section className="settings-section">
          <h3>Diagnostics</h3>
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
      </div>
    </aside>
  );
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
