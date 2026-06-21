import { useRef } from "react";
import { RecentSession } from "../../runtime/desktopApi";
import { useDrawerKeyboard } from "./useDrawerKeyboard";

type DeleteSessionDialogProps = {
  session: RecentSession;
  onCancel: () => void;
  onConfirm: () => void;
};

export function DeleteSessionDialog({
  session,
  onCancel,
  onConfirm,
}: DeleteSessionDialogProps) {
  const cancelButtonRef = useRef<HTMLButtonElement | null>(null);
  const dialogRef = useDrawerKeyboard<HTMLElement>({
    isOpen: true,
    onClose: onCancel,
    initialFocusRef: cancelButtonRef,
  });

  return (
    <div className="confirm-backdrop" role="presentation">
      <section
        aria-labelledby="delete-session-title"
        aria-modal="true"
        className="confirm-dialog"
        ref={dialogRef}
        role="dialog"
        tabIndex={-1}
      >
        <div>
          <h2 id="delete-session-title">Delete session?</h2>
          <p>
            {session.title} will be removed from this desktop app. This cannot
            be undone.
          </p>
        </div>
        <div className="confirm-dialog-meta">
          <span>{session.model}</span>
          <span>{session.message_count} messages</span>
        </div>
        <div className="confirm-dialog-actions">
          <button ref={cancelButtonRef} type="button" onClick={onCancel}>
            Cancel
          </button>
          <button className="danger" type="button" onClick={onConfirm}>
            Delete
          </button>
        </div>
      </section>
    </div>
  );
}
