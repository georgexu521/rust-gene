import { PermissionRequest, TimelineStatus, TimelineSummary, TraceItem } from "./types";

export function traceTool(
  id: string,
  title: string,
  detail?: string,
  extras: Partial<Pick<TraceItem, "facts" | "status" | "summary">> = {},
): TraceItem {
  return {
    id,
    kind: "tool",
    title,
    detail,
    ...extras,
  };
}

export type ToolPresentation = {
  title: string;
  detail?: string;
  facts?: string[];
  summary?: TimelineSummary;
  status: "completed" | "failed";
};

export type PermissionEvidenceSummary = {
  permission_family?: string;
  request_kind?: string;
  risk_level?: string;
  decision?: string;
  reasons?: unknown;
  reason?: string;
  recovery?: unknown;
  command_classification?: unknown;
  allowed_always_rules?: unknown;
};

export type PermissionReviewSummary = {
  risk?: string;
  reason?: string;
  recovery_hint?: string;
  risk_facts?: unknown;
  matched_rules?: unknown;
  classifier_result?: unknown;
};

export type ActionReviewDesktopSummary = {
  decision?: string;
  reason?: string;
  permission?: string;
  risk?: string;
  recovery?: string;
  sideEffect?: string;
  network?: string;
  checkpoint?: string;
  checkpointApproval?: boolean;
  scopeAllowed?: boolean;
  budgetAllowed?: boolean;
};

export type ToolSummary = {
  tool?: string;
  success?: boolean;
  duration_ms?: number;
  command?: string;
  command_category?: string;
  validation_family?: string;
  command_kind?: string;
  path?: string;
  pattern?: string;
  action?: string;
  replacements?: number;
  operations?: number;
  additions?: number;
  deletions?: number;
  diff_preview?: string;
  diff_preview_truncated?: boolean;
  checkpoint_id?: string;
  rollback_id?: string;
  failure_kind?: string;
  guardrail_reason?: string;
  diagnostics_delta_status?: string;
  diagnostics_delta_diagnostic_count?: number;
  diagnostics_delta_error_count?: number;
  diagnostics_delta_warning_count?: number;
  line_start?: number;
  line_end?: number;
  read_coverage?: string;
  output_chars?: number;
  terminal_task?: Record<string, unknown>;
  terminal_tasks_count?: number;
  error_preview?: string;
  recovery_action?: string;
  user_note?: string;
};

export function presentToolCompletion(resultPreview: string, metadata: unknown): ToolPresentation {
  const summary = toolSummary(metadata);
  if (!summary) {
    return {
      title: "Tool completed",
      detail: resultPreview,
      status: "completed",
    };
  }

  const status = summary.success === false ? "failed" : "completed";
  const title = toolTitle(summary);
  const detail = toolDetail(summary, resultPreview);
  const facts = toolFacts(summary);
  const specialSummary = timelineSummary(summary, resultPreview);

  return {
    title,
    detail,
    facts,
    summary: specialSummary,
    status,
  };
}

export function permissionTimelineSummary(
  event: PermissionRequest,
): Extract<TimelineSummary, { kind: "permission" }> | undefined {
  const evidence = permissionEvidence(event.metadata);
  const review = permissionReview(event.review);
  const actionReview = actionReviewSummary(event.metadata);
  const command = commandClassification(evidence?.command_classification || review?.classifier_result);
  if (!evidence && !review && !actionReview && !command) {
    return undefined;
  }
  return {
    kind: "permission",
    family: evidence?.permission_family,
    requestKind: evidence?.request_kind,
    risk: actionReview?.risk || evidence?.risk_level || review?.risk,
    decision: evidence?.decision,
    reason:
      actionReview?.reason || firstString(evidence?.reasons) || evidence?.reason || review?.reason,
    recovery:
      actionReview?.recovery ||
      recoveryAction(evidence?.recovery) ||
      review?.recovery_hint ||
      "Approve only if the request matches the intended task.",
    commandCategory: stringField(command, "category"),
    parserStatus: stringField(command, "parser_status"),
    mutation: booleanField(command, "mutation"),
    actionDecision: actionReview?.decision,
    actionReason: actionReview?.reason,
    sideEffect: actionReview?.sideEffect,
    network: actionReview?.network,
    checkpoint: actionReview?.checkpoint,
    checkpointApproval: actionReview?.checkpointApproval,
    allowedRule: firstString(evidence?.allowed_always_rules),
    scopeAllowed: actionReview?.scopeAllowed,
    budgetAllowed: actionReview?.budgetAllowed,
  };
}

export function permissionTraceDetail(
  event: PermissionRequest,
  summary: Extract<TimelineSummary, { kind: "permission" }> | undefined,
) {
  const parts = compactFacts([
    event.prompt,
    summary?.actionDecision ? `review ${summary.actionDecision}` : null,
    summary?.actionReason ? `reason ${summary.actionReason}` : null,
    summary?.risk ? `risk ${summary.risk}` : null,
    summary?.requestKind ? `kind ${summary.requestKind}` : null,
    summary?.sideEffect ? `effect ${summary.sideEffect}` : null,
    summary?.network ? `network ${summary.network}` : null,
    summary?.checkpoint ? `checkpoint ${summary.checkpoint}` : null,
    summary?.commandCategory ? `command ${summary.commandCategory}` : null,
    summary?.recovery,
  ]);
  return parts.join(" · ");
}

export function permissionEvidence(metadata: unknown): PermissionEvidenceSummary | null {
  if (!isRecord(metadata)) {
    return null;
  }
  const evidence = metadata.permission_evidence;
  return isRecord(evidence) ? (evidence as PermissionEvidenceSummary) : null;
}

export function permissionReview(review: unknown): PermissionReviewSummary | null {
  return isRecord(review) ? (review as PermissionReviewSummary) : null;
}

export function actionReviewSummary(metadata: unknown): ActionReviewDesktopSummary | null {
  if (!isRecord(metadata)) {
    return null;
  }
  const review = recordField(metadata, "action_review");
  if (!review) {
    return null;
  }
  const permission = recordField(review, "permission");
  const sideEffects = recordField(review, "side_effects");
  const network = recordField(sideEffects, "network");
  const checkpoint = recordField(review, "checkpoint");
  const scope = recordField(review, "scope");
  const budget = recordField(review, "budget");
  return {
    decision: stringField(review, "decision"),
    reason: stringField(review, "primary_reason"),
    permission: stringField(permission, "decision"),
    risk: stringField(permission, "risk_level"),
    recovery: stringField(review, "model_recovery") || stringField(review, "user_reason"),
    sideEffect: stringField(sideEffects, "external_side_effect"),
    network: stringField(network, "class"),
    checkpoint: stringField(checkpoint, "status"),
    checkpointApproval: booleanField(checkpoint, "requires_user_approval"),
    scopeAllowed: booleanField(scope, "allowed"),
    budgetAllowed: booleanField(budget, "allowed"),
  };
}

export function commandClassification(value: unknown): Record<string, unknown> | null {
  return isRecord(value) ? value : null;
}

export function recoveryAction(value: unknown) {
  if (!isRecord(value)) {
    return undefined;
  }
  return stringField(value, "recommended_action") || stringField(value, "action");
}

export function firstString(value: unknown) {
  if (typeof value === "string") {
    return value;
  }
  if (!Array.isArray(value)) {
    return undefined;
  }
  return value.find((item): item is string => typeof item === "string");
}

export function toolSummary(metadata: unknown): ToolSummary | null {
  if (!isRecord(metadata)) {
    return null;
  }
  return metadata as ToolSummary;
}

export function toolTitle(summary: ToolSummary): string {
  switch (summary.tool) {
    case "bash":
      return summary.validation_family
        ? validationLabel(summary.validation_family)
        : "Shell command";
    case "file_edit":
      return "Edited file";
    case "file_write":
      return "Wrote file";
    case "file_read":
      return "Read file";
    case "file_patch":
      return "Patched files";
    case "grep":
      return "Searched project";
    case "git":
      return summary.action ? `Git ${summary.action}` : "Git";
    default:
      return summary.tool || "Tool completed";
  }
}

export function toolDetail(summary: ToolSummary, resultPreview: string): string | undefined {
  if (summary.error_preview) {
    return summary.error_preview;
  }
  if (summary.command) {
    return summary.command;
  }
  if (summary.path) {
    return summary.path;
  }
  if (summary.pattern) {
    return summary.pattern;
  }
  return resultPreview || undefined;
}

export function toolFacts(summary: ToolSummary): string[] {
  const facts = compactFacts([
    summary.tool ? `tool ${summary.tool}` : null,
    summary.validation_family ? `validation ${summary.validation_family}` : null,
    summary.command_category ? `category ${summary.command_category}` : null,
    summary.command_kind ? `kind ${summary.command_kind}` : null,
    summary.path ? `path ${summary.path}` : null,
    summary.replacements !== undefined ? `${summary.replacements} replacements` : null,
    summary.operations !== undefined ? `${summary.operations} operations` : null,
    summary.action ? `action ${summary.action}` : null,
    durationFact(summary.duration_ms),
    terminalFact(summary.terminal_task),
    summary.terminal_tasks_count ? `${summary.terminal_tasks_count} terminal tasks` : null,
    summary.output_chars !== undefined ? `${summary.output_chars} chars` : null,
  ]);

  return facts.slice(0, 6);
}

export function timelineSummary(summary: ToolSummary, resultPreview: string): TimelineSummary | undefined {
  if (summary.success === false) {
    return {
      kind: "failure",
      reason: summary.guardrail_reason || summary.error_preview || resultPreview || "Tool failed",
      recovery: summary.user_note || summary.recovery_action,
      outputPreview: resultPreview,
      outputTruncated: resultPreview.length >= 2000,
    };
  }

  if (summary.tool === "bash" && summary.command) {
    return {
      kind: "shell",
      command: summary.command,
      validation: summary.validation_family
        ? validationLabel(summary.validation_family)
        : summary.command_category,
      exitCode: terminalExitCode(summary.terminal_task),
      duration: durationFact(summary.duration_ms) || undefined,
    };
  }

  if (isFileTool(summary.tool)) {
    return {
      kind: "file",
      action: fileAction(summary.tool),
      path: summary.path,
      operations: summary.operations,
      replacements: summary.replacements,
      additions: summary.additions,
      deletions: summary.deletions,
      diffPreview: summary.diff_preview,
      diffTruncated: summary.diff_preview_truncated,
      rollbackId: summary.rollback_id || summary.checkpoint_id,
      diagnosticsDelta: summary.diagnostics_delta_status,
      diagnosticsErrorDelta: summary.diagnostics_delta_error_count,
      diagnosticsWarningDelta: summary.diagnostics_delta_warning_count,
      lineStart: summary.line_start,
      lineEnd: summary.line_end,
      readCoverage: summary.read_coverage,
    };
  }

  return undefined;
}

export function isFileTool(tool: string | undefined): tool is "file_read" | "file_write" | "file_edit" | "file_patch" {
  return tool === "file_read" || tool === "file_write" || tool === "file_edit" || tool === "file_patch";
}

export function fileAction(tool: "file_read" | "file_write" | "file_edit" | "file_patch") {
  switch (tool) {
    case "file_read":
      return "read";
    case "file_write":
      return "write";
    case "file_edit":
      return "edit";
    case "file_patch":
      return "patch";
  }
}

export function terminalFact(task: Record<string, unknown> | undefined) {
  if (!task) {
    return null;
  }
  const exitCode = typeof task.exit_code === "number" ? task.exit_code : null;
  const status = typeof task.status === "string" ? task.status : null;
  if (exitCode !== null) {
    return `exit ${exitCode}`;
  }
  return status ? `terminal ${status}` : null;
}

export function terminalExitCode(task: Record<string, unknown> | undefined) {
  if (!task) {
    return undefined;
  }
  return typeof task.exit_code === "number" ? task.exit_code : undefined;
}

export function durationFact(durationMs: number | undefined) {
  if (typeof durationMs !== "number") {
    return null;
  }
  if (durationMs >= 1000) {
    return `${(durationMs / 1000).toFixed(1)}s`;
  }
  return `${durationMs}ms`;
}

export function validationLabel(value: string) {
  return value
    .split("_")
    .filter(Boolean)
    .map((part) => part[0].toUpperCase() + part.slice(1))
    .join(" ");
}

export function compactFacts(values: Array<string | null | undefined>) {
  return values.filter((value): value is string => Boolean(value && value.trim()));
}

export function formatTokenCount(tokens: number) {
  if (tokens >= 1000) {
    return `${Math.round(tokens / 100) / 10}k`;
  }
  return `${tokens}`;
}

export function recordField(value: Record<string, unknown> | null, key: string): Record<string, unknown> | null {
  const field = value?.[key];
  return isRecord(field) ? field : null;
}

export function arrayField(value: Record<string, unknown> | null, key: string): unknown[] {
  const field = value?.[key];
  return Array.isArray(field) ? field : [];
}

export function stringField(value: Record<string, unknown> | null, key: string) {
  const field = value?.[key];
  return typeof field === "string" ? field : undefined;
}

export function booleanField(value: Record<string, unknown> | null, key: string) {
  const field = value?.[key];
  return typeof field === "boolean" ? field : undefined;
}

export function numericField(value: Record<string, unknown> | null, key: string) {
  const field = value?.[key];
  return typeof field === "number" && Number.isFinite(field) ? field : undefined;
}

export function timelineStatusFromPartStatus(status: string | null | undefined): TimelineStatus {
  switch (status) {
    case "pending":
    case "running":
      return "running";
    case "waiting":
      return "waiting";
    case "completed":
    case "verified":
    case "not_verified":
    case "partial":
      return "completed";
    case "failed":
    case "timed_out":
    case "cancelled":
      return "failed";
    default:
      return "info";
  }
}

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
