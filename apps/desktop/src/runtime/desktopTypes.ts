export type DesktopHealth = {
  status: string;
  version: string;
  cwd: string;
};

export type DesktopContextSnapshot = {
  history_messages: number;
  history_tokens: number;
  tool_schema_tokens: number;
  memory_snapshot_tokens: number;
  total_estimated_tokens: number;
  max_context_tokens: number;
  usage_percent: number;
  stable_prefix_fingerprint: string;
  prompt_cache_cached_tokens: number;
  prompt_cache_miss_tokens: number;
  prompt_cache_hit_rate_percent: number;
  prompt_cache_diagnostic_count: number;
  prompt_cache_last_reason?: string | null;
  compact: DesktopCompactState;
};

export type DesktopWorkbenchSnapshot = {
  selected_project: string;
  project_map: DesktopProjectMapSnapshot;
  symbol_index: DesktopSymbolIndexSnapshot;
  runtime_context?: DesktopContextSnapshot | null;
  lab_status: DesktopLabStatusSnapshot;
  subagent_tasks: DesktopSubagentTaskSnapshot[];
};

export type DesktopLabStatusSnapshot = {
  available: boolean;
  state: string;
  detail: string;
  lab_run_id?: string | null;
  proposal_id?: string | null;
  proposal_status?: string | null;
  run_status?: string | null;
  stage?: string | null;
  owner?: string | null;
  needs_user: boolean;
  cycle_count: number;
  artifact_count: number;
  meeting_count: number;
  task_total: number;
  task_open: number;
  task_blocked: number;
  blockers: string[];
  validation_retry_count: number;
  validation_retry_escalated_count: number;
  latest_validation_retry?: string | null;
  meeting_recommended: boolean;
  meeting_topic?: string | null;
  latest_report_path?: string | null;
  daemon_policy?: DesktopLabDaemonPolicySnapshot | null;
  artifacts: DesktopLabArtifactSnapshot[];
  reports: DesktopLabReportSnapshot[];
  evidence_refs: DesktopLabEvidenceSnapshot[];
};

export type DesktopLabDaemonPolicySnapshot = {
  enabled: boolean;
  mode: string;
  max_steps: number;
  max_steps_per_cycle: number;
  interval_ms: number;
  last_started_at?: string | null;
  last_start_error?: string | null;
};

export type DesktopLabArtifactSnapshot = {
  artifact_id: string;
  artifact_type: string;
  stage: string;
  owner: string;
  status: string;
  validation_status?: string | null;
  title: string;
  created_at: string;
  updated_at: string;
  report_path?: string | null;
  report_preview?: string | null;
  report_preview_truncated: boolean;
  evidence_refs: string[];
};

export type DesktopLabReportSnapshot = {
  artifact_id: string;
  path: string;
  preview?: string | null;
  truncated: boolean;
};

export type DesktopLabArtifactBody = {
  artifact_id: string;
  artifact_type: string;
  title: string;
  stage: string;
  owner: string;
  status: string;
  validation_status?: string | null;
  content: string;
};

export type DesktopLabEvidenceSnapshot = {
  evidence_id: string;
  kind: string;
  role: string;
  reference: string;
  summary: string;
  artifact_id?: string | null;
  cycle_id?: string | null;
  created_at: string;
  estimated_summary_tokens: number;
};

export type DesktopLabDaemonActionResult = {
  action: string;
  output: string;
  lab_status: DesktopLabStatusSnapshot;
};

export type DesktopSubagentTaskSnapshot = {
  task_id: string;
  agent_id: string;
  profile?: string | null;
  role: string;
  status: string;
  description: string;
  child_session_id?: string | null;
  result_artifact_id?: number | null;
  artifact_status?: string | null;
  result_preview?: string | null;
  tools_used: string[];
  proof_kind?: string | null;
  completion_sink?: string | null;
  recovery_status?: string | null;
  recovery_action?: string | null;
  updated_at: string;
};

export type DesktopProjectMapSnapshot = {
  available: boolean;
  source?: string | null;
  freshness: string;
  chars: number;
  truncated: boolean;
  content_preview: string;
};

export type DesktopSymbolIndexSnapshot = {
  schema_version: number;
  total_symbols: number;
  files: DesktopIndexedFile[];
  truncated: boolean;
};

export type DesktopIndexedFile = {
  path: string;
  hash: string;
  lines: number;
  summary: string;
  symbols: DesktopIndexedSymbol[];
};

export type DesktopIndexedSymbol = {
  name: string;
  kind: string;
  line: number;
  signature: string;
};

export type DesktopCompactState = {
  compression_count: number;
  circuit_open: boolean;
  latest_strategy?: string | null;
  latest_boundary_id?: string | null;
  latest_attempt_decision?: string | null;
  latest_attempt_reason?: string | null;
  latest_attempt_trigger?: string | null;
  latest_attempt_tokens_before?: number | null;
  latest_attempt_tokens_after?: number | null;
};

export type DesktopCompactionAttempt = {
  trigger: string;
  strategy: string;
  decision: string;
  before_tokens: number;
  after_tokens?: number | null;
  messages_before: number;
  messages_after?: number | null;
  reason: string;
  attempt_index: number;
  consecutive_no_gain: number;
  consecutive_failures: number;
  circuit_open: boolean;
  boundary_id?: string | null;
};

export type SelectedProject = {
  path: string;
};

export type RecentSession = {
  id: string;
  title: string;
  updated_at: string;
  model: string;
  message_count: number;
};

export type DesktopMessage = {
  id: number;
  role: string;
  content: string;
  created_at: string;
};

export type DesktopCompactBoundary = {
  boundary_id: string;
  strategy: string;
  trigger: string;
  before_tokens: number;
  after_tokens: number;
  messages_before: number;
  messages_after: number;
  summary: string;
  created_at: string;
};

export type ResumedSession = {
  session_id: string;
  messages: DesktopMessage[];
  compact_boundaries: DesktopCompactBoundary[];
  session_parts: DesktopSessionPart[];
};

export type DesktopSessionPart = {
  id: number;
  part_index: number;
  part_id: string;
  kind: string;
  tool_call_id?: string | null;
  tool_name?: string | null;
  status?: string | null;
  payload: Record<string, unknown>;
  projected_to_seq: number;
  updated_at: string;
};

export type DesktopSessionRevertRecord = {
  id: number;
  session_id: string;
  operation: string;
  status: string;
  message_id?: string | null;
  target_part_id?: string | null;
  part_ids: string[];
  checkpoint_ids: string[];
  snapshot_checkpoint_id?: string | null;
  paths: string[];
  restored_files: string[];
  removed_files: string[];
  errors: string[];
  diff_summary?: string | null;
  unrevert_possible: boolean;
  unreverted: boolean;
  payload: Record<string, unknown>;
  created_at: string;
};

export type DesktopToolOutputPage = {
  id: string;
  uri: string;
  tool_name: string;
  mime: string;
  content: string;
  offset: number;
  limit: number;
  total_bytes: number;
  has_more: boolean;
};

export type DesktopLabReportPage = {
  path: string;
  content: string;
  offset: number;
  limit: number;
  total_bytes: number;
  has_more: boolean;
};

export type DesktopFilePreview = {
  path: string;
  content: string;
  line_count: number;
  total_bytes: number;
  truncated: boolean;
};

export type DesktopToolOutputMeta = {
  id: string;
  uri: string;
  tool_call_id: string;
  tool_name: string;
  mime: string;
  original_bytes: number;
  created_at_ms: number;
};

export type DesktopRevertResult = {
  session_id: string;
  status: string;
  message_id?: string | null;
  part_ids: string[];
  tool_round_id?: string | null;
  file_change_ids: string[];
  checkpoint_ids: string[];
  paths: string[];
  restored_files: string[];
  removed_files: string[];
  errors: string[];
  change_count: number;
};

export type DesktopSettings = {
  selected_project: string;
  active_session_id?: string | null;
  permission_mode: PermissionModeId;
  detail_level: DetailLevelId;
  agent_mode: AgentModeId;
  provider_name?: string | null;
  model?: string | null;
  settings_path: string;
  diagnostic_logs_path: string;
  recent_projects: string[];
  archived_session_ids: string[];
  startup_state: DesktopStartupState;
};

export type DesktopStartupState = {
  status: string;
  detail: string;
  lab_run_id?: string | null;
  lab_stage?: string | null;
  lab_owner?: string | null;
  lab_pause_reason?: string | null;
};

export type DetailLevelId = "coding" | "daily";

export type AgentModeId = "auto" | "build" | "plan" | "explore" | "review";

export type AgentModeOption = {
  id: AgentModeId;
  label: string;
  description: string;
};

export type PermissionModeId = "default" | "auto_low_risk" | "auto" | "read_only";

export type PermissionModeOption = {
  id: PermissionModeId;
  label: string;
  description: string;
};

export type DiagnosticStatus = "ok" | "warning" | "error";

export type DesktopDiagnostic = {
  id: string;
  label: string;
  status: DiagnosticStatus;
  detail: string;
};

export type DesktopDiagnosticsResponse = {
  items: DesktopDiagnostic[];
};

export type DesktopExportResult = {
  session_id: string;
  path: string;
  format: string;
  privacy: string;
};

export type ProviderSetupInfo = {
  shell_profile_path: string;
  provider_env_vars: string[];
  example: string;
};

export type ProviderModelStatus = {
  active_provider?: string | null;
  active_provider_label?: string | null;
  active_model: string;
  active_base_url: string;
  runtime_model?: string | null;
  runtime_provider_ready: boolean;
  selection_source: string;
  configured_count: number;
  providers: DesktopProviderOption[];
  models: DesktopModelOption[];
};

export type DesktopCurrentDiffContextDetail = {
  type: "current_diff";
  label: string;
  shortstat: string;
  files: string[];
  stat: string;
  patch_preview: string;
  truncated: boolean;
};

export type DesktopFileContextDetail = {
  type: "file";
  label: string;
  path: string;
  relative_path: string;
  size_bytes: number;
  line_count: number;
  line_start?: number | null;
  line_end?: number | null;
  preview: string;
  truncated: boolean;
};

export type DesktopRunContextDetail =
  | DesktopCurrentDiffContextDetail
  | DesktopFileContextDetail;

export type DesktopRunContext =
  | {
      type: "current_diff";
      label: string;
      detail?: DesktopRunContextDetail | null;
    }
  | {
      type: "file";
      label: string;
      path: string;
      line_start?: number | null;
      line_end?: number | null;
      selection_text?: string | null;
      detail?: DesktopRunContextDetail | null;
    };

export type DesktopProviderOption = {
  id: string;
  label: string;
  provider_type: string;
  model: string;
  base_url: string;
  configured: boolean;
  active: boolean;
  note: string;
};

export type DesktopModelOption = {
  id: string;
  label: string;
  provider_id: string;
  active: boolean;
  note: string;
};

export type DesktopRuntimeDiagnostic = {
  schema?: string;
  task_state?: Record<string, unknown>;
  verification_proof?: Record<string, unknown>;
  control_loop?: Record<string, unknown>;
  [key: string]: unknown;
};

export type DesktopRunEvent =
  | { type: "run_started"; run_id: string; session_id?: string | null }
  | { type: "assistant_delta"; text: string }
  | { type: "thinking_started" }
  | { type: "thinking_delta"; text: string }
  | { type: "thinking_completed" }
  | { type: "tool_started"; id: string; name: string }
  | { type: "tool_args_delta"; id: string; delta: string }
  | { type: "tool_call_completed"; id: string }
  | { type: "tool_execution_progress"; id: string; progress: string }
  | { type: "tool_completed"; id: string; result_preview: string; metadata?: unknown }
  | {
      type: "permission_request";
      id: string;
      tool_name: string;
      arguments: unknown;
      prompt: string;
      metadata?: unknown;
      review?: unknown;
    }
  | {
      type: "usage";
      prompt_tokens: number;
      completion_tokens: number;
      reasoning_tokens?: number | null;
      cached_tokens?: number | null;
      cache_write_tokens?: number | null;
    }
  | { type: "runtime_diagnostic"; diagnostic: DesktopRuntimeDiagnostic }
  | { type: "closeout"; status: string; evidence_summary?: string | null }
  | { type: "run_completed" }
  | { type: "output_truncated" }
  | { type: "run_error"; message: string };

export type ProviderCatalogEntry = {
  provider_id: string;
  label: string;
  enabled: boolean;
  source: string;
  base_url_host: string;
  default_model: string;
  available_model_ids: string[];
  context_limit: number | null;
  output_limit: number | null;
  protocol_family: string;
  supports_streaming: boolean;
  requires_nonstreaming: boolean;
  last_health_status: string | null;
  last_latency_ms: number | null;
  recent_timeout_category: string | null;
};

export type FileMutationResult = {
  operation: string;
  changed_paths: string[];
  checkpoint_id: string | null;
  diff_preview: string | null;
  additions: number;
  deletions: number;
  stale_state: string | null;
  diagnostics_delta: unknown;
  rollback_status: string | null;
  error_hint: string | null;
};

export type SessionJobItem = {
  job_id: string;
  session_id: string;
  command: string;
  cwd: string | null;
  status: string;
  started_at: string;
  completed_at: string | null;
  exit_code: number | null;
  timed_out: boolean;
  tool_output_uri: string | null;
  cancelled: boolean;
};

export type SessionContext = {
  session_id: string;
  compact_boundary_id: string | null;
  estimated_history_tokens: number;
  tool_schema_tokens: number;
  memory_snapshot_tokens: number;
  stable_prefix_hash: string | null;
  dynamic_tail_hash: string | null;
  latest_compaction: CompactionSummary | null;
  message_count_after_compaction: number;
};

export type CompactionSummary = {
  boundary_id: string;
  strategy: string;
  trigger: string;
  before_tokens: number;
  after_tokens: number;
  messages_before: number;
  messages_after: number;
  preserved_tail_count: number;
};

export type SessionRunStatus = {
  session_id: string;
  status: string;
};

export type DesktopGoalStatus = {
  goal_id: string | null;
  objective: string | null;
  status: string | null;
  turn_count: number | null;
  max_turns: number | null;
  last_decision: string | null;
  last_closeout: string | null;
  last_proof: string | null;
  last_blocker: string | null;
  step_count: number;
  steps: DesktopGoalStep[];
};

export type DesktopGoalStep = {
  turn_index: number;
  decision: string;
  closeout_status: string | null;
  verification_status: string | null;
  changed_files: number;
  validation_items: number;
  summary: string;
};

export type DesktopGoalCommandResult = {
  status: DesktopGoalStatus;
  next_prompt: string | null;
};
