import { DesktopRunContext } from "../runtime/desktopApi";

export function desktopRunContextKey(context: DesktopRunContext): string {
  if (context.type === "current_diff") {
    return "current_diff";
  }
  const lineStart = context.line_start ?? "";
  const lineEnd = context.line_end ?? "";
  return `file:${context.path}:${lineStart}:${lineEnd}`;
}
