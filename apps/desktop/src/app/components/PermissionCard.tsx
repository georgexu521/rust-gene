import { PermissionRequest } from "../types";

type PermissionCardProps = {
  request: PermissionRequest | null;
  onAnswer: (approved: boolean) => void;
};

export function PermissionCard({ request, onAnswer }: PermissionCardProps) {
  if (!request) {
    return null;
  }

  return (
    <section className="permission-card">
      <div>
        <div className="permission-title">Permission needed: {request.tool_name}</div>
        <div className="permission-prompt">{request.prompt}</div>
      </div>
      <div className="permission-actions">
        <button type="button" onClick={() => onAnswer(false)}>
          Reject
        </button>
        <button type="button" onClick={() => onAnswer(true)}>
          Approve
        </button>
      </div>
    </section>
  );
}

