import {
  Clock3,
  ChevronLeft,
  ChevronRight,
  Folder,
  Search,
  Settings,
  SquarePen,
} from "lucide-react";
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
      <button className="nav-action">
        <SquarePen aria-hidden="true" size={17} />
        <span>New Chat</span>
      </button>
      <button className="nav-action">
        <Search aria-hidden="true" size={17} />
        <span>Search</span>
      </button>
      <div className="sidebar-section">Projects</div>
      <div className="project-pill">
        <Folder aria-hidden="true" size={16} />
        <span>rust-agent</span>
      </div>
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
                <Clock3 aria-hidden="true" size={12} />
                {session.message_count} msgs · {session.model}
              </small>
            </button>
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
