import {
  Check,
  Clock3,
  ChevronLeft,
  ChevronRight,
  Archive,
  Trash2,
  Edit3,
  Folder,
  Search,
  Settings,
  SquarePen,
  X,
} from "lucide-react";
import { FormEvent, useState } from "react";
import { RecentSession } from "../../runtime/desktopApi";

type SidebarProps = {
  projectPath: string;
  recentProjects: string[];
  sessions: RecentSession[];
  sessionSearch: string;
  selectedSessionId: string | null;
  onArchiveSession: (session: RecentSession) => void;
  onBrowseProject: () => void;
  onDeleteSession: (session: RecentSession) => void;
  onNewChat: () => void;
  onRenameSession: (session: RecentSession, title: string) => void;
  onSearchChange: (query: string) => void;
  onSelectRecentProject: (path: string) => void;
  onLoadSession: (session: RecentSession) => void;
  onOpenSettings: () => void;
};

export function Sidebar({
  projectPath,
  recentProjects,
  sessions,
  sessionSearch,
  selectedSessionId,
  onArchiveSession,
  onBrowseProject,
  onDeleteSession,
  onNewChat,
  onRenameSession,
  onSearchChange,
  onSelectRecentProject,
  onLoadSession,
  onOpenSettings,
}: SidebarProps) {
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null);
  const [draftTitle, setDraftTitle] = useState("");
  const projectLabel = basename(projectPath) || "Select project";

  function beginRename(session: RecentSession) {
    setEditingSessionId(session.id);
    setDraftTitle(session.title);
  }

  function submitRename(event: FormEvent<HTMLFormElement>, session: RecentSession) {
    event.preventDefault();
    const title = draftTitle.trim();
    if (title && title !== session.title) {
      onRenameSession(session, title);
    }
    setEditingSessionId(null);
  }

  return (
    <aside className="sidebar">
      <div className="window-spacer">
        <div className="traffic-lights" aria-hidden="true">
          <span className="traffic-light close" />
          <span className="traffic-light minimize" />
          <span className="traffic-light zoom" />
        </div>
        <div className="history-controls" aria-hidden="true">
          <ChevronLeft size={16} />
          <ChevronRight size={16} />
        </div>
      </div>
      <button className="nav-action" type="button" onClick={onNewChat}>
        <SquarePen aria-hidden="true" size={17} />
        <span>New Chat</span>
      </button>
      <label className="sidebar-search">
        <Search aria-hidden="true" size={17} />
        <input
          aria-label="Search sessions"
          value={sessionSearch}
          onChange={(event) => onSearchChange(event.target.value)}
          placeholder="Search"
        />
      </label>
      <div className="sidebar-section">Projects</div>
      <button
        className="project-pill"
        type="button"
        title={projectPath}
        onClick={onBrowseProject}
      >
        <Folder aria-hidden="true" size={16} />
        <span>{projectLabel}</span>
      </button>
      <div className="project-list">
        {recentProjects
          .filter((path) => path !== projectPath)
          .slice(0, 3)
          .map((path) => (
            <button
              className="project-shortcut"
              key={path}
              title={path}
              type="button"
              onClick={() => onSelectRecentProject(path)}
            >
              <span>{basename(path)}</span>
            </button>
          ))}
      </div>
      <div className="sidebar-section sidebar-section-row">
        <span>Recent</span>
        <small>{sessions.length}</small>
      </div>
      <div className="recent-list">
        {sessions.length === 0 ? (
          <div className="recent-empty">
            {sessionSearch.trim() ? "No matching sessions" : "No saved sessions"}
          </div>
        ) : (
          sessions.map((session) => (
            <div
              className={`recent-item ${session.id === selectedSessionId ? "active" : ""}`}
              key={session.id}
            >
              {editingSessionId === session.id ? (
                <form className="recent-rename-form" onSubmit={(event) => submitRename(event, session)}>
                  <input
                    aria-label="Session name"
                    autoFocus
                    value={draftTitle}
                    onChange={(event) => setDraftTitle(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Escape") {
                        setEditingSessionId(null);
                      }
                    }}
                  />
                  <button aria-label="Save session title" type="submit">
                    <Check aria-hidden="true" size={13} />
                  </button>
                  <button
                    aria-label="Cancel rename"
                    type="button"
                    onClick={() => setEditingSessionId(null)}
                  >
                    <X aria-hidden="true" size={13} />
                  </button>
                </form>
              ) : (
                <>
                  <button
                    className="recent-item-main"
                    onClick={() => onLoadSession(session)}
                    type="button"
                  >
                    <span className="recent-title">{session.title}</span>
                    <small className="recent-meta">
                      <span>
                        <Clock3 aria-hidden="true" size={12} />
                        {session.message_count} msgs
                      </span>
                      <span>{session.model}</span>
                    </small>
                  </button>
                  <button
                    aria-label={`Rename ${session.title}`}
                    className="recent-icon-button"
                    type="button"
                    onClick={() => beginRename(session)}
                  >
                    <Edit3 aria-hidden="true" size={13} />
                  </button>
                  <button
                    aria-label={`Archive ${session.title}`}
                    className="recent-icon-button"
                    type="button"
                    onClick={() => onArchiveSession(session)}
                  >
                    <Archive aria-hidden="true" size={13} />
                  </button>
                  <button
                    aria-label={`Delete ${session.title}`}
                    className="recent-icon-button danger"
                    type="button"
                    onClick={() => onDeleteSession(session)}
                  >
                    <Trash2 aria-hidden="true" size={13} />
                  </button>
                </>
              )}
            </div>
          ))
        )}
      </div>
      <button className="sidebar-footer" type="button" onClick={onOpenSettings}>
        <Settings aria-hidden="true" size={17} />
        <span>Settings</span>
      </button>
    </aside>
  );
}

function basename(path: string) {
  return path.split(/[\\/]/).filter(Boolean).at(-1) || path;
}
