import { Cpu, Zap, Coins, Brain, Folder, Wifi, WifiOff } from "lucide-react";
import { DesktopContextSnapshot, DesktopHealth, ProviderModelStatus } from "../../runtime/desktopApi";

export type StatusBarProps = {
  health: DesktopHealth | null;
  providerStatus: ProviderModelStatus | null;
  contextSnapshot: DesktopContextSnapshot | null;
  projectPath: string;
  isRunning: boolean;
};

function formatTokens(n: number): string {
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
  return `${n}`;
}

function formatCacheHitRate(snapshot: DesktopContextSnapshot | null): string {
  if (!snapshot) return "--";
  const denom = snapshot.prompt_cache_cached_tokens + snapshot.prompt_cache_miss_tokens;
  if (denom === 0) return "--";
  return `${snapshot.prompt_cache_hit_rate_percent.toFixed(1)}%`;
}

function cacheHitTitle(snapshot: DesktopContextSnapshot | null): string {
  if (!snapshot) return "Cache: no data";
  const cached = snapshot.prompt_cache_cached_tokens;
  const miss = snapshot.prompt_cache_miss_tokens;
  const total = cached + miss;
  if (total === 0) return "Cache: no tokens yet";
  return `Cache: ${cached} / ${total} tokens (${snapshot.prompt_cache_hit_rate_percent.toFixed(1)}%)`;
}

function apiHost(url: string | undefined | null): string {
  if (!url) return "--";
  try {
    return new URL(url).hostname;
  } catch {
    return url.replace(/^https?:\/\//, "");
  }
}

export function StatusBar({
  health,
  providerStatus,
  contextSnapshot,
  projectPath,
  isRunning,
}: StatusBarProps) {
  const online = health?.status === "ok";
  const model = providerStatus?.active_model || "--";
  const totalTokens = contextSnapshot?.total_estimated_tokens ?? 0;
  const workspaceName = projectPath.split(/[\\/]/).pop() || "ws";

  return (
    <footer className="statusbar">
      {/* API connection status */}
      <span className="statusbar-seg" title={online ? "API connected" : "API offline"}>
        {online ? <Wifi size={11} /> : <WifiOff size={11} style={{ color: "var(--danger, #e55)" }} />}
        <span className="statusbar-label">{apiHost(providerStatus?.active_provider)}</span>
        <span className={`statusbar-val ${online ? "" : "warn"}`}>
          {online ? (isRunning ? "running" : "online") : "offline"}
        </span>
      </span>

      {/* Cache hit rate */}
      <span className="statusbar-seg" title={cacheHitTitle(contextSnapshot)}>
        <Zap size={11} style={{ color: "var(--accent, #7c3aed)" }} />
        <span className="statusbar-label">Cache</span>
        <span className="statusbar-val acc">{formatCacheHitRate(contextSnapshot)}</span>
      </span>

      {/* Token count */}
      <span className="statusbar-seg">
        <Cpu size={11} />
        <span className="statusbar-label">Tokens</span>
        <span className="statusbar-val">{formatTokens(totalTokens)}</span>
      </span>

      {/* Context usage */}
      {contextSnapshot ? (
        <span className="statusbar-seg" title={`${contextSnapshot.total_estimated_tokens} / ${contextSnapshot.max_context_tokens} tokens`}>
          <span className="statusbar-label">Context</span>
          <span className="statusbar-val">{contextSnapshot.usage_percent}%</span>
        </span>
      ) : null}

      <span className="statusbar-grow" />

      {/* Model name */}
      <span className="statusbar-seg" title={`model · ${model}`}>
        <Brain size={11} style={{ color: "var(--violet, #8b5cf6)" }} />
        <span className="statusbar-val vio">{model}</span>
      </span>

      {/* Workspace */}
      <span className="statusbar-seg" title={`workspace: ${projectPath}`}>
        <Folder size={11} />
        <span className="statusbar-val">{workspaceName}</span>
      </span>
    </footer>
  );
}
