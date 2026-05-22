import { FormEvent } from "react";
import { ProviderModelStatus } from "../../runtime/desktopApi";

type ComposerProps = {
  composer: string;
  projectPath: string;
  providerStatus: ProviderModelStatus | null;
  isRunning: boolean;
  onComposerChange: (value: string) => void;
  onProjectPathChange: (value: string) => void;
  onBrowseProject: () => void;
  onSelectProject: () => void;
  onProviderModelChange: (providerId: string, model: string) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
};

export function Composer({
  composer,
  projectPath,
  providerStatus,
  isRunning,
  onComposerChange,
  onProjectPathChange,
  onBrowseProject,
  onSelectProject,
  onProviderModelChange,
  onSubmit,
}: ComposerProps) {
  const activeProvider = providerStatus?.active_provider || "";
  const activeModel = providerStatus?.active_model || "";

  return (
    <form className="composer" onSubmit={onSubmit}>
      <textarea
        aria-label="Message"
        value={composer}
        onChange={(event) => onComposerChange(event.target.value)}
        placeholder="Ask Liz to inspect, edit, or verify this project..."
      />
      <div className="composer-controls">
        <input
          aria-label="Project path"
          value={projectPath}
          onChange={(event) => onProjectPathChange(event.target.value)}
        />
        <button type="button" onClick={onSelectProject}>
          Select
        </button>
        <button type="button" onClick={onBrowseProject}>
          Browse
        </button>
        <button disabled={isRunning || composer.trim().length === 0} type="submit">
          {isRunning ? "Running" : "Send"}
        </button>
      </div>
      <div className="runtime-selectors" aria-label="Runtime selectors">
        <label>
          <span>Provider</span>
          <select
            aria-label="Provider"
            value={activeProvider}
            onChange={(event) => {
              const provider = providerStatus?.providers.find(
                (option) => option.id === event.target.value,
              );
              if (!provider || !provider.configured) {
                return;
              }
              onProviderModelChange(provider.id, provider.model);
            }}
          >
            <option value="" disabled>
              No provider
            </option>
            {providerStatus?.providers.map((provider) => (
              <option
                disabled={!provider.configured}
                key={provider.id}
                value={provider.id}
              >
                {provider.label} {provider.configured ? "" : `(${provider.note})`}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>Model</span>
          <select
            aria-label="Model"
            disabled={!activeProvider || providerStatus?.models.length === 0}
            value={activeModel}
            onChange={(event) => {
              if (!activeProvider) {
                return;
              }
              onProviderModelChange(activeProvider, event.target.value);
            }}
          >
            <option value="" disabled>
              No model
            </option>
            {providerStatus?.models.map((model) => (
              <option key={model.id} value={model.id}>
                {model.label}
              </option>
            ))}
          </select>
        </label>
        <div className="runtime-status">
          {providerStatus
            ? `${providerStatus.configured_count} configured · ${activeModel}`
            : "Checking provider"}
        </div>
      </div>
    </form>
  );
}
