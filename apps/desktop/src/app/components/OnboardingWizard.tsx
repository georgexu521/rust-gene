import { useMemo, useState } from "react";
import {
  CheckCircle2,
  ChevronLeft,
  ChevronRight,
  FolderOpen,
  KeyRound,
  ShieldCheck,
  Terminal,
} from "lucide-react";
import type {
  DesktopOnboardingInput,
  DesktopSettings,
  DesktopWorkspaceTrustInput,
  PermissionModeId,
  PermissionModeOption,
  ProviderModelStatus,
} from "../../runtime/desktopApi";

type OnboardingWizardProps = {
  settings: DesktopSettings;
  permissionOptions: PermissionModeOption[];
  providerStatus: ProviderModelStatus | null;
  projectPath: string;
  onBrowseProject: () => Promise<string | null>;
  onComplete: (input: DesktopOnboardingInput) => Promise<void>;
  onSkip: () => Promise<void>;
};

const steps = ["Project", "Provider", "Credentials", "Permissions", "Trust", "Start"];

export function OnboardingWizard({
  settings,
  permissionOptions,
  providerStatus,
  projectPath,
  onBrowseProject,
  onComplete,
  onSkip,
}: OnboardingWizardProps) {
  const [step, setStep] = useState(0);
  const [projectDraft, setProjectDraft] = useState(projectPath || settings.selected_project);
  const [permissionMode, setPermissionMode] = useState<PermissionModeId>(
    settings.permission_mode || "auto_low_risk",
  );
  const [credentialAck, setCredentialAck] = useState(false);
  const [developerAutoAck, setDeveloperAutoAck] = useState(false);
  const [packageScripts, setPackageScripts] =
    useState<DesktopWorkspaceTrustInput["package_scripts"]>("ask");
  const [shellValidation, setShellValidation] =
    useState<DesktopWorkspaceTrustInput["shell_validation"]>("ask");
  const [labDaemon, setLabDaemon] = useState(false);
  const [startingMode, setStartingMode] = useState<"direct" | "labrun">("direct");
  const [busy, setBusy] = useState(false);
  const providerReady = providerStatus?.runtime_provider_ready === true;
  const developerAutoBlocked = permissionMode === "auto" && !developerAutoAck;
  const canComplete = Boolean(projectDraft.trim()) && !developerAutoBlocked;
  const activeStep = steps[step] || steps[0];

  const trustInput = useMemo<DesktopWorkspaceTrustInput>(
    () => ({
      package_scripts: packageScripts,
      shell_validation: shellValidation,
      lab_daemon_supervision: labDaemon,
      developer_auto_acknowledged: developerAutoAck,
    }),
    [developerAutoAck, labDaemon, packageScripts, shellValidation],
  );

  async function handleBrowse() {
    const selected = await onBrowseProject();
    if (selected) {
      setProjectDraft(selected);
    }
  }

  async function handleComplete(skipped = false) {
    if (busy || (!skipped && !canComplete)) {
      return;
    }
    setBusy(true);
    try {
      await onComplete({
        project_root: projectDraft,
        permission_mode: skipped ? "auto_low_risk" : permissionMode,
        workspace_trust: skipped
          ? {
              package_scripts: "ask",
              shell_validation: "ask",
              lab_daemon_supervision: false,
              developer_auto_acknowledged: false,
            }
          : trustInput,
        credential_storage_acknowledged: skipped ? false : credentialAck,
        starting_mode: skipped ? "direct" : startingMode,
        skipped,
      });
    } finally {
      setBusy(false);
    }
  }

  async function handleSkip() {
    if (busy) {
      return;
    }
    setBusy(true);
    try {
      await onSkip();
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="onboarding-backdrop" role="presentation">
      <section className="onboarding-wizard" role="dialog" aria-modal="true" aria-label="First-run setup">
        <header className="onboarding-header">
          <div>
            <div className="settings-eyebrow">Desktop setup</div>
            <h2>{activeStep}</h2>
          </div>
          <div className="onboarding-stepper" aria-label="Setup progress">
            {steps.map((label, index) => (
              <button
                className={index === step ? "active" : index < step ? "done" : ""}
                key={label}
                type="button"
                onClick={() => setStep(index)}
              >
                {index < step ? <CheckCircle2 aria-hidden="true" size={13} /> : index + 1}
                <span>{label}</span>
              </button>
            ))}
          </div>
        </header>

        <div className="onboarding-body">
          {step === 0 ? (
            <section className="onboarding-panel">
              <label>
                <span>Project folder</span>
                <input
                  value={projectDraft}
                  onChange={(event) => setProjectDraft(event.target.value)}
                  aria-label="Onboarding project folder"
                />
              </label>
              <button type="button" onClick={() => void handleBrowse()}>
                <FolderOpen aria-hidden="true" size={15} />
                Browse
              </button>
              <dl className="onboarding-kv">
                <div>
                  <dt>Current repository</dt>
                  <dd>{settings.workspace_trust.repo_identity || "unknown"}</dd>
                </div>
                <div>
                  <dt>Trust source</dt>
                  <dd>{settings.workspace_trust.trust_source}</dd>
                </div>
              </dl>
            </section>
          ) : null}

          {step === 1 ? (
            <section className="onboarding-panel">
              <div className={`onboarding-status ${providerReady ? "ok" : "warning"}`}>
                <Terminal aria-hidden="true" size={18} />
                <span>
                  <strong>{providerReady ? "Provider ready" : "Provider not configured"}</strong>
                  <small>
                    {providerStatus?.active_provider_label || providerStatus?.active_provider || "No provider"} /{" "}
                    {providerStatus?.active_model || settings.model || "provider default"}
                  </small>
                </span>
              </div>
              <p className="settings-copy">
                You can skip provider setup and still inspect local project state.
              </p>
            </section>
          ) : null}

          {step === 2 ? (
            <section className="onboarding-panel">
              <div className="onboarding-status warning">
                <KeyRound aria-hidden="true" size={18} />
                <span>
                  <strong>{settings.credential_storage.active_store}</strong>
                  <small>{settings.credential_storage.detail}</small>
                </span>
              </div>
              <label className="settings-toggle-row">
                <input
                  type="checkbox"
                  checked={credentialAck}
                  onChange={(event) => setCredentialAck(event.target.checked)}
                />
                <span>I understand saved desktop keys use the local dotenv fallback in this build.</span>
              </label>
            </section>
          ) : null}

          {step === 3 ? (
            <section className="onboarding-panel">
              <div className="permission-options onboarding-permission-options">
                {permissionOptions.map((option) => (
                  <button
                    className={`permission-option ${permissionMode === option.id ? "active" : ""}`}
                    key={option.id}
                    type="button"
                    onClick={() => setPermissionMode(option.id)}
                  >
                    <span>
                      <strong>{option.label}</strong>
                      <small>{option.description}</small>
                    </span>
                    <i aria-hidden="true" />
                  </button>
                ))}
              </div>
              {permissionMode === "auto" ? (
                <label className="settings-toggle-row">
                  <input
                    type="checkbox"
                    checked={developerAutoAck}
                    onChange={(event) => setDeveloperAutoAck(event.target.checked)}
                  />
                  <span>I explicitly trust this project before using Developer Auto.</span>
                </label>
              ) : null}
            </section>
          ) : null}

          {step === 4 ? (
            <section className="onboarding-panel">
              <TrustChoice
                label="Package-script validation"
                value={packageScripts}
                onChange={setPackageScripts}
              />
              <TrustChoice
                label="Shell validation"
                value={shellValidation}
                onChange={setShellValidation}
              />
              <label className="settings-toggle-row">
                <input
                  type="checkbox"
                  checked={labDaemon}
                  onChange={(event) => setLabDaemon(event.target.checked)}
                />
                <span>Allow Lab daemon supervision while the desktop app is open.</span>
              </label>
            </section>
          ) : null}

          {step === 5 ? (
            <section className="onboarding-panel">
              <div className="onboarding-start-options">
                <button
                  className={startingMode === "direct" ? "active" : ""}
                  type="button"
                  onClick={() => setStartingMode("direct")}
                >
                  <ShieldCheck aria-hidden="true" size={18} />
                  <span>
                    <strong>Direct task</strong>
                    <small>Start in the normal coding agent surface.</small>
                  </span>
                </button>
                <button
                  className={startingMode === "labrun" ? "active" : ""}
                  type="button"
                  onClick={() => setStartingMode("labrun")}
                >
                  <ShieldCheck aria-hidden="true" size={18} />
                  <span>
                    <strong>LabRun</strong>
                    <small>Open the long-running project governance surface.</small>
                  </span>
                </button>
              </div>
            </section>
          ) : null}
        </div>

        {developerAutoBlocked ? (
          <div className="onboarding-warning" role="alert">
            Developer Auto requires explicit project trust acknowledgement.
          </div>
        ) : null}

        <footer className="onboarding-actions">
          <button type="button" onClick={() => void handleSkip()} disabled={busy}>
            Skip setup
          </button>
          <div>
            <button type="button" onClick={() => setStep(Math.max(0, step - 1))} disabled={step === 0 || busy}>
              <ChevronLeft aria-hidden="true" size={14} />
              Back
            </button>
            {step < steps.length - 1 ? (
              <button type="button" onClick={() => setStep(Math.min(steps.length - 1, step + 1))} disabled={busy}>
                Next
                <ChevronRight aria-hidden="true" size={14} />
              </button>
            ) : (
              <button type="button" onClick={() => void handleComplete(false)} disabled={!canComplete || busy}>
                Start
              </button>
            )}
          </div>
        </footer>
      </section>
    </div>
  );
}

function TrustChoice({
  label,
  value,
  onChange,
}: {
  label: string;
  value: "ask" | "trusted";
  onChange: (value: "ask" | "trusted") => void;
}) {
  return (
    <div className="trust-choice-row">
      <span>{label}</span>
      <div role="group" aria-label={label}>
        <button className={value === "ask" ? "active" : ""} type="button" onClick={() => onChange("ask")}>
          Ask
        </button>
        <button className={value === "trusted" ? "active" : ""} type="button" onClick={() => onChange("trusted")}>
          Trusted
        </button>
      </div>
    </div>
  );
}
