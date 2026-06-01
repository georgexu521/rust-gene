import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Search, Plus, Settings, FolderOpen, Activity, Trash2 } from "lucide-react";

export type CommandGroup = "nav" | "action" | "workspace" | "settings";

export type Command = {
  id: string;
  label: string;
  hint?: string;
  icon: ReactNode;
  group: CommandGroup;
  run: () => void;
};

type CommandPaletteProps = {
  open: boolean;
  onClose: () => void;
  commands: Command[];
};

const GROUP_LABELS: Record<CommandGroup, string> = {
  nav: "Navigate",
  action: "Actions",
  workspace: "Workspace",
  settings: "Settings",
};

export function useCommandPalette() {
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      if (mod && (e.key === "k" || e.key === "K")) {
        e.preventDefault();
        setOpen((v) => !v);
      } else if (e.key === "Escape") {
        setOpen(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return { open, setOpen };
}

export function CommandPalette({ open, onClose, commands }: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const filtered = useMemo(() => {
    if (!query.trim()) return commands;
    const lower = query.toLowerCase();
    return commands.filter(
      (c) =>
        c.label.toLowerCase().includes(lower) ||
        c.id.toLowerCase().includes(lower) ||
        (c.hint && c.hint.toLowerCase().includes(lower)),
    );
  }, [commands, query]);

  // Group filtered commands
  const grouped = useMemo(() => {
    const map = new Map<CommandGroup, Command[]>();
    for (const cmd of filtered) {
      const list = map.get(cmd.group) || [];
      list.push(cmd);
      map.set(cmd.group, list);
    }
    return map;
  }, [filtered]);

  useEffect(() => {
    if (open) {
      setQuery("");
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [open]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  const flatFiltered = filtered;
  const selected = flatFiltered[selectedIndex];

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((i) => Math.min(i + 1, flatFiltered.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter" && selected) {
      e.preventDefault();
      selected.run();
      onClose();
    } else if (e.key === "Escape") {
      onClose();
    }
  }

  if (!open) return null;

  return (
    <div className="cmd-palette-backdrop" onClick={onClose} role="presentation">
      <div className="cmd-palette" role="dialog" aria-label="Command palette" onClick={(e) => e.stopPropagation()}>
        <div className="cmd-palette-input-wrap">
          <Search size={14} className="cmd-palette-search-icon" />
          <input
            ref={inputRef}
            className="cmd-palette-input"
            placeholder="Type a command..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
          />
        </div>
        <div className="cmd-palette-list">
          {Array.from(grouped.entries()).map(([group, cmds]) => (
            <div key={group} className="cmd-palette-group">
              <div className="cmd-palette-group-label">{GROUP_LABELS[group]}</div>
              {cmds.map((cmd) => (
                <button
                  key={cmd.id}
                  type="button"
                  className={`cmd-palette-item${cmd === selected ? " selected" : ""}`}
                  onClick={() => {
                    cmd.run();
                    onClose();
                  }}
                >
                  <span className="cmd-palette-item-icon">{cmd.icon}</span>
                  <span className="cmd-palette-item-label">{cmd.label}</span>
                  {cmd.hint ? <span className="cmd-palette-item-hint">{cmd.hint}</span> : null}
                </button>
              ))}
            </div>
          ))}
          {flatFiltered.length === 0 ? (
            <div className="cmd-palette-empty">No commands found</div>
          ) : null}
        </div>
      </div>
    </div>
  );
}
