#!/usr/bin/env bash
# Validate LabRun command surfaces and, optionally, the provider-backed
# lab-daemon path. The default mode is offline and never calls a provider.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

MODE="offline"
ARTIFACT_DIR="${PRIORITY_AGENT_LAB_VALIDATION_ARTIFACT_DIR:-$ROOT_DIR/target/lab-live-validation/$(date +%Y%m%d-%H%M%S)}"
BIN="${PRIORITY_AGENT_LAB_VALIDATION_BIN:-$ROOT_DIR/target/debug/priority-agent}"

usage() {
  cat <<'EOF'
Usage: scripts/lab-live-validation.sh [--offline|--live|--live-control-plane|--live-graduate] [--artifact-dir DIR]

Modes:
  --offline  Build priority-agent and validate deterministic Lab command surfaces.
  --live-control-plane
             Validate provider-backed Professor/control-plane paths and lab-daemon only.
  --live-graduate
             Validate full provider-backed graduate tool use, runtime verification, worktree merge, and lab-daemon.
  --live     Alias for --live-graduate. Requires a configured provider.

Environment:
  PRIORITY_AGENT_LAB_VALIDATION_BIN          Override priority-agent binary path.
  PRIORITY_AGENT_LAB_VALIDATION_ARTIFACT_DIR Override report/artifact directory.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --offline) MODE="offline"; shift ;;
    --live) MODE="live_graduate"; shift ;;
    --live-graduate) MODE="live_graduate"; shift ;;
    --live-control-plane) MODE="live_control_plane"; shift ;;
    --artifact-dir) ARTIFACT_DIR="${2:?missing artifact dir}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ "$ARTIFACT_DIR" != /* ]]; then
  ARTIFACT_DIR="$ROOT_DIR/$ARTIFACT_DIR"
fi

mkdir -p "$ARTIFACT_DIR"
cargo build -q

WORKSPACE="$ARTIFACT_DIR/workspace"
rm -rf "$WORKSPACE"
mkdir -p "$WORKSPACE"

run_lab() {
  local command="$1"
  local output_file="$2"
  (cd "$WORKSPACE" && "$BIN" lab --command "$command") >"$output_file" 2>&1
}

run_lab_provider() {
  local command="$1"
  local output_file="$2"
  (cd "$WORKSPACE" && "$BIN" lab --command "$command" --with-provider) >"$output_file" 2>&1
}

CERTIFICATION_RECORD_WRITTEN=0

record_provider_certification_failure() {
  local status="$1"
  if [[ "$MODE" != "live_control_plane" && "$MODE" != "live_graduate" ]]; then
    return 0
  fi
  if [[ "$CERTIFICATION_RECORD_WRITTEN" == "1" ]]; then
    return 0
  fi
  if [[ ! -d "$WORKSPACE" || ! -x "$BIN" ]]; then
    return 0
  fi
  local kind="control-plane"
  if [[ "$MODE" == "live_graduate" ]]; then
    kind="graduate"
  fi
  local output_file="$ARTIFACT_DIR/provider-record-${kind}-failed.txt"
  set +e
  (cd "$WORKSPACE" && "$BIN" lab --command "provider record $kind failed $report live $MODE validation failed with exit status $status; see report and artifacts" --with-provider) >"$output_file" 2>&1
  set -e
}

on_exit() {
  local status="$1"
  if [[ "$status" -ne 0 ]]; then
    record_provider_certification_failure "$status"
  fi
}

trap 'on_exit "$?"' EXIT

report="$ARTIFACT_DIR/report.md"
{
  echo "# Lab Live Validation"
  echo
  echo "- mode: \`$MODE\`"
  echo "- workspace: \`$WORKSPACE\`"
  echo "- binary: \`$BIN\`"
  echo
} >"$report"

run_lab "propose Validate LabRun provider surfaces" "$ARTIFACT_DIR/01-propose.txt"
proposal_id="$(sed -n 's/^Lab proposal created: //p' "$ARTIFACT_DIR/01-propose.txt" | head -n 1)"
if [[ -z "$proposal_id" ]]; then
  echo "failed to create Lab proposal" >&2
  cat "$ARTIFACT_DIR/01-propose.txt" >&2
  exit 1
fi

run_lab "approve $proposal_id" "$ARTIFACT_DIR/02-approve.txt"
grep -q "LabRun created:" "$ARTIFACT_DIR/02-approve.txt"
run_lab "dashboard" "$ARTIFACT_DIR/03-dashboard.txt"
grep -q "Lab dashboard:" "$ARTIFACT_DIR/03-dashboard.txt"
run_lab "meeting recommend" "$ARTIFACT_DIR/04-meeting-recommend.txt"
grep -q "Lab meeting recommendation:" "$ARTIFACT_DIR/04-meeting-recommend.txt"
run_lab "context professor" "$ARTIFACT_DIR/05-context-professor.txt"
grep -q "Lab context packet:" "$ARTIFACT_DIR/05-context-professor.txt"
grep -q "L6 artifact-and-gate-evidence-refs" "$ARTIFACT_DIR/05-context-professor.txt"
run_lab "daemon service status" "$ARTIFACT_DIR/06-daemon-service-status.txt"
grep -q "Lab daemon service status." "$ARTIFACT_DIR/06-daemon-service-status.txt"
run_lab "task create Validate structured graduate output | src/lab/model.rs | cargo check -q | Bind a structured GraduateResult from agent JSON" "$ARTIFACT_DIR/07-task-create.txt"
task_id="$(sed -n 's/^Created graduate task: //p' "$ARTIFACT_DIR/07-task-create.txt" | head -n 1)"
if [[ -z "$task_id" ]]; then
  echo "failed to create Lab graduate task" >&2
  cat "$ARTIFACT_DIR/07-task-create.txt" >&2
  exit 1
fi
cat >"$ARTIFACT_DIR/08-graduate-result.json" <<EOF
{
  "result": "{\"graduate_result\":{\"summary\":\"Bound structured graduate output for validation.\",\"changed_files\":[\"src/lab/model.rs\"],\"validation_results\":[\"cargo check -q passed\"],\"blockers\":[],\"evidence_ids\":[\"labevidence_lab_validation\"]}}"
}
EOF
run_lab "task bind-json $task_id $ARTIFACT_DIR/08-graduate-result.json" "$ARTIFACT_DIR/09-task-bind-json.txt"
grep -q "Bound graduate agent JSON result" "$ARTIFACT_DIR/09-task-bind-json.txt"
grep -q "Gate status: satisfied" "$ARTIFACT_DIR/09-task-bind-json.txt"
run_lab "task list" "$ARTIFACT_DIR/10-task-list.txt"
grep -q "Completed" "$ARTIFACT_DIR/10-task-list.txt"
run_lab "runs" "$ARTIFACT_DIR/10a-runs-index.txt"
grep -q "Lab runs:" "$ARTIFACT_DIR/10a-runs-index.txt"
grep -q "runs_index.json" "$ARTIFACT_DIR/10a-runs-index.txt"
run_lab "status" "$ARTIFACT_DIR/10b-status-index.txt"
grep -q "Index:" "$ARTIFACT_DIR/10b-status-index.txt"
grep -q "SQLite index:" "$ARTIFACT_DIR/10b-status-index.txt"
grep -q "lab_index.sqlite3" "$ARTIFACT_DIR/10b-status-index.txt"
run_lab "dashboard" "$ARTIFACT_DIR/10c-dashboard-index.txt"
grep -q "Indexed dashboard: sqlite=" "$ARTIFACT_DIR/10c-dashboard-index.txt"
grep -q "lab_index.sqlite3" "$ARTIFACT_DIR/10c-dashboard-index.txt"
run_lab "provider" "$ARTIFACT_DIR/10d-provider-no-context.txt"
grep -q "requires the Lab Mode shell provider" "$ARTIFACT_DIR/10d-provider-no-context.txt"

cargo test -q hybrid_run_plans_queues_graduate_syncs_and_reaches_user_report \
  >"$ARTIFACT_DIR/10e-hybrid-full-offline-spine-test.txt" 2>&1
cargo test -q hybrid_run_syncs_completed_durable_graduate_and_reaches_user_report \
  >"$ARTIFACT_DIR/10f-hybrid-durable-graduate-resume-test.txt" 2>&1
cargo test -q scheduler_syncs_completed_durable_graduate_task_before_blocking_in_progress \
  >"$ARTIFACT_DIR/10g-scheduler-durable-graduate-sync-test.txt" 2>&1
cargo test -q scheduler_blocks_completed_durable_graduate_task_without_artifact \
  >"$ARTIFACT_DIR/10h-scheduler-durable-graduate-failure-test.txt" 2>&1
cargo test -q lab_graduate_provider_compare_reports_durable_subagent_proof \
  >"$ARTIFACT_DIR/10i-lab-graduate-provider-compare-proof-test.txt" 2>&1
cargo test -q provider_run_commands_without_context_point_to_provider_shell \
  >"$ARTIFACT_DIR/10j-provider-run-no-context-guard-test.txt" 2>&1
cargo test -q step_llm_command_runs_provider_stage_step \
  >"$ARTIFACT_DIR/10k-step-llm-command-test.txt" 2>&1
cargo test -q run_llm_command_reaches_graduate_boundary \
  >"$ARTIFACT_DIR/10l-run-llm-command-test.txt" 2>&1
cargo test -q run_hybrid_command_enters_strict_graduate_scheduler_boundary \
  >"$ARTIFACT_DIR/10m-run-hybrid-command-test.txt" 2>&1
cargo test -q run_hybrid_cycles_command_continues_after_user_report_with_bound \
  >"$ARTIFACT_DIR/10n-run-hybrid-cycles-command-test.txt" 2>&1
cargo test -q run_hybrid_cycles_command_stops_when_cycle_token_budget_is_exceeded \
  >"$ARTIFACT_DIR/10o-run-hybrid-cycles-budget-gate-test.txt" 2>&1
cargo test -q run_hybrid_cycles_command_records_compression_after_completed_cycle \
  >"$ARTIFACT_DIR/10p-run-hybrid-cycles-compression-test.txt" 2>&1
cargo test -q meeting_request_persists_professor_trigger_artifact \
  >"$ARTIFACT_DIR/10q-meeting-request-artifact-test.txt" 2>&1
cargo test -q meeting_open_creates_read_only_report_from_recommendation_signal \
  >"$ARTIFACT_DIR/10r-meeting-open-request-command-test.txt" 2>&1
cargo test -q scheduler_refuses_to_run_without_active_lease \
  >"$ARTIFACT_DIR/10s-scheduler-active-lease-gate-test.txt" 2>&1
cargo test -q background_start_refuses_missing_active_lease \
  >"$ARTIFACT_DIR/10t-background-active-lease-gate-test.txt" 2>&1
cargo test -q artifact_gate_validation_blocks_missing_handoff_fields \
  >"$ARTIFACT_DIR/10u-artifact-gate-typed-artifact-test.txt" 2>&1
cargo test -q graduate_dispatch_records_workspace_snapshots_around_execution \
  >"$ARTIFACT_DIR/10v-graduate-workspace-snapshot-test.txt" 2>&1
cargo test -q review_and_dashboard_render_graduate_workspace_snapshots \
  >"$ARTIFACT_DIR/10w-graduate-workspace-snapshot-surface-test.txt" 2>&1
cargo test -q postdoc_integration_summary_includes_workspace_snapshot_evidence \
  >"$ARTIFACT_DIR/10x-postdoc-workspace-snapshot-evidence-test.txt" 2>&1
cargo test -q professor_review_accepts_valid_postdoc_integration \
  >"$ARTIFACT_DIR/10y-professor-review-evidence-inheritance-test.txt" 2>&1
cargo test -q continue_from_user_report_starts_next_cycle_with_fresh_professor_gate \
  >"$ARTIFACT_DIR/10z-cycle-summary-evidence-inheritance-test.txt" 2>&1
cargo test -q provider_professor_review_enforces_postdoc_evidence_boundary \
  >"$ARTIFACT_DIR/10aa-provider-professor-review-evidence-inheritance-test.txt" 2>&1
cargo test -q meeting_summary_writes_read_only_artifact_and_tracks_meeting_id \
  >"$ARTIFACT_DIR/10ab-runtime-meeting-evidence-propagation-test.txt" 2>&1
cargo test -q provider_meeting_writes_read_only_summary_and_usage \
  >"$ARTIFACT_DIR/10ac-provider-meeting-evidence-propagation-test.txt" 2>&1
cargo test -q llm_draft_structured_postdoc_plan_gate_inherits_revision_evidence \
  >"$ARTIFACT_DIR/10ad-structured-draft-gate-evidence-test.txt" 2>&1
cargo test -q postdoc_plan_consumes_pending_professor_revision_task \
  >"$ARTIFACT_DIR/10ae-revision-task-evidence-inheritance-test.txt" 2>&1
cargo test -q blocker_report_writes_postdoc_handoff_artifact \
  >"$ARTIFACT_DIR/10af-blocker-report-evidence-propagation-test.txt" 2>&1
cargo test -q artifact_gate_evidence_refs_live_in_dynamic_tail \
  >"$ARTIFACT_DIR/10ag-context-artifact-gate-evidence-layer-test.txt" 2>&1
cargo test -q context_command_renders_packet_fingerprints \
  >"$ARTIFACT_DIR/10ah-context-command-artifact-gate-evidence-layer-test.txt" 2>&1
cargo test -q prepare_injects_lab_context_only_when_enabled \
  >"$ARTIFACT_DIR/10ai-live-request-context-artifact-gate-evidence-layer-test.txt" 2>&1
cargo test -q background_hybrid_command_starts_reports_and_stops_scheduler \
  >"$ARTIFACT_DIR/10aj-background-hybrid-command-test.txt" 2>&1
cargo test -q background_hybrid_command_requires_provider_context \
  >"$ARTIFACT_DIR/10ak-background-hybrid-provider-guard-test.txt" 2>&1
cargo test -q background_hybrid_cycles_command_starts_reports_and_stops_scheduler \
  >"$ARTIFACT_DIR/10al-background-hybrid-cycles-command-test.txt" 2>&1
cargo test -q background_hybrid_cycles_command_requires_provider_context \
  >"$ARTIFACT_DIR/10am-background-hybrid-cycles-provider-guard-test.txt" 2>&1
cargo test -q daemon_policy_persists_hybrid_cycles_mode \
  >"$ARTIFACT_DIR/10an-daemon-hybrid-cycles-policy-test.txt" 2>&1
cargo test -q daemon_enable_accepts_hybrid_cycles_mode \
  >"$ARTIFACT_DIR/10ao-daemon-hybrid-cycles-command-test.txt" 2>&1
(
  cd apps/desktop/src-tauri
  cargo test -q desktop_smoke_lab_status_reads_file_backed_labrun_state
) \
  >"$ARTIFACT_DIR/10ap-desktop-daemon-cycle-bound-status-test.txt" 2>&1
cargo test -q provider_compare_recovers_generic_foreground_from_durable_sink \
  >"$ARTIFACT_DIR/10aq-provider-compare-foreground-durable-recovery-test.txt" 2>&1

{
  echo "## Offline Checks"
  echo
  echo "- proposal creation: passed"
  echo "- LabRun approval: passed"
  echo "- dashboard: passed"
  echo "- meeting recommendation: passed"
  echo "- professor context packet: passed"
  echo "- daemon service status: passed"
  echo "- graduate JSON result binding: passed"
  echo "- runs/status/dashboard indexed summaries: passed"
  echo "- provider certification no-context guard: passed"
  echo "- hybrid provider planning -> graduate durable sync -> user_report spine test: passed"
  echo "- hybrid durable graduate resume spine test: passed"
  echo "- scheduler durable graduate auto-sync test: passed"
  echo "- scheduler durable graduate missing-artifact failure test: passed"
  echo "- Lab graduate provider-compare durable proof test: passed"
  echo "- provider foreground run no-context guard test: passed"
  echo "- provider /lab step llm command test: passed"
  echo "- provider /lab run llm command test: passed"
  echo "- provider /lab run hybrid command test: passed"
  echo "- provider /lab run hybrid-cycles command test: passed"
  echo "- provider /lab run hybrid-cycles budget gate test: passed"
  echo "- provider /lab run hybrid-cycles compression test: passed"
  echo "- professor-triggered meeting request artifact test: passed"
  echo "- /lab meeting open request command test: passed"
  echo "- scheduler active lease gate test: passed"
  echo "- background active lease gate test: passed"
  echo "- typed artifact gate validation test: passed"
  echo "- graduate workspace snapshot event test: passed"
  echo "- graduate workspace snapshot review/dashboard surface test: passed"
  echo "- postdoc workspace snapshot evidence test: passed"
  echo "- professor review evidence inheritance test: passed"
  echo "- cycle summary evidence inheritance test: passed"
  echo "- provider professor review evidence inheritance test: passed"
  echo "- runtime meeting evidence propagation test: passed"
  echo "- provider meeting evidence propagation test: passed"
  echo "- structured draft gate evidence propagation test: passed"
  echo "- revision task evidence inheritance test: passed"
  echo "- blocker report evidence propagation test: passed"
  echo "- context artifact/gate evidence layer test: passed"
  echo "- context command artifact/gate evidence layer test: passed"
  echo "- live request context artifact/gate evidence layer test: passed"
  echo "- background hybrid command test: passed"
  echo "- background hybrid provider guard test: passed"
  echo "- background hybrid-cycles command test: passed"
  echo "- background hybrid-cycles provider guard test: passed"
  echo "- daemon hybrid-cycles policy test: passed"
  echo "- daemon hybrid-cycles command test: passed"
  echo "- desktop daemon cycle-bound status test: passed"
  echo "- provider compare foreground durable recovery test: passed"
  echo
} >>"$report"

if [[ "$MODE" == "live_control_plane" || "$MODE" == "live_graduate" ]]; then
  run_lab "intervene Please classify this as a lab meeting about validation risk" "$ARTIFACT_DIR/11-intervene.txt"
  grep -q "LabRun intervention queued:" "$ARTIFACT_DIR/11-intervene.txt"
  run_lab_provider "messages classify latest Lab live validation sponsor classification" "$ARTIFACT_DIR/12-classify-sponsor.txt"
  grep -q "Professor classified sponsor message:" "$ARTIFACT_DIR/12-classify-sponsor.txt"
  run_lab_provider "provider" "$ARTIFACT_DIR/12a-provider-certification.txt"
  grep -q "Lab provider certification:" "$ARTIFACT_DIR/12a-provider-certification.txt"
  grep -q "Control-plane validation:" "$ARTIFACT_DIR/12a-provider-certification.txt"
  grep -q "Graduate validation:" "$ARTIFACT_DIR/12a-provider-certification.txt"
  run_lab_provider "provider diagnose-tools" "$ARTIFACT_DIR/12b-provider-tool-diagnostics.txt"
  grep -q "Provider tool-call diagnostics:" "$ARTIFACT_DIR/12b-provider-tool-diagnostics.txt"
  grep -q "Probe: minimal_auto" "$ARTIFACT_DIR/12b-provider-tool-diagnostics.txt"
  grep -q "Probe: runtime_file_write_auto" "$ARTIFACT_DIR/12b-provider-tool-diagnostics.txt"
  grep -q "Probe: runtime_subagent_allowed_auto" "$ARTIFACT_DIR/12b-provider-tool-diagnostics.txt"
  run_lab "resume" "$ARTIFACT_DIR/13-resume-after-sponsor-classification.txt"
  grep -q "Resumed LabRun" "$ARTIFACT_DIR/13-resume-after-sponsor-classification.txt"
  run_lab_provider "provider compare" "$ARTIFACT_DIR/13a-provider-compare.txt"
  grep -q "Provider subagent comparison:" "$ARTIFACT_DIR/13a-provider-compare.txt"
  grep -q "Generic subagent:" "$ARTIFACT_DIR/13a-provider-compare.txt"
  grep -q "Background subagent:" "$ARTIFACT_DIR/13a-provider-compare.txt"
  grep -q "completion_sink:" "$ARTIFACT_DIR/13a-provider-compare.txt"
  grep -q "Lab graduate:" "$ARTIFACT_DIR/13a-provider-compare.txt"

  if [[ "$MODE" == "live_graduate" ]]; then
    run_lab "task create Live graduate proof | lab-live-graduate-proof.md | test -f lab-live-graduate-proof.md | Create lab-live-graduate-proof.md with one line, run the required validation, and return the required graduate_result JSON." "$ARTIFACT_DIR/14-live-task-create.txt"
    live_task_id="$(sed -n 's/^Created graduate task: //p' "$ARTIFACT_DIR/14-live-task-create.txt" | head -n 1)"
    if [[ -z "$live_task_id" ]]; then
      echo "failed to create live graduate task" >&2
      cat "$ARTIFACT_DIR/14-live-task-create.txt" >&2
      exit 1
    fi
    run_lab_provider "task run $live_task_id" "$ARTIFACT_DIR/15-live-task-run.txt"
    grep -q "Graduate task run dispatched:" "$ARTIFACT_DIR/15-live-task-run.txt"
    if ! grep -q "Status: Succeeded" "$ARTIFACT_DIR/15-live-task-run.txt"; then
      echo "live graduate task did not pass runtime certification; see $ARTIFACT_DIR/15-live-task-run.txt" >&2
      cat "$ARTIFACT_DIR/15-live-task-run.txt" >&2
      exit 1
    fi
    run_lab "task list" "$ARTIFACT_DIR/16-live-task-list.txt"
    grep -q "$live_task_id Completed" "$ARTIFACT_DIR/16-live-task-list.txt"
    run_lab_provider "task worktree review $live_task_id" "$ARTIFACT_DIR/17-live-worktree-review.txt"
    grep -q "Lab graduate worktree review succeeded" "$ARTIFACT_DIR/17-live-worktree-review.txt"
    grep -q "lab-live-graduate-proof.md" "$ARTIFACT_DIR/17-live-worktree-review.txt"
    run_lab_provider "task worktree merge $live_task_id" "$ARTIFACT_DIR/18-live-worktree-merge.txt"
    grep -q "Lab graduate worktree merge succeeded" "$ARTIFACT_DIR/18-live-worktree-merge.txt"
    test -f "$WORKSPACE/lab-live-graduate-proof.md"
    run_lab_provider "task worktree cleanup $live_task_id force" "$ARTIFACT_DIR/19-live-worktree-cleanup.txt"
    grep -q "Lab graduate worktree cleanup succeeded" "$ARTIFACT_DIR/19-live-worktree-cleanup.txt"
  fi

  run_lab "daemon enable hybrid 2 100 Lab live validation provider smoke" "$ARTIFACT_DIR/20-daemon-enable.txt"
  grep -q "Enabled Lab daemon policy" "$ARTIFACT_DIR/20-daemon-enable.txt"
  set +e
  (cd "$WORKSPACE" && "$BIN" lab-daemon) >"$ARTIFACT_DIR/21-lab-daemon.txt" 2>&1
  daemon_status=$?
  set -e
  {
    echo "## Live Provider Checks"
    echo
    echo "- sponsor intervention: passed"
    echo "- sponsor classification command: completed"
    echo "- provider certification report: completed"
    echo "- provider direct tool-call diagnostics: completed"
    echo "- provider generic/background-vs-Lab subagent comparison: completed"
    if [[ "$MODE" == "live_graduate" ]]; then
      echo "- live graduate task run, runtime validation, and JSON binding: passed"
      echo "- live graduate worktree review/merge/cleanup: passed"
    else
      echo "- live graduate task run: skipped in control-plane mode"
    fi
    echo "- daemon enable hybrid: passed"
    echo "- lab-daemon exit status: \`$daemon_status\`"
    echo
  } >>"$report"
  if [[ "$daemon_status" -ne 0 ]]; then
    echo "lab-daemon failed; see $ARTIFACT_DIR/21-lab-daemon.txt" >&2
    exit "$daemon_status"
  fi
  if [[ "$MODE" == "live_graduate" ]]; then
    run_lab_provider "provider record graduate passed $ARTIFACT_DIR/report.md live graduate task, runtime validation, worktree review, merge, cleanup, and daemon validation passed" "$ARTIFACT_DIR/21a-provider-record-graduate.txt"
    grep -q "Recorded provider certification:" "$ARTIFACT_DIR/21a-provider-record-graduate.txt"
    CERTIFICATION_RECORD_WRITTEN=1
  else
    run_lab_provider "provider record control-plane passed $ARTIFACT_DIR/report.md live control-plane sponsor classification, provider diagnostics, provider comparison, and daemon validation passed" "$ARTIFACT_DIR/21a-provider-record-control-plane.txt"
    grep -q "Recorded provider certification:" "$ARTIFACT_DIR/21a-provider-record-control-plane.txt"
    CERTIFICATION_RECORD_WRITTEN=1
  fi
  run_lab "cost" "$ARTIFACT_DIR/22-cost.txt"
  run_lab "review" "$ARTIFACT_DIR/23-review.txt"
else
  {
    echo "## Live Provider Checks"
    echo
    echo "- skipped: run \`scripts/lab-live-validation.sh --live-control-plane\` or \`--live-graduate\` with a configured provider"
    echo
  } >>"$report"
fi

echo "Lab validation complete"
echo "report: $report"
