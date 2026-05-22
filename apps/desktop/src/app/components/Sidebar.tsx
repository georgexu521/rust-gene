import { RecentSession } from "../../runtime/desktopApi";

type SidebarProps = {
  sessions: RecentSession[];
  selectedSessionId: string | null;
  onLoadSession: (session: RecentSession) => void;
  onOpenSettings: () => void;
};

export function Sidebar({
  sessions,
  selectedSessionId,
  onLoadSession,
  onOpenSettings,
}: SidebarProps) {
  return (
    <aside className="sidebar">
      <div className="window-spacer" />
      <button className="nav-action">New Chat</button>
      <button className="nav-action">Search</button>
      <div className="sidebar-section">Projects</div>
      <div className="project-pill">rust-agent</div>
      <div className="sidebar-section">Recent</div>
      <div className="recent-list">
        {sessions.length === 0 ? (
          <div className="recent-empty">No saved sessions</div>
        ) : (
          sessions.map((session) => (
            <button
              className={`recent-item ${session.id === selectedSessionId ? "active" : ""}`}
              key={session.id}
              onClick={() => onLoadSession(session)}
              type="button"
            >
              <span>{session.title}</span>
              <small>
                {session.message_count} msgs · {session.model}
              </small>
            </button>
          ))
        )}
      </div>
      <button className="sidebar-footer" type="button" onClick={onOpenSettings}>
        Settings
      </button>
    </aside>
  );
}
