import { type ReactNode } from "react";

export function MetricGrid({ children }: { children: ReactNode }) {
  return <div className="inspector-metric-grid">{children}</div>;
}

export function InspectorMetric({
  label,
  value,
  detail,
}: {
  label: string;
  value: string;
  detail: string;
}) {
  return (
    <div className="inspector-metric">
      <span>{label}</span>
      <strong title={value}>{value}</strong>
      <small title={detail}>{detail}</small>
    </div>
  );
}

export function KeyValue({ label, value }: { label: string; value: string }) {
  return (
    <div className="inspector-kv">
      <span>{label}</span>
      <strong title={value}>{value}</strong>
    </div>
  );
}

export function EmptyInspector({
  icon,
  title,
  detail,
}: {
  icon: ReactNode;
  title: string;
  detail: string;
}) {
  return (
    <div className="inspector-empty-state">
      {icon}
      <strong>{title}</strong>
      <p>{detail}</p>
    </div>
  );
}

export function nullableTokenCount(value?: number | null) {
  return value === undefined || value === null ? "unavailable" : formatTokens(value);
}

export function formatTokens(tokens: number) {
  if (tokens >= 1_000_000) {
    return `${(tokens / 1_000_000).toFixed(1)}m`;
  }
  if (tokens >= 10_000) {
    return `${Math.round(tokens / 1_000)}k`;
  }
  if (tokens >= 1_000) {
    return `${(tokens / 1_000).toFixed(1)}k`;
  }
  return `${tokens}`;
}

export function formatBytes(bytes: number) {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
  if (bytes >= 1024) {
    return `${Math.round(bytes / 1024)} KB`;
  }
  return `${bytes} B`;
}
