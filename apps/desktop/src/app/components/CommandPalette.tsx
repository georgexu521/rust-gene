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
  const paletteRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const previousActiveRef = useRef<HTMLElement | null>(null);

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
  const flatFiltered = filtered;
  const selected = flatFiltered[selectedIndex];
  const selectedOptionId = selected ? commandOptionId(selected.id) : undefined;

  useEffect(() => {
    if (open) {
      previousActiveRef.current =
        document.activeElement instanceof HTMLElement ? document.activeElement : null;
      setQuery("");
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 50);
    }

    return () => {
      if (!open) {
        return;
      }
      const previous = previousActiveRef.current;
      const activeElement =
        document.activeElement instanceof HTMLElement ? document.activeElement : null;
      const shouldRestoreFocus =
        !activeElement ||
        activeElement === document.body ||
        !activeElement.isConnected ||
        Boolean(paletteRef.current?.contains(activeElement));
      if (shouldRestoreFocus && previous?.isConnected) {
        previous.focus();
      }
    };
  }, [open]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  useEffect(() => {
    if (!open) {
      return;
    }
    if (selectedIndex >= flatFiltered.length) {
      setSelectedIndex(Math.max(0, flatFiltered.length - 1));
    }
  }, [flatFiltered.length, open, selectedIndex]);

  useEffect(() => {
    if (!open || !selected) {
      return;
    }
    document
      .getElementById(commandOptionId(selected.id))
      ?.scrollIntoView({ block: "nearest" });
  }, [open, selected]);

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Tab") {
      trapFocus(e);
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      if (flatFiltered.length > 0) {
        setSelectedIndex((i) => (i + 1) % flatFiltered.length);
      }
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      if (flatFiltered.length > 0) {
        setSelectedIndex((i) => (i - 1 + flatFiltered.length) % flatFiltered.length);
      }
    } else if (e.key === "Home") {
      e.preventDefault();
      setSelectedIndex(0);
    } else if (e.key === "End") {
      e.preventDefault();
      setSelectedIndex(Math.max(0, flatFiltered.length - 1));
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
      <div
        ref={paletteRef}
        className="cmd-palette"
        role="dialog"
        aria-label="Command palette"
        aria-modal="true"
        onKeyDown={handleKeyDown}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="cmd-palette-input-wrap">
          <Search size={14} className="cmd-palette-search-icon" />
          <input
            ref={inputRef}
            aria-activedescendant={selectedOptionId}
            aria-autocomplete="list"
            aria-controls="command-palette-list"
            aria-expanded="true"
            aria-label="Command search"
            className="cmd-palette-input"
            placeholder="Type a command..."
            role="combobox"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        <div
          id="command-palette-list"
          className="cmd-palette-list"
          role="listbox"
          aria-label="Command results"
        >
          {Array.from(grouped.entries()).map(([group, cmds]) => (
            <div key={group} className="cmd-palette-group">
              <div className="cmd-palette-group-label">{GROUP_LABELS[group]}</div>
              {cmds.map((cmd) => (
                <button
                  id={commandOptionId(cmd.id)}
                  key={cmd.id}
                  type="button"
                  aria-selected={cmd === selected}
                  className={`cmd-palette-item${cmd === selected ? " selected" : ""}`}
                  onClick={() => {
                    cmd.run();
                    onClose();
                  }}
                  role="option"
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

  function trapFocus(event: React.KeyboardEvent) {
    const focusable = paletteRef.current
      ? Array.from(
          paletteRef.current.querySelectorAll<HTMLElement>(
            'button:not([disabled]), input:not([disabled]), [href], [tabindex]:not([tabindex="-1"])',
          ),
        ).filter((element) => !element.hasAttribute("disabled") && element.tabIndex >= 0)
      : [];
    if (focusable.length === 0) {
      event.preventDefault();
      return;
    }

    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    const active = document.activeElement;
    if (event.shiftKey) {
      if (active === first || !paletteRef.current?.contains(active)) {
        event.preventDefault();
        last.focus();
      }
      return;
    }

    if (active === last || !paletteRef.current?.contains(active)) {
      event.preventDefault();
      first.focus();
    }
  }
}

function commandOptionId(id: string) {
  return `command-option-${id.replace(/[^a-zA-Z0-9_-]/g, "-")}`;
}
