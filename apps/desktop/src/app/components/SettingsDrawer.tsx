import { useRef, useState, type ReactNode } from "react";
import {
  Activity,
  Code2,
  Folder,
  FolderOpen,
  KeyRound,
  MessageCircle,
  RefreshCw,
  Settings,
  ShieldCheck,
  Terminal,
} from "lucide-react";
import {
  DetailLevelId,
  DesktopDiagnostic,
  DesktopSettings,
  PermissionModeId,
  PermissionModeOption,
  ProviderModelStatus,
  ProviderSetupInfo,
  DesktopWorkspaceTrustInput,
} from "../../runtime/desktopApi";
import { saveProviderCredential as saveCred } from "../../runtime/desktopApi";
import { useDrawerKeyboard } from "./useDrawerKeyboard";

type SettingsDrawerProps = {
  isOpen: boolean;
  projectPath: string;
  selectedSessionTitle: string | null;
  activeSessionId: string | null;
  settings: DesktopSettings | null;
  diagnostics: DesktopDiagnostic[];
  providerSetup: ProviderSetupInfo | null;
  providerStatus: ProviderModelStatus | null;
  permissionOptions: PermissionModeOption[];
  onClose: () => void;
  onSelectRecentProject: (path: string) => void;
  onRefresh: () => void;
  onDetailLevelChange: (level: DetailLevelId) => void;
  onPermissionModeChange: (mode: PermissionModeId) => void;
  onLabDaemonSupervisionChange: (enabled: boolean) => void;
  onWorkspaceTrustChange: (input: DesktopWorkspaceTrustInput) => void;
  onWorkspaceTrustReset: () => void;
  onExportDiagnosticsBundle: () => void;
  onOpenDiagnosticsFolder: () => void;
  onOpenSettingsFolder: () => void;
  onOpenShellProfile: () => void;
};

type SettingsCategory = "general" | "provider" | "permissions" | "trust" | "diagnostics";

export function SettingsDrawer({
  isOpen,
  projectPath,
  selectedSessionTitle,
  activeSessionId,
  settings,
  diagnostics,
  providerSetup,
  providerStatus,
  permissionOptions,
  onClose,
  onSelectRecentProject,
  onRefresh,
  onDetailLevelChange,
  onPermissionModeChange,
  onLabDaemonSupervisionChange,
  onWorkspaceTrustChange,
  onWorkspaceTrustReset,
  onExportDiagnosticsBundle,
  onOpenDiagnosticsFolder,
  onOpenSettingsFolder,
  onOpenShellProfile,
}: SettingsDrawerProps) {
  const [activeCategory, setActiveCategory] = useState<SettingsCategory>("general");
  const backButtonRef = useRef<HTMLButtonElement>(null);
  const drawerRef = useDrawerKeyboard<HTMLElement>({
    initialFocusRef: backButtonRef,
    isOpen,
    onClose,
  });

  if (!isOpen) {
    return null;
  }

  const providerDiagnostic = diagnostics.find((item) => item.id === "provider_keys");
  const needsProviderSetup = providerDiagnostic?.status !== "ok";

  return (
    <aside ref={drawerRef} className="settings-drawer" aria-label="Settings">
      <nav className="settings-nav" aria-label="Settings categories">
        <button ref={backButtonRef} className="settings-back" type="button" onClick={onClose}>
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
          active={activeCategory === "trust"}
          icon={<ShieldCheck aria-hidden="true" size={17} />}
          label="Trust"
          onClick={() => setActiveCategory("trust")}
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
              <section className="settings-section">
                <h3>Lab daemon supervision</h3>
                <p className="settings-copy">
                  Automatic LabRun daemon supervision is off by default. Manual
                  supervision stays available from the LabRun panel.
                </p>
                <label className="settings-toggle-row">
                  <input
                    type="checkbox"
                    checked={settings?.lab_daemon_supervision_enabled === true}
                    onChange={(event) =>
                      onLabDaemonSupervisionChange(event.target.checked)
                    }
                  />
                  <span>Run automatic supervision while the desktop app is open</span>
                </label>
                <dl className="settings-kv">
                  <div>
                    <dt>Last supervision</dt>
                    <dd>{settings?.lab_daemon_last_supervision || "Not run"}</dd>
                  </div>
                  <div>
                    <dt>Last result</dt>
                    <dd>{settings?.lab_daemon_last_supervision_result || "No result"}</dd>
                  </div>
                  <div>
                    <dt>Next supervision</dt>
                    <dd>{settings?.lab_daemon_next_supervision || "Not scheduled"}</dd>
                  </div>
                </dl>
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
                providerStatus={providerStatus}
                onOpenSettingsFolder={onOpenSettingsFolder}
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
                <div>
                  <dt>Credential store</dt>
                  <dd>{settings?.credential_storage.active_store || "Not loaded"}</dd>
                </div>
                <div>
                  <dt>System keychain</dt>
                  <dd>{settings?.credential_storage.system_keychain_available ? "available" : "not active"}</dd>
                </div>
              </dl>
              {settings?.credential_storage ? (
                <p className="settings-copy">
                  {settings.credential_storage.detail} Fallback path:{" "}
                  {settings.credential_storage.dotenv_fallback_path}
                </p>
              ) : null}
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

          {activeCategory === "trust" ? (
            <WorkspaceTrustSection
              settings={settings}
              onChange={onWorkspaceTrustChange}
              onReset={onWorkspaceTrustReset}
            />
          ) : null}

          {activeCategory === "diagnostics" ? (
            <section className="settings-section">
              <h3>Diagnostics</h3>
              <div className="settings-actions">
                <button type="button" onClick={onExportDiagnosticsBundle}>
                  <RefreshCw aria-hidden="true" size={14} />
                  <span>Export redacted diagnostics</span>
                </button>
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
    case "trust":
      return "Workspace Trust";
    case "diagnostics":
      return "Diagnostics";
    default:
      return "General";
  }
}

function WorkspaceTrustSection({
  settings,
  onChange,
  onReset,
}: {
  settings: DesktopSettings | null;
  onChange: (input: DesktopWorkspaceTrustInput) => void;
  onReset: () => void;
}) {
  const trust = settings?.workspace_trust;
  const current: DesktopWorkspaceTrustInput = {
    package_scripts: trust?.package_scripts === "trusted" ? "trusted" : "ask",
    shell_validation: trust?.shell_validation === "trusted" ? "trusted" : "ask",
    lab_daemon_supervision: trust?.lab_daemon_supervision === true,
    developer_auto_acknowledged: trust?.developer_auto_acknowledged === true,
  };
  const update = (patch: Partial<DesktopWorkspaceTrustInput>) => onChange({ ...current, ...patch });

  return (
    <section className="settings-section">
      <h3>Workspace trust</h3>
      <p className="settings-copy">
        Trust is scoped to the selected project and is revocable from this panel.
      </p>
      <div className="settings-trust-grid">
        <TrustCard label="Project" value={trust?.canonical_project_path || settings?.selected_project || "Not loaded"} />
        <TrustCard label="Repository" value={trust?.repo_identity || "unknown"} />
        <TrustCard label="Source" value={trust?.trust_source || "not loaded"} />
        <TrustCard
          label="Capabilities"
          value={trust?.trusted_capabilities.length ? trust.trusted_capabilities.join(", ") : "none"}
        />
      </div>
      <div className="trust-choice-row">
        <span>Package-script validation</span>
        <div role="group" aria-label="Package-script validation trust">
          <button className={current.package_scripts === "ask" ? "active" : ""} type="button" onClick={() => update({ package_scripts: "ask" })}>Ask</button>
          <button className={current.package_scripts === "trusted" ? "active" : ""} type="button" onClick={() => update({ package_scripts: "trusted" })}>Trusted</button>
        </div>
      </div>
      <div className="trust-choice-row">
        <span>Shell validation</span>
        <div role="group" aria-label="Shell validation trust">
          <button className={current.shell_validation === "ask" ? "active" : ""} type="button" onClick={() => update({ shell_validation: "ask" })}>Ask</button>
          <button className={current.shell_validation === "trusted" ? "active" : ""} type="button" onClick={() => update({ shell_validation: "trusted" })}>Trusted</button>
        </div>
      </div>
      <label className="settings-toggle-row">
        <input
          type="checkbox"
          checked={current.lab_daemon_supervision}
          onChange={(event) => update({ lab_daemon_supervision: event.target.checked })}
        />
        <span>Allow automatic Lab daemon supervision for this project.</span>
      </label>
      <label className="settings-toggle-row">
        <input
          type="checkbox"
          checked={current.developer_auto_acknowledged}
          onChange={(event) => update({ developer_auto_acknowledged: event.target.checked })}
        />
        <span>Allow Developer Auto after explicit project trust acknowledgement.</span>
      </label>
      <div className="settings-actions">
        <button type="button" onClick={onReset}>
          Revoke project trust
        </button>
      </div>
    </section>
  );
}

function TrustCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="settings-trust-card">
      <strong>{label}</strong>
      <span>{value}</span>
    </div>
  );
}

function basename(path: string) {
  return path.split(/[\\/]/).filter(Boolean).at(-1) || path;
}

type ProviderSetupGuideProps = {
  needsSetup: boolean;
  diagnostic?: DesktopDiagnostic;
  providerSetup: ProviderSetupInfo | null;
  providerStatus: ProviderModelStatus | null;
  onOpenSettingsFolder: () => void;
  onOpenShellProfile: () => void;
  onRefresh: () => void;
};

function ProviderSetupGuide({
  needsSetup,
  diagnostic,
  providerSetup,
  providerStatus,
  onOpenSettingsFolder,
  onOpenShellProfile,
  onRefresh,
}: ProviderSetupGuideProps) {
  const [apiKey, setApiKey] = useState("");
  const [selectedProviderId, setSelectedProviderId] = useState("");
  const [saving, setSaving] = useState(false);
  const [saveMsg, setSaveMsg] = useState("");
  const providerOptions = providerStatus?.providers || [];
  const selectedProvider =
    providerOptions.find((provider) => provider.id === selectedProviderId) ||
    providerOptions.find((provider) => !provider.configured) ||
    providerOptions[0];
  const effectiveProviderId = selectedProvider?.id || selectedProviderId;

  const handleSave = async () => {
    if (!apiKey.trim() || !effectiveProviderId) return;
    setSaving(true);
    setSaveMsg("");
    try {
      const result = await saveCred(effectiveProviderId, apiKey.trim());
      setSaveMsg(result);
      setApiKey("");
      onRefresh();
    } catch (err) {
      setSaveMsg(String(err));
    } finally {
      setSaving(false);
    }
  };

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
        Paste your API key below to save it directly, or follow the shell
        profile steps.
      </p>
      <p className="settings-copy">
        Desktop key saving currently writes to the local Priority Agent dotenv
        file, not the system keychain. Use a non-shared machine and avoid
        production secrets until keychain storage is available.
      </p>
      <button className="settings-link-button" type="button" onClick={onOpenSettingsFolder}>
        Open settings folder
      </button>
      <div className="provider-credential-row">
        <select
          aria-label="Provider"
          className="provider-select"
          value={effectiveProviderId}
          onChange={(event) => setSelectedProviderId(event.target.value)}
        >
          {providerOptions.map((provider) => (
            <option key={provider.id} value={provider.id}>
              {provider.label}
              {provider.configured ? " (configured)" : ""}
            </option>
          ))}
        </select>
        <input
          className="provider-key-input"
          type="password"
          placeholder={
            selectedProvider
              ? `Paste ${selectedProvider.label} API key here`
              : "Paste your API key here"
          }
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
        />
        <button
          className="provider-save-button"
          type="button"
          onClick={handleSave}
          disabled={saving || !apiKey.trim() || !effectiveProviderId}
        >
          {saving ? "Saving..." : "Save key"}
        </button>
      </div>
      {saveMsg ? <p className="settings-copy">{saveMsg}</p> : null}
      <p className="provider-shell-profile-copy">
        Alternatively, add one provider key to your shell profile, then restart
        the desktop app.
      </p>
      <ol>
        <li>Open the shell profile file.</li>
        <li>
          Add an export line such as{" "}
          <code>{providerSetup?.example || 'export MINIMAX_API_KEY="your-key-here"'}</code>.
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
