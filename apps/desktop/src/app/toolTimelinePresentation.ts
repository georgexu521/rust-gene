import { TimelineSummary, TranscriptItem } from "./types";

type TimelineItem = Extract<TranscriptItem, { role: "timeline" }>;

export function toolUsageStats(tools: TimelineItem[]): string[] {
  const counts = new Map<string, number>();
  for (const item of tools) {
    const name = toolNameFromTimeline(item);
    if (!name) {
      continue;
    }
    counts.set(name, (counts.get(name) || 0) + toolExecutionCount(item));
  }

  const labels = Array.from(counts.entries())
    .sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]))
    .slice(0, 4)
    .map(([name, count]) => (count > 1 ? `${name} x${count}` : name));
  const hidden = counts.size - labels.length;
  return hidden > 0 ? [...labels, `+${hidden} tools`] : labels;
}

export function runtimeStatsFromRunSummary(summary: TimelineSummary | undefined): string[] {
  if (summary?.kind !== "run") {
    return [];
  }
  return (summary.stats || []).filter(
    (stat) =>
      stat.startsWith("stage ") ||
      stat.startsWith("verification ") ||
      stat.startsWith("proof ") ||
      stat.startsWith("spine "),
  );
}

export function timelineTools(items: TranscriptItem[]): TimelineItem[] {
  return items.filter(
    (item): item is TimelineItem => item.role === "timeline" && item.kind === "tool",
  );
}

export function toolExecutionCount(item: TimelineItem): number {
  const repeatCount =
    item.summary?.kind === "file" && item.summary.action === "read"
      ? item.summary.repeatCount
      : undefined;
  return Math.max(1, repeatCount || 1);
}

function toolNameFromTimeline(item: TimelineItem): string | null {
  const factName = item.facts
    ?.find((fact) => fact.startsWith("tool "))
    ?.replace(/^tool\s+/, "")
    .trim();
  if (factName) {
    return factName;
  }
  return item.title || null;
}
