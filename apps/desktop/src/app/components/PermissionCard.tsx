import { PermissionRequest } from "../types";

type PermissionCardProps = {
  request: PermissionRequest | null;
  onAnswer: (approved: boolean) => void;
};

export function PermissionCard({ request, onAnswer }: PermissionCardProps) {
  if (!request) {
    return null;
  }
  const summary = permissionSummary(request);
  const command = commandFromArguments(request.arguments);
  const mutation = isMutationRequest(request);

  return (
    <section className="permission-card">
      <div>
        <div className="permission-title">
          {mutation ? "Workspace change needs approval" : "Permission needed"}: {request.tool_name}
        </div>
        {command ? <code className="permission-command">{command}</code> : null}
        <div className="permission-prompt">{request.prompt}</div>
        {summary.length > 0 ? (
          <div className="permission-evidence">
            {summary.map((item) => (
              <span key={item}>{item}</span>
            ))}
          </div>
        ) : null}
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

function permissionSummary(request: PermissionRequest) {
  const evidence = isRecord(request.metadata) && isRecord(request.metadata.permission_evidence)
    ? request.metadata.permission_evidence
    : null;
  const review = isRecord(request.review) ? request.review : null;
  return [
    stringTag("risk", stringField(evidence, "risk_level") || stringField(review, "risk")),
    stringTag("kind", stringField(evidence, "request_kind")),
    stringTag("family", stringField(evidence, "permission_family")),
    firstReason(evidence?.reasons) || stringField(review, "reason"),
  ].filter((item): item is string => Boolean(item));
}

function commandFromArguments(value: unknown) {
  if (!isRecord(value)) {
    return undefined;
  }
  const command = value.command;
  return typeof command === "string" && command.trim() ? command : undefined;
}

function isMutationRequest(request: PermissionRequest) {
  const evidence = isRecord(request.metadata) && isRecord(request.metadata.permission_evidence)
    ? request.metadata.permission_evidence
    : null;
  const classification = isRecord(evidence?.command_classification)
    ? evidence.command_classification
    : null;
  return classification?.mutation === true;
}

function stringTag(label: string, value: string | undefined) {
  return value ? `${label} ${value.replaceAll("_", " ")}` : undefined;
}

function firstReason(value: unknown) {
  if (typeof value === "string") {
    return value;
  }
  if (!Array.isArray(value)) {
    return undefined;
  }
  return value.find((item): item is string => typeof item === "string");
}

function stringField(value: Record<string, unknown> | null, key: string) {
  const field = value?.[key];
  return typeof field === "string" ? field : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
