import { DesktopRunEvent } from "../runtime/desktopApi";

export type TranscriptItem =
  | { id: string; role: "user"; text: string }
  | { id: string; role: "assistant"; text: string }
  | { id: string; role: "tool"; text: string };

export type TraceItem = {
  id: string;
  kind: "run" | "tool" | "permission" | "usage" | "error";
  title: string;
  detail?: string;
};

export type PermissionRequest = Extract<DesktopRunEvent, { type: "permission_request" }>;
