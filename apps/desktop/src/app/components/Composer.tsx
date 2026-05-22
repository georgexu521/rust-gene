import { FormEvent } from "react";

type ComposerProps = {
  composer: string;
  projectPath: string;
  isRunning: boolean;
  onComposerChange: (value: string) => void;
  onProjectPathChange: (value: string) => void;
  onBrowseProject: () => void;
  onSelectProject: () => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
};

export function Composer({
  composer,
  projectPath,
  isRunning,
  onComposerChange,
  onProjectPathChange,
  onBrowseProject,
  onSelectProject,
  onSubmit,
}: ComposerProps) {
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
    </form>
  );
}

