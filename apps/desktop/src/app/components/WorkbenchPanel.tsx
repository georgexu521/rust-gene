import { type ReactNode } from "react";
import {
  Activity,
  CheckCircle2,
  Database,
  ExternalLink,
  FileCode2,
  Gauge,
  Map,
  MessageSquare,
  RefreshCw,
  RotateCcw,
  UsersRound,
} from "lucide-react";
import { DesktopLabStatusSnapshot, DesktopWorkbenchSnapshot } from "../../runtime/desktopApi";

type WorkbenchPanelProps = {
  snapshot: DesktopWorkbenchSnapshot | null;
  onOpenLabReport: (path: string) => void;
  onRefresh: () => void;
  onStageLabCommand: (command: string) => void;
  onSuperviseLabDaemon: () => void;
};

export function WorkbenchPanel({
  snapshot,
  onOpenLabReport,
  onRefresh,
  onStageLabCommand,
  onSuperviseLabDaemon,
}: WorkbenchPanelProps) {
  const projectMap = snapshot?.project_map;
  const symbolIndex = snapshot?.symbol_index;
  const runtime = snapshot?.runtime_context;
  const lab = snapshot?.lab_status;
  const subagentTasks = snapshot?.subagent_tasks || [];
  const topFiles = symbolIndex?.files.slice(0, 4) || [];

  return (
    <section className="workbench-panel" aria-label="Frontend workbench">
      <div className="workbench-header">
        <div>
          <span className="workbench-eyebrow">Workbench</span>
          <h2>Project intelligence</h2>
        </div>
        <button
          aria-label="Refresh workbench snapshot"
          className="workbench-refresh"
          type="button"
          onClick={onRefresh}
        >
          <RefreshCw aria-hidden="true" size={15} />
          <span>Refresh</span>
        </button>
      </div>

      <div className="workbench-metrics">
        <WorkbenchMetric
          detail={projectMap?.freshness || "loading"}
          icon={<Map aria-hidden="true" size={16} />}
          label="Project map"
          value={projectMap?.available ? "Available" : "Missing"}
        />
        <WorkbenchMetric
          detail={symbolIndex?.truncated ? "budgeted preview" : "complete preview"}
          icon={<FileCode2 aria-hidden="true" size={16} />}
          label="Symbol index"
          value={`${symbolIndex?.total_symbols ?? 0} symbols`}
        />
        <WorkbenchMetric
          detail={runtime ? `${runtime.history_messages} messages` : "no active runtime"}
          icon={<Gauge aria-hidden="true" size={16} />}
          label="Runtime context"
          value={runtime ? `${runtime.usage_percent}%` : "Idle"}
        />
        <WorkbenchMetric
          detail={
            runtime
              ? `${runtime.prompt_cache_cached_tokens} cached · ${runtime.prompt_cache_miss_tokens} miss · ${runtime.prompt_cache_last_reason || "no reason"}`
              : "starts after run"
          }
          icon={<Database aria-hidden="true" size={16} />}
          label="Cache surface"
          value={
            runtime
              ? `${runtime.prompt_cache_hit_rate_percent.toFixed(1)}% hit`
              : "not ready"
          }
        />
        <WorkbenchMetric
          detail={lab ? lab.detail : "loading"}
          icon={<Activity aria-hidden="true" size={16} />}
          label="LabRun"
          value={lab ? labStatusValue(lab) : "Loading"}
        />
      </div>

      <div className="workbench-body">
        <section className="workbench-map" aria-label="Project map preview">
          <div className="workbench-section-title">
            <Map aria-hidden="true" size={15} />
            <span>Map preview</span>
            {projectMap?.truncated ? <small>truncated</small> : null}
          </div>
          <pre>{projectMap?.content_preview || "Loading project map..."}</pre>
        </section>

        <section className="workbench-lab" aria-label="Lab status panel">
          <div className="workbench-section-title">
            <Activity aria-hidden="true" size={15} />
            <span>Lab status</span>
            <small>{lab ? lab.state : "loading"}</small>
          </div>
          {lab ? (
            <div className="workbench-lab-grid">
              <LabStatusRow label="Run" value={lab.lab_run_id || lab.proposal_id || "none"} />
              <LabStatusRow label="Stage" value={lab.stage || lab.proposal_status || lab.state} />
              <LabStatusRow label="Owner" value={lab.owner || "none"} />
              <LabStatusRow label="Tasks" value={`${lab.task_open}/${lab.task_total} open · ${lab.task_blocked} blocked`} />
              <LabStatusRow label="Artifacts" value={`${lab.artifact_count} artifacts · ${lab.meeting_count} meetings`} />
              <LabStatusRow label="Retries" value={`${lab.validation_retry_count} total · ${lab.validation_retry_escalated_count} escalated`} />
              <LabStatusRow label="Meeting" value={lab.meeting_recommended ? "recommended" : "quiet"} />
              <LabStatusRow label="Daemon" value={labDaemonPolicyValue(lab)} />
              <div className="workbench-lab-actions" aria-label="Lab actions">
                {lab.latest_report_path ? (
                  <button
                    aria-label="Open latest Lab report"
                    title="Open latest Lab report"
                    type="button"
                    onClick={() => onOpenLabReport(lab.latest_report_path!)}
                  >
                    <ExternalLink aria-hidden="true" size={13} />
                  </button>
                ) : null}
                <button
                  aria-label="Supervise Lab daemon"
                  title="Supervise Lab daemon"
                  type="button"
                  onClick={onSuperviseLabDaemon}
                >
                  <Activity aria-hidden="true" size={13} />
                </button>
                {lab.meeting_recommended ? (
                  <button
                    aria-label="Stage Lab meeting"
                    title="Stage Lab meeting"
                    type="button"
                    onClick={() => onStageLabCommand("/lab meeting open")}
                  >
                    <UsersRound aria-hidden="true" size={13} />
                  </button>
                ) : null}
                <button
                  aria-label="Stage Lab intervention"
                  title="Stage Lab intervention"
                  type="button"
                  onClick={() => onStageLabCommand("/lab intervene ")}
                >
                  <MessageSquare aria-hidden="true" size={13} />
                </button>
                <button
                  aria-label="Stage Lab continue"
                  title="Stage Lab continue"
                  type="button"
                  onClick={() => onStageLabCommand("/lab continue ")}
                >
                  <RotateCcw aria-hidden="true" size={13} />
                </button>
                <button
                  aria-label="Stage Lab closeout"
                  title="Stage Lab closeout"
                  type="button"
                  onClick={() => onStageLabCommand("/lab closeout auto")}
                >
                  <CheckCircle2 aria-hidden="true" size={13} />
                </button>
              </div>
              {lab.meeting_topic ? <p>{lab.meeting_topic}</p> : <p>{lab.detail}</p>}
              {lab.latest_validation_retry ? <p>{lab.latest_validation_retry}</p> : null}
              {lab.blockers.length > 0 ? (
                <div className="workbench-lab-blockers" aria-label="Lab blockers">
                  {lab.blockers.slice(0, 3).map((blocker) => (
                    <code key={blocker} title={blocker}>{blocker}</code>
                  ))}
                </div>
              ) : null}
              {lab.latest_report_path ? <code title={lab.latest_report_path}>{lab.latest_report_path}</code> : null}
            </div>
          ) : (
            <div className="workbench-empty">Loading LabRun status...</div>
          )}
        </section>

        <section className="workbench-subagents" aria-label="Sub-agent tasks">
          <div className="workbench-section-title">
            <UsersRound aria-hidden="true" size={15} />
            <span>Sub-agents</span>
            <small>{subagentTasks.length} recent</small>
          </div>
          {subagentTasks.length === 0 ? (
            <div className="workbench-empty">No durable sub-agent tasks</div>
          ) : (
            <div className="workbench-file-list">
              {subagentTasks.slice(0, 5).map((task) => (
                <article className="workbench-file" key={`${task.task_id}:${task.agent_id}`}>
                  <div>
                    <strong title={task.task_id}>{task.task_id}</strong>
                    <span>
                      {task.status} · {task.profile || task.role}
                    </span>
                  </div>
                  <p title={task.description}>{task.description}</p>
                  <div className="workbench-symbols">
                    <code title={task.agent_id}>{task.agent_id}</code>
                    {task.result_artifact_id ? (
                      <code>
                        artifact {task.result_artifact_id}
                        {task.artifact_status ? `:${task.artifact_status}` : ""}
                      </code>
                    ) : null}
                    {task.completion_sink ? <code>{task.completion_sink}</code> : null}
                    {task.proof_kind ? <code>{task.proof_kind}</code> : null}
                    {task.recovery_status ? <code>{task.recovery_status}</code> : null}
                  </div>
                  {task.tools_used.length > 0 ? (
                    <div className="workbench-agent-tools" title={task.tools_used.join(", ")}>
                      {task.tools_used.slice(0, 5).map((tool) => (
                        <code key={`${task.task_id}:${tool}`}>{tool}</code>
                      ))}
                      {task.tools_used.length > 5 ? <code>+{task.tools_used.length - 5}</code> : null}
                    </div>
                  ) : null}
                  {task.result_preview ? <p title={task.result_preview}>{task.result_preview}</p> : null}
                  {task.recovery_action ? <p title={task.recovery_action}>{task.recovery_action}</p> : null}
                </article>
              ))}
            </div>
          )}
        </section>

        <section className="workbench-index" aria-label="Symbol index preview">
          <div className="workbench-section-title">
            <FileCode2 aria-hidden="true" size={15} />
            <span>Symbol index</span>
            <small>schema v{symbolIndex?.schema_version ?? 1}</small>
          </div>
          {topFiles.length === 0 ? (
            <div className="workbench-empty">No indexed symbols yet</div>
          ) : (
            <div className="workbench-file-list">
              {topFiles.map((file) => (
                <article className="workbench-file" key={file.path}>
                  <div>
                    <strong title={file.path}>{file.path}</strong>
                    <span>
                      {file.lines} lines · {file.hash.slice(0, 8)}
                    </span>
                  </div>
                  <p>{file.summary}</p>
                  {file.symbols.length > 0 ? (
                    <div className="workbench-symbols">
                      {file.symbols.slice(0, 6).map((symbol) => (
                        <code key={`${file.path}:${symbol.kind}:${symbol.name}:${symbol.line}`}>
                          {symbol.kind} {symbol.name}:{symbol.line}
                        </code>
                      ))}
                    </div>
                  ) : null}
                </article>
              ))}
            </div>
          )}
        </section>
      </div>
    </section>
  );
}

function labStatusValue(lab: DesktopLabStatusSnapshot) {
  if (lab.run_status) return lab.run_status;
  if (lab.proposal_status) return lab.proposal_status;
  return lab.available ? lab.state : "None";
}

function labDaemonPolicyValue(lab: DesktopLabStatusSnapshot) {
  const policy = lab.daemon_policy;
  if (!policy) {
    return "not configured";
  }
  const mode = policy.mode.replace(/_/g, " ");
  if (policy.mode === "hybrid_cycles") {
    return `${policy.enabled ? "enabled" : "disabled"} ${mode}: ${policy.max_steps} cycles / ${policy.max_steps_per_cycle} steps`;
  }
  return `${policy.enabled ? "enabled" : "disabled"} ${mode}: ${policy.max_steps} steps`;
}

function LabStatusRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="workbench-lab-row">
      <span>{label}</span>
      <strong title={value}>{value}</strong>
    </div>
  );
}

function WorkbenchMetric({
  detail,
  icon,
  label,
  value,
}: {
  detail: string;
  icon: ReactNode;
  label: string;
  value: string;
}) {
  return (
    <div className="workbench-metric">
      {icon}
      <span>{label}</span>
      <strong title={value}>{value}</strong>
      <small title={detail}>{detail}</small>
    </div>
  );
}
