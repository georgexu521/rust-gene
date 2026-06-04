import { DesktopRuntimeDiagnostic } from "../runtime/desktopApi";
import { TimelineSummary } from "./types";

export function runtimeDiagnosticFacts(diagnostic: DesktopRuntimeDiagnostic): string[] {
  const taskState = recordField(diagnostic, "task_state");
  const verification = recordField(taskState, "verification");
  const proof = recordField(diagnostic, "verification_proof");
  const controlLoop = recordField(diagnostic, "control_loop");
  const activeFiles = arrayField(taskState, "active_files");

  return compactFacts([
    stringField(taskState, "stage") ? `stage ${stringField(taskState, "stage")}` : null,
    stringField(verification, "status")
      ? `verification ${stringField(verification, "status")}`
      : null,
    stringField(proof, "status") ? `proof ${stringField(proof, "status")}` : null,
    stringField(controlLoop, "coverage") ? `spine ${stringField(controlLoop, "coverage")}` : null,
    activeFiles.length > 0 ? `files ${activeFiles.length}` : null,
  ]).slice(0, 5);
}

export function runtimeDiagnosticDetail(diagnostic: DesktopRuntimeDiagnostic): string {
  return (
    runtimeDiagnosticFacts(diagnostic).join(" · ") ||
    stringField(diagnostic, "schema") ||
    "runtime diagnostic"
  );
}

export function runtimeDiagnosticRunStage(
  diagnostic: DesktopRuntimeDiagnostic,
): Extract<TimelineSummary, { kind: "run" }>["stage"] {
  const proof = recordField(diagnostic, "verification_proof");
  const proofStatus = stringField(proof, "status");
  if (proofStatus === "failed" || proofStatus === "blocked") {
    return "failed";
  }
  return "running";
}

function compactFacts(values: Array<string | null | undefined>) {
  return values.filter((value): value is string => Boolean(value && value.trim()));
}

function recordField(value: Record<string, unknown> | null, key: string): Record<string, unknown> | null {
  const field = value?.[key];
  return isRecord(field) ? field : null;
}

function arrayField(value: Record<string, unknown> | null, key: string): unknown[] {
  const field = value?.[key];
  return Array.isArray(field) ? field : [];
}

function stringField(value: Record<string, unknown> | null, key: string) {
  const field = value?.[key];
  return typeof field === "string" ? field : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
