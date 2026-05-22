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
      status?: TimelineStatus;
    };

export type TimelineKind = "run" | "tool" | "permission" | "usage" | "error";

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
