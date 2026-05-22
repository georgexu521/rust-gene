import { DesktopRunEvent } from "../runtime/desktopApi";

export type TranscriptItem =
  | { id: string; role: "user"; text: string }
  | { id: string; role: "assistant"; text: string }
  | { id: string; role: "tool"; text: string }
  | {
      id: string;
      role: "timeline";
      kind: TimelineKind;
      title: string;
      detail?: string;
      facts?: string[];
      summary?: TimelineSummary;
      status?: TimelineStatus;
      traceId?: string;
    };

export type TimelineKind = "run" | "tool" | "permission" | "usage" | "error";

export type TimelineSummary =
  | {
      kind: "run";
      stage: "running" | "waiting" | "completed" | "failed";
      headline: string;
      detail?: string;
      recovery?: string;
      sessionId?: string;
      stats?: string[];
    }
  | {
      kind: "shell";
      command: string;
      validation?: string;
      exitCode?: number;
      duration?: string;
    }
  | {
      kind: "file";
      action: "read" | "write" | "edit" | "patch";
      path?: string;
      operations?: number;
      replacements?: number;
      additions?: number;
      deletions?: number;
      diffPreview?: string;
      diffTruncated?: boolean;
    }
  | {
      kind: "failure";
      reason: string;
      recovery?: string;
      outputPreview?: string;
      outputTruncated?: boolean;
    };

export type TimelineStatus =
  | "running"
  | "waiting"
  | "completed"
  | "failed"
  | "info";

export type TraceItem = {
  id: string;
  kind: "run" | "tool" | "permission" | "usage" | "error";
  title: string;
  detail?: string;
};

export type PermissionRequest = Extract<DesktopRunEvent, { type: "permission_request" }>;
