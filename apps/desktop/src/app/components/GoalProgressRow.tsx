import { Target, Pause, Play, X, Edit3 } from "lucide-react";
import { DesktopGoalStatus } from "../../runtime/desktopApi";
import { useState } from "react";

export type GoalProgressRowProps = {
  goal: DesktopGoalStatus | null;
  onPause: () => void;
  onResume: () => void;
  onClear: () => void;
  onEdit: (objective: string) => void;
};

export function GoalProgressRow({ goal, onPause, onResume, onClear, onEdit }: GoalProgressRowProps) {
  const [editing, setEditing] = useState(false);
  const [editText, setEditText] = useState("");

  if (!goal || !goal.goal_id) return null;

  const isActive = goal.status === "Active";
  const isPaused = goal.status === "Paused";
  const isTerminal = goal.status === "Completed" || goal.status === "Failed"
    || goal.status === "Blocked" || goal.status === "Cancelled"
    || goal.status === "NeedsUser";

  const turnLabel = goal.turn_count != null && goal.max_turns != null
    ? `${goal.turn_count}/${goal.max_turns}`
    : goal.turn_count != null ? `${goal.turn_count}` : "-";

  const handleEditSubmit = () => {
    if (editText.trim()) {
      onEdit(editText.trim());
    }
    setEditing(false);
  };

  const cancelEdit = () => {
    setEditText(goal.objective ?? "");
    setEditing(false);
  };

  return (
    <div className="goal-progress-row">
      <span className="goal-progress-seg" title={goal.objective ?? ""}>
        <Target size={13} />
        <span className="goal-progress-label">Goal</span>
        <span className="goal-progress-val">{goal.objective ?? "-"}</span>
      </span>

      <span className="goal-progress-seg">
        <span className="goal-progress-label">Status</span>
        <span className={`goal-progress-val goal-status-${(goal.status ?? "").toLowerCase()}`}>
          {goal.status ?? "-"}
        </span>
      </span>

      <span className="goal-progress-seg">
        <span className="goal-progress-label">Turns</span>
        <span className="goal-progress-val">{turnLabel}</span>
      </span>

      {goal.last_decision ? (
        <span className="goal-progress-seg">
          <span className="goal-progress-label">Last</span>
          <span className="goal-progress-val">{goal.last_decision}</span>
        </span>
      ) : null}

      {goal.last_proof ? (
        <span className="goal-progress-seg">
          <span className="goal-progress-label">Proof</span>
          <span className="goal-progress-val">{goal.last_proof}</span>
        </span>
      ) : null}

      <span className="goal-progress-grow" />

      {editing ? (
        <span className="goal-progress-edit">
          <input
            aria-label="Goal objective"
            className="goal-edit-input"
            value={editText}
            onChange={(e) => setEditText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                handleEditSubmit();
              }
              if (e.key === "Escape") {
                e.preventDefault();
                cancelEdit();
              }
            }}
            onBlur={handleEditSubmit}
            autoFocus
            placeholder="New objective..."
          />
        </span>
      ) : (
        <span className="goal-progress-actions">
          <button
            aria-label="Edit goal objective"
            className="goal-btn"
            title="Edit objective"
            onClick={() => { setEditText(goal.objective ?? ""); setEditing(true); }}
            disabled={isTerminal}
          >
            <Edit3 size={12} />
          </button>
          {isActive ? (
            <button aria-label="Pause goal" className="goal-btn" title="Pause" onClick={onPause}>
              <Pause size={12} />
            </button>
          ) : isPaused ? (
            <button aria-label="Resume goal" className="goal-btn" title="Resume" onClick={onResume}>
              <Play size={12} />
            </button>
          ) : null}
          <button aria-label="Clear goal" className="goal-btn" title="Clear" onClick={onClear}>
            <X size={12} />
          </button>
        </span>
      )}
    </div>
  );
}
