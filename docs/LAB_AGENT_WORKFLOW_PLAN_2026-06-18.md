# Lab Agent Workflow Plan

Date: 2026-06-18
Status: P0 implementation started
Owner: gex / Liz

## Summary

This plan turns the "professor - postdoc - graduate student" lab model into a
first-class priority-agent workflow.

The product goal is not to create a theatrical swarm. The goal is to separate
three different levels of project cognition:

- **Professor agent**: guards project direction, architecture, product thesis,
  risks, non-goals, and strategic completeness.
- **Postdoc agent**: owns technical decomposition, code-aware planning,
  integration, review, and final implementation responsibility.
- **Graduate agent**: executes narrow coding tasks with explicit instructions,
  scoped files, and validation commands.

The workflow should feel different from generic coding agents because it gives
the user a visible research-lab loop: proposal, implementation plan, delegated
work, postdoc integration review, professor-level architectural review, and
optional lab meeting.

## Implementation Progress

### Completed in P0 skeleton

- Added `pa lab` / `priority-agent lab` as a Lab Mode CLI entry path.
- Added thin product entry modules for Direct Mode and Lab Mode.
- Added `src/lab/` with typed models for `LabProposal`, `LabRun`,
  `SponsorMessage`, cost policy, retry budget, roles, and closeout status.
- Added file-backed Lab store under `.priority-agent/lab/` with proposal
  creation, proposal approval, LabRun state creation, event JSONL append,
  pause/resume, sponsor message, and read-only meeting request support.
- Added built-in `lab-professor`, `lab-postdoc`, and `lab-graduate` agent
  profiles with distinct prompt/persona contracts and tool boundaries.
- Added `ArtifactGate` model with minimal stage-gate satisfaction checks.
- Added CLI `/lab` commands for `propose`, `approve`, `start`, `status`,
  `pause`, `resume`, `professor`, `note`, `meeting`, `open`, and `close`.
- Added human-facing report template directory under `docs/lab/`.

### Completed in P0.5 runtime skeleton

- Added file-backed active LabRun lease model and persistence.
- `/lab approve` creates an active LabRun and acquires a lease.
- `/lab pause` releases the active lease.
- `/lab resume` reacquires the active lease and records lease metadata.
- Fresh foreign leases block new mutating LabRun ownership.
- Artifact gates can be written to `.priority-agent/lab/runs/<id>/artifact_gates/`
  and validated before stage transitions.
- Missing `artifact_id`, `next_action`, or evidence/validation status blocks
  artifact gate validation.

### Completed in P0.6 orchestration skeleton

- Added minimal deterministic `LabOrchestrator`.
- LabRun approval now creates the initial required professor-plan gate.
- `/lab gate` shows the required artifact gate for the current stage.
- `/lab gate satisfy <artifact_id> [validation_status] [evidence_ref]` writes
  a satisfied gate for the current stage.
- `/lab advance` advances only when the current stage gate validates.
- Advancing from one stage creates the next stage's required gate.
- Current deterministic stage path covers:
  `professor_discussion -> postdoc_plan -> graduate_work -> postdoc_review ->
  professor_review -> user_report`.

### Completed in P0.7 artifact skeleton

- Added structured stage artifacts for `ProfessorPlan`, `PostdocPlan`,
  `GraduateResult`, `PostdocIntegrationSummary`, and `ProfessorReview`.
- Added typed artifact envelopes with schema version, artifact ID, LabRun ID,
  stage, owner, status, evidence refs, and validation status.
- Added artifact persistence under
  `.priority-agent/lab/runs/<id>/artifacts/<artifact_id>.json`.
- Artifact writes now update `LabRun.artifact_ids` and
  `resume_cursor.active_artifact_id`.
- `/lab plan <note>` creates the required artifact for the current stage,
  writes it to disk, and satisfies the current artifact gate.
- The deterministic stage gate path now has a real structured artifact behind
  the handoff instead of only a manually supplied `artifact_id`.

### Completed in P0.8 stale lease recovery

- Added stale active lease recovery to the file-backed Lab store.
- Entering `pa lab` / `priority-agent lab` now checks for an expired active
  LabRun lease before opening the shell.
- A stale lease is removed, the corresponding run is preserved, and the run is
  marked `PausedShutdown` with `pause_reason=stale_heartbeat`.
- Recovery writes a `lab_stale_lease_recovered` event so resume behavior has
  auditable evidence.
- The active run pointer is preserved so `/lab status` and `/lab resume` can
  continue from the interrupted LabRun.

### Completed in P0.9 clean shutdown pause

- Lab Mode shell exit now attempts to pause an active LabRun before returning.
- Normal Lab Mode shutdown marks the run `PausedShutdown` with
  `pause_reason=app_shutdown`.
- Shutdown pause releases the active lease and preserves the active run pointer
  so the next `/lab resume` continues the same LabRun.
- Direct Mode shell behavior is unchanged.

### Completed in P0.10 artifact report rendering

- Added `src/lab/report.rs` to render structured Lab artifacts into Markdown.
- `/lab plan <note>` now writes both the structured JSON artifact and a
  human-readable report under
  `.priority-agent/lab/runs/<id>/reports/<artifact_id>.md`.
- Generated reports include `lab_run_id`, `artifact_id`, artifact type, stage,
  owner, status, validation status, timestamp, stage-specific body fields, and
  evidence refs.
- Report writes emit a `lab_report_written` event.
- JSON artifacts remain the source of truth; Markdown reports are derived
  human-facing views.

### Completed in P0.11 LabRun cost ledger

- Added `LabCostUsage`, `LabCostSummary`, and per-role cost summaries.
- LabRun cost usage records now include prompt, completion, reasoning, cached,
  cache write, cache miss, total tokens, estimated cost, role, model, cycle ID,
  and meeting ID.
- Added file-backed `cost_usage.jsonl` under each LabRun directory.
- Added LabStore APIs to record usage, list usage, and aggregate LabRun cost by
  role.
- Added `/lab cost` to show latest LabRun cost/cache summary.
- Added `/lab cost record <role> <model> <prompt> <completion> [reasoning]
  [cached] [cache_write] [cost] [note]` as a manual/test hook until real LLM
  usage is wired into LabRun orchestration.

### Completed in P0.12 LabRun context packet skeleton

- Added `src/lab/context.rs` with `LabContextPacket`, `LabContextLayer`, and
  stable/dynamic layer classification.
- Context packets now separate stable prefix layers from dynamic tail layers.
- Stable prefix currently includes role profile, prompt version, model policy,
  project root, user goal, and LabRun cost policy.
- Dynamic tail currently includes current stage, owner, active artifact,
  artifact IDs, task IDs, cycle/failure counts, and LabRun cost/cache summary.
- Added stable prefix and dynamic tail fingerprints so cache-impacting drift is
  visible before real provider calls.
- Added `/lab context [role]` to render packet fingerprints, token estimates,
  and layer composition for professor/postdoc/graduate/runtime roles.

### Completed in P0.13 refs-only evidence index

- Added `LabEvidenceRef` and `LabEvidenceKind` for file/diff/log/command/
  artifact/url/note references.
- Added file-backed `evidence_refs.jsonl` under each LabRun directory.
- Added LabStore APIs to record and list evidence references without copying
  large logs, diffs, or file contents into LabRun state.
- Local file references record a metadata fingerprint from path, size, and
  modified time.
- Added `/lab evidence add <kind> <ref> <summary>` and `/lab evidence list`.
- Lab context packets now include an `L4 refs-only-evidence-index` dynamic
  layer so professor/postdoc/graduate roles can see evidence refs without
  blowing up context.

### Completed in P0.14 cycle summary artifact

- Added `LabCycleSummary` as a structured artifact type.
- Cycle summaries use the existing artifact/report pipeline and write both JSON
  and Markdown report files under the LabRun directory.
- Added `LabOrchestrator::create_cycle_summary_for_latest`.
- Added `/lab cycle summary <text>` to write a cycle summary and increment
  `LabRun.cycle_count`.
- Cycle summaries include current stage, owner, summary text, evidence IDs for
  the current cycle, total tokens, cache hit rate, estimated cost, and next
  action.

### Completed in P0.15 compression decision trigger

- Added `LabCompressionDecision` and `LabCompressionAction`.
- Added deterministic context compression evaluation from `LabContextPacket`
  token estimates and role-specific context budgets.
- Compression action is `required` at or above 80% of the role context budget,
  `recommend` at or above 65%, and `none` below that.
- Compression decisions record packet tokens, budget, usage ratio, stable prefix
  fingerprint, dynamic tail fingerprint, role, cycle ID, and reason.
- Added file-backed `compression_decisions.jsonl` under each LabRun directory.
- Added `/lab compression [role]` to evaluate and persist the current LabRun
  compression decision.

### Completed in P0.16 compression summary execution

- Added `LabCompressionSummary` as a structured artifact type.
- Added Markdown report rendering for compression summaries.
- Added `LabOrchestrator::create_compression_summary_for_latest`.
- Added `/lab compress [role]` to evaluate compression and, when the decision
  is `recommend` or `required`, write a refs-only compression summary artifact
  and report.
- Compression summaries retain decision ID, action, reason, before tokens,
  target budget, usage ratio, stable/dynamic fingerprints, retained layers,
  evidence IDs, compressed summary text, and next action.
- If compression is not needed, `/lab compress` records no artifact and reports
  that the current LabRun context is within budget.

### Completed in P0.17 deterministic LabRun tick

- Added `LabOrchestrator::tick_latest` as a one-step runtime-controlled LabRun
  loop.
- Added `/lab tick` to create the required current-stage artifact, satisfy the
  stage gate, advance to the next stage, and record `lab_tick_completed`.
- Tick runs exactly one deterministic step; it does not run an unbounded
  background loop.
- Tick stops at `user_report` and marks the run `NeedsUser` instead of silently
  continuing.
- When `auto_compress_after_cycle` is enabled, tick evaluates compression after
  the stage advance and writes a compression summary if the decision is
  `recommend` or `required`.

### Completed in P0.18 graduate task queue skeleton

- Added `GraduateTask` and `LabTaskStatus` as structured, persisted task
  envelopes for postdoc-to-graduate delegation.
- Graduate tasks record `allowed_scope`, `required_validation`, instructions,
  assigned role, task status, result artifact ID, evidence IDs, blocker, and
  cycle ID.
- Added file-backed task persistence under
  `.priority-agent/lab/runs/<id>/tasks/<task_id>.json`.
- Creating, starting, blocking, completing, and cancelling tasks now record
  auditable LabRun events.
- Open graduate tasks are synchronized into both `LabRun.open_task_ids` and
  `resume_cursor.open_task_ids`, so pause/resume can recover unfinished work.
- Added `/lab task list`, `/lab task create`, `/lab task start`,
  `/lab task complete`, `/lab task block`, `/lab task cancel`, and `/lab tasks`
  alias support.
- This is a task lifecycle and recovery slice only; later P0.23-P0.25 and
  P0.30 add the agent execution hook and post-execution scope enforcement.

### Completed in P0.19 graduate dispatch adapter

- Added deterministic `GraduateTask -> AgentTaskEnvelope` conversion in
  `src/lab/delegation.rs`.
- Dispatch conversion preserves task ID, LabRun ID, allowed scope, required
  validation, evidence refs, expected artifacts, and hard scope/validation
  constraints.
- Dispatch conversion targets the existing `lab-graduate` profile and prepares
  agent tool params with `isolated_worktree_fork` context.
- Graduate tasks without `allowed_scope` or `required_validation` cannot be
  converted into dispatch envelopes.
- Added `/lab task envelope <task_id>` to inspect the generated envelope and
  agent tool params without launching the subagent.
- This still does not execute the subagent; it is the runtime adapter needed
  before safe automatic dispatch.

### Completed in P0.20 graduate result artifact binding

- Added `LabOrchestrator::create_graduate_result_for_task_latest`.
- Graduate task results can now be converted into structured `GraduateResult`
  artifacts and Markdown reports.
- Result binding updates the corresponding `GraduateTask` to `completed`,
  stores the result artifact ID, records evidence IDs, and removes the task
  from resumable open-task cursors.
- Result artifacts use validation status
  `subagent_report_not_parent_verified`, so graduate output is explicitly
  treated as a claim until postdoc/runtime verification.
- Added `/lab task result <task_id> | <changed_csv> | <validation_csv> |
  <blockers_csv> | <evidence_csv> | <summary>` as a manual/test hook for
  result binding.
- When the LabRun is currently in `graduate_work`, result binding also satisfies
  the current graduate-work artifact gate with the result artifact reference.

### Completed in P0.21 graduate result scope check

- `GraduateResult` binding now checks reported changed files against the
  task's `allowed_scope`.
- A result that claims changed files outside `allowed_scope` is rejected before
  the artifact is written or the task is marked completed.
- This is a deterministic runtime check on the result contract. Later P0.30 and
  P0.40 add post-execution workspace scope enforcement and explicit graduate
  worktree review/merge/cleanup controls.

### Completed in P0.22 graduate dispatch persistence

- Added `GraduateDispatchRecord` and `GraduateDispatchStatus`.
- Prepared graduate dispatches persist under
  `.priority-agent/lab/runs/<id>/dispatches/<dispatch_id>.json`.
- Dispatch records store the generated `AgentTaskEnvelope`, agent tool params,
  task ID, LabRun ID, status, and later result/error slots.
- Added LabStore APIs to record, load, and list graduate dispatch records.
- Added `/lab task dispatch <task_id>` to persist a prepared dispatch without
  launching the subagent.

### Completed in P0.23 approved agent-tool execution adapter

- Added `execute_graduate_task_with_agent_tool` in `src/lab/delegation.rs`.
- The adapter accepts an externally supplied `ToolContext` and calls the
  existing `AgentTool` with the generated `lab-graduate` dispatch params.
- The adapter does not fabricate runtime context and does not bypass the need
  for `AgentManager`, worktree manager, permissions, session store, or other
  tool-runtime dependencies.
- This is the callable bridge needed by the LabRun scheduler; the synchronous
  `/lab` command surface still only prepares and audits dispatch state.

### Completed in P0.24 orchestrator graduate execution hook

- Added `LabOrchestrator::execute_graduate_task_latest_with_context`.
- The orchestrator can now prepare a dispatch, persist it, mark the task
  in-progress, call the existing agent tool through a supplied `ToolContext`,
  and persist `running` / `succeeded` / `failed` dispatch status.
- If the agent tool cannot run or fails, the graduate task becomes explicitly
  `blocked` with the failure reason.
- Successful subagent execution does not automatically complete the
  `GraduateTask`; the output still needs result binding and postdoc/runtime
  verification before it becomes a `GraduateResult` artifact.

### Completed in P0.25 shell-backed graduate run command

- Added async `handle_lab_command_with_context` for Lab commands that need
  runtime `ToolContext`.
- Added `/lab task run <task_id>` as the first command path that calls
  `LabOrchestrator::execute_graduate_task_latest_with_context`.
- The shell `/lab` command now passes `ShellHost::build_tool_context()` into
  the Lab command layer.
- When the current host lacks `AgentManager` or other required runtime
  dependencies, `/lab task run` records a failed dispatch and blocks the
  graduate task instead of pretending execution succeeded.
- The existing synchronous Lab command path remains available for tests,
  documentation, and read-only/preparation commands.

### Completed in P0.26 strict Lab scheduler step

- Added `LabOrchestrator::run_scheduler_step_latest_with_context`.
- Added `/lab step` as a strict scheduler step that uses runtime `ToolContext`.
- Outside `graduate_work`, the scheduler step delegates to the existing
  one-step tick path.
- In `graduate_work`, the scheduler no longer creates placeholder
  `GraduateResult` artifacts. It only dispatches a queued `GraduateTask`.
- If `graduate_work` has no queued task, the scheduler returns an explicit
  blocked result and leaves artifacts unchanged.
- If a queued graduate task exists, the scheduler invokes the graduate execution
  hook and records dispatch lifecycle/failure state.

### Completed in P0.27 bounded foreground scheduler run

- Added `LabOrchestrator::run_scheduler_steps_latest_with_context`.
- Added `/lab run [max_steps]` as a bounded foreground scheduler loop.
- The loop runs strict scheduler steps and stops on blocked, needs-user, or
  graduate-dispatch states.
- The command clamps max steps to a small finite range instead of starting an
  unbounded background loop.
- This provides the control surface needed before a real background scheduler
  can be enabled safely.

### Completed in P0.28 process-local background scheduler

- Added `src/lab/scheduler.rs` with a process-local background scheduler
  registry.
- Added `LabSchedulerState` and `LabSchedulerStatus` persisted under each
  LabRun as `scheduler_state.json`.
- Added `/lab background status`, `/lab background start [max_steps]
  [interval_ms]`, and `/lab background stop`.
- Background scheduling uses the strict scheduler step as its only progression
  primitive.
- The loop refreshes the active LabRun heartbeat, stops on blocked,
  needs-user, graduate-dispatch, completion, cancellation, or max-step limit,
  and records final scheduler state.
- This is intentionally process-local. It is enough for Lab Mode foreground
  sessions; later P0.71-P0.97 add persistent daemon policy, a non-interactive
  worker, LaunchAgent management, and desktop supervision.

### Completed in P0.29 restart recovery for scheduler state

- Added `LabSchedulerStatus::PausedRestart`.
- Added `LabStore::recover_interrupted_scheduler()` to convert persisted
  `Running` or `Stopping` scheduler state into an explicit resumable pause after
  a process restart.
- Lab CLI startup now runs scheduler-state recovery after stale lease recovery.
- Added `/lab background recover` as an explicit command surface for manual
  recovery and tests.
- Restart recovery does not silently resume background execution. The user still
  controls continuation through `/lab background start`, matching the LabRun
  pause/resume product rule.

### Completed in P0.30 post-execution scope enforcement

- Added workspace change snapshots around graduate agent execution.
- After `/lab task run` or scheduler-triggered graduate execution, the runtime
  compares files changed by that execution against the task `allowed_scope`.
- If the graduate agent modifies files outside `allowed_scope`, the dispatch is
  marked `Failed` and the graduate task is blocked instead of being treated as a
  successful handoff.
- Existing dirty files are fingerprinted before execution, so pre-existing user
  or prior-agent changes do not by themselves trigger a LabRun scope failure.
- Internal `.priority-agent/` state changes are excluded from the scope check.

### Completed in P0.31 structured graduate result auto-binding

- Successful graduate agent execution now attempts to parse a structured JSON
  result from the AgentTool payload.
- When the payload includes a summary plus validation evidence, LabRun writes a
  `GraduateResult` artifact, completes the graduate task, satisfies the
  graduate-work gate when applicable, and binds the artifact ID back to the
  dispatch record.
- The parser is intentionally conservative. It accepts explicit JSON fields such
  as `graduate_result.summary`, `changed_files`, `validation_results`, blockers,
  and evidence refs.
- Plain natural-language subagent output is not guessed into an artifact. In
  that case the dispatch can still succeed, but postdoc/runtime binding remains
  required through `/lab task result ...`.

### Completed in P0.32 graduate JSON output contract

- Updated the built-in `lab-graduate` profile prompt to require a final JSON
  object with a top-level `graduate_result`.
- Updated generated graduate task prompts to include the exact JSON shape used
  by the P0.31 parser.
- The requested fields are `summary`, `changed_files`, `validation_results`,
  `blockers`, and `evidence_ids`.
- This makes the automatic result-binding path explicit in the task contract
  instead of relying on informal natural-language reporting.

### Completed in P0.33 read-only lab meeting summaries

- Added `LabMeetingSummary` as a first-class Lab artifact type.
- `/lab meeting [topic]` now writes a JSON artifact and Markdown report instead
  of only recording a request event.
- Meeting summaries include the topic, current stage, professor/postdoc views,
  decision, next actions, evidence refs, token total, and cache hit rate.
- The command is still read-only: it records state/report artifacts but does not
  mutate project code or start implementation work.
- Each generated meeting summary is tracked in `LabRun.meeting_ids` and
  `LabRun.artifact_ids`.

### Completed in P0.34 professor-trigger meeting recommendation signals

- Added deterministic meeting recommendation evaluation on top of persisted
  LabRun state.
- `/lab meeting recommend` now reports whether a professor-triggered meeting is
  recommended, the proposed topic, the reason, and concrete signals.
- Current signals include blocked graduate tasks, repeated failed graduate
  dispatches for the same task, and LabRun failure budget exhaustion.
- The recommendation command is read-only. It does not automatically start a
  meeting or mutate project code; the user or scheduler can explicitly create a
  meeting report with `/lab meeting <topic>`.
- This gives desktop/TUI a stable backend for a quiet "meeting recommended"
  indicator.

### Completed in P0.35 LabRun failure accounting and retry escalation

- Added `LabStore::record_lab_failure()` as the shared failure accounting path.
- Graduate execution failures and post-execution scope violations now increment
  `LabRun.failure_count`.
- When `failure_count` reaches `retry_budget.max_cycle_retries`, the LabRun is
  escalated to `NeedsUser`, `closeout_status` becomes `BlockedNeedsUser`, and a
  concrete `blocked_reason` is persisted.
- Each failure writes a `lab_failure_recorded` event with source, reason,
  current count, retry budget, and user-escalation status.
- Meeting recommendation logic can now use real failure budget exhaustion in
  addition to blocked tasks and failed dispatch counts.

### Completed in P0.36 postdoc blocker reports

- Added `LabBlockerReport` as a first-class Lab artifact type owned by the
  postdoc role.
- Added `/lab blocker status` to summarize blocked tasks, failure count, and
  current blocked reason.
- Added `/lab blocker report [note]` to write a structured blocker artifact and
  Markdown report.
- Blocker reports summarize blocked graduate tasks, failed dispatches,
  `failure_count`, and a recommendation for professor review.
- The generated report is tracked in `LabRun.artifact_ids` and the LabRun
  `blocked_reason` points at the blocker report ID for resume/debug context.

### Completed in P0.37 validation retry accounting and repair tasks

- Added `LabValidationRetry` records persisted under each LabRun.
- Added `LabStore::record_validation_retry_and_repair_task()` as the shared
  validation retry path.
- Added `/lab task retry <task_id> | <validation_summary>`.
- While the task is within `retry_budget.max_validation_retries_per_slice`, a
  validation retry blocks the original task and creates a scoped repair
  `GraduateTask` with the same allowed scope and validation commands.
- When the validation retry budget is exhausted, the retry is marked escalated,
  no repair task is created, and LabRun failure accounting is updated.
- Retry records preserve attempt number, validation summary, generated repair
  task ID, and escalation status for later postdoc/professor review.

### Completed in P0.38 retry history in blocker/status context

- Added validation retry history to LabRun context packets as dynamic-tail layer
  `L5 validation-retry-history`.
- Retry history is intentionally not part of the stable prefix, preserving
  provider prompt-cache stability for role/profile/charter content.
- `/lab context [role]` now includes retry history fingerprints in its dynamic
  tail.
- `/lab blocker status` now shows validation retry count and escalated retry
  count alongside blocked task and failure counts.
- This gives resume, meeting, and professor/postdoc review flows access to
  repair history without opening raw JSON files.

### Completed in P0.39 CLI LabRun dashboard summary

- Added `/lab dashboard` as a text status-panel backend.
- The dashboard summarizes LabRun status, stage, owner, user-needed state,
  cycle/failure/artifact/meeting counts, task counts, validation retry counts,
  cost/cache totals, meeting recommendation, scheduler status, and blocked
  reason.
- This is intentionally CLI-first and dependency-free, but it creates a stable
  field set for later TUI/desktop status panels.
- The command is read-only and does not start or resume internal work.

### Completed in P0.40 graduate worktree review/merge/cleanup wrapper

- Added async `/lab task worktree <review|merge|cleanup> <task_id> [force]`.
- The command finds the latest graduate dispatch for the task with an
  `agent_id`, then delegates to the existing WorktreeTool `agent_review`,
  `agent_merge`, or `agent_cleanup` implementation.
- LabRun does not reimplement git merge logic. It records a
  `lab_graduate_worktree_action` event with task ID, dispatch ID, agent ID,
  action, success, and error.
- The command refuses to run if no graduate dispatch has an `agent_id`, which
  prevents accidental merge attempts before real subagent execution exists.
- This creates the explicit review/merge/reject control surface needed for
  isolated graduate worktrees while preserving existing permission and
  worktree-manager gates.

### Completed in P0.41 blocker-to-professor review transition

- Added `LabOrchestrator::escalate_latest_blocker_to_professor_review()`.
- Added `/lab blocker escalate`.
- After a postdoc-owned `LabBlockerReport` exists, the LabRun can explicitly
  move from the current implementation stage into `professor_review`.
- Escalation updates the resume cursor, internal owner, stage, and event log,
  then creates the normal `ProfessorReview` artifact gate.
- This gives blocked implementation work a real state-machine path back to the
  professor instead of leaving blocker reports as side-channel documents.

### Completed in P0.42 explicit LabRun closeout

- Added `LabStore::closeout_latest_run()` as the single persistence path for
  terminal LabRun outcomes.
- Added `/lab closeout <verified|not_verified|partial|blocked|failed> [note]`.
- `/lab close` now reuses the same closeout path with `cancelled` semantics.
- Closeout releases the active lease, clears pause/lease fields, records
  `closeout_status`, updates the durable run status, and appends a
  `lab_closeout_recorded` event.
- Status mapping is explicit:
  `verified` / `not_verified` / `partial` -> `Completed`,
  `blocked` -> `NeedsUser`, `failed` -> `Failed`,
  `close` -> `Cancelled`.
- Added store and CLI tests for verified completion and lease release.
- This completes the manual/CLI closeout surface. The remaining gap is
  automated evidence-driven closeout gating from the professor/postdoc review
  pipeline.

### Completed in P0.43 final-gate closeout derivation

- Added `LabStore::load_artifact_gate()` so orchestrator code can read the
  final gate instead of duplicating gate path logic.
- Added `LabOrchestrator::closeout_latest_from_user_report()`.
- Added `/lab closeout auto [note]`.
- Automatic closeout is only allowed after the LabRun reaches `user_report`.
- The method validates the final `professor_review` gate, derives closeout
  status from its `validation_status`, then calls the same
  `LabStore::closeout_latest_run()` persistence path used by manual closeout.
- Mapping is conservative:
  `verified` / `validated` / `passed` -> `completed_verified`,
  `partial` -> `partial`,
  `blocked` / `needs_user` -> `blocked_needs_user`,
  `failed` -> `failed`,
  unknown or absent validation -> `completed_not_verified`.
- Added orchestrator and CLI tests that run the deterministic LabRun to
  `user_report`, close it from final evidence, and verify the lease is
  released.
- This gives the state machine a real final closeout path without making
  ordinary `/lab tick` silently complete a project before the user-facing report
  is visible.

### Completed in P0.44 sponsor intervention control

- Added `LabStore::intervene_latest_run()`.
- Added `/lab intervene <message>`.
- Intervention records a high-urgency `SponsorMessage` with
  `PauseRequest` semantics.
- Intervention moves the run to `NeedsUser`, sets `needs_user=true`, records a
  sponsor-facing blocked reason, releases the active lease, and appends
  `lab_intervention_recorded`.
- It does not create graduate tasks, alter allowed scope, or directly command
  postdoc/graduate agents.
- Added store and CLI tests for the intervention path.
- This completes the CLI/runtime foundation for the user side channel. The
  remaining work is desktop/TUI presentation and professor-side processing of
  queued sponsor messages.

### Completed in P0.45 durable sponsor message inbox

- Sponsor messages are now appended to
  `.priority-agent/lab/runs/<lab_run_id>/sponsor_messages.jsonl` in addition to
  the event log.
- Added `LabStore::list_sponsor_messages()`.
- Added `/lab messages` plus `/lab message` and `/lab sponsor` aliases.
- The inbox is read-only: listing messages does not mutate code, create
  graduate tasks, or change LabRun stage.
- Normal `/lab professor <message>` entries persist as `Concern/normal`.
- `/lab intervene <message>` entries persist as `PauseRequest/high`.
- Added store and CLI tests for inbox persistence and listing.
- This makes the side-channel inspectable by CLI/TUI/desktop surfaces. The
  remaining work is professor-role processing that turns reviewed messages into
  steering decisions, meetings, or scoped new tasks.

### Completed in P0.46 sponsor message status workflow

- Added `LabStore::update_latest_sponsor_message_status()`.
- Added `/lab messages review <message_id> [note]`.
- Added `/lab messages meeting <message_id> [note]`.
- Added `/lab messages task <message_id> [note]`.
- Added `/lab messages reject <message_id> [note]`.
- The command rewrites `sponsor_messages.jsonl`, preserves the message body and
  type, and records a `sponsor_message_status_updated` event.
- Status changes are still metadata-only. `ConvertedToMeeting` and
  `ConvertedToTask` do not secretly launch meetings or mutate code; they create
  an explicit, inspectable handoff point for future professor processing.
- Added store and CLI tests for status updates.

### Completed in P0.47 automatic LabRun provider usage capture

- The main `ConversationLoop` provider usage path now mirrors real provider
  usage into an active LabRun cost ledger when the current working directory has
  a non-terminal LabRun.
- The hook runs after the existing global `CostTracker` records the API call.
- It records real provider usage fields:
  `prompt_tokens`, `completion_tokens`, `reasoning_tokens`, `cached_tokens`,
  `cache_write_tokens`, and computed `cache_miss_tokens`.
- It uses the active LabRun `internal_owner` as the role, so professor,
  postdoc, graduate, and runtime phases can be separated in LabRun cost
  summaries.
- It reuses the global cost tracker delta for `estimated_cost_usd` instead of
  introducing a second pricing implementation.
- It is best-effort. Missing LabRun state or LabRun ledger write errors do not
  break the main conversation loop.
- Terminal LabRuns (`Completed`, `Cancelled`, `Failed`) are ignored.
- Added a session processor test proving a provider usage event is mirrored to
  `.priority-agent/lab/runs/<lab_run_id>/cost_usage.jsonl`.

### Completed in P0.48 sponsor message apply workflow

- Added `SponsorMessageStatus::Applied`.
- Added `/lab messages apply <message_id> [note]`.
- Applying a `ConvertedToMeeting` message creates a read-only
  `LabMeetingSummary` artifact and Markdown report, then marks the message
  `Applied`.
- Applying a `ConvertedToTask` message creates a graduate task but immediately
  blocks it with an explicit reason that postdoc must assign `allowed_scope`
  before execution.
- Applying a queued/reviewed/rejected/applied message is rejected until it is
  explicitly marked as `meeting` or `task`.
- This keeps the sponsor side-channel from becoming a direct command console
  while still giving reviewed professor decisions a durable path into LabRun.
- Added CLI tests for meeting apply and blocked-task apply.

### Completed in P0.49 live LabRun context injection

- Added an explicit Lab context enable flag through `StreamingQueryEngine`,
  `StreamingConfig`, `ConversationLoopBuilder`, and `ConversationLoop`.
- Lab Mode entry enables the flag; Direct Mode leaves it off.
- Request preparation now injects a `<lab-context>` dynamic context block only
  when the flag is enabled and the current working directory has a non-terminal
  LabRun.
- The injected block is built from the existing `LabContextPacket` path, so it
  includes role/profile/project charter, cost policy, current LabRun state,
  cost/cache summary, refs-only evidence, and validation retry history.
- The block includes stable and dynamic fingerprints plus token estimates so
  cache behavior remains inspectable.
- Terminal LabRuns (`Completed`, `Cancelled`, `Failed`) are ignored.
- Added request-preparation coverage proving Lab context enters live provider
  request messages only when enabled.

### Completed in P0.50 live LabRun compression decision recording

- Live LabRun request preparation now evaluates compression with the same
  `LabContextPacket` that is injected into provider-bound messages.
- The decision is persisted to `compression_decisions.jsonl` through the
  existing `LabStore::record_compression_decision()` path.
- The injected `<lab-context>` block includes the compression decision ID,
  action, usage ratio, and reason so the model sees the active context pressure.
- This records real request-time 65%/80% threshold decisions instead of only
  recording decisions through manual `/lab compression`.
- Added request-preparation coverage proving live Lab context injection also
  writes a compression decision for the active LabRun.
- Automatic summary execution from these live decisions is completed in P0.51.

### Completed in P0.51 live LabRun automatic compression summaries

- Live LabRun request preparation now attempts automatic compression summary
  execution when the recorded compression decision is `Recommend` or
  `Required`.
- The automatic path uses the same `LabOrchestrator` artifact writer as
  `/lab compress`, so summaries are structured `CompressionSummary` artifacts
  plus Markdown reports.
- Automatic execution respects `LabCostPolicy.auto_compress_after_cycle`; when
  that policy is disabled, live decisions are still recorded but no summary is
  written.
- Automatic execution dedupes by LabRun role and cycle, so repeated requests in
  the same cycle do not create duplicate compression summaries.
- The injected `<lab-context>` block includes `auto_compression_artifact` when
  an automatic summary is written for the turn.
- Added request-preparation coverage proving required live decisions write one
  compression summary artifact and do not duplicate it within the same cycle.

### Completed in P0.52 explicit next-cycle continuation

- Added `LabOrchestrator::continue_latest_from_user_report()` as the explicit
  path for continuing a LabRun after it reaches `user_report`.
- Added `/lab continue [note]` so the user/professor-facing surface can start
  the next professor/postdoc/graduate cycle without closing the LabRun.
- Continuing writes a structured `CycleSummary` artifact and Markdown report
  for the completed cycle before resetting the run to
  `professor_discussion`.
- Continuing clears `needs_user`, restores `Active` status, resets the internal
  owner to professor, increments `cycle_count`, and creates a fresh
  `ProfessorPlan` gate for the next cycle.
- Terminal LabRuns (`Completed`, `Cancelled`, `Failed`) cannot be continued.
- Added orchestrator and command coverage for continuing from `user_report`
  into the next cycle.

### Completed in P0.53 LLM-drafted current-stage artifacts

- Added `src/lab/draft.rs` as the provider-backed draft path for current-stage
  LabRun artifacts.
- Added `/lab draft [instructions]` in the shell path. It calls the current
  `StreamingQueryEngine` provider with a lightweight artifact-draft prompt,
  then persists the result through the existing structured artifact, Markdown
  report, and artifact-gate pipeline.
- Draft prompts include the current required artifact type and the active
  `LabContextPacket` layers so the provider writes from LabRun state instead
  of free-form chat history.
- Provider usage from the draft call is recorded into the LabRun cost ledger
  with prompt, completion, reasoning, cached, and cache-write tokens when the
  provider reports usage.
- Added mock-provider coverage for writing a professor-plan artifact, satisfying
  the current gate, and recording draft usage.
- This is the first real LLM-authored artifact body path. Remaining work is
  acceptance/revision flows and automatic multi-role orchestration.

### Completed in P0.54 strict JSON parsing for LLM-drafted artifacts

- `/lab draft` now prefers strict role-specific JSON output before falling
  back to plain note text.
- Added structured JSON parsing for the current-stage artifact types:
  `ProfessorPlan`, `PostdocPlan`, `GraduateResult`,
  `PostdocIntegrationSummary`, and `ProfessorReview`.
- The parser accepts either a direct artifact body object, an `artifact`
  wrapper, or the role-specific wrapper such as `professor_plan` or
  `postdoc_plan`.
- Successfully parsed JSON writes typed artifact body fields directly into the
  existing JSON artifact, Markdown report, and gate-satisfaction pipeline.
- Incomplete structured JSON now fails hard with the serde missing-field error
  instead of silently filling placeholders.
- Plain text drafts still fall back to the existing note-based artifact writer,
  preserving compatibility for providers that do not follow the JSON request.
- Added mock-provider coverage for structured professor-plan parsing and for
  rejecting incomplete structured JSON without writing an artifact.

### Completed in P0.55 artifact acceptance and revision gates

- Added explicit artifact review commands:
  `/lab accept <artifact_id> [note]` and
  `/lab revise <artifact_id> <note>`.
- Accepted artifacts are marked with `LabArtifactStatus::Accepted` and
  `validation_status=accepted`, then the matching stage gate is rewritten so
  normal advancement can continue.
- Revision requests are marked with `LabArtifactStatus::NeedsRevision` and
  `validation_status=needs_revision`; the revision note is recorded as a gate
  blocker.
- Artifact gate validation now blocks advancement when a gate has blockers or
  `needs_revision` status, so review feedback affects the real state machine
  instead of being a side note.
- Review state updates rewrite both the structured JSON artifact and derived
  Markdown report, preserving the file-backed source of truth.
- Added command coverage proving a revised artifact blocks `/lab advance` until
  the same artifact is accepted.

### Completed in P0.56 provider-backed artifact review decisions

- Added provider-backed artifact review through
  `/lab review artifact <artifact_id> [instructions]` in the shell path.
- The review prompt includes the current LabRun state, artifact type, reviewer
  role, instructions, and the full artifact JSON.
- The provider must return strict JSON:
  `{"decision":"accept"|"revise","note":"..."}`.
- The runtime parses that decision and then calls the deterministic
  `accept_artifact_latest()` or `revise_artifact_latest()` path; the provider
  never mutates gates directly.
- Review usage is recorded into the LabRun cost ledger with real provider
  prompt, completion, reasoning, cached, and cache-write tokens when available.
- Added mock-provider coverage for both accept and revise decisions. The revise
  path writes a real gate blocker and continues to block advancement.

### Completed in P0.57 provider-backed stage step

- Added `/lab step llm [instructions]` as the first provider-backed
  draft/review/advance step for non-graduate LabRun stages.
- The step drafts the current required artifact with the current provider,
  reviews the artifact through the provider-backed accept/revise decision path,
  then advances the LabRun only when the deterministic review gate is accepted.
- Revision decisions keep the LabRun on the same stage and leave the gate
  blocked with the review note.
- `graduate_work` is intentionally excluded from this provider-only step; real
  graduate implementation still uses the strict graduate task scheduler and
  agent-tool/worktree flow.
- Added mock-provider coverage for accepted provider steps advancing from
  `professor_discussion` to `postdoc_plan`, and for revision steps blocking the
  current stage.

### Completed in P0.58 bounded provider-backed run

- Added `/lab run llm [max_steps] [instructions]` as a bounded foreground
  provider-backed run for non-graduate LabRun stages.
- The run repeatedly calls the provider-backed draft/review/advance step until
  it reaches `max_steps`, a revision request, a user/terminal state, a non-active
  run, or the `graduate_work` boundary.
- The `graduate_work` boundary is intentionally treated as a stop condition,
  not as a provider-only implementation step. Real code execution remains owned
  by the strict graduate task scheduler, agent-tool dispatch, worktree, and
  evidence flow.
- The command prints every step, final stage, and stop reason so the user can
  see why the run stopped.
- Added mock-provider coverage proving accepted professor and postdoc artifacts
  advance the run to `graduate_work`, and that a provider revision request stops
  the run while keeping the current stage blocked.

### Completed in P0.59 hybrid provider/graduate scheduler run

- Added `/lab run hybrid [max_steps] [instructions]` as a bounded foreground
  run that combines provider-backed professor/postdoc stages with the strict
  graduate scheduler.
- Non-`graduate_work` stages still use provider-backed draft/review/advance
  with deterministic accept/revise gate application.
- `graduate_work` now hands off to
  `run_scheduler_step_latest_with_context()`, so code-writing work is still
  controlled by queued `GraduateTask` records, the agent-tool dispatch path,
  worktree checks, and evidence binding.
- A missing queued graduate task stops as a strict scheduler `Blocked` result
  instead of letting the provider invent implementation output.
- Added mock-provider coverage proving hybrid runs advance through professor
  and postdoc artifacts, then stop on the strict graduate scheduler boundary
  when generated graduate work is blocked by missing scope.

### Completed in P0.60 accepted PostdocPlan graduate task queueing

- Accepted `PostdocPlan` artifacts now automatically queue `GraduateTask`
  records from the plan's `slices`, `files_expected`, `validation_plan`, and
  `graduate_handoff`.
- The task instructions include the source `PostdocPlan` artifact id, and the
  runtime dedupes on that marker so repeated artifact acceptance does not create
  duplicate graduate tasks.
- Missing `files_expected` or `validation_plan` does not widen permissions.
  The runtime creates the task for auditability and immediately marks it
  blocked, forcing the postdoc/professor loop to repair scope or validation
  before graduate execution.
- Because task generation lives in `LabOrchestrator::accept_artifact_latest()`,
  manual `/lab accept`, provider-backed artifact review, `/lab step llm`,
  `/lab run llm`, and `/lab run hybrid` all share the same deterministic bridge.
- Added unit coverage for valid accepted postdoc plans queueing one task per
  slice, deduping repeated acceptance, and blocking generated tasks when
  required scope is missing.

### Completed in P0.61 blocked graduate task revision

- Added `/lab task revise <task_id> | <scope_csv> | <validation_csv> |
  [instructions]` as the deterministic repair path for blocked graduate tasks
  whose `allowed_scope` or `required_validation` was missing or wrong.
- The shared store API updates open graduate tasks only; completed/cancelled
  tasks cannot be silently rewritten.
- A complete revision moves the task back to `Queued` and clears its blocker so
  the strict graduate scheduler can dispatch it.
- An incomplete revision keeps the task `Blocked` with a concrete missing
  scope/validation reason, preserving the hard execution boundary.
- Revision instructions are appended under a `Postdoc revision` section instead
  of replacing the original task instructions, keeping the audit trail and
  source `PostdocPlan` marker intact.
- Added store and command coverage for requeueing blocked tasks and keeping
  incomplete revisions blocked.

### Completed in P0.62 postdoc integration summary bridge

- Added `/lab integrate [note]` as the deterministic bridge from completed
  `GraduateResult` artifacts into a postdoc-owned
  `PostdocIntegrationSummary`.
- The integration summary gathers accepted graduate results, evidence refs,
  validation attempts, remaining risks, and a professor handoff note from the
  persisted artifacts instead of relying on free-form context.
- When graduate results have no blockers and include validation attempts, the
  `postdoc_review` gate is satisfied and can advance to `professor_review`.
- If a graduate result has blockers, lacks validation attempts, or no acceptable
  result exists, the summary still lands on disk for auditability, but the
  `postdoc_review` gate is marked `needs_revision` with concrete blockers.
- The summary explicitly preserves the fact that graduate output is a subagent
  report pending parent/professor verification; it does not turn subagent
  claims into proof.
- Added orchestrator and command coverage for satisfied and blocked integration
  summaries.

### Completed in P0.63 deterministic professor review bridge

- Added `/lab professor-review [note]` as the deterministic bridge from the
  latest `PostdocIntegrationSummary` into a professor-owned `ProfessorReview`.
- The bridge accepts only postdoc integrations that have accepted graduate
  results and are not marked `needs_revision`.
- Accepted professor reviews write `validation_status=validated` on the
  `professor_review` gate, so `/lab advance` can move the run to
  `user_report` and `/lab closeout auto` can derive a verified closeout from
  the final gate.
- Rejected professor reviews still persist a `ProfessorReview` report, but the
  gate is marked `needs_revision` with concrete required revisions from the
  postdoc integration summary.
- The user-facing report text is generated from persisted postdoc evidence and
  explicit remaining risks rather than from ungrounded conversation context.
- Added orchestrator and command coverage for accepted professor review,
  blocked professor review, and advancement to `user_report`.

### Completed in P0.64 hybrid review bridge execution

- `/lab run hybrid [max_steps] [instructions]` now uses the deterministic
  postdoc/professor review bridges for `postdoc_review` and
  `professor_review` instead of asking the provider to draft those artifacts.
- At `postdoc_review`, hybrid run writes a `PostdocIntegrationSummary`,
  advances to `professor_review` when the gate is satisfied, or stops with a
  deterministic gate-blocked reason when graduate blockers or missing
  validation remain.
- At `professor_review`, hybrid run writes a `ProfessorReview`, advances to
  `user_report` when the gate is validated, then stops because the LabRun is
  waiting for user review.
- Shell output now distinguishes provider, strict scheduler, and deterministic
  review steps in the same hybrid run.
- Added mock-provider coverage proving the hybrid loop can move from
  `postdoc_review` through professor review to `user_report` without provider
  calls, and stops on deterministic review blockers.

### Completed in P0.65 scheduler review bridge execution

- `LabOrchestrator::run_scheduler_step_latest_with_context()` now runs the
  deterministic `postdoc_review` and `professor_review` bridges before falling
  back to generic stage ticking.
- This makes `/lab step`, `/lab run [max_steps]`, and `/lab background start`
  able to advance from bound `GraduateResult` evidence through postdoc
  integration, professor review, and `user_report` without provider access.
- If the postdoc or professor bridge writes a blocked gate, the scheduler
  returns `LabSchedulerStepAction::Blocked` and leaves the LabRun at the review
  stage for repair.
- If professor review advances to `user_report`, the scheduler returns
  `NeedsUser`, so the process-local background loop persists a `NeedsUser`
  stop instead of continuing silently.
- Added scheduler coverage for successful postdoc/professor review bridge
  execution and blocked postdoc review evidence.

### Completed in P0.66 provider-backed hybrid background scheduler

- Added a separate provider-backed background entrypoint,
  `/lab background hybrid [max_steps] [interval_ms] [instructions]`.
- The original `/lab background start` remains the strict deterministic
  process-local scheduler. It does not call providers and keeps the old safety
  boundary.
- The new hybrid background scheduler reuses the same process-local scheduler
  registry and persisted `LabSchedulerState`, so only one strict or hybrid
  background loop can run for the active LabRun in the current process.
- Each hybrid background iteration runs one bounded
  `/lab run hybrid`-equivalent step, then persists a concrete stop reason:
  `needs_user`, `blocked`, `not_active`, `graduate_dispatched_waiting_for_result`,
  `error`, or `max_steps_reached`.
- This makes provider-backed professor/postdoc planning usable in the
  background during an active shell session while still stopping at user,
  blocker, and graduate-dispatch boundaries.
- Added shell argument parsing coverage for numeric `max_steps`,
  `interval_ms`, and free-form instruction tails.

### Completed in P0.67 provider-backed professor strategic review

- Added `/lab professor-review llm [instructions]` as an explicit
  provider-backed Professor review entrypoint for the `professor_review` stage.
- The provider drafts a strict `ProfessorReview` JSON body from the latest
  `PostdocIntegrationSummary`, LabRun context layers, evidence refs, retries,
  and optional user instructions.
- Runtime, not the model, writes the final professor gate. If the model says
  `accepted=true` but the postdoc integration has no accepted graduate results
  or is marked `needs_revision`, the gate is forced to `needs_revision` with
  concrete blockers.
- Accepted provider professor reviews set the `professor_review` gate to
  `validated` and can advance toward `user_report`; rejected or evidence-poor
  reviews keep the LabRun at `professor_review` for postdoc revision.
- Usage is recorded in the Lab cost ledger under the Professor role with
  prompt, completion, reasoning, cached, and cache-write tokens when the
  provider returns usage.
- Added draft-layer coverage for the hard boundary where a provider tries to
  accept closeout despite insufficient postdoc evidence.

### Completed in P0.68 professor revision task artifact

- Added `LabRevisionTask` as a structured artifact for professor-to-postdoc
  revision handoff.
- When deterministic `/lab professor-review` or provider-backed
  `/lab professor-review llm` rejects closeout, runtime now writes a
  `LabRevisionTask` artifact and Markdown report tied to the source
  `ProfessorReview`.
- The revision artifact records the source review artifact, assigned role
  (`Postdoc`), required revision items, evidence refs, and a concrete next
  action.
- This keeps professor rejection from being only a gate blocker string; it
  becomes a durable handoff artifact for the next postdoc repair pass.
- Added provider draft coverage proving that evidence-poor provider professor
  review output creates a `LabRevisionTask`.

### Completed in P0.69 LabRevisionTask consumption by postdoc planning

- The next `postdoc_plan` artifact creation now checks for the latest
  unconsumed `LabRevisionTask` assigned to Postdoc.
- Provider-backed `/lab draft` for `postdoc_plan` includes the pending
  professor revision task as a dynamic context layer, so the postdoc model sees
  the required revision items before drafting a new `PostdocPlan`.
- Deterministic postdoc planning also attaches the revision task as evidence,
  prepends a repair slice, and annotates graduate handoff with the source
  `ProfessorReview`.
- Once a `PostdocPlan` consumes the revision task, runtime marks the
  `LabRevisionTask` artifact `validation_status=consumed`, writes an updated
  Markdown report, and records a `lab_revision_task_consumed_by_postdoc_plan`
  run event.
- Added orchestrator coverage proving that a blocked professor review creates a
  revision task and the following postdoc planning pass consumes it.

### Completed in P0.70 app-owned Lab lifecycle checkpoint

- Added `LabAppLifecycleState` persisted at
  `.priority-agent/lab/app_lifecycle.json`.
- `pa lab` startup now records app lifecycle startup, recovers stale active
  leases, and marks interrupted `Running`/`Stopping` scheduler state as
  `PausedRestart` through one auditable entrypoint.
- Lab Mode shell shutdown now records app lifecycle shutdown and pauses the
  active LabRun with `PausedShutdown` / `app_shutdown` through the same store
  layer.
- Added `/lab lifecycle` to show launch mode, process id, last startup,
  last shutdown, recovered scheduler state, shutdown-paused LabRun, and last
  lifecycle message.
- Added store coverage for lifecycle startup recovery and shutdown pause state.
- This is the durable lifecycle checkpoint/control surface that later
  P0.71-P0.97 build on for restart-surviving daemon policy, worker execution,
  service management, and desktop supervision.

### Completed in P0.71 persistent Lab daemon policy

- Added `LabDaemonState` persisted at `.priority-agent/lab/daemon_state.json`.
- Added `/lab daemon status`, `/lab daemon enable [strict|hybrid] [max_steps]
  [interval_ms] [instructions]`, and `/lab daemon disable [reason]`.
- The daemon policy records whether app-owned background continuation is
  enabled, which scheduler mode should be used (`strict` or `hybrid`), bounded
  max steps, interval, and optional instructions for a future host process.
- Enable/disable actions append project events and preserve enough state for a
  desktop-owned process to restart with the user's intended background policy.
- Added store coverage for persisted daemon enable/disable state.
- This is still a control-plane slice: it does not spawn a restart-surviving
  provider process by itself.

### Completed in P0.72 daemon policy execution hook

- Added `start_daemon_scheduler_from_policy()` to consume persisted
  `LabDaemonState` and start the matching strict or hybrid background scheduler.
- Added `/lab daemon start` as the manual execution trigger. It uses the active
  shell provider/model and `ToolContext`, so hybrid daemon policy can start the
  same provider-backed background loop as `/lab background hybrid`.
- `pa lab` startup now checks enabled daemon policy after shell host creation
  and auto-starts the configured strict/hybrid scheduler in the current process.
- Daemon start attempts update `daemon_state.json` with `last_started_at`,
  `last_started_lab_run_id`, and `last_start_error`, making restart behavior
  auditable from `/lab daemon status`.
- Added store coverage for persisted daemon start-result fields.
- This makes daemon policy executable across app restarts when `pa lab` is
  opened again. It is still not a detached desktop service that runs while the
  app is closed.

### Completed in P0.73 non-interactive Lab daemon worker

- Added `priority-agent lab-daemon` / `pa lab-daemon` as a non-interactive
  daemon worker entrypoint that does not require a TTY.
- The worker reads persisted `LabDaemonState`, records lifecycle startup, builds
  a runtime `ToolContext`, records daemon start metadata, then runs the
  configured strict or hybrid Lab scheduler in the foreground until a boundary.
- Strict worker mode calls the deterministic scheduler step loop. Hybrid worker
  mode calls the provider-backed hybrid loop with the configured instructions.
- Worker failures are written back to `daemon_state.json` through
  `last_start_error`, so desktop or launch agents can inspect why a background
  pass did not start or finish.
- This gives desktop/launchd/systemd a concrete executable to run in the
  background. Later P0.92-P0.96 slices add LaunchAgent install/load/supervise
  management and a desktop Workbench action for this worker.

### Completed in P0.74 macOS LaunchAgent manifest generation

- Added `/lab daemon launchd [label]` to generate a macOS LaunchAgent plist for
  the existing non-interactive `lab-daemon` worker.
- The generated plist is written under `.priority-agent/lab/launchd/` and
  includes `ProgramArguments`, `WorkingDirectory`, `RunAtLoad`, bounded
  `KeepAlive=false`, and stdout/stderr log paths under `.priority-agent/lab/`.
- The command prints explicit `launchctl bootstrap` and `launchctl kickstart`
  hints, while later P0.92-P0.96 slices add explicit install, load, unload,
  supervise, and desktop Workbench invocation paths.
- Added plist rendering coverage for the worker entrypoint and XML escaping.
- This gives the desktop or user-controlled OS layer a concrete launch
  manifest. Periodic desktop-owned supervision remains separate from the
  explicit service management path added later.

### Completed in P0.75 professor-backed sponsor message classification

- Added provider-backed `/lab messages classify <message_id|latest>
  [instructions]` in the Lab Mode shell.
- The Professor model receives the current LabRun context packet and the
  sponsor side-channel message, then must return strict JSON with
  `decision=review|meeting|task|reject` and a short note.
- The runtime maps the decision to the existing persisted sponsor-message
  statuses: `Reviewed`, `ConvertedToMeeting`, `ConvertedToTask`, or `Rejected`.
- Classification records provider token usage under the Professor role and
  appends the existing `sponsor_message_status_updated` audit event.
- The model does not directly execute meetings or tasks. Meeting/task decisions
  still require the existing `/lab messages apply <message_id>` action, which
  preserves the sponsor side-channel boundary.
- Added parser coverage for common model output such as `convert_to_task`.

### Completed in P0.76 explicit Lab role prompt profile versions

- Added optional `prompt_version` to `AgentProfile` and `AgentDefinition`.
- Built-in `lab-professor`, `lab-postdoc`, and `lab-graduate` profiles now
  expose `lab-professor.v1`, `lab-postdoc.v1`, and `lab-graduate.v1` directly
  in their profile definitions.
- Agent contract lines and envelope constraints now include prompt version
  metadata when present, so subagent dispatches can be audited against the role
  prompt version that produced them.
- Added coverage tying the `lab-professor` profile prompt version to the
  persisted `LabRoles::default()` version, reducing drift between runtime
  LabRun state and the runnable agent profile registry.

### Completed in P0.77 LabRun recovery options backend

- Added read-only `/lab recovery` and `/lab recover` to summarize the latest
  LabRun's recovery state.
- The command shows run status, stage, owner, pause reason, paused timestamp,
  resume cursor, open task IDs, lease metadata, scheduler state, and app
  lifecycle checkpoint summary.
- For paused, shutdown-paused, or needs-user runs, the command prints explicit
  next actions: continue with `/lab resume`, inspect with `/lab dashboard`,
  keep paused by doing nothing, or close/cancel with `/lab close` /
  `/lab closeout blocked <note>`.
- The command does not reacquire a lease, start the scheduler, create artifacts,
  or resume mutating work. It is the CLI/runtime backend for the future desktop
  startup prompt.
- Added CLI coverage proving recovery options are shown for a paused run while
  the run remains paused and lease-free.

### Completed in P0.78 top-level LabRun report viewer

- Added read-only `/lab report [list|latest|artifact_id]` and `/lab reports`
  to inspect generated Markdown reports for the latest LabRun.
- The command lists report paths in artifact order, shows the latest report by
  default, and supports selecting a specific artifact report by artifact ID.
- Report viewing reads existing `.priority-agent/lab/runs/<id>/reports/*.md`
  files only. It does not create artifacts, satisfy gates, advance stages,
  reacquire leases, or start background work.
- Added store helpers for report path lookup so command code does not duplicate
  LabRun directory layout details.
- Added CLI coverage proving a generated `ProfessorPlan` report can be listed
  and previewed through the top-level report command.

### Completed in P0.79 safe LabRun open pointer

- Upgraded `/lab open <lab_run_id>` from a state-file path display into a real
  active-pointer switch for inspecting historical or paused LabRuns.
- `LabStore::open_run_pointer()` loads the requested run, refuses to switch if
  another LabRun has a fresh active lease, writes the project `active_run`
  pointer, and records a `lab_run_opened` event.
- Opening a LabRun does not acquire a lease, resume the run, start the
  scheduler, create artifacts, or mutate project code.
- Added CLI coverage with two paused LabRuns proving `/lab open` switches the
  latest LabRun pointer while leaving the selected run paused and lease-free.

### Completed in P0.80 LabRun history list

- Added `LabStore::list_runs()` to list file-backed LabRuns in updated-time
  order.
- Added read-only `/lab runs` to show recent LabRuns, status, stage, owner,
  updated timestamp, pause reason, and the current active-pointer marker.
- The command prints `/lab open <lab_run_id>` guidance so users can inspect or
  recover historical runs without guessing IDs from disk.
- Listing LabRuns does not acquire a lease, resume work, create artifacts, or
  mutate project code.
- Added CLI coverage proving multiple paused LabRuns are listed and the current
  LabRun pointer is marked.

### Completed in P0.81 top-level LabRun review summary

- Replaced the placeholder `/lab review` response with a read-only review
  summary for the latest LabRun.
- The summary reports run status, stage, owner, cycle, artifacts, latest report
  path, graduate task counts, evidence count, blocker state, and the current
  artifact gate state.
- It prints concrete next review actions, including provider artifact review,
  postdoc/professor review bridge commands where stage-appropriate, blocker
  inspection/escalation, and latest report inspection.
- `/lab review artifact <artifact_id> [instructions]` remains the provider
  artifact review command in the Lab Mode shell. The synchronous path now gives
  a clear provider-shell hint instead of a stale "planned later" placeholder.
- Added CLI coverage proving `/lab review` summarizes a generated
  `ProfessorPlan` artifact and no longer returns the placeholder text.

### Completed in P0.82 SponsorMessage steering decision renderer

- Added read-only `/lab messages decision <message_id|latest>` and alias
  `/lab messages decide ...`.
- The command renders the current SponsorMessage status as a Professor steering
  decision: `pending_professor_review`, `no_change`, `open_lab_meeting`,
  `create_postdoc_task`, `reject`, `applied`, or `superseded`.
- The renderer prints message type, urgency, compact message body, and the next
  explicit command, without applying meetings/tasks or mutating status.
- Existing `/lab messages classify`, manual review/meeting/task/reject status
  updates, and `/lab messages apply` remain the only paths that change state.
- Added CLI coverage proving a meeting-converted message renders as
  `open_lab_meeting` while remaining `ConvertedToMeeting`.

### Completed in P0.83 explicit Lab meeting open action

- Added `/lab meeting open [topic]` as the CLI/runtime backend for a future
  TUI/desktop "Lab Meeting" button.
- With an explicit topic, the command writes the same read-only meeting summary
  and Markdown report as `/lab meeting <topic>`.
- Without a topic, the command opens a meeting only when the current
  professor-trigger recommendation has real signals such as blocked graduate
  tasks or repeated failures; otherwise it refuses and points users to manual
  `/lab meeting <topic>`.
- `/lab meeting recommend` now prints the button-style follow-up
  `/lab meeting open <topic>` when a meeting is recommended.
- Added CLI coverage proving no-signal `meeting open` does not create a
  meeting, while a blocked-task recommendation creates a read-only report
  without acquiring the active mutating lease.

### Completed in P0.84 Lab daemon health supervision backend

- Added read-only `/lab daemon health` for desktop/OS supervisors to inspect
  Lab daemon health without starting, stopping, or installing anything.
- The health view combines daemon policy, process-local scheduler status,
  persisted scheduler state, lifecycle checkpoint, last daemon start result,
  last start error, and expected LaunchAgent plist path.
- Health states distinguish `no_policy`, `disabled`, `enabled_not_started`,
  `running_in_process`, `running_persisted`, `paused_restart`,
  `attention_blocked`, `needs_user`, `unhealthy_failed`,
  `unhealthy_start_error`, and related stopped/idle/completed states.
- This does not install/load/unload the OS service; it gives the desktop,
  launchd, or future supervisor a stable read-only health endpoint.
- Added CLI coverage proving enabled-but-not-started policy and persisted
  start errors render distinct health statuses.

### Completed in P0.85 Lab Mode professor intake welcome hint

- Lab Mode startup welcome now includes a Professor intake line in both compact
  and full-width shell banners.
- When no LabRun exists, the hint points users to `/lab propose <idea>` or the
  latest pending proposal approval command.
- When a LabRun exists, the hint shows the active LabRun ID, current stage,
  owner, and the inspection/recovery commands `/lab dashboard` and
  `/lab recovery`.
- Direct Mode welcome output remains unchanged because this is gated on
  `ShellOptions.lab_mode`.
- Added shell coverage for empty intake, pending proposal, and active LabRun
  welcome hints.

### Completed in P0.86 TUI Lab command palette and slash backend

- Registered `/lab` in the TUI command registry as a production Lab command,
  so command palette/help surfaces can discover LabRun workflows by searching
  for Lab, meeting, dashboard, recovery, or daemon-health terms.
- The TUI slash dispatcher now routes `/lab ...` to the shared Lab command
  backend using the current TUI workspace root and current session ID.
- Palette acceptance for `/lab` executes the shared Lab help/status surface
  instead of showing an unknown command.
- This gives the TUI a command-palette entry for Lab Mode without changing
  Direct Mode or adding a separate visual dashboard yet.
- Added registry and TUI app coverage proving `/lab` is searchable/executable
  and that `/lab propose` writes proposal state under the selected workspace.

### Completed in P0.87 desktop Lab command palette entries

- Added desktop command-palette entries for `Lab Dashboard`,
  `Lab Meeting`, `Lab Recovery`, and `Lab Daemon Health`.
- These entries stage `/lab dashboard`, `/lab meeting open`, `/lab recovery`,
  and `/lab daemon health` into the composer instead of auto-submitting them,
  preserving an explicit user confirmation boundary before Lab Mode actions run
  from the desktop UI.
- The entries use existing desktop palette grouping and icons, so no new visual
  surface or Lab dashboard drawer is introduced in this slice.
- Added Playwright coverage proving desktop palette search can stage the Lab
  meeting and daemon-health slash commands into the composer.

### Completed in P0.88 desktop Lab status panel

- Added a read-only Lab status snapshot to the desktop Workbench snapshot.
- The Tauri side reads the selected project's file-backed Lab state through
  `LabStore` and reports no-run, proposal, or active LabRun state without
  creating proposals, acquiring leases, or starting schedulers.
- The snapshot includes run/proposal IDs, proposal/run status, stage, owner,
  needs-user state, cycle/artifact/meeting counts, graduate task counts,
  meeting recommendation state, meeting topic, and latest report path.
- The desktop Workbench now renders a `LabRun` metric and a `Lab status` panel
  beside project-map and symbol-index previews.
- Added Tauri coverage for empty/proposal/run Lab status reads and Playwright
  coverage proving the desktop Workbench shows the Lab status panel and meeting
  recommendation.

### Completed in P0.89 desktop Lab visual actions and blocker/retry panel

- Extended the desktop Lab status snapshot with structured blocker summaries,
  validation retry count, escalated retry count, and latest validation retry
  summary from `LabStore`.
- The desktop Workbench Lab panel now renders blocker/retry history alongside
  stage, owner, task, artifact, meeting, and report state.
- Added an explicit open-report icon button backed by the existing desktop
  `open_file_path` command.
- Added visual Lab action buttons for intervention, continue, and closeout.
  These buttons stage `/lab intervene`, `/lab continue`, and
  `/lab closeout auto` in the composer instead of auto-running them, preserving
  the explicit user confirmation boundary for mutating Lab actions.
- Added Tauri coverage for blocker/retry snapshot fields and Playwright
  coverage proving the visual action buttons stage the expected Lab slash
  commands.

### Completed in P0.90 TUI Lab runtime panel

- Added `Lab` to the TUI runtime panel registry, so `/panel lab` and
  `/runtime lab` render a LabRun-specific status panel inside the existing TUI
  panel system.
- The Lab panel reads the current workspace's file-backed Lab state through
  `LabStore` without starting schedulers, acquiring leases, or mutating LabRun
  state.
- The panel shows proposal/no-run state, active LabRun status, stage, owner,
  needs-user state, cycle/failure/artifact/meeting counts, graduate task
  totals, blocker summaries, validation retry totals, latest retry summary,
  cost summary, meeting recommendation, latest report path, and safe next
  commands.
- `/panel all` now includes the Lab panel alongside context, approvals, hooks,
  tasks, agents, MCP, bridge, trace, diff, and skills.
- Added TUI runtime-panel coverage proving `/panel lab` parses and renders
  file-backed LabRun state with blocker/retry action guidance.

### Completed in P0.91 startup Lab recovery prompts/buttons

- Desktop startup state now prefers recoverable LabRun state over ordinary
  restored-session state when the selected project has a latest LabRun in
  `Paused`, `PausedShutdown`, or `NeedsUser`.
- The desktop startup banner renders a `Lab recovery` prompt with LabRun ID,
  stage, owner, and pause reason.
- The recovery prompt exposes safe buttons: `Resume` stages `/lab resume`,
  `Dashboard` stages `/lab dashboard` and opens Workbench, and `Keep paused`
  only dismisses the prompt in the UI without mutating LabRun state.
- The Lab Mode shell welcome hint now calls out recoverable LabRuns directly and
  points to `/lab resume` plus `/lab recovery`.
- Added Tauri coverage for startup recovery precedence, shell coverage for
  recoverable Lab welcome hints, and Playwright coverage for the desktop
  recovery prompt buttons.

### Completed in P0.92 Lab daemon service install plan

- Added `/lab daemon service [status|install|uninstall|commands] [label]` as a
  desktop/runtime-facing service management surface for the existing
  `lab-daemon` worker.
- `status` and `commands` report the generated plist path, installed
  LaunchAgent path, current file presence, and exact `launchctl bootstrap`,
  `bootout`, `kickstart`, and `print` commands without mutating system service
  state.
- `install` regenerates the worker LaunchAgent plist and copies it into
  `~/Library/LaunchAgents` by default. Tests can redirect this path with
  `PRIORITY_AGENT_LAUNCH_AGENTS_DIR`, so coverage never touches the real user
  LaunchAgents directory.
- `uninstall` removes the installed plist file when present and reports the
  corresponding `launchctl bootout` command the host must run before or around
  file removal.
- Added command coverage for read-only service planning plus install/uninstall
  file management.

### Completed in P0.93 explicit Lab daemon service load/unload

- Extended `/lab daemon service` with explicit `load`, `unload`, and `restart`
  actions.
- `load` regenerates and installs the LaunchAgent plist, then executes
  `launchctl bootstrap <gui-domain> <installed-plist>`.
- `unload` executes `launchctl bootout <gui-domain>/<label>`.
- `restart` executes `launchctl kickstart -k <gui-domain>/<label>`.
- The command layer resolves the real user `gui/<uid>` domain by default, while
  tests can inject `PRIORITY_AGENT_LAUNCHCTL_BIN` and
  `PRIORITY_AGENT_LAUNCHCTL_DOMAIN` to verify command arguments without
  touching the host service manager.
- Added mock-launchctl coverage for load, unload, and restart.

### Completed in P0.94 Lab daemon active supervision backend

- Added `/lab daemon service supervise [label]` as the deterministic backend a
  desktop host or user can call to keep the LaunchAgent-backed `lab-daemon`
  service present.
- Supervision first checks persisted daemon policy. If no policy exists or the
  policy is disabled, it reports a skipped result and does not mutate service
  state.
- When policy is enabled, supervision runs `launchctl print <gui-domain>/<label>`.
  If the service is present, it reports healthy.
- If `launchctl print` fails, supervision regenerates/installs the plist and
  runs the same explicit `launchctl bootstrap` repair path as `service load`.
- Added mock-launchctl coverage for skipped supervision and missing-service
  repair without touching the host service manager.

### Completed in P0.95 desktop Lab daemon supervision command

- Added a Tauri `lab_daemon_supervise` command for the desktop host to invoke
  the same `/lab daemon service supervise` backend against the selected project.
- The command returns the supervise action output plus a refreshed Lab status
  snapshot, so the desktop shell can update its Workbench state after daemon
  repair.
- Added desktop smoke coverage with a mock `launchctl` binary to verify that
  the desktop backend repairs a missing service without touching the real host
  service manager.

### Completed in P0.96 desktop Lab daemon supervision button

- Added a Workbench Lab action button that calls the desktop
  `lab_daemon_supervise` command and refreshes the Workbench snapshot.
- The button is explicit and user-triggered, matching the current LabRun safety
  boundary: desktop can repair daemon service presence on demand before the
  app-scoped timer added in P0.97.
- Added Playwright coverage for the visible Workbench button and refresh path.

### Completed in P0.97 desktop daemon supervision timer

- Added a conservative desktop timer that invokes the same
  `lab_daemon_supervise` path when the desktop app is running and then every
  120 seconds.
- The timer has a local busy guard to avoid overlapping supervise calls.
- Because the backend first checks persisted daemon policy, the timer does not
  start or repair the service unless the user has already enabled Lab daemon
  policy.
- This closes the desktop-visible supervision loop without adding an
  unconditional background mutation path.

### Completed in P0.98 desktop visual open-meeting action

- Added a desktop Workbench Lab action button for recommended meetings.
- The button stages `/lab meeting open` in the composer instead of auto-running
  the command, preserving the explicit user confirmation boundary for opening
  read-only Lab meetings.
- Added Playwright coverage proving the visible meeting action stages the
  expected slash command.

### Completed in P0.99 TUI recommended meeting action text

- Updated `/panel lab` to surface a concrete recommended action when the
  professor-triggered meeting signal is active:
  `recommended: /lab meeting open <topic>`.
- This keeps TUI behavior non-mutating while making the open-meeting action
  visually prominent in the Lab panel.
- Added runtime-panel coverage proving the recommended meeting action is
  rendered from file-backed LabRun state.

### Completed in P0.100 non-interactive Lab command and validation harness

- Added `pa lab --command "<lab command>"` for deterministic, non-interactive
  Lab command execution without requiring a provider or interactive terminal.
- Added `pa lab --command "<lab command>" --with-provider` for
  provider-backed non-interactive Lab commands that need an active model and
  `ToolContext`, such as sponsor-message classification.
- The command accepts both bare command bodies such as `dashboard` and prefixed
  commands such as `/lab dashboard`, then routes through the same file-backed
  Lab command backend.
- Added coverage proving `pa lab --command` can create a proposal through the
  deterministic Lab backend without provider setup.
- Added `scripts/lab-live-validation.sh`:
  - default `--offline` mode builds the binary, creates a temporary Lab
    workspace, runs proposal/approval/dashboard/meeting/context/daemon-service
    checks through `pa lab --command`, and writes a report under
    `target/lab-live-validation/`;
  - optional live modes record a sponsor intervention, classify it via
    `pa lab --command ... --with-provider`, enable hybrid daemon policy, and
    run `pa lab-daemon`, giving live provider validation a stable
    evidence-producing entrypoint;
  - `--live-control-plane` validates provider-backed Professor/control-plane
    paths without requiring graduate tool use;
  - `--live-graduate` validates full graduate tool use, runtime verification,
    worktree review/merge/cleanup, and daemon execution;
  - `--live` remains a backwards-compatible alias for `--live-graduate`.
- Verified `scripts/lab-live-validation.sh --offline`.

### Completed in P0.101 graduate JSON binding validation surface

- Added `LabOrchestrator::bind_graduate_agent_json_for_task_latest()` so the
  same structured graduate result parser used after real agent execution can be
  exercised directly from a validation harness.
- Added `/lab task bind-json <task_id> <json_file>`:
  - reads a graduate agent JSON output file;
  - accepts the same `graduate_result` contract and wrapped `{"result":"..."}`
    shape that agent-tool outputs commonly use;
  - enforces the graduate task `allowed_scope` and required validation presence
    through the existing result-binding path;
  - writes a `GraduateResult` artifact and Markdown report;
  - completes the graduate task and satisfies the `graduate_work` gate when the
    active LabRun is in that stage.
- Added command coverage proving `bind-json` converts a structured graduate
  output file into a completed task and bound `GraduateResult`.
- Extended `scripts/lab-live-validation.sh --offline` to create a graduate task,
  bind a wrapped graduate JSON result, verify gate satisfaction, and verify the
  task is completed.
- Verified the new offline evidence report at
  `target/lab-live-validation/20260619-212002/report.md`.

### Completed in P0.102 DeepSeek live LabRun control-plane validation

- Ran `scripts/lab-live-validation.sh --live` with
  `PRIORITY_AGENT_DEFAULT_PROVIDER=deepseek` and
  `DEEPSEEK_MODEL=deepseek-v4-flash`.
- Fixed the live validation harness so it follows the real LabRun state machine:
  sponsor intervention moves the run to `NeedsUser`, so the harness now resumes
  the LabRun after provider-backed sponsor classification before running
  graduate work.
- Added provider-backed non-interactive Lab command `ToolContext` wiring for the
  lazy `AgentManager`, allowing
  `pa lab --command "task run <task_id>" --with-provider` to execute real
  Lab graduate subagents instead of failing with missing `AgentManager`.
- Strengthened graduate result contract handling:
  - successful agent execution without bindable `GraduateResult` JSON now marks
    the dispatch `Failed`, blocks the graduate task, records a LabRun failure,
    and stores a compact result preview for diagnosis;
  - the graduate result parser accepts raw JSON, fenced JSON, prose-wrapped JSON,
    and `{"result":"..."}` wrappers while still requiring structured validation
    attempts;
  - `lab-graduate` and generated graduate task prompts now explicitly forbid
    XML-like pseudo tool tags such as `<bash>` and require tool use for file
    changes and validation;
  - `lab-graduate` dispatches now include `file_write`, so scoped new-file tasks
    can be executed in isolated worktrees.
- Improved provider error diagnostics for professor sponsor-message
  classification by surfacing the full error chain.
- DeepSeek live control-plane evidence:
  - report: `target/lab-live-validation/20260619-214442/report.md`;
  - sponsor classification completed with a provider-backed Professor decision;
  - graduate subagent dispatch returned a bindable structured result with
    `agent_b2b9ced6`;
  - live graduate task completed under the prior JSON-binding acceptance rule
    with artifact
    `artifact_graduateresult_afebeb462fb3495e9e80e29e2fb4a9fe`;
  - daemon worker completed with exit status `0`.

### Completed in P0.103 graduate runtime verification and worktree live harness

- Tightened the graduate execution acceptance rule: a successful subagent JSON
  result is no longer enough to complete a graduate task.
- `execute_graduate_task_latest_with_context()` now performs parent/runtime
  verification before binding `GraduateResult`:
  - resolves the persisted isolated worktree for the graduate agent;
  - collects actual git changed paths from that worktree;
  - rejects empty real diffs even when the model claims `changed_files`;
  - enforces `allowed_scope` against runtime-observed paths;
  - runs every `required_validation` command in the verified worktree and
    records runtime validation attempts only on success.
- `pa lab --command ... --with-provider` and `pa lab-daemon` now share a
  project-local persistent Lab session store at
  `.priority-agent/lab/sessions.db`, so subagent worktree state survives across
  non-interactive Lab commands.
- Fixed subagent durable-state persistence so completion updates preserve the
  original `isolated_worktree` payload instead of overwriting it with result
  metadata.
- Extended `scripts/lab-live-validation.sh --live` to require:
  - provider-backed graduate task execution;
  - runtime validation of the graduate worktree;
  - `/lab task worktree review <task_id>`;
  - `/lab task worktree merge <task_id>`;
  - proof that the merged file exists in the parent validation workspace;
  - `/lab task worktree cleanup <task_id> force`.
- Split live validation modes:
  - `--live-control-plane` is the provider-backed Professor/daemon smoke for
    models such as DeepSeek v4 flash that are useful for control-plane work but
    not yet certified for graduate tool use;
  - `--live-graduate` is the full certification gate for providers that should
    autonomously edit files through tools;
  - `--live` aliases `--live-graduate` so old callers keep the strict behavior.
- Updated `agent_merge` so new files created by a graduate isolated worktree are
  prepared with `git add -N` and merged as tracked diffs instead of being
  rejected solely because they are initially untracked.
- Updated parent dirty-worktree filtering for agent merges to ignore Lab's own
  `.priority-agent/` runtime state in addition to `.claude/worktrees/`.
- Strengthened `lab-graduate` profile and generated graduate prompts: file
  tasks must call `file_write`/`file_edit`, validation tasks must call `bash`,
  and inability to call tools must be reported as a blocker rather than a done
  claim.
- Added coverage for:
  - runtime rejection when no actual file changes are observed;
  - runtime validation inside the isolated worktree;
  - preserving isolated worktree payload across subagent completion;
  - agent worktree review/merge/cleanup with real git flow.
- DeepSeek v4 flash follow-up evidence under the stricter rule:
  - reports: `target/lab-live-validation/20260619-220330/report.md` and
    `target/lab-live-validation/20260619-220702/report.md`;
  - sponsor classification still completed;
  - the graduate agent returned structured JSON but used no tools
    (`tools_used=[]`);
  - no real file changes were observed in the isolated worktree;
  - runtime correctly failed the dispatch with
    `graduate runtime verification found no actual file changes`.
- Current certification conclusion: DeepSeek v4 flash is usable for Professor
  control-plane classification/drafting paths, but is not yet certified for
  autonomous Lab graduate code-writing under the tool-backed runtime contract.
- Recorded this product-level distinction in `docs/PROVIDER_CERTIFICATION_MATRIX.md`
  under `LabRun Live Certification`.

### Completed in P0.104 graduate provider certification gate

- Added a runtime Lab graduate provider certification gate.
- Known-unsupported graduate providers are blocked before launching a costly
  subagent run, while unverified providers can still be attempted for future
  certification.
- DeepSeek `deepseek-v4-flash` is currently marked known-unsupported for
  autonomous graduate code-writing because the live certification evidence
  showed structured completion claims with `tools_used=[]` and no real worktree
  diff.
- The gate records a failed graduate dispatch, blocks the graduate task, and
  records a LabRun failure with a clear certification message.
- `PRIORITY_AGENT_LAB_ALLOW_UNCERTIFIED_GRADUATE_PROVIDER=1` can override the
  gate for explicit experimental runs.
- `pa lab --command ... --with-provider` now includes `provider_id` metadata
  from `PRIORITY_AGENT_DEFAULT_PROVIDER`, while the gate can also infer DeepSeek
  from the model name.
- `scripts/lab-live-validation.sh --live-graduate` now prints the graduate task
  run output before exiting when certification fails, making known provider
  limitations diagnosable instead of a silent grep failure.
- Added coverage proving DeepSeek v4 flash is blocked before an AgentManager run
  and that unknown providers remain unverified rather than pre-blocked.
- Latest live evidence:
  - `scripts/lab-live-validation.sh --live-control-plane` with DeepSeek v4
    flash passed at
    `target/lab-live-validation/20260619-223257/report.md`;
  - `scripts/lab-live-validation.sh --live` with DeepSeek v4 flash now fails
    before another graduate agent run, with the expected certification message
    in `target/lab-live-validation/20260619-223243/15-live-task-run.txt`.

### Completed in P0.105 file-backed LabRun query index

- Added a derived `LabRunIndex` / `LabRunIndexEntry` model for queryable
  dashboard/recovery summaries without making the index a second source of
  truth.
- `LabStore::save_run()` and proposal approval now refresh
  `.priority-agent/lab/runs_index.json` from the authoritative `LabRun`
  state.
- Added `LabStore::rebuild_runs_index()` so older or missing indexes can be
  regenerated from `runs/*/state.json`.
- `/lab runs` now rebuilds and renders the index, including the index path,
  task count, artifact count, stage, owner, and pause state for recent runs.
- Added coverage proving index updates after state changes and can be rebuilt
  after deletion, plus command coverage for the indexed `/lab runs` surface.
- Tightened `ArtifactGate::is_satisfied()` so blocked gates and
  `needs_revision` gates are not treated as satisfied merely because required
  fields are present.
- Fixed `/lab review` to read the persisted current gate first, falling back to
  a required-gate template only when no gate has been written yet.
- Verified the broader Lab slice with `cargo test -q --lib lab`.

### Completed in P0.106 provider-backed Professor intake proposal draft

- Added `LabProposalIntakeDraft` so Professor intake can fill structured
  proposal fields before the user formally approves a LabRun.
- Added `draft_lab_proposal_with_provider()` for provider-backed Professor
  intake. The provider returns strict JSON with problem statement, desired
  outcome, scope, non-goals, constraints, risks, success criteria, recommended
  mode, and professor rationale.
- Added `/lab propose llm <idea>` in provider-backed Lab command handling.
  This creates a structured proposal only; it does not create a LabRun and does
  not bypass `/lab approve <proposal_id>`.
- Kept deterministic `/lab propose <idea>` and `/lab start <goal>` unchanged
  for offline and no-provider workflows.
- Added parser, provider draft, and command coverage proving structured
  proposals persist and no LabRun is created before explicit approval.

### Completed in P0.107 durable Professor steering decision artifacts

- Added `ProfessorSteeringDecision` as a first-class Lab artifact type owned by
  the Professor.
- `/lab messages decision <message_id|latest>` now writes a
  `ProfessorSteeringDecision` artifact and Markdown report while still leaving
  the SponsorMessage status unchanged.
- The artifact records source message ID, status, message type, urgency,
  decision, rationale, next action, and compact message summary.
- Applying a meeting or task remains explicitly separated through
  `/lab messages apply <message_id> [note]`; rendering/writing the steering
  decision does not create meetings, graduate tasks, or code changes.
- Added command coverage proving the steering artifact/report is persisted and
  the source message remains `ConvertedToMeeting` until explicit apply.

### Completed in P0.108 autonomous postdoc revision resume

- Added a deterministic runtime path that resumes a LabRun from a pending
  Professor-owned `LabRevisionTask` back to `postdoc_plan`.
- `/lab repair [note]` now exposes the same resume path manually, without
  changing the rejected professor review into an accepted gate.
- Strict scheduler and hybrid scheduler paths now detect a blocked
  `professor_review` caused by required revisions, persist the revision task,
  and immediately return the run to Postdoc ownership for repair planning.
- The next `PostdocPlan` still consumes the pending revision task through the
  existing artifact/evidence path, marks it `validation_status=consumed`, and
  records the consumption event.
- Added scheduler coverage proving professor rejection creates the revision
  task, resumes at `postdoc_plan`, and the following postdoc plan consumes the
  task.

### Completed in P0.109 optional SQLite LabRun index import

- Added `lab_index.sqlite3` as an optional query mirror under
  `.priority-agent/lab/`.
- The SQLite schema now covers `lab_runs`, `lab_artifacts`, `lab_events`, and
  `lab_tasks`.
- `LabStore::rebuild_sqlite_index()` imports existing file-backed LabRun state,
  artifacts, events, and graduate tasks into the SQLite mirror while keeping
  JSON/JSONL files as the authoritative source of truth.
- `LabStore::rebuild_runs_index()` now also refreshes the SQLite mirror, so the
  existing `/lab runs` rebuild path prepares both file-backed and SQLite
  indexes.
- Added store coverage proving the four SQLite tables are populated from real
  file-backed LabRun, artifact, event, and task records.

### Completed in P0.110 indexed status visibility

- `/lab status` still reads the authoritative latest LabRun from file-backed
  state, preserving the JSON/JSONL source-of-truth boundary.
- The status output now includes the derived `runs_index.json` summary for the
  latest LabRun when present.
- The status output also includes the optional SQLite mirror summary with run,
  artifact, event, and task counts when `lab_index.sqlite3` has been rebuilt.
- Added command coverage proving `/lab runs` rebuilds indexes and `/lab status`
  reports both file-backed and SQLite index summaries.

### Completed in P0.111 indexed dashboard summary

- Added a SQLite dashboard summary query over the derived `lab_index.sqlite3`
  mirror.
- `/lab dashboard` still reads authoritative run, task, retry, cost, scheduler,
  and meeting state from existing runtime/file-backed sources.
- When the SQLite mirror exists, `/lab dashboard` now also reports indexed run,
  artifact, event, and task counts plus the latest Professor and Postdoc
  artifact summaries.
- Added command coverage proving `/lab runs` rebuilds indexes and
  `/lab dashboard` consumes the SQLite mirror for Professor/Postdoc artifact
  state.
- Extended `scripts/lab-live-validation.sh --offline` to verify `/lab runs`,
  `/lab status`, and `/lab dashboard` indexed summaries.

### Completed in P0.112 DeepSeek live control-plane revalidation

- Re-ran `scripts/lab-live-validation.sh --live-control-plane` with
  `PRIORITY_AGENT_DEFAULT_PROVIDER=deepseek` and
  `DEEPSEEK_MODEL=deepseek-v4-flash` after the indexed status/dashboard work.
- Latest report:
  `target/lab-live-validation/20260620-000209/report.md`.
- The live control-plane run passed proposal/approval, dashboard, meeting
  recommendation, professor context, daemon service status, graduate JSON
  binding, and indexed runs/status/dashboard summaries.
- DeepSeek Professor sponsor-message classification completed and converted
  the message to a meeting decision.
- `lab-daemon` completed with exit status `0` in hybrid daemon smoke mode.
- Graduate tool-backed code execution remains intentionally skipped in
  `--live-control-plane`; `deepseek-v4-flash` remains blocked for autonomous
  graduate code-writing by the provider certification gate.
- Re-ran `scripts/lab-live-validation.sh --live` with the same DeepSeek model
  and confirmed the graduate path fails before launching another subagent run,
  with the certification message in
  `target/lab-live-validation/20260620-000318/15-live-task-run.txt`.

### Completed in P0.113 provider certification status surface

- Added `/lab provider` for provider-backed Lab command contexts.
- The command reports active provider ID, model, graduate certification state,
  whether graduate execution is allowed, whether the experimental override is
  enabled, and the exact live validation commands to run.
- The offline `/lab provider` entrypoint now gives a clear provider-shell hint
  instead of silently guessing from environment state.
- Extended `scripts/lab-live-validation.sh --offline` to verify the no-context
  provider guard.
- Extended live validation modes to write and verify a provider certification
  report after provider-backed sponsor classification.
- Verified with DeepSeek v4 flash live control-plane at
  `target/lab-live-validation/20260620-001212/report.md`; the provider report
  shows `Graduate certification: known_unsupported` and
  `Graduate execution allowed: false`.

### Completed in P0.114 same-provider subagent comparison diagnostic

- Added `/lab provider compare` for provider-backed Lab command contexts.
- The command runs a same-provider diagnostic that compares:
  - the generic `implementer` subagent path through the existing `AgentTool`;
  - the formal Lab graduate dispatch path through
    `LabOrchestrator::execute_graduate_task_latest_with_context()`.
- The generic path now reports provider/model, profile, context mode,
  allowed tools, current `tools_used`, a durable-state isolated-worktree file
  proof, a short result preview, and any error.
- The Lab graduate path reports dispatch status, dispatch ID, task ID, agent
  ID, result artifact ID, and the exact gate or execution error.
- Extended live validation to run `/lab provider compare` after the
  sponsor-classification resume step, so the LabRun is Active before the
  graduate gate is evaluated.
- DeepSeek v4 flash live comparison evidence:
  `target/lab-live-validation/20260620-003844/13a-provider-compare.txt`.
  Result:
  - generic `implementer` subagent completed, but `tools_used` remained empty
    and the expected smoke file did not exist in the isolated worktree;
  - the generic result text claimed `file_write`, but runtime file proof did
    not confirm the claim;
  - formal Lab graduate dispatch was correctly blocked before agent launch by
    the provider certification gate.
- Current conclusion: this is not just a `lab-graduate` prompt/envelope issue.
  DeepSeek v4 flash still lacks hard evidence for autonomous tool-backed
  coding even through the generic subagent path, and the generic subagent
  runtime still needs better tool-use instrumentation because
  `AgentResult.tools_used` is currently not reliable proof.

### Completed in P0.115 runtime-observed subagent `tools_used` instrumentation

- Added runtime-observed tool-name accumulation to the non-streaming
  `ConversationLoop`.
- `LoopResult` now carries ordered, de-duplicated `tools_used` names for tool
  calls that entered the runtime tool execution pipeline.
- `QueryResult` now carries those names back to callers of
  `QueryEngine::query_with_tools_with_system_prompt()`.
- `Agent` now stores the query result's runtime-observed tool names and returns
  them through `AgentResult.tools_used` instead of the previous hard-coded
  empty vector.
- Existing agent-tool persistence and trace metadata now receive real subagent
  tool-use data when a provider actually emits tool calls.
- Added coverage:
  - turn-loop state records executed tool names once and in order;
  - e2e mock-provider flows assert `LoopResult.tools_used` for pure text,
    single-tool, multi-tool, and tool-failure flows;
  - existing `agent_tool` coverage still verifies persisted `tools_used`.
- Re-ran DeepSeek v4 flash `/lab provider compare` after the instrumentation:
  `target/lab-live-validation/20260620-121036/13a-provider-compare.txt`.
  Historical result at that point: generic `implementer` still reported
  `tools_used: none`, the isolated worktree smoke file was absent, and Lab
  graduate remained blocked by the provider certification gate. This was later
  resolved for the generic subagent path in P0.117 by durable completion
  recovery, top-level `tools_used` propagation, and refreshed live provider
  evidence.
- Re-ran `scripts/lab-live-validation.sh --live` with DeepSeek v4 flash and
  confirmed the full graduate certification gate still fails before launching a
  graduate subagent:
  `target/lab-live-validation/20260620-121300/15-live-task-run.txt`.
- Cross-provider graduate certification with a stronger coding provider remains
  pending because the current local provider configuration only exposes
  DeepSeek v4 flash.

### Completed in P0.117 generic subagent product closeout

- Fixed foreground generic subagent recovery when the in-memory completion
  waiter times out but the durable completion sink has already persisted the
  result artifact.
- Propagated runtime-observed `tools_used` to the parent `agent` tool result
  data so provider diagnostics no longer have to infer tool use from text.
- Added durable `task_id` lifecycle semantics for product use:
  - `resume` by `task_id` or `agent_id` can read durable state without a live
    `AgentManager`;
  - `cancel` by `task_id` can mark durable state cancelled while preserving
    cleanup metadata.
- Expanded API and desktop workbench projections with artifact status,
  `tools_used`, proof kind, completion sink, and recovery fields.
- Refreshed DeepSeek v4 flash provider compare with current code:
  `target/lab-live-validation/20260620-generic-subagent-product-closeout/evidence/provider-compare.txt`.
  Current result: generic foreground subagent succeeds with
  `tools_used: bash,file_write,file_read` and isolated-worktree file proof;
  background subagent also succeeds with hard file proof. Formal Lab graduate
  remains blocked by the provider certification gate and is still not certified.

### Completed in P0.118 Lab graduate durable subagent alignment

- Reused the generic subagent durable task algorithm for Lab graduate dispatch.
- `GraduateTask -> AgentTool` params now include stable
  `task_id = lab-graduate-<graduate_task_id>` and explicit foreground
  execution, so Lab graduate runs participate in the same durable read/resume/
  cancel/artifact path as generic subagents.
- The generated graduate envelope records the same durable agent task ID as a
  runtime constraint for auditability.
- Runtime graduate verification now resolves the isolated worktree by the
  returned `agent_id` first, then falls back to the stable durable graduate
  task ID. This keeps verification strict while allowing recovery when the
  parent only has the durable task handle.
- Updated the DeepSeek v4 flash Lab graduate certification message to reflect
  the current evidence: generic subagent tool use with `tool_choice=auto` is
  proven, but formal Lab graduate certification still requires isolated task
  execution, observed file changes, required validation, and worktree
  review/merge/cleanup proof.
- Added regression coverage for graduate dispatch task IDs and durable-task-ID
  worktree fallback.

### Completed in P0.116 provider tool-call root-cause probes

- Added `/lab provider diagnose-tools` to run direct provider function-call
  probes through the same Lab provider shell used by live validation.
- The diagnostic now covers:
  - a minimal echo tool with `tool_choice=auto`;
  - `tool_choice=required` and forced named-tool attempts;
  - production runtime tool schemas for `file_write`, `file_write,bash`, and
    the generic implementer allowed-tool surface
    `file_write,file_edit,bash,diff`.
- Integrated the diagnostic into `scripts/lab-live-validation.sh
  --live-control-plane`; artifacts now include
  `12b-provider-tool-diagnostics.txt`.
- Fixed two real runtime mismatches exposed by the diagnostics:
  - explicit subagent `allowed_tools` now bypass route-scoped request exposure,
    after the registry has already applied the explicit allowlist;
  - prepared requests with non-empty tool schemas now set
    `tool_choice=auto` instead of relying on provider defaults.
- Hardened forked subagent prompting:
  - the child task message repeats the directive summary instead of only
    saying "execute the directive above";
  - subagent system prompts now explain that exposed tools are authorized for
    the scoped subagent task while runtime gates still enforce safety.
- Shortened `/lab provider compare`'s generic smoke timeout to 90 seconds so
  live-control-plane diagnostics fail closed with a concrete timeout instead
  of hanging for several minutes.
- DeepSeek v4 flash evidence after the fixes:
  - direct provider probes succeed for `tool_choice=auto` with minimal and
    production runtime tool schemas:
    `target/lab-live-validation/20260620-125252/12b-provider-tool-diagnostics.txt`;
  - DeepSeek rejects `tool_choice=required` and forced named-tool calls in
    thinking mode with `invalid_request_error`;
  - the full generic `implementer` Agent loop still times out without
    runtime-observed `tools_used` or isolated-worktree file proof:
    `target/lab-live-validation/20260620-125252/13a-provider-compare.txt`.
- Current conclusion: DeepSeek v4 flash is no longer classified as "raw
  function calling impossible"; it can emit direct `file_write` tool calls with
  `tool_choice=auto`. It remains uncertified for Lab graduate code-writing
  because the complete Agent loop has not produced real worktree mutation and
  validation proof.

### Completed in P0.117 opencode-aligned subagent session policy

- Audited local opencode source and aligned the Priority Agent subagent path
  with its most important runtime idea: subagents should run as scoped child
  sessions with explicit tool exposure and inherited permission rules, not as
  prompt-only wrappers.
- Added `src/agent/subagent_session.rs` with `SubagentSessionPolicy`.
- The policy now derives:
  - the exact tools exposed to the child model;
  - inherited parent deny rules, including a parent read-only mode guard;
  - profile-level disallowed tool denies;
  - read-only profile mutation denies;
  - the permission mode and session-local permission rules passed into the
    child execution loop.
- `AgentConfig` can now carry the derived subagent session policy.
- `QueryOptions` can now pass scoped permission mode and session-local
  permission rules into `ConversationLoop`, so the child run uses the same main
  tool pipeline while applying subagent-specific policy.
- `AgentTool` now derives the policy before spawning a child, passes the
  policy into the child agent, and persists both requested tools and actually
  exposed tools in the durable task payload.
- This keeps the DeepSeek diagnosis honest: if a provider claims work without
  `tools_used` or file proof, the runtime can now distinguish "model did not
  call an exposed tool" from "runtime never exposed the tool."
- Added coverage for:
  - read-only subagents hiding mutating tools while keeping read tools;
  - parent deny rules being inherited into child policy;
  - existing route-scoped explicit allowed-tool behavior.
- Remaining larger alignment with opencode: persistent subagent sessions with
  `task_id` resume and full parent/child transcript threading are still pending.

### Completed in P0.118 durable subagent `task_id` and child session threading

- Added an opencode-style `task_id` parameter to the `agent` tool.
- `task_id` is now the stable durable identity for a subagent task, while
  `agent_id` remains the runtime identity for one concrete execution attempt.
- Single-agent dispatches with a supplied `task_id` now persist
  `agent_task_states.task_id` as that stable ID instead of overwriting it with
  the generated runtime `agent_id`.
- Reusing a `task_id` with an existing running, completed, failed, timed-out, or
  cancelled task now reads the durable task state instead of silently launching
  a duplicate child run.
- `action=read` now works with `task_id` directly, and resume/cancel can resolve
  a `task_id` back to the current runtime `agent_id` when a durable state
  exists.
- Added durable child session creation for subagent runs when `SessionStore` is
  available:
  - child session ID: `<parent_session_id>:subagent:<task_id>`;
  - `parent_session_id` points back to the parent conversation;
  - workspace root is carried forward;
  - the child `ConversationLoop` receives this child session ID so its durable
    transcript can be separated from the parent.
- `AgentConfig` and `QueryOptions` now carry the child `SessionStore` and child
  session ID down to the main `ConversationLoop`.
- Durable task payloads now include `task_id`, `parent_session_id`, and
  `child_session_id`, making parent/child relationships inspectable without
  parsing prompt text.
- Added coverage for task ID sanitization and child session parent-link
  persistence.
- Remaining opencode-alignment gap: background subagent completion still needs a
  runtime completion hook that persists the final artifact and injects a
  synthetic result into the parent timeline without requiring a foreground wait.

### Completed in P0.119 subagent completion sink and old artifact-path cleanup

- Added `AgentCompletionSink` to `AgentManager`.
- The AgentManager result collector now owns durable subagent completion
  persistence:
  - writes the `agent_artifacts` row;
  - updates `agent_task_states` with final status, result artifact ID,
    `tools_used`, confidence, conflict flag, and completed runtime `agent_id`;
  - preserves existing cleanup hooks, permission requests, transcript path, and
    task payload fields;
  - injects a synthetic `<subagent-result ...>` assistant event into the parent
    session through `SessionEventWriter`, with child output XML-escaped so the
    child cannot break the result container boundary.
- `AgentTool` now attaches a completion sink when a `SessionStore` is available,
  so foreground and future background subagent runs share the same completion
  persistence path.
- Added `background=true` for single `agent` tool dispatches. The tool returns
  after launch with a running status, and the AgentManager completion sink
  persists the final artifact plus parent-session synthetic result later.
- Removed the older foreground-only `persist_agent_artifact` call path from
  `AgentTool`; artifact/state writes now flow through the AgentManager
  completion collector instead of being duplicated after `wait_for_result()`.
- Added coverage proving completion sink writes artifact state and injects a
  parent-session event.

### Completed in P0.120 subagent UI, live validation, and restart recovery closeout

- TUI runtime agent panel now renders durable subagent task IDs, runtime agent
  IDs, child session IDs, result artifact IDs, completion sink source, recovery
  status, and artifact preview.
- Desktop Workbench snapshot now includes recent active-session subagent tasks,
  including `task_id`, `agent_id`, profile/role, result artifact ID, result
  preview, completion sink, child session ID, and `paused_restart` recovery
  metadata.
- Desktop Workbench renders a dedicated `Sub-agents` section so background
  subagent results and interrupted tasks are visible without manually querying
  SQLite.
- The HTTP agent-task projection now exposes child session, result artifact,
  completion sink, and recovery fields.
- Added `SessionStore::recover_interrupted_agent_task_states()`:
  - converts stale `running`/`stopping` subagent states to `paused_restart`;
  - clears in-progress tool IDs;
  - preserves permission requests, cleanup hooks, transcript path, payload, and
    artifact linkage;
  - records `previous_status`, `recovery_status`, `recovery_reason`, and
    `recovery_action` in the task payload.
- Interactive runtime session binding, desktop session-store startup, and the
  Lab daemon worker now call the recovery hook.
- `/lab provider compare` now includes a provider-backed background subagent
  smoke path that launches `agent(background=true, task_id=...)`, waits for the
  AgentManager completion sink, and reports durable status, artifact ID,
  completion sink, tools used, and file proof.
- `scripts/lab-live-validation.sh --live-graduate` now asserts that provider
  comparison output includes the background subagent completion-sink path.
- Boundary: process restart recovery is honest. A subagent that lived only in
  the previous process is marked `paused_restart`; the runtime does not pretend
  it kept executing after the process died. Continuing it still requires an
  explicit relaunch/resume action using the durable `task_id`.

### Completed in P0.121 Lab graduate worktree durable task fallback

- `/lab task worktree <review|merge|cleanup> <task_id>` now aligns with the
  generic subagent durable task path.
- The command still prefers a concrete runtime `agent_id` when the graduate
  dispatch has one.
- If `agent_id` is unavailable, the command falls back to the persisted
  graduate dispatch `agent_tool_params.task_id`, such as
  `lab-graduate-<graduate_task_id>`, and passes it to `WorktreeTool` as a
  durable task reference.
- The LabRun worktree event now records both the original dispatch `agent_id`
  and the actual `agent_ref_kind`/`agent_ref` used for review/merge/cleanup.
- This removes the old command-layer assumption that graduate worktree
  operations are impossible without `agent_id`; the lower WorktreeTool/session
  store proof path still decides whether a durable task has a real isolated
  worktree.
- Added command-level integration coverage that creates a real git repository,
  a real isolated graduate worktree, a durable `lab-graduate-...` task state,
  and proves `/lab task worktree review <task_id>` can inspect that worktree
  through the durable task ID fallback.
- Added command-level merge/cleanup coverage for the same durable task ID path:
  a graduate worktree edit is merged back into the parent repository through
  `/lab task worktree merge <task_id>`, then removed through
  `/lab task worktree cleanup <task_id> force`.
- `lab_graduate_worktree_action` events now persist WorktreeTool runtime proof
  metadata, including `result_data` and a bounded `result_content_preview`, so
  later LabRun review can inspect merge kind, dirty state, paths, and durable
  task references without treating graduate prose as proof.
- `/lab review` now surfaces the latest graduate worktree proof lines from
  those events, including action, success, durable agent reference, merge kind,
  dirty state, and worktree path summary.
- `/lab dashboard` now surfaces the same graduate worktree proof summary, so
  the status panel used by desktop/TUI callers does not hide recent graduate
  runtime evidence.

### Completed in P0.122 Postdoc handoff consumes graduate worktree proof

- `PostdocIntegrationSummary` generation now reads recent
  `lab_graduate_worktree_action` events for the current LabRun.
- Successful worktree actions are folded into `accepted_results` as runtime
  worktree proof, including action, durable task reference, merge kind, dirty
  state, and worktree path summary.
- Failed worktree actions are folded into `remaining_risks`, so a failed
  runtime proof path is not hidden behind graduate prose.
- Each consumed worktree event is added to the artifact `evidence_refs` as an
  `event:<event_id>` reference, which carries the proof through rendered
  postdoc reports and later professor review prompts.
- Added orchestration coverage proving a graduate worktree merge event survives
  into the formal postdoc integration artifact and report with
  `merge_kind=tracked_diff` evidence.

### Completed in P0.123 Graduate scheduler advances after verified result

- The strict LabRun scheduler now recognizes the state where `graduate_work`
  has no open graduate tasks and the `GraduateResult` artifact gate is already
  satisfied.
- In that state, the scheduler advances directly from `graduate_work` to
  `postdoc_review`, instead of incorrectly blocking with "requires a queued
  GraduateTask".
- This closes the handoff gap after a tool-backed graduate subagent run has
  produced a verified `GraduateResult`.
- Added scheduler coverage proving a completed graduate task plus satisfied
  graduate gate moves the LabRun into postdoc ownership.

### Completed in P0.124 Durable graduate subagent result sync

- Added `LabOrchestrator::sync_graduate_agent_task_latest_with_context()` as the
  explicit bridge from generic durable subagent completion state back into
  LabRun graduate artifacts.
- Added `/lab task sync <task_id>` for shell/TUI callers to sync a completed
  durable `lab-graduate-<task_id>` subagent result after foreground timeout,
  background completion, or restart recovery.
- The sync path only accepts durable task states whose profile is
  `lab-graduate`, whose state is `completed`, and whose completion artifact is
  present and completed.
- The completion artifact must still parse as the `graduate_result` contract,
  but parsing is not enough: the runtime reuses graduate verification to inspect
  the persisted isolated worktree, collect actual changed files, and run the
  task's required validation commands.
- Successful sync writes a `GraduateResult`, completes the graduate task,
  satisfies the graduate gate when applicable, records `agent_task:<id>` and
  `agent_artifact:<id>` evidence refs, and updates the matching Lab graduate
  dispatch to `Succeeded`.
- Failed sync attempts for completed but unbindable or unverifiable subagent
  results block the graduate task, record a LabRun failure, and mark the matching
  dispatch as `Failed` instead of treating subagent prose as proof.
- Added orchestrator and command coverage using an in-memory `SessionStore`, a
  durable lab-graduate task state, a completion artifact, and a real isolated
  git worktree file proof.

### Completed in P0.125 Scheduler auto-syncs completed durable graduates

- The strict `graduate_work` scheduler now checks in-progress graduate tasks for
  completed durable `lab-graduate-<task_id>` subagent state before blocking on
  "already in progress".
- When a completed durable Lab graduate state is present, the scheduler reuses
  the P0.124 sync path: completion artifact parsing, isolated-worktree changed
  file proof, required validation, `GraduateResult` binding, task completion,
  and dispatch status update all stay on the same runtime-verified path.
- If the sync completes the last open graduate task and the graduate gate is
  satisfied, the scheduler advances from `graduate_work` to `postdoc_review` in
  the same step.
- If other graduate tasks remain open, the scheduler returns a progress step and
  keeps the run in `graduate_work` for the next task.
- If a completed durable state cannot be synced, the scheduler stops as blocked
  after the sync path records the graduate failure instead of treating the
  subagent output as proof.
- Completed durable state without a completion artifact is treated as a failed
  graduate proof path: the graduate task is blocked, the matching dispatch is
  marked `Failed`, and LabRun failure accounting is updated.
- Added scheduler coverage proving an in-progress graduate task with completed
  durable subagent state is synced and advanced to postdoc ownership without a
  manual `/lab task sync`.
- Added scheduler failure coverage proving a completed durable state with no
  result artifact blocks the graduate task and records the failed dispatch.

### Completed in P0.126 Hybrid run crosses durable graduate completion

- Added hybrid-run coverage proving `/lab run hybrid` can continue across a
  completed durable graduate subagent result without manual `/lab task sync`.
- The hybrid loop starts at `graduate_work` with an in-progress graduate task,
  a completed durable `lab-graduate-<task_id>` subagent state, a completion
  artifact, and an isolated worktree file proof.
- The first hybrid step routes through the strict scheduler, syncs the durable
  graduate result, writes the verified `GraduateResult`, updates the graduate
  dispatch to `Succeeded`, and advances to `postdoc_review`.
- The same hybrid run then executes deterministic postdoc integration and
  professor review bridges, reaching `user_report` and `NeedsUser` in one
  bounded run.
- This is the first offline proof that the provider/hybrid LabRun loop can
  cross the graduate durable-completion boundary and finish the
  graduate -> postdoc -> professor handoff without user intervention.

### Completed in P0.127 Provider planning queues graduate, durable resume closes out

- Added offline end-to-end coverage for the full non-live LabRun spine:
  provider-backed professor planning, provider-backed postdoc planning,
  deterministic graduate task queueing, durable Lab graduate completion sync,
  postdoc integration, professor review, and final `user_report`.
- The first phase uses the provider stage runner to draft and accept
  `ProfessorPlan` and `PostdocPlan`, then stops at the `graduate_work` boundary
  after queueing a scoped graduate task from `files_expected` and
  `validation_plan`.
- The second phase simulates a completed durable `lab-graduate-<task_id>`
  subagent with completion artifact and isolated worktree file proof, then runs
  the hybrid loop to sync the `GraduateResult`, advance through
  `postdoc_review` and `professor_review`, and stop at `NeedsUser`.
- The test deliberately avoids pretending graduate execution can run without an
  `AgentManager`; provider planning and graduate execution/resume remain
  separate runtime responsibilities.
- This provides an offline proof of the professor -> postdoc -> graduate ->
  postdoc -> professor -> user_report control spine without relying on a live
  provider's coding ability.

### Completed in P0.128 Offline validation gates the LabRun spine

- Extended `scripts/lab-live-validation.sh --offline` so the formal offline
  validation entrypoint now runs the high-value LabRun spine regression tests,
  not only command-surface smoke checks.
- The offline validation script now captures artifacts for:
  - provider planning -> queued graduate task -> durable graduate sync ->
    postdoc/professor/user_report spine;
  - hybrid durable graduate resume to `user_report`;
  - scheduler durable graduate auto-sync;
  - scheduler completed durable state with missing artifact failure handling.
- The report now records these offline spine checks as passed when the tests
  complete, and each test output is saved under the validation artifact
  directory for review.
- This turns the P0.124-P0.127 runtime proof chain into a repeatable command:
  `scripts/lab-live-validation.sh --offline`.
- Verified with
  `scripts/lab-live-validation.sh --offline --artifact-dir target/lab-live-validation/p0-128-offline-spine`;
  proof report:
  `target/lab-live-validation/p0-128-offline-spine/report.md`.

### Completed in P0.129 Lab graduate durable subagent completion fallback

- Tightened `LabOrchestrator::execute_graduate_task_latest_with_context()` so a
  foreground Lab graduate dispatch does not treat an immediate `AgentTool`
  failure as final when the generic subagent completion sink has already
  persisted a completed durable `lab-graduate-<task_id>` state.
- In that case LabRun now reuses the same durable sync path as scheduler and
  `/lab task sync`: parse the completion artifact, verify the isolated worktree
  file changes and required validation, then bind a `GraduateResult`.
- Added regression coverage for a Lab graduate dispatch with no foreground
  `AgentManager` where the durable child task/artifact already exists. The
  task now completes from runtime-verified durable evidence instead of failing
  on the missing foreground manager.
- Existing negative behavior remains: without completed durable proof, missing
  foreground execution still blocks the task and records the failure.
- Verified with
  `scripts/lab-live-validation.sh --offline --artifact-dir target/lab-live-validation/p0-129-graduate-durable-fallback`;
  proof report:
  `target/lab-live-validation/p0-129-graduate-durable-fallback/report.md`.

### Completed in P0.130 Provider graduate certification records

- Added project-level provider certification records under
  `.priority-agent/lab/provider_certifications.jsonl`.
- Added a provider-backed command:
  `/lab provider record <control-plane|graduate> <passed|failed> <evidence_path> [summary]`.
- `/lab provider` now shows the latest control-plane and graduate certification
  records for the active provider/model, plus the certification store path.
- The graduate execution gate now treats a latest local `graduate passed` record
  as `certified`, so a provider that passes the full live graduate gate can be
  used without weakening the static block for known-unsupported providers.
- `scripts/lab-live-validation.sh --live-control-plane` and `--live-graduate`
  now write provider certification records only after the relevant live checks,
  including daemon validation, have passed.
- Added tests for provider certification persistence, local gate certification,
  and `/lab provider record` command behavior.
- Verified with
  `scripts/lab-live-validation.sh --offline --artifact-dir target/lab-live-validation/p0-130-provider-cert-records`;
  proof report:
  `target/lab-live-validation/p0-130-provider-cert-records/report.md`.

### Completed in P0.131 Live validation failed-certification evidence

- `scripts/lab-live-validation.sh` now has a live-mode exit hook that records a
  provider certification failure when `--live-control-plane` or
  `--live-graduate` exits non-zero before writing a success record.
- The failure hook writes `provider record control-plane failed ...` or
  `provider record graduate failed ...` into the same project-level
  certification store, while preserving the original script exit status.
- Failed graduate certification records are visible through `/lab provider` but
  do not promote a provider/model to `certified`; the static known-unsupported
  gate remains in force unless a later `graduate passed` record exists.
- Added tests proving failed graduate certification records are persisted and
  visible without allowing Lab graduate execution.
- Verified with
  `scripts/lab-live-validation.sh --offline --artifact-dir target/lab-live-validation/p0-131-failed-cert-records`;
  proof report:
  `target/lab-live-validation/p0-131-failed-cert-records/report.md`.

### Completed in P0.132 Provider-backed read-only Lab meeting summaries

- Added provider-backed `/lab meeting llm [topic]` for a read-only Professor /
  Postdoc Lab meeting summary.
- The provider prompt uses a strict JSON contract with separate Professor
  strategy view, Postdoc implementation view, meeting decision, next actions,
  and evidence IDs.
- The runtime remains the source of truth for LabRun IDs, meeting IDs, artifact
  IDs, stage, total/cache usage, and known evidence references. Provider-supplied
  evidence IDs are filtered against the persisted evidence index, with a
  refs-only fallback to recent evidence refs when the provider omits valid IDs.
- The command writes a `LabMeetingSummary` artifact plus Markdown report and
  records provider usage with `meeting_id` and `note=llm_lab_meeting`.
- This command does not mutate code, dispatch graduate tasks, satisfy graduate
  execution gates, or bypass provider certification. It gives the LabRun loop a
  real LLM-backed group-meeting surface while preserving runtime control.
- Added tests for strict meeting JSON validation, provider-backed meeting
  persistence/usage, shell guidance when no provider context exists, and the
  `/lab meeting llm` command path.

### Completed in P0.133 Lab graduate durable subagent proof in provider compare

- Strengthened `/lab provider compare` so the Lab graduate path now reports the
  same kind of hard subagent evidence as the generic implementer path.
- The Lab graduate section now includes:
  - the stable durable task ID `lab-graduate-<task_id>`;
  - durable state presence/status, profile, context mode, and agent ID;
  - durable result artifact ID and completion sink;
  - `tools_used`;
  - isolated worktree file proof for `lab-provider-compare-lab.txt`;
  - `hard_file_proof`, `permission_denied`, and a durable result preview.
- The compare conclusion no longer treats Lab graduate dispatch success alone
  as mutating-tool proof. It requires the runtime-visible durable subagent
  evidence and isolated-worktree file proof.
- Added regression coverage for rendering Lab graduate durable proof from
  `SessionStore`, plus the existing provider compare path coverage.
- This makes the Lab graduate path auditable against the same subagent
  runtime contract used by Direct Mode instead of relying on LLM text claims.

### Completed in P0.134 Offline gate covers Lab graduate durable compare proof

- Added the P0.133 durable-proof regression to
  `scripts/lab-live-validation.sh --offline`.
- The offline validation artifact set now includes
  `10i-lab-graduate-provider-compare-proof-test.txt`, which proves the
  provider-compare Lab graduate section can render durable child-session state,
  `tools_used`, isolated worktree file proof, and hard mutation proof from
  `SessionStore`.
- The offline report now records
  `Lab graduate provider-compare durable proof test: passed`, so future
  LabRun validation runs catch regressions where the Lab graduate path stops
  exposing the same hard evidence expected from generic subagents.

### Completed in P0.135 Provider-backed foreground run command wiring

- Fixed the command-surface drift where the plan and help text documented
  `/lab step llm`, `/lab run llm`, and `/lab run hybrid`, but provider-context
  command routing still fell through to the strict scheduler-only path.
- `/lab step llm [instructions]` now calls the existing provider-backed
  `run_provider_stage_step()` helper and reports from/to stage, artifact ID,
  provider review decision, and whether the stage advanced.
- `/lab run llm [max_steps] [instructions]` now calls the existing
  `run_provider_stage_steps_until_boundary()` helper and stops at max steps,
  user-needed state, inactive/terminal state, provider revision, or the
  `graduate_work` boundary.
- `/lab run hybrid [max_steps] [instructions]` now calls the existing
  `run_hybrid_lab_steps_until_boundary()` helper, so provider-backed
  professor/postdoc stages, strict graduate scheduling, and deterministic
  postdoc/professor review bridges are available through the actual Lab shell
  command surface.
- Plain `/lab` without provider context now returns explicit provider-shell
  usage for these commands instead of silently presenting the wrong execution
  path.
- Added command-layer mock-provider coverage for:
  - provider usage hints without a provider context;
  - `/lab step llm` advancing `professor_discussion -> postdoc_plan`;
  - `/lab run llm` reaching the graduate boundary;
  - `/lab run hybrid` entering the strict graduate scheduler boundary.

### Completed in P0.136 Offline gate covers provider foreground run commands

- Added the P0.135 command-route regressions to
  `scripts/lab-live-validation.sh --offline`.
- The offline validation artifact set now includes:
  - `10j-provider-run-no-context-guard-test.txt`;
  - `10k-step-llm-command-test.txt`;
  - `10l-run-llm-command-test.txt`;
  - `10m-run-hybrid-command-test.txt`.
- The offline report now records provider foreground run coverage for no-provider
  guards, `/lab step llm`, `/lab run llm`, and `/lab run hybrid`.
- This keeps the documented provider/hybrid LabRun foreground loop tied to the
  real command surface instead of only testing lower-level runtime helpers.

### Completed in P0.137 Explicit bounded hybrid multi-cycle run

- Added a bounded multi-cycle LabRun helper that composes existing runtime
  pieces instead of creating a second scheduler:
  `run_hybrid_lab_cycles_until_boundary()`.
- Added `/lab run hybrid-cycles [max_cycles] [max_steps_per_cycle] [instructions]`.
- The command is explicit and user-triggered. It does not enable hidden
  background autonomy.
- Each cycle runs the existing hybrid path:
  provider-backed professor/postdoc stages, strict graduate scheduling, and
  deterministic postdoc/professor review bridges.
- When a cycle reaches `user_report` / `NeedsUser`, the command calls the
  existing `continue_latest_from_user_report()` path. That writes a
  `CycleSummary`, increments `cycle_count`, resets the stage to
  `professor_discussion`, and creates a fresh Professor gate.
- The command stops at `max_cycles`, `max_steps_per_cycle`, inactive state,
  provider revision, deterministic gate block, or strict scheduler boundary.
- Added command-layer coverage proving the command can move from an existing
  professor review to `user_report`, explicitly continue into the next cycle,
  and then stop on the next cycle's step bound with a persisted cycle summary.
- Added the `hybrid-cycles` command test to
  `scripts/lab-live-validation.sh --offline` as
  `10n-run-hybrid-cycles-command-test.txt`.

### Completed in P0.138 Hybrid-cycles cost budget and compression gate

- `/lab run hybrid-cycles` now checks current-cycle token usage against
  `LabCostPolicy.max_cycle_tokens` before starting the next provider/scheduler
  step.
- If the current cycle is already at or above budget, the command stops with
  `CostBudgetExceeded { cycle_id, total_tokens, max_cycle_tokens }` and does
  not call the provider or schedule more work.
- When a bounded hybrid cycle reaches `user_report`, the runner now attempts
  existing compression summary creation for Professor, Postdoc, and Runtime
  roles when `auto_compress_after_cycle` is enabled.
- Cycle output reports compression artifact IDs so the operator can see whether
  context compression actually produced persisted artifacts.
- Added command-layer coverage proving budget stop behavior and compression
  artifact creation after a completed cycle.
- Added both regressions to `scripts/lab-live-validation.sh --offline` as:
  - `10o-run-hybrid-cycles-budget-gate-test.txt`;
  - `10p-run-hybrid-cycles-compression-test.txt`.

### Completed in P0.139 Professor-triggered meeting request artifact

- Added a Professor-owned `LabMeetingRequest` stage artifact for
  professor-triggered meeting recommendations.
- `/lab meeting open` now writes that durable request artifact and Markdown
  report before creating the read-only `LabMeetingSummary` when it is opened
  from an actual recommendation signal.
- Manual `/lab meeting open <topic>` remains a user-triggered read-only
  meeting and does not fabricate a professor-triggered request.
- The request artifact records topic, current stage, trigger reason, matched
  signals, requesting role, and next action.
- Added orchestrator and command coverage proving the request artifact is
  persisted and appears in the recommendation-open flow.
- Added both regressions to `scripts/lab-live-validation.sh --offline` as:
  - `10q-meeting-request-artifact-test.txt`;
  - `10r-meeting-open-request-command-test.txt`.

### Completed in P0.140 Live control-plane validation refresh

- Ran `scripts/lab-live-validation.sh --live-control-plane` with DeepSeek
  `deepseek-v4-flash`; the script completed normally and wrote:
  `target/lab-live-validation/p0-139-live-control-plane/report.md`.
- Live control-plane checks passed for sponsor intervention, sponsor
  classification, provider certification reporting, direct tool diagnostics,
  generic/background-vs-Lab provider comparison, daemon hybrid policy enable,
  and `lab-daemon` exit status.
- The run recorded a provider certification artifact:
  `kind=control_plane`, `outcome=passed` for provider `deepseek`, model
  `deepseek-v4-flash`.
- Direct diagnostics show DeepSeek v4 flash supports `tool_choice=auto` tool
  calls for simple echo, file write, file write + bash, and subagent-style
  allowed tools. It still rejects `tool_choice=required` and forced function
  choice with `Thinking mode does not support this tool_choice`.
- Provider comparison now shows hard tool proof for generic subagents:
  foreground generic subagent used `bash,file_write,file_read`, background
  generic subagent used `file_write,file_read,bash`, and both produced
  file-proof artifacts in isolated worktrees.
- Lab graduate execution remains blocked before spending a graduate run because
  DeepSeek v4 flash is still `known_unsupported` for the formal graduate
  certification path: isolated graduate execution, runtime-observed file
  changes, required validation, and worktree review/merge/cleanup proof.

### Completed in P0.141 Active lease scheduling gate

- Added a shared `LabStore::ensure_current_process_holds_fresh_lease()` guard
  for mutating LabRun scheduling paths.
- Strict scheduler steps now refuse to run when the active lease is missing,
  owned by another run/process, stale, or mismatched with the persisted LabRun
  state.
- Background strict and hybrid scheduler startup now checks the same lease gate
  before spawning an in-process worker or writing `scheduler_state=Running`.
- This closes the documented boundary that losing the active lease must stop
  scheduling new internal work.
- Added regressions proving:
  - strict scheduler step refuses a missing active lease without writing new
    artifacts;
  - `/lab background start` refuses a missing active lease without writing a
    scheduler state.
- Added both regressions to `scripts/lab-live-validation.sh --offline` as:
  - `10s-scheduler-active-lease-gate-test.txt`;
  - `10t-background-active-lease-gate-test.txt`.

### Completed in P0.142 Typed artifact gate validation

- Strengthened `LabStore::validate_artifact_gate()` so a satisfied gate must
  reference a real, parseable stage artifact, not just a non-empty
  `artifact_id`.
- Gate validation now checks that the referenced artifact belongs to the same
  LabRun and matches the gate's stage, required artifact type, and owner.
- Missing, malformed, stale-stage, wrong-type, or wrong-owner artifacts now
  block progression before `advance_latest()` can move the LabRun forward.
- Existing blocker and `needs_revision` gates still surface their explicit
  blocker/revision messages instead of being hidden by artifact lookup.
- Added coverage proving:
  - incomplete gates still report missing handoff fields;
  - gates referencing missing artifacts fail;
  - gates referencing an artifact with the wrong stage/type fail;
  - gates referencing the correct typed artifact pass.
- Added the regression to `scripts/lab-live-validation.sh --offline` as
  `10u-artifact-gate-typed-artifact-test.txt`.

### Completed in P0.143 Graduate workspace snapshot evidence

- Graduate task execution now records durable
  `lab_graduate_workspace_snapshot` events immediately before and after the
  graduate agent execution attempt.
- Snapshot events include task ID, dispatch ID, phase, dirty path count, dirty
  paths, changed path count, and changed paths.
- This makes pre-existing user/workspace changes visible in LabRun event
  history instead of relying only on an in-memory before/after map for scope
  validation.
- The event intentionally stores paths and counts, not file contents, so it can
  support review/reporting without copying large or sensitive data.
- Added coverage proving a pre-existing dirty file is recorded in the `before`
  snapshot and unchanged files are not misreported as graduate changes.
- Added the regression to `scripts/lab-live-validation.sh --offline` as
  `10v-graduate-workspace-snapshot-test.txt`.

### Completed in P0.144 Graduate workspace snapshot review surface

- `/lab review` now renders recent `lab_graduate_workspace_snapshot` events as
  `Graduate workspace snapshots`, alongside existing graduate worktree proof.
- `/lab dashboard` renders the same snapshot summary with a smaller recent
  event limit for status-panel use.
- The surface shows phase, task ID, dispatch ID, dirty path count/list, and
  changed path count/list so reviewers can distinguish pre-existing user
  changes from graduate-created changes.
- Empty runs explicitly show `Graduate workspace snapshots: none`, avoiding a
  silent missing-evidence state.
- Added command-layer coverage proving both review and dashboard display
  before/after snapshot events.
- Added the regression to `scripts/lab-live-validation.sh --offline` as
  `10w-graduate-workspace-snapshot-surface-test.txt`.

### Completed in P0.145 Postdoc workspace snapshot evidence chain

- `PostdocIntegrationSummary` now consumes recent
  `lab_graduate_workspace_snapshot` events instead of leaving them only in
  `/lab review` and `/lab dashboard`.
- `before` snapshots with dirty paths are carried into `remaining_risks` as
  pre-existing workspace changes, so the postdoc handoff does not silently mix
  user/prior changes with graduate work.
- `after` snapshots with changed paths are carried into `accepted_results` as
  runtime workspace deltas, giving professor review a direct summary of files
  the graduate execution actually changed.
- Snapshot event IDs are carried into `evidence_refs`, preserving the trace from
  postdoc summary back to durable LabRun event history.
- Added regression coverage for postdoc integration summary generation and
  wired it into `scripts/lab-live-validation.sh --offline` as
  `10x-postdoc-workspace-snapshot-evidence-test.txt`.

### Completed in P0.146 Professor review evidence inheritance

- `ProfessorReview` artifacts now inherit the evidence refs already collected
  by the latest `PostdocIntegrationSummary`, instead of only pointing to the
  postdoc artifact and stage.
- Professor review gates also carry the inherited refs, so audit/status
  consumers can see bottom-level runtime evidence directly from the final review
  gate.
- The inherited refs include durable `event:*` entries from graduate worktree
  proof and workspace snapshots, preserving the trace from professor review back
  to runtime-observed graduate execution evidence.
- The `professor_review_written` LabRun event records the propagated evidence
  refs for easier debugging of review-chain provenance.
- Added regression coverage to the professor acceptance path and wired it into
  `scripts/lab-live-validation.sh --offline` as
  `10y-professor-review-evidence-inheritance-test.txt`.

### Completed in P0.147 Cross-cycle summary evidence inheritance

- `CycleSummary` artifacts now preserve the latest `ProfessorReview` artifact
  ref plus the evidence refs inherited by that professor review before starting
  the next LabRun cycle.
- This keeps the previous cycle's final review provenance available after
  `/lab continue` moves the run back to `professor_discussion`.
- Cycle summaries now mark their artifact validation status as
  `read_only_runtime_summary` and expose the inherited refs through the
  artifact envelope, report Markdown, and returned gate.
- To handle both deterministic tick-generated reviews and explicit professor
  review artifacts, the cycle summary collects evidence from the latest
  `PostdocIntegrationSummary` and the latest `ProfessorReview`.
- Added regression coverage for `/lab continue` cycle-summary evidence
  inheritance and wired it into `scripts/lab-live-validation.sh --offline` as
  `10z-cycle-summary-evidence-inheritance-test.txt`.

### Completed in P0.148 Provider professor review evidence inheritance

- Provider-backed `draft_professor_review_with_provider()` now uses the same
  evidence inheritance policy as deterministic professor review.
- The generated `ProfessorReview` artifact inherits the latest
  `PostdocIntegrationSummary` evidence refs, including bottom-level
  `artifact:*` and `event:*` runtime evidence.
- The provider professor review gate and `provider_professor_review_written`
  event now carry the propagated refs, so provider-authored final review does
  not lose provenance compared with the deterministic bridge.
- Extended provider professor review coverage to assert inherited evidence refs
  on both the created artifact and gate.
- Added the regression to `scripts/lab-live-validation.sh --offline` as
  `10aa-provider-professor-review-evidence-inheritance-test.txt`.

### Completed in P0.149 Lab meeting evidence propagation

- Runtime-created `LabMeetingSummary` artifacts now propagate their selected
  evidence refs into the returned `lab_meeting` gate and the
  `lab_meeting_summary_written` event.
- Provider-created `LabMeetingSummary` artifacts now do the same for the
  `lab_provider_meeting_summary_written` path.
- This keeps group-meeting provenance queryable from gates/events without
  requiring every reviewer or dashboard to reopen the Markdown artifact first.
- Extended runtime and provider meeting tests to assert evidence refs on both
  the created artifact and gate.
- Added both regressions to `scripts/lab-live-validation.sh --offline` as
  `10ab-runtime-meeting-evidence-propagation-test.txt` and
  `10ac-provider-meeting-evidence-propagation-test.txt`.

### Completed in P0.150 Postdoc integration gate evidence propagation

- `PostdocIntegrationSummary` gates now carry the same graduate evidence refs
  that are written into the postdoc integration artifact envelope.
- The `postdoc_integration_summary_written` event records those refs as well,
  making graduate result, worktree, and workspace snapshot provenance available
  from event history without reopening the artifact.
- Extended the workspace snapshot postdoc integration regression to assert
  `event:*` evidence refs on the `postdoc_review` gate.
- This uses the existing `10x-postdoc-workspace-snapshot-evidence-test.txt`
  offline gate in `scripts/lab-live-validation.sh`.

### Completed in P0.151 Structured draft gate evidence propagation

- Added a reusable `StageArtifact::evidence_refs()` accessor so artifact-level
  provenance can be propagated without hand-written matches at every call site.
- Added
  `LabOrchestrator::write_satisfied_gate_for_latest_with_evidence_refs()` while
  preserving the existing single-ref helper as a wrapper.
- Provider-backed structured artifact drafts now write the generated artifact's
  envelope evidence refs into the satisfied stage gate, not only the artifact
  file path.
- This closes the revision-resume case where `apply_pending_revision_task_to_postdoc_plan()`
  adds a professor revision artifact ref to a structured `PostdocPlan`, but the
  satisfied `postdoc_plan` gate previously did not expose that provenance.
- Added a provider structured PostdocPlan regression and wired it into
  `scripts/lab-live-validation.sh --offline` as
  `10ad-structured-draft-gate-evidence-test.txt`.

### Completed in P0.152 Revision task evidence inheritance

- `LabRevisionTask` artifacts now inherit the source `ProfessorReview` artifact
  ref plus the review's propagated evidence refs, instead of only pointing to
  the source review artifact.
- The `postdoc_revision` gate and `lab_revision_task_written` event now carry
  the same refs, keeping professor rejection evidence visible through the
  repair handoff without reopening the revision artifact.
- Extended the professor-revision consumption regression to assert inherited
  evidence refs on the revision artifact and gate.
- Added the regression to `scripts/lab-live-validation.sh --offline` as
  `10ae-revision-task-evidence-inheritance-test.txt`.

### Completed in P0.153 Blocker report evidence propagation

- `LabBlockerReport` artifacts now collect refs for blocked graduate tasks and
  failed dispatches using refs-only identifiers such as `task:<id>`,
  `dispatch:<id>`, `artifact:<id>`, and existing task evidence IDs.
- The `blocker_report` gate and `lab_blocker_report_written` event now carry
  the same refs, so professor-side blocker escalation can be audited from gate
  or event history without reopening the report first.
- Extended blocker report coverage to assert blocked task refs on both the
  artifact and gate.
- Added the regression to `scripts/lab-live-validation.sh --offline` as
  `10af-blocker-report-evidence-propagation-test.txt`.

### Completed in P0.154 Context artifact/gate evidence layer

- LabRun context packets now include dynamic-tail layer
  `L6 artifact-and-gate-evidence-refs`.
- The new layer is refs-only and is derived from persisted stage artifacts and
  artifact gates, so provider prompts can see recent provenance without opening
  every artifact body or Markdown report.
- `/lab context`, `/lab compression`, compression-summary creation,
  provider-backed stage drafting, sponsor-message classification, Lab meetings,
  and provider professor reviews now all build context packets with this
  artifact/gate evidence layer.
- The layer intentionally lives in the dynamic tail, preserving the stable
  prefix cache boundary for role profile, project charter, and cost policy.
- Added regressions for the packet layer and command surface, and wired them
  into `scripts/lab-live-validation.sh --offline` as
  `10ag-context-artifact-gate-evidence-layer-test.txt` and
  `10ah-context-command-artifact-gate-evidence-layer-test.txt`.

### Completed in P0.155 Live request context artifact/gate evidence layer

- Live LabRun request preparation now builds its injected `<lab-context>` block
  with the same artifact/gate evidence refs used by `/lab context`,
  `/lab compression`, and provider-backed Lab draft paths.
- This closes the gap where P0.154 made the L6 layer available to explicit Lab
  commands, but ordinary provider-bound conversation turns could still omit the
  persisted artifact/gate provenance.
- The request-time compression decision is now evaluated against the same
  packet the model sees, including `L6 artifact-and-gate-evidence-refs`.
- Added request-preparation coverage proving the injected live `<lab-context>`
  includes the L6 layer and a persisted `professor_discussion` gate ref.
- Wired the regression into `scripts/lab-live-validation.sh --offline` as
  `10ai-live-request-context-artifact-gate-evidence-layer-test.txt`.

### Completed in P0.156 Provider-backed background hybrid command

- Wired the documented `/lab background hybrid [max_steps] [interval_ms]
  [instructions]` command to the existing provider-backed hybrid background
  scheduler.
- The command now requires an active Lab Mode provider/model, starts the
  in-process hybrid background loop, persists scheduler state, and remains
  visible through `/lab background status` and stoppable through
  `/lab background stop`.
- Argument parsing accepts default bounds, explicit step/interval bounds, and
  free-form provider instructions while keeping `background start` unchanged as
  the strict deterministic scheduler.
- This closes the product mismatch where help text and the plan described
  provider-backed background hybrid scheduling, but the command surface only
  handled strict `start/status/stop/recover`.
- Added regressions for successful startup/status/stop and the no-provider
  guard, wired into `scripts/lab-live-validation.sh --offline` as
  `10aj-background-hybrid-command-test.txt` and
  `10ak-background-hybrid-provider-guard-test.txt`.

### Completed in P0.157 Explicit background hybrid-cycles command

- Added `/lab background hybrid-cycles [max_cycles] [max_steps_per_cycle]
  [interval_ms] [instructions]` as an explicit provider-backed background
  entrypoint for bounded multi-cycle LabRun progression.
- The command reuses the existing foreground
  `run_hybrid_lab_cycles_until_boundary()` runner, so cycle continuation,
  `user_report` handling, token-budget stops, compression summaries, strict
  graduate scheduler boundaries, and deterministic review bridges stay on the
  same runtime path.
- Scheduler state records the run as `HybridCycleBackground`, using
  `max_steps` as the explicit max-cycle bound and `steps_completed` as cycles
  completed for status/recovery visibility.
- This does not change `/lab background hybrid`; single-cycle provider-backed
  background scheduling remains available as the less aggressive default.
- Added startup/status/stop and no-provider guard regressions, wired into
  `scripts/lab-live-validation.sh --offline` as
  `10al-background-hybrid-cycles-command-test.txt` and
  `10am-background-hybrid-cycles-provider-guard-test.txt`.

### Completed in P0.158 Daemon hybrid-cycles policy mode

- Extended persisted daemon policy with `LabDaemonMode::HybridCycles`.
- `/lab daemon enable hybrid-cycles [max_cycles] [interval_ms] [instructions]`
  now records an app-owned restart-surviving multi-cycle LabRun policy.
- Interactive daemon startup (`start_daemon_scheduler_from_policy()`) maps
  `HybridCycles` to the explicit background hybrid-cycle scheduler added in
  P0.157.
- The non-interactive `pa lab-daemon` worker also consumes `HybridCycles` by
  calling the same `run_hybrid_lab_cycles_until_boundary()` path used by
  foreground `/lab run hybrid-cycles`, preserving user-report continuation,
  token-budget stops, compression summaries, strict graduate scheduler
  boundaries, and deterministic review bridges.
- P0.159 adds a dedicated persisted per-cycle step bound; in this slice,
  `max_steps` introduced the max-cycle bound for `HybridCycles`.
- Added persistence and command-surface regressions, wired into
  `scripts/lab-live-validation.sh --offline` as
  `10an-daemon-hybrid-cycles-policy-test.txt` and
  `10ao-daemon-hybrid-cycles-command-test.txt`.

### Completed in P0.159 Daemon hybrid-cycles per-cycle bound

- Added `LabDaemonState.max_steps_per_cycle` with a serde default of `5`, so
  existing `.priority-agent/lab/daemon_state.json` files remain readable.
- Added `LabStore::enable_daemon_with_cycle_bound()` while keeping
  `enable_daemon()` as a compatibility wrapper for strict/hybrid call sites.
- `/lab daemon enable hybrid-cycles [max_cycles] [max_steps_per_cycle]
  [interval_ms] [instructions]` now persists both the max-cycle bound and the
  per-cycle step bound.
- `/lab daemon status` and `/lab daemon health` show the persisted per-cycle
  bound, giving desktop/service supervisors enough information to explain what
  will resume after restart.
- `start_daemon_scheduler_from_policy()` and `pa lab-daemon` now pass the
  persisted `max_steps_per_cycle` into the shared hybrid-cycles runner instead
  of using a hard-coded default.
- Existing offline regressions for daemon hybrid-cycles policy and command
  parsing now assert the persisted per-cycle bound through
  `10an-daemon-hybrid-cycles-policy-test.txt` and
  `10ao-daemon-hybrid-cycles-command-test.txt`.

### Completed in P0.160 Desktop daemon cycle-bound status surface

- Added `daemon_policy` to the desktop/Tauri Lab status snapshot so Workbench
  and desktop supervision can read the persisted daemon mode, max-cycle bound,
  per-cycle step bound, interval, last start time, and last start error from the
  same file-backed Lab store used by CLI/daemon commands.
- Workbench now renders the daemon policy row directly in the Lab status panel,
  including the `hybrid_cycles` `max_steps_per_cycle` value.
- The desktop web preview fixture now includes the same `hybrid_cycles` daemon
  policy shape so frontend preview mode exercises the new row.
- Added a desktop regression that writes a `hybrid_cycles` daemon policy through
  `LabStore::enable_daemon_with_cycle_bound()` and verifies the desktop snapshot
  reads `max_steps_per_cycle` back from disk.
- Wired that desktop regression into `scripts/lab-live-validation.sh --offline`
  as `10ap-desktop-daemon-cycle-bound-status-test.txt`.

### Completed in P0.161 Provider compare foreground durable recovery

- Ran `scripts/lab-live-validation.sh --live-control-plane` against the
  configured DeepSeek v4 flash provider. Professor/control-plane checks,
  sponsor classification, tool diagnostics, background subagent completion
  sink, daemon enablement, and `pa lab-daemon` all completed.
- The live run exposed a narrower foreground generic subagent reporting issue:
  the isolated worktree file and durable completion artifact existed, but
  `/lab provider compare` reported `agent_id: none` because the foreground wait
  path timed out before reading the durable sink.
- `/lab provider compare` now assigns a stable foreground generic `task_id` and
  recovers the generic result from the durable completion sink after a foreground
  wait timeout, including tools used, completion sink, artifact id, and hard
  isolated-worktree file proof.
- Added deterministic coverage for this recovery path as
  `10aq-provider-compare-foreground-durable-recovery-test.txt`.

### Completed in P0.162 Command-mode LabRun lease ownership

- Ran an explicit DeepSeek v4 flash `--live-graduate` experiment with
  `PRIORITY_AGENT_LAB_ALLOW_UNCERTIFIED_GRADUATE_PROVIDER=1`. The provider
  compare phase proved the Lab graduate route can create an isolated-worktree
  file with durable completion-sink proof under the override.
- The full live graduate script still failed, but the failure was not tool
  exposure: a later `lab --command` process recovered a stale lease from a
  previous one-shot command and changed the LabRun to `PausedShutdown`, so
  `/lab task run` rejected the queued task as not active.
- Added command-mode lease ownership: one-shot `lab --command` and
  provider-backed `lab --command --with-provider` startup now skip stale-lease
  pause recovery, claim the latest active LabRun lease for the current process,
  and release that process-owned lease at command exit without pausing the
  LabRun.
- The live validation workspace is now initialized as its own small git
  repository before LabRun commands execute. Worktree review/merge/cleanup
  therefore validates against the temporary LabRun project instead of depending
  on whether the outer `rust-agent` development checkout is clean.
- The follow-up live run reached graduate task execution and worktree review,
  then exposed a runtime proof filter bug: `.claude/worktrees/...` internal
  storage was counted as an out-of-scope graduate file change. Runtime graduate
  change detection now filters `.claude/worktrees`, `.priority-agent`, and
  `.git` paths before allowed-scope validation.
- Added deterministic coverage as
  `10ar-command-lease-claim-test.txt` and
  `10as-runtime-internal-path-filter-test.txt`.

### Completed in P0.163 Runtime-verified graduate fallback and live proof

- Added a runtime-verified fallback for successful graduate subagents that miss
  the structured JSON contract. The fallback still requires parent-side
  evidence: actual scoped file changes from the graduate worktree and all
  required validation commands must pass before a `GraduateResult` can be
  bound.
- Added deterministic coverage as
  `10at-unbound-runtime-verified-graduate-result-test.txt`.
- Re-ran DeepSeek v4 flash with
  `PRIORITY_AGENT_LAB_ALLOW_UNCERTIFIED_GRADUATE_PROVIDER=1` after the lease,
  workspace, internal-path, and runtime fallback fixes. The live graduate task
  succeeded, runtime validation passed, and worktree review/merge/cleanup
  passed in
  `target/lab-live-validation/p0-162-live-graduate-cert-skip/report.md`.
- Because the run used the uncertified-provider override, the validation script
  now records it as experimental runtime-path evidence and intentionally skips
  formal `graduate passed` provider certification.

### Still pending

- Full release-ready autonomous multi-cycle professor/postdoc/graduate LLM
  orchestration. Provider-backed proposal intake, sponsor classification,
  planning, read-only meetings, structured graduate JSON binding, durable
  graduate completion sync, and explicit bounded `hybrid-cycles` foreground
  runs exist, but LabRun is still not a hidden self-driving background project
  mode.
- Full live-provider validation of multi-cycle professor/postdoc/graduate
  orchestration. Local runtime control for next-cycle continuation, app
  lifecycle checkpoints, professor revision tasks, postdoc revision resume, and
  revision-task consumption now exists.
- Full live graduate subagent execution now requires runtime-observed file
  changes and validation, not only bindable JSON. DeepSeek v4 flash has passed
  this runtime path under the explicit uncertified-provider override, but it is
  still not formally certified as a supported graduate provider.
- Desktop-managed persistent background ownership is implemented at the
  control-plane level: provider calls restart from persisted policy when
  `pa lab` opens, can run via the non-interactive `lab-daemon` worker, have a
  generated macOS LaunchAgent manifest, expose `/lab daemon health`, can
  install/uninstall the LaunchAgent plist file, can explicitly
  load/unload/restart the service via `launchctl`, and can repair a missing
  loaded service through CLI, Tauri, a Workbench button, and an app-scoped
  desktop timer. The desktop Lab status snapshot now also exposes persisted
  daemon policy and `hybrid_cycles` per-cycle bounds. Long-running live
  validation of this ownership path remains pending.
- Cross-provider validation that graduate agents consistently emit bindable JSON
  and perform real tool-backed edits.
- `/lab provider compare` now exposes generic foreground, generic background,
  and Lab graduate durable evidence side by side. Remaining provider-matrix
  work is to run and record this evidence across stronger coding providers,
  then promote only providers with hard graduate worktree proof.
- Persistent opencode-style subagent sessions with `task_id` resume, parent
  session IDs, durable artifacts, parent synthetic result injection, UI/API
  result projection, and restart recovery are implemented through
  P0.118-P0.121. Remaining work is live background completion-sink validation
  across multiple providers and long-running desktop sessions.
- DeepSeek proves Professor-side provider control-plane use, structured
  graduate JSON emission, direct provider `tool_choice=auto` function calling
  with production tool schemas, and generic foreground/background subagent
  isolated-worktree mutation with durable completion-sink proof. It is still
  blocked from the formal Lab graduate path by certification until it passes the
  stricter isolated graduate task, validation, review/merge/cleanup proof.
  `/lab task bind-json` remains covered by deterministic offline validation.
- Live end-to-end validation of worktree review/merge/cleanup with real
  graduate agent output remains pending until at least one provider performs
  actual tool-backed changes in the isolated worktree.
- Live execution of `scripts/lab-live-validation.sh --live` now intentionally
  fails DeepSeek v4 flash at the graduate runtime verification gate; provider
  matrix repetition remains pending.
- Desktop Workbench and TUI runtime panels now expose read-only Lab status,
  blocker/retry history, safe next-action commands, and startup recovery
  prompts/buttons.
- Professor-side sponsor classification is live-validated on DeepSeek v4 flash;
  reliability across supported models remains pending.

## Core Product Decisions

### 0. Lab mode is a second top-level mode, not a replacement for direct mode

priority-agent should have two product modes:

- **Direct Mode**: the current opencode/Codex-style interaction model. The user
  gives commands turn by turn, the main coding agent responds directly, and the
  user stays closely involved.
- **Lab Mode**: a project-sponsor model. The user discusses the objective with
  the professor agent, the professor turns it into a lab project, and the lab
  runs professor/postdoc/graduate loops internally until it needs user input or
  has a stage report.

Direct Mode should remain fast, low-ceremony, and unchanged for small tasks.
Lab Mode should be reserved for medium/large work where the user wants to
delegate the project after initial direction setting.

In Lab Mode, the user-facing conversation defaults to the professor agent. The
user is effectively the sponsor or funding committee: they define goals, review
major reports, approve direction changes, and intervene when needed. They should
not have to manage every implementation step.

Routing should therefore be mode/state based, not a free-for-all agent router:

```text
User Input
  -> TopLevelModeRouter
      -> Direct Mode: existing conversation loop
      -> Lab Mode: professor-facing lab conversation
```

Inside Lab Mode:

```text
Professor-facing user conversation
  -> LabOrchestrator
      -> ProfessorPlan
      -> PostdocPlan
      -> GraduateTask(s)
      -> PostdocReview
      -> ProfessorReview
      -> UserReport
```

The professor, postdoc, and graduate agents are hierarchical. They should not
compete to answer the user. The runtime decides which internal role owns the
current stage.

### 0.1. Project intake comes before formal LabRun creation

Lab Mode should include a pre-project intake phase. This is the equivalent of
talking with a professor before a project is formally funded.

The first professor conversation should not immediately create a mutating
LabRun. It should help the user clarify:

- project goal;
- expected outcome;
- why this is worth doing;
- scope and non-goals;
- constraints and risks;
- success criteria;
- whether this should be Direct Mode, a small goal, or a full LabRun.

The professor should then produce a `LabProposal`, not a LabRun:

```text
User idea
  -> Professor intake discussion
  -> LabProposal draft
  -> Professor asks: "Approve this as a LabRun?"
  -> User clicks "Start Project" / "Approve LabRun"
  -> LabRun is created
  -> professor/postdoc/graduate loop begins
```

This matters because LabRun implies durable project state, cost tracking,
leases, pause/resume, internal scheduling, and possible implementation cycles.
Those should begin only after explicit user approval.

MVP rule:

- `pa lab` opens professor intake mode;
- `/lab propose <idea>` creates or updates a proposal draft;
- "Start Project" button or `/lab approve <proposal_id>` creates the LabRun;
- `/lab start <goal>` may exist as a shortcut, but it should still show the
  professor proposal and require explicit approval before mutating work begins;
- no graduate task, postdoc implementation cycle, or mutating lease starts
  before formal approval.

This gives the product the real project-initiation feel: discussion first,
proposal second, formal launch third, execution loop last.

### 0.2. Lab mode is separate from goal mode

Existing goal mode is useful for a bounded objective inside one user-driven
session. It should remain part of Direct Mode and the general agent experience,
matching mainstream coding-agent workflows where the user can ask the agent to
keep working toward a specific goal.

Lab Mode should not depend on goal mode. LabRun already owns the project-level
loop:

- professor discussion;
- professor plan;
- postdoc plan;
- graduate work;
- postdoc review;
- professor review;
- lab meetings;
- pause/resume;
- final report.

LabRun is the durable project-level source of truth. It owns role state,
project cycles, meetings, reports, blockers, strategic reviews, and
cross-session resume. Goal mode remains valuable for Direct Mode, but Lab Mode
should use LabOrchestrator/LabRun directly rather than wrapping itself in a
GoalRun.

### 0.3. Program entrypoints should be separate at the product layer

Direct Mode and Lab Mode should be two parallel product modes, but the MVP
should not start by creating two fully independent binaries.

Recommended shape:

```text
priority-agent / pa
  -> direct mode by default

priority-agent lab / pa lab
  -> lab mode project launcher
```

In other words:

- **Conceptually separate**: Direct Mode and Lab Mode should have different
  entry controllers, command surfaces, startup recovery, and user-facing
  lifecycle.
- **Physically shared at first**: keep one binary and one bootstrap path so
  provider loading, config, permissions, logging, session storage, model
  routing, cost tracking, and tool registry behavior cannot drift.
- **Internally split**: move `src/main.rs` toward thin argument parsing and
  dispatch into explicit entry modules.

Suggested module layout:

```text
src/main.rs
src/entry/mod.rs
src/entry/direct.rs
src/entry/lab.rs
src/entry/api.rs
src/entry/eval.rs
src/lab/mod.rs
src/lab/app.rs
src/lab/orchestrator.rs
src/lab/store.rs
src/lab/context.rs
```

`src/main.rs` should only decide the startup mode and call an entry function:

```text
StartupMode::DirectCli -> entry::direct::run_cli(...)
StartupMode::LabCli    -> entry::lab::run_cli(...)
StartupMode::Api       -> entry::api::run(...)
StartupMode::EvalRun   -> entry::eval::run(...)
```

This keeps Lab Mode first-class without copying the whole runtime.

Do not duplicate these responsibilities in a second binary:

- environment/config loading;
- provider initialization;
- permission/checkpoint setup;
- memory and session store setup;
- tool registry construction;
- cost tracker initialization;
- tracing/logging setup;
- startup recovery scanning.

If packaging later needs a separate executable, add it as a thin wrapper:

```text
src/bin/priority-agent-lab.rs
  -> priority_agent::entry::lab::run_cli(...)
```

That wrapper should not own separate bootstrap logic. It should only provide a
different command name or default mode.

### 1. Markdown is a human report, not the source of truth

Markdown files are useful for reading, discussion, and long-term project
history, but they are inefficient and brittle as the only machine protocol.

Use a hybrid model:

- **Structured state**: JSON / JSONL for run state, task envelopes, reviews,
  evidence, role decisions, and machine routing.
- **Durable storage**: start with files under `.priority-agent/lab/`; move hot
  indexes into SQLite once the workflow stabilizes.
- **Markdown reports**: generated or written as human-facing summaries under
  `docs/lab/`, with a manifest link back to structured state.

Markdown should never be the only place where status, ownership, dependencies,
or acceptance criteria live.

### 1.1. Pause/resume and local durability are P0 requirements

Lab Mode must not rely on one long in-memory conversation. Once a lab project is
started, its project state, artifacts, reports, progress, blockers, and results
must be persisted to local disk.

Pause/resume behavior:

- user can press a pause button or run `/lab pause`;
- closing the app or shutting down the machine should automatically move active
  lab projects into a paused/recoverable state;
- the next app launch should detect paused lab projects and ask the user to
  continue, inspect report, keep paused, or close the project;
- Lab Mode should not automatically resume mutating work just because the app
  opened;
- resume should continue from the last persisted project state, not restart
  from the beginning.

The right mental model:

```text
User-visible window = professor-facing project session
Machine state = LabRun persisted on local disk
Internal communication = lab_events / task envelopes / artifacts / reports
Context = rebuilt from persisted project state on every continuation
```

The existing 80% context compression and message compaction mechanisms remain
useful, but they are not the source of truth. Compression preserves language
continuity; persisted LabRun state preserves project truth.

### 1.2. One project needs one active mutating LabRun lease

Lab Mode can be long-running, so the runtime must prevent two windows or two
processes from mutating the same project through the same LabRun at the same
time.

MVP rule:

- one project root can have many historical LabRuns;
- one project root can have at most one active mutating LabRun lease;
- read-only inspection and meeting report viewing can be concurrent;
- code mutation, task scheduling, pause/resume, and status transition require
  the active LabRun lease;
- if another process holds a fresh lease, the second process should show the
  active owner and offer read-only inspect/status, not start work.

Lease state should be persisted separately from UI state:

```json
{
  "lease_id": "lease_...",
  "lease_owner": "pid:12345:host:gex-mac",
  "lease_acquired_at": "2026-06-18T00:00:00Z",
  "heartbeat_at": "2026-06-18T00:00:00Z",
  "lease_ttl_seconds": 90
}
```

Unexpected shutdown is handled by heartbeat expiry. Graceful pause should
release the lease after persisting `state.json` and appending a `lab_paused`
event.

### 1.3. Lab Mode must keep user approval and mutation boundaries explicit

Lab Mode is more autonomous than Direct Mode, but it must not become a hidden
background committer. It should reuse the existing permission, checkpoint, and
validation boundaries.

MVP mutation policy:

- lab meetings are read-only unless the user explicitly starts or resumes an
  implementation cycle;
- professor role is read-only by default and can create plans/reviews, not code
  edits;
- postdoc may edit only inside an active implementation cycle and only after
  the LabRun has a clear accepted plan;
- graduate tasks may edit only inside their `allowed_scope`;
- high-risk tools, broad file writes, external publishing, dependency changes,
  or destructive commands still require the existing approval gates;
- every mutating cycle must create a checkpoint or use the existing
  checkpoint-backed tool path before file changes.

Dirty worktree policy:

- LabRun must snapshot `git status --short` before mutating work;
- if unrelated user changes are present, the postdoc must either work around
  them or ask for user approval;
- graduate tasks should not silently overwrite files changed outside their
  task scope;
- reports must distinguish user/pre-existing changes from LabRun changes.

### 1.4. Sponsor oversight goes through professor, not internal agents

During an active LabRun, the user should be able to see what the lab loop is
doing, but should not directly command professor/postdoc/graduate agents as
separate chat targets.

The right analogy is a funding committee talking to the professor:

- user can observe current stage, reports, tasks, blockers, and evidence;
- user can send notes/questions/concerns to the professor at any time;
- user cannot directly tell the postdoc or graduate agent what to do;
- professor decides whether the user note changes project direction, creates a
  postdoc task, opens a lab meeting, pauses the run, or needs formal approval.

This prevents the workflow from collapsing into ad hoc multi-chat control while
still keeping the user in charge of direction.

Runtime rule:

- side-channel user input is recorded as `SponsorMessage`;
- professor responds with `ProfessorSteeringDecision`;
- no side-channel message directly mutates code or graduate task scope;
- substantial scope changes become proposal revisions or change requests;
- urgent safety/correctness concerns can pause scheduling until professor
  reviews them.

Recommended UI:

- main LabRun panel shows the internal loop, current owner, stage, tasks,
  evidence, and reports;
- side panel or side input box is "Message Professor";
- professor replies in that side channel or emits a project steering event;
- if the professor accepts the concern, the runtime inserts the resulting
  change into the LabRun queue as a structured artifact.

### 2. Professor responsibilities must stay high-level

The professor agent should not become a senior code reviewer that comments on
line-level implementation details. Its job is to prevent wrong direction,
missing architecture concerns, confused product goals, and local-optimum work.

Professor review should check:

- Does the implementation still match the product thesis?
- Did the postdoc preserve the intended architecture boundaries?
- Are important non-goals and risks still respected?
- Are there missing user-facing or system-level implications?
- Did the postdoc provide enough evidence for "implemented" claims?
- Is the next strategic step obvious, or is the project drifting?

Professor review should avoid:

- Nitpicking style or local Rust implementation details.
- Replacing postdoc technical ownership.
- Expanding scope without explicitly labeling it as a new proposal.
- Treating runtime hints or subagent reports as proof without evidence.

### 3. Postdoc owns code and result quality

The postdoc agent is the responsible engineering owner. It can delegate narrow
tasks, but it cannot delegate accountability.

Postdoc responsibilities:

- Read professor plan and current code.
- Produce concrete implementation plans.
- Split work into narrow graduate tasks.
- Decide which tasks it should implement directly.
- Review graduate reports and diffs.
- Run integration validation.
- Write final implementation report for professor review.

If graduate work is incomplete or wrong, the postdoc must either repair it or
return a precise correction task.

### 4. Graduate agent is the current subagent pattern with a stricter contract

The graduate agent should map closely to existing subagent / agent-tool work.
It receives explicit instructions, works in narrow scope, runs specified
validation, and returns a report.

Graduate agent responsibilities:

- Implement one narrow task.
- Stay inside allowed files and allowed actions.
- Ask or report blocker when the task is underspecified.
- Produce a structured result plus a short Markdown report.
- Never change professor-level goals or postdoc architecture decisions.

### 5. Lab meetings can be user-triggered or professor-triggered

Do not start with automatic meetings on every app launch. That risks becoming
annoying and expensive. But the architecture should support both explicit user
meetings and professor-triggered meetings.

MVP should expose:

- `/lab meeting` in CLI/TUI.
- A desktop/TUI "Lab Meeting" button or command palette action.

User-triggered meeting:

1. Postdoc reviews recent project progress.
2. Postdoc writes a progress report.
3. Professor reviews that report against current code/docs/status at a
   strategic level.
4. User sees one concise meeting report with suggested next steps.

Professor-triggered meeting:

1. Postdoc reports a hard blocker, repeated failed experiment, unclear
   implementation tradeoff, or strategic conflict.
2. Professor reviews the postdoc audit.
3. If the issue is strategic enough, professor opens a lab meeting.
4. Professor and postdoc discuss in parallel: professor protects direction,
   postdoc supplies implementation facts.
5. Runtime records a meeting decision: continue, revise plan, ask user,
   change scope, or stop.

This matches the real lab model: many internal professor/postdoc/graduate loops
can happen after project approval, and meetings are inserted when coordination
or strategic judgment is needed.

## Existing Project Assets To Reuse

The lab workflow should reuse current infrastructure instead of building a
parallel agent framework.

Relevant existing surfaces:

- `src/agent/`: agent profiles, task envelopes, manager, transcript, memory.
- `src/tools/agent_tool/`: current subagent execution surface.
- `src/tools/team_tool.rs`: agent-to-agent message envelope pattern.
- `src/engine/goal/`: keep existing Direct Mode goal behavior separate; do not
  make LabRun depend on GoalRun.
- `src/engine/task_context/`: task stage, evidence, progress, observations.
- `src/engine/workflow/`: planner, executor, gate, metrics.
- `src/session_store/`: events, session parts, todos, durable timeline.
- `src/tui/runtime_panels.rs`: status/debug panels for running agents/tasks.
- `docs/workflow/`: existing workflow specs, reports, and gates.

The lab workflow should become an orchestrated layer above these pieces.

### Integration rule

LabRun should reference existing runtime records instead of duplicating them:

- conversation/session history stays in `src/session_store/`;
- tool traces and validation evidence stay in existing trace/event stores where
  possible;
- LabRun stores project-level orchestration state, artifact IDs, role decisions,
  task ownership, and evidence refs;
- reports should link to evidence refs rather than copying full logs by
  default.

This keeps Lab Mode as an orchestration layer over the current runtime, not a
second runtime with its own incompatible history model.

## Proposed Storage Model

### Directory layout

```text
.priority-agent/
  lab/
    runs/
      labrun_20260618_001/
        state.json
        events.jsonl
        context_snapshot.json
        professor_plan.json
        postdoc_plan.json
        tasks/
          grad_task_001.json
          grad_task_001_result.json
        reports/
          2026-06-18-professor-architecture-plan.md
          2026-06-18-postdoc-implementation-plan.md
          2026-06-18-grad-task-001-report.md
          2026-06-18-postdoc-integration-report.md
          2026-06-18-professor-review.md

docs/
  lab/
    2026-06-18-professor-agent-workflow-plan.md
    2026-06-18-lab-meeting-report.md
```

Rules:

- `.priority-agent/lab/runs/*` is runtime-owned mutable state.
- `docs/lab/*.md` is human-facing durable project history.
- each active LabRun is bound to one user-facing session, but can contain many
  internal agent tasks;
- each active mutating LabRun must hold a fresh lease for the project root;
- Every Markdown report must include `lab_run_id` and matching structured
  artifact IDs.
- Every structured artifact should include `schema_version`.
- active LabRun state must be fsynced or transactionally written before
  launching new internal work.

### Why JSON plus JSONL

Use JSON for current state and typed artifacts:

- easy to validate with serde;
- easy to inspect manually;
- good for task envelopes and review results;
- stable enough before committing to DB migrations.

Use JSONL for append-only event history:

- preserves timeline;
- easy to stream into UI;
- supports replay/debug;
- avoids rewriting a large state file for every event.

SQLite can be phase 2:

- indexes for recent lab runs;
- queryable task status;
- cross-session dashboards;
- migration-backed durability once schemas settle.

### Persistence strategy

Phase 1 should use file-backed artifacts for iteration speed, but it should
borrow the shape of the existing durable session system:

- session DB already persists conversation/session events, parts, todos,
  goal runs, and traces;
- LabRun should be its own project-level state that references those sessions
  and traces;
- lab events should be append-only JSONL so interruption recovery can replay
  the project timeline;
- lab state should be atomically rewritten through temp-file + rename;
- generated Markdown reports should be recoverable from structured artifacts,
  not the other way around.

Phase 2 can add SQLite indexes:

```text
lab_runs
lab_events
lab_artifacts
lab_tasks
lab_meetings
```

The file-backed JSON artifacts remain useful as portable exports and debugging
evidence even after SQLite indexes exist.

### Pause and resume state

Pause must be a real persisted state, not a UI-only flag.

```json
{
  "lab_run_id": "labrun_20260618_001",
  "status": "paused",
  "pause_reason": "user | app_shutdown | stale_heartbeat | needs_user | risk_gate",
  "paused_at": "2026-06-18T00:00:00Z",
  "resume_cursor": {
    "last_event_seq": 128,
    "current_stage": "graduate_work",
    "internal_owner": "postdoc",
    "active_artifact_id": "artifact_postdoc_plan_001",
    "open_task_ids": ["grad_task_001"]
  }
}
```

Shutdown handling:

- graceful shutdown writes `lab_paused` with `pause_reason=app_shutdown`;
- unexpected shutdown is detected by stale heartbeat;
- startup scanner marks stale active runs as `paused_shutdown`;
- user must explicitly resume before work continues.

Heartbeat:

```json
{
  "heartbeat_at": "2026-06-18T00:00:00Z",
  "heartbeat_owner": "lab_orchestrator",
  "lease_id": "lease_...",
  "lease_owner": "pid:12345:host:gex-mac",
  "lease_ttl_seconds": 90
}
```

The heartbeat prevents ambiguous state after force-quit, crash, or machine
shutdown.

### Context reconstruction on resume

Resume should rebuild context from durable project facts:

- professor charter / project thesis;
- current LabRun stage and owner;
- current postdoc implementation plan;
- open graduate tasks and latest results;
- latest professor/postdoc/meeting reports;
- blockers and risk gates;
- validation evidence and changed-file summary;
- recent important lab events;
- compressed conversation/session summary only as supporting continuity.

Do not resume by replaying the whole raw chat transcript into the model.

### Completion and closeout evidence

LabRun should not mark work complete just because a role says it is complete.

Completion requires:

- accepted postdoc integration report;
- professor strategic verdict when the run is architecture/product-facing;
- changed-file summary with ownership labels;
- validation evidence or explicit `not_verified` status with reason;
- open blocker list empty or intentionally deferred;
- cost and context summary recorded for the run;
- final user-facing report written or linked.

Closeout states:

```text
completed_verified
completed_not_verified
partial
blocked_needs_user
cancelled
failed
```

Current implementation status:

- `/lab closeout verified [note]` records `completed_verified`.
- `/lab closeout not_verified [note]` records `completed_not_verified`.
- `/lab closeout partial [note]` records `partial`.
- `/lab closeout blocked [note]` records `blocked_needs_user` and leaves the
  run in `NeedsUser`.
- `/lab closeout failed [note]` records `failed`.
- `/lab close` records `cancelled`.
- `/lab closeout auto [note]` can close a run from the validated final
  `professor_review` gate after the run reaches `user_report`.
- Each path persists `state.json`, releases the active lease, and records a
  `lab_closeout_recorded` event.
- Fully LLM-authored professor/postdoc evidence is still pending; the current
  auto path derives status from deterministic gate metadata.

`completed_not_verified` is allowed only when the runtime explains what was not
verified and why. Graduate completion never directly closes a LabRun; postdoc
and, when needed, professor review must bind the result to evidence.

## LabRun Cost, Cache, and Compression Policy

Lab Mode does not need to optimize for minimum possible cost. It should spend
tokens where quality matters. But it must avoid invisible token burn because
LabRun can be long-running, multi-role, and partially autonomous.

The policy goal is:

- preserve strategic and implementation quality;
- keep stable prefix cache hit rates high;
- keep graduate tasks small and cheap;
- avoid sending full project history by default;
- persist raw evidence locally and pass references unless detail is needed;
- track cost by role, cycle, meeting, and LabRun.

### Context layers

LabRun context should be assembled from five layers:

```text
L0 Stable Role Prefix
  Professor/postdoc/graduate role contract.
  Stable for cache and rarely changed.

L1 Stable Project Charter
  Project thesis, goals, non-goals, architecture constraints, acceptance.
  Versioned; changes only after professor-approved revision.

L2 Current Lab State
  Stage, cycle, owner, open tasks, blockers, resume cursor.
  Small, structured, and dynamic.

L3 Role-Specific Working Packet
  Professor gets strategic reports.
  Postdoc gets plans, code evidence summaries, validation, diffs.
  Graduate gets only a narrow task envelope.

L4 On-Demand Evidence
  Raw diffs, file contents, logs, old reports, and full transcripts.
  Stored locally; referenced by ID/path unless required.
```

Cache-sensitive rule:

- L0 and L1 belong in the stable prefix.
- L2 belongs in dynamic tail.
- L3 is role-specific and should be rebuilt per internal owner.
- L4 should be refs-only by default.

Do not put `current_stage`, latest event IDs, heartbeat, or rapidly changing
blockers in the stable prefix. That would destroy provider prompt-cache hits.

### Role-specific budget policy

Professor:

- low frequency, higher quality;
- receives charter, postdoc report, strategic risks, decisions, and evidence
  summaries;
- avoids raw code unless architecture depends on it;
- spends tokens on depth, not on full logs.

Postdoc:

- medium frequency;
- receives code-aware packets, implementation plan, changed-file summaries,
  validation results, blockers, and relevant evidence refs;
- can request raw evidence on demand;
- owns compression of graduate outputs into integration reports.

Graduate:

- high frequency, cheapest/smallest context;
- receives one narrow `GraduateTask`;
- receives allowed files/snippets, acceptance criteria, validation command, and
  parent postdoc slice;
- does not receive full professor discussion or full project history.

Recommended starting budgets:

```json
{
  "mode": "balanced",
  "max_cycle_tokens": 200000,
  "max_meeting_rounds": 3,
  "professor_context_budget": 24000,
  "postdoc_context_budget": 30000,
  "graduate_context_budget": 12000,
  "meeting_context_budget": 36000,
  "auto_compress_after_cycle": true,
  "evidence_default": "refs_only"
}
```

These are policy defaults, not hard product promises. The runtime should record
actual provider usage when available: prompt, completion, reasoning, cached,
cache_write, cache_miss, and estimated cost.

### Cycle compression

Do not treat LabRun as one long chat to compact. Treat it as a project with
structured cycle summaries.

At the end of every meaningful cycle, generate a `CycleSummary`:

```json
{
  "cycle_id": "cycle_003",
  "goal": "...",
  "completed": [],
  "changed_files": [],
  "validation": [],
  "decisions": [],
  "blockers": [],
  "next_steps": [],
  "evidence_refs": []
}
```

Future context should include the latest cycle summaries and evidence refs, not
the full intermediate professor/postdoc/graduate discussion. Raw details stay
on disk and can be pulled into L4 on demand.

### Meeting cost policy

A lab meeting should not default to unlimited multi-agent debate.

Default meeting flow:

1. Runtime builds a compact meeting packet.
2. Postdoc writes the technical progress/blocker report.
3. Professor writes strategic review from charter + postdoc report.
4. If there is conflict or uncertainty, run at most a small number of
   professor/postdoc discussion rounds.
5. Runtime writes one decision report for the user.

Default `max_meeting_rounds` should be 3. More rounds require a clear reason:
strategic conflict, repeated failure, major scope change, or user request.

### Prompt-cache policy

LabRun should leverage the existing prompt-cache diagnostics, but add LabRun
labels:

- `lab_run_id`;
- `role`;
- `cycle_id`;
- `meeting_id`;
- stable charter fingerprint;
- role prefix fingerprint;
- dynamic tail fingerprint;
- cache hit/miss/write tokens.

Target behavior:

- professor and postdoc should get stable prefix hits across repeated review
  cycles;
- graduate tasks may cold-start more often, but their context must be small;
- cache misses caused by moving dynamic LabRun state into stable prefix should
  be treated as a bug.

### Compression triggers

LabRun should use multiple triggers:

- model context pressure near existing 80% threshold;
- end of each cycle;
- before lab meeting;
- before pause/resume snapshot;
- after large tool output or validation log;
- before professor review if postdoc report is too large.

Compression output should be structured:

- `CycleSummary`;
- `PostdocIntegrationSummary`;
- `GraduateTaskSummary`;
- `MeetingDecisionSummary`;
- `EvidenceIndex`.

### Cost observability

LabRun needs a cost report that is more specific than the global session cost:

```json
{
  "lab_run_id": "labrun_20260618_001",
  "total_prompt_tokens": 0,
  "total_completion_tokens": 0,
  "total_cached_tokens": 0,
  "total_cache_write_tokens": 0,
  "total_cache_miss_tokens": 0,
  "estimated_cost_usd": 0.0,
  "by_role": {
    "professor": {},
    "postdoc": {},
    "graduate": {}
  },
  "by_cycle": {},
  "by_meeting": {}
}
```

User-facing UI should show cost posture without making Lab Mode feel cheap or
timid:

- current LabRun estimated cost;
- cache hit rate;
- last cycle tokens;
- meeting tokens;
- largest cost driver;
- warning when budget is near limit.

### P0/P1 boundary

P0 must include:

- role-specific context budgets;
- stable vs dynamic LabRun context split;
- cycle summaries;
- refs-only evidence default;
- cost usage recorded per LabRun and role.

P1 can include:

- SQLite cost indexes;
- cache miss diagnostics grouped by LabRun role/cycle;
- UI budget controls;
- automatic cost-policy tuning.

## Core Schemas

These are conceptual schemas. Exact Rust structs should live under a new
`src/lab/` module.

### LabProposal

```json
{
  "schema_version": 1,
  "proposal_id": "labproposal_20260618_001",
  "status": "draft | awaiting_approval | approved | rejected | superseded",
  "created_at": "2026-06-18T00:00:00Z",
  "updated_at": "2026-06-18T00:00:00Z",
  "project_root": "/Users/georgexu/Desktop/rust-agent",
  "user_session_id": "session_...",
  "user_goal": "...",
  "problem_statement": "...",
  "desired_outcome": "...",
  "scope": [],
  "non_goals": [],
  "constraints": [],
  "risks": [],
  "success_criteria": [],
  "recommended_mode": "direct | goal | labrun",
  "professor_rationale": "...",
  "approval": {
    "approved_by_user": false,
    "approved_at": null,
    "created_lab_run_id": null
  }
}
```

### SponsorMessage

```json
{
  "schema_version": 1,
  "message_id": "sponsor_msg_001",
  "lab_run_id": "labrun_20260618_001",
  "created_at": "2026-06-18T00:00:00Z",
  "message_type": "question | concern | correction | scope_change | pause_request",
  "body": "I think the current implementation is drifting from the original product goal.",
  "urgency": "normal | high",
  "status": "queued | reviewed | converted_to_task | converted_to_meeting | rejected | superseded"
}
```

### ProfessorSteeringDecision

```json
{
  "schema_version": 1,
  "decision_id": "prof_steer_001",
  "lab_run_id": "labrun_20260618_001",
  "source_message_id": "sponsor_msg_001",
  "decision": "no_change | update_charter | create_postdoc_task | open_lab_meeting | pause_and_ask_user | revise_scope",
  "rationale": "...",
  "created_artifact_ids": [],
  "requires_user_approval": false
}
```

### LabRun

```json
{
  "schema_version": 1,
  "lab_run_id": "labrun_20260618_001",
  "kind": "architecture_plan | implementation | lab_meeting | professor_review",
  "status": "created | active | paused | paused_shutdown | blocked | completed | failed | needs_user | cancelled",
  "created_at": "2026-06-18T00:00:00Z",
  "updated_at": "2026-06-18T00:00:00Z",
  "user_goal": "Integrate professor-postdoc-grad workflow into priority-agent",
  "project_root": "/Users/georgexu/Desktop/rust-agent",
  "user_session_id": "session_...",
  "top_level_mode": "lab",
  "user_visible_role": "professor",
  "current_stage": "professor_plan",
  "internal_owner": "professor | postdoc | graduate | runtime",
  "needs_user": false,
  "pause_reason": null,
  "paused_at": null,
  "heartbeat_at": "2026-06-18T00:00:00Z",
  "lease_id": null,
  "lease_owner": null,
  "lease_ttl_seconds": 90,
  "resume_cursor": {
    "last_event_seq": 0,
    "current_stage": "professor_plan",
    "internal_owner": "professor",
    "active_artifact_id": null,
    "open_task_ids": []
  },
  "roles": {
    "professor": { "profile": "lab-professor", "model_policy": "high_reasoning" },
    "postdoc": { "profile": "lab-postdoc", "model_policy": "code_reasoning" },
    "graduate": { "profile": "lab-graduate", "model_policy": "coding_worker" }
  },
  "cost_policy": {
    "mode": "balanced",
    "max_cycle_tokens": 200000,
    "max_meeting_rounds": 3,
    "professor_context_budget": 24000,
    "postdoc_context_budget": 30000,
    "graduate_context_budget": 12000,
    "meeting_context_budget": 36000,
    "auto_compress_after_cycle": true,
    "evidence_default": "refs_only"
  },
  "artifact_ids": [],
  "cycle_count": 0,
  "failure_count": 0,
  "retry_budget": {
    "max_cycle_retries": 2,
    "max_graduate_retries_per_task": 2,
    "max_validation_retries_per_slice": 2
  },
  "meeting_ids": [],
  "open_task_ids": [],
  "blocked_reason": null,
  "closeout_status": null
}
```

### LabArtifact

```json
{
  "schema_version": 1,
  "artifact_id": "artifact_professor_plan_001",
  "lab_run_id": "labrun_20260618_001",
  "role": "professor | postdoc | graduate | runtime",
  "artifact_type": "plan | task | report | review | evidence | user_decision",
  "title": "Professor architecture plan",
  "status": "draft | accepted | rejected | superseded",
  "md_path": "docs/lab/2026-06-18-professor-architecture-plan.md",
  "json_path": ".priority-agent/lab/runs/labrun_20260618_001/professor_plan.json",
  "parent_artifact_id": null,
  "evidence_refs": [],
  "created_at": "2026-06-18T00:00:00Z"
}
```

### ProfessorPlan

```json
{
  "schema_version": 1,
  "artifact_id": "artifact_professor_plan_001",
  "role": "professor",
  "decision_level": "strategic_architecture",
  "problem_statement": "...",
  "product_thesis": "...",
  "principles": [],
  "non_goals": [],
  "architecture_constraints": [],
  "risks": [],
  "acceptance_criteria": [],
  "handoff_to_postdoc": {
    "required_analysis": [],
    "expected_plan_sections": [],
    "must_not_do": []
  }
}
```

### PostdocPlan

```json
{
  "schema_version": 1,
  "artifact_id": "artifact_postdoc_plan_001",
  "role": "postdoc",
  "parent_professor_plan_id": "artifact_professor_plan_001",
  "code_findings": [],
  "implementation_slices": [
    {
      "slice_id": "slice_001",
      "title": "Add lab schemas and file store",
      "scope": ["src/lab/", "docs/lab/"],
      "owner": "postdoc | graduate",
      "risk": "low | medium | high",
      "acceptance": [],
      "validation": ["cargo test -q lab"]
    }
  ],
  "delegation_plan": [],
  "integration_validation": []
}
```

### GraduateTask

```json
{
  "schema_version": 1,
  "task_id": "grad_task_001",
  "lab_run_id": "labrun_001",
  "created_by": "postdoc",
  "assigned_role": "graduate",
  "status": "queued | in_progress | blocked | completed | cancelled",
  "parent_slice_id": "slice_001",
  "title": "Implement LabRun serde model",
  "objective": "...",
  "allowed_scope": ["src/lab/model.rs", "src/lab/mod.rs"],
  "explicit_instructions": [],
  "forbidden_actions": [],
  "acceptance": [],
  "required_validation": ["cargo test -q lab_model"],
  "evidence_ids": [],
  "result_artifact_id": null,
  "blocker": null,
  "cycle_id": "0",
  "report_required": true
}
```

### GraduateResult

```json
{
  "schema_version": 1,
  "task_id": "grad_task_001",
  "status": "completed | blocked | failed | partial",
  "changed_files": [],
  "validation_results": [
    {
      "command": "cargo test -q lab_model",
      "status": "passed",
      "summary": "..."
    }
  ],
  "blockers": [],
  "handoff_notes": [],
  "md_report_path": "docs/lab/2026-06-18-grad-task-001-report.md"
}
```

### ProfessorReview

```json
{
  "schema_version": 1,
  "artifact_id": "artifact_professor_review_001",
  "role": "professor",
  "review_scope": "strategic_architecture",
  "verdict": "accepted | needs_revision | rejected",
  "checks": [
    {
      "name": "product_thesis_alignment",
      "status": "pass | fail | uncertain",
      "comment": "..."
    }
  ],
  "revision_request": {
    "required": false,
    "postdoc_task": null
  }
}
```

## Role Profiles

### lab-professor

Purpose: high-level project direction and architecture review.

Recommended tool surface:

- read/search docs;
- read/search code selectively;
- inspect status, git log, reports, tests summary;
- no direct code mutation by default;
- can create professor plan/review artifacts;
- can request postdoc revision.

Prompt stance:

- Think like a project PI and product architect.
- Focus on direction, completeness, tradeoffs, risk, and system boundaries.
- Do not nitpick implementation details unless they violate architecture.
- Require evidence for completion claims.

### lab-postdoc

Purpose: code-aware technical owner.

Recommended tool surface:

- read/search code;
- create implementation plans;
- spawn graduate tasks;
- edit code when acting directly;
- run validation;
- review graduate output;
- write integration report.

Prompt stance:

- Own the implementation.
- Translate strategic plan into concrete slices.
- Keep tasks narrow and verifiable.
- Do not claim done until evidence exists.

### lab-graduate

Purpose: narrow coding worker.

Recommended tool surface:

- scoped read/edit;
- narrow validation commands;
- no broad architecture decisions;
- no unrelated refactors;
- no permission to change professor/postdoc plans except reporting blocker.

Prompt stance:

- Follow the task envelope exactly.
- Stay inside allowed scope.
- Report blockers instead of improvising major direction changes.
- Return structured result and short report.

## Role Prompt Design

The professor/postdoc/graduate split only works if each role has a carefully
designed prompt and persona. Generic "you are a coding agent" prompts will make
the roles collapse into the same behavior.

Prompt design principles:

- keep each role's identity distinct;
- make each role's output contract explicit;
- give each role enough agency to be useful;
- keep role authority bounded by runtime state and permissions;
- avoid turning professor into a code reviewer or graduate into an architect;
- preserve a consistent lab metaphor without making the product theatrical.

The role prompts should be stored as versioned built-in profile templates. A
LabRun should record the prompt/profile version used for each role so future
reviews can explain why an agent behaved a certain way.

### Professor prompt contract

Persona:

- principal investigator, product architect, and strategic reviewer;
- thinks in project thesis, architecture direction, non-goals, risk, and
  long-term value;
- talks to the user as the project sponsor/funding committee.

Primary jobs:

- clarify whether an idea deserves LabRun treatment;
- produce `LabProposal` and `ProfessorPlan`;
- decide whether user sponsor messages require steering changes;
- review postdoc reports for strategic alignment;
- decide when to open a lab meeting, revise scope, ask user, or stop.

Inputs:

- user/project sponsor messages;
- LabProposal / LabRun charter;
- postdoc plans and reports;
- evidence summaries and validation status;
- project status and strategic constraints.

Outputs:

- `LabProposal`;
- `ProfessorPlan`;
- `ProfessorReview`;
- `ProfessorSteeringDecision`;
- lab meeting decision report.

Behavioral boundaries:

- does not directly edit code by default;
- does not assign low-level implementation details unless architecture depends
  on them;
- does not accept completion claims without evidence;
- does not expand scope without labeling it as proposal revision or follow-up.

### Postdoc prompt contract

Persona:

- senior technical owner and integration lead;
- translates professor strategy into concrete implementation slices;
- owns code quality, validation, and truthful status.

Primary jobs:

- read code and project evidence;
- turn professor plans into executable technical plans;
- split work into graduate tasks when useful;
- implement directly when delegation adds overhead;
- review graduate output and repair or reject weak work;
- write postdoc integration reports with validation evidence.

Inputs:

- accepted LabProposal and ProfessorPlan;
- current LabRun state and cycle summary;
- code/search evidence;
- graduate task results;
- validation output and dirty worktree state.

Outputs:

- `PostdocPlan`;
- `GraduateTask` envelopes;
- `PostdocIntegrationSummary`;
- blocker reports;
- validation and changed-file summaries.

Behavioral boundaries:

- may mutate code only inside an approved implementation cycle;
- cannot redefine product direction without professor/user approval;
- cannot mark work done without validation or explicit `not_verified` reason;
- must label user/pre-existing changes separately from LabRun changes.

### Graduate prompt contract

Persona:

- focused implementation worker;
- optimizes for precise execution of a narrow task;
- reports blockers honestly instead of improvising broad design changes.

Primary jobs:

- implement one scoped task;
- stay inside allowed files/actions;
- run required validation;
- return structured result, changed files, and blockers.

Inputs:

- one `GraduateTask`;
- allowed scope;
- specific snippets or evidence refs;
- required validation command;
- acceptance criteria.

Outputs:

- `GraduateResult`;
- short task report;
- changed-file list;
- validation result summary;
- blocker report if needed.

Behavioral boundaries:

- cannot change professor/postdoc plans;
- cannot broaden scope;
- cannot self-certify project completion;
- cannot ignore validation failures.

### Prompt plus control, not prompt instead of control

Prompts should make the roles vivid and capable, but runtime control remains
mandatory:

- `LabOrchestrator` owns state transitions;
- permissions/checkpoints own mutation safety;
- cost/cache policy owns context budget;
- leases own concurrency;
- evidence refs own proof;
- closeout states own completion truth.

This is the intended split: prompts create role intelligence and flavor;
runtime control keeps the lab reliable.

## Process-First Orchestration

LabRun should be driven primarily by runtime routing and a strict workflow, not
by letting one LLM freely improvise a multi-agent collaboration.

The LLMs provide judgment, plans, reviews, explanations, and implementation
work. The runtime provides:

- stage transitions;
- role ownership;
- required artifacts;
- handoff gates;
- permissions and checkpoints;
- retry budgets;
- pause/resume and leases;
- evidence and closeout requirements.

This matters because professor/postdoc/graduate collaboration creates many
opportunities for hallucinated handoffs:

- professor thinks postdoc already read code when it has not;
- postdoc thinks graduate ran validation when it did not;
- graduate claims completion without evidence;
- meeting discussion changes scope without recording a new decision;
- resume reconstructs the wrong project state from chat text.

The answer is not more prompt text. The answer is process.

### Required artifacts are stage gates

Even when a document feels unnecessary, the workflow should still write the
required structured artifact or Markdown summary. The artifact is not just
paperwork. It is the durable handoff between agents and the evidence boundary
for the next stage.

Required stage gates:

| From | Required artifact | Why |
|---|---|---|
| intake -> approval | `LabProposal` | user approves a concrete project, not a vague chat |
| approval -> postdoc | `ProfessorPlan` | postdoc receives explicit strategic constraints |
| postdoc -> graduate | `GraduateTask` | graduate gets scoped work and validation |
| graduate -> postdoc | `GraduateResult` | postdoc reviews actual output and evidence |
| postdoc -> professor | `PostdocIntegrationSummary` | professor reviews strategy from a grounded report |
| professor -> next cycle | `ProfessorReview` or `ProfessorSteeringDecision` | scope and direction changes are recorded |
| cycle end -> resume | `CycleSummary` | future context does not depend on raw chat history |
| final closeout | closeout report | user sees verified / partial / blocked truth |

Runtime should refuse to advance a stage when the required artifact is missing,
malformed, stale, or not tied to evidence.

### Minimal artifacts are allowed, missing artifacts are not

For small cycles, the artifact can be short and structured. It does not need to
be a long Markdown document. But it must exist and it must carry the fields
needed by the next stage:

- owner;
- scope;
- decision;
- evidence refs;
- validation status;
- blockers;
- next action.

This keeps the process lightweight while preserving the rigor that makes
multi-agent collaboration reliable.

## Runtime Workflow

### Top-level routing

Direct Mode and Lab Mode must be separated before ordinary intent routing.

```text
User message
  -> TopLevelModeRouter
      -> Direct Mode
          -> current intent router / conversation loop / tools
      -> Lab Mode
          -> professor-facing conversation
          -> LabOrchestrator state machine
```

Direct Mode keeps the current mainline coding-agent behavior. Lab Mode uses the
same runtime/tool/checkpoint/evidence infrastructure, but the user-visible
surface is the professor.

### Project intake lifecycle

```text
pa lab
  -> professor_intake
  -> clarify goal / scope / success criteria
  -> LabProposal draft
  -> user approval gate
      -> approve: create LabRun and enter professor_discussion/professor_plan
      -> revise: continue professor_intake
      -> reject: close proposal
      -> direct_mode: return to ordinary agent flow
```

The intake conversation is allowed to write proposal artifacts and human-facing
notes, but it should not schedule internal implementation work.

### Lab Mode stages

```text
approved LabProposal
  -> professor_discussion
  -> professor_plan
  -> postdoc_plan
  -> graduate_work
  -> postdoc_review
  -> professor_review
  -> user_report | needs_user | next_cycle
```

Stage ownership:

| Stage | User-facing role | Internal owner |
|---|---|---|
| `professor_discussion` | professor | professor |
| `professor_plan` | professor | professor |
| `postdoc_plan` | hidden unless requested | postdoc |
| `graduate_work` | hidden unless requested | graduate/postdoc |
| `postdoc_review` | hidden unless requested | postdoc |
| `professor_review` | professor | professor |
| `lab_meeting` | professor-led | professor + postdoc, optionally graduate reports |
| `user_report` | professor | professor |

The user can inspect or interrupt at any time through `/lab status`,
`/lab report`, `/lab pause`, or `/lab intervene`.

### Sponsor side-channel during active LabRun

```text
Active LabRun loop
  -> user observes internal progress
  -> user sends side-channel message to professor
  -> SponsorMessage is appended to LabRun events
  -> professor reviews message against proposal, charter, state, and evidence
      -> no_change: professor explains why current loop continues
      -> update_charter: create revised professor artifact
      -> create_postdoc_task: insert task/change request into queue
      -> open_lab_meeting: start professor/postdoc discussion
      -> pause_and_ask_user: stop scheduling and request explicit decision
      -> revise_scope: create proposal revision requiring approval
```

The side channel should not interrupt every internal step. It becomes active
work only when the professor turns it into a structured steering decision.

### Pause/resume lifecycle

```text
Active LabRun
  -> user clicks pause or runs /lab pause
      -> stop scheduling new internal work
      -> persist lab_paused event
      -> update state.status=paused
  -> app graceful shutdown
      -> persist lab_paused event with pause_reason=app_shutdown
      -> update state.status=paused
  -> crash / force quit / power loss
      -> heartbeat becomes stale
      -> startup scanner marks state.status=paused_shutdown
```

Resume:

```text
App starts
  -> scan .priority-agent/lab/runs
  -> detect paused / paused_shutdown LabRun
  -> show "Continue Lab Project" prompt
      -> Continue: rebuild context from LabRun + resume_cursor
      -> Inspect: open latest report/status
      -> Keep paused: do nothing
      -> Close project: mark cancelled or archived
```

Resume must not automatically start mutating code. User intent is required.
When resumed, LabOrchestrator continues from `resume_cursor`, not from a fresh
plan unless the persisted state is invalid.

### Architecture planning run

```text
User idea
  -> ProfessorPlan
  -> PostdocPlan
  -> optional GraduateTasks
  -> PostdocIntegrationReport
  -> ProfessorReview
  -> User decision
```

### Implementation run

```text
Accepted ProfessorPlan
  -> repeat until done / blocked / needs_user:
      -> Postdoc reads code
      -> Postdoc creates or revises slices
      -> Graduate tasks execute
      -> Postdoc reviews and integrates
      -> Validation gates
      -> Postdoc progress or integration report
      -> Professor strategic review when stage boundary or risk requires it
      -> Optional lab meeting if coordination is needed
```

### Lab meeting run

```text
User clicks "Lab Meeting", runs /lab meeting, or professor triggers meeting
  -> Runtime collects recent context
  -> Postdoc progress review
  -> Professor strategic review
  -> Professor/Postdoc discussion loop
  -> Meeting decision
  -> One meeting report for the user
  -> Suggested next actions
```

### Internal multi-round loop

After a project is approved, the lab should support multiple internal cycles
without asking the user every time.

```text
ProfessorPlan
  -> PostdocPlan
  -> GraduateTask batch
  -> GraduateResult(s)
  -> PostdocReview
  -> if accepted:
       continue next slice or write integration report
     if technical blocker:
       postdoc blocker report -> professor decision
     if strategic concern:
       professor-triggered lab meeting
     if user decision required:
       user_report with explicit question
  -> ProfessorReview at stage boundary
  -> next cycle or completed
```

Cycle stop conditions:

- professor marks project strategically complete;
- postdoc marks implementation complete and validation passes;
- repeated graduate failures exceed threshold;
- retry budget is exhausted for the current task, slice, or cycle;
- professor decides scope must change;
- permission/risk gate requires user;
- active lease is lost or heartbeat cannot be refreshed;
- cost budget requires user approval;
- user pauses or intervenes.

Failure escalation:

- first narrow failure: postdoc can create one repair task;
- repeated same-slice failure: postdoc writes a blocker report;
- strategic or repeated blocker: professor decides whether to revise plan,
  open a lab meeting, ask user, or stop;
- validation failure after retry budget: LabRun enters `blocked_needs_user`
  unless the professor explicitly narrows scope.

### Lab meeting as multi-agent discussion

A lab meeting is not just a report generator. It is a structured multi-agent
discussion where professor and postdoc run side by side:

- professor focuses on direction, architecture, risk, and project thesis;
- postdoc focuses on concrete code state, failed experiments, validation, and
  implementation constraints;
- graduate agents may contribute written experiment reports but should not
  drive the meeting unless invited for a narrow technical explanation.

Meeting output:

- decision;
- rationale;
- revised constraints or non-goals;
- postdoc follow-up tasks;
- graduate task changes;
- whether user approval is required.

## Lab Meeting Context Collection

The lab meeting should collect evidence, not just ask the LLM to summarize from
memory.

Inputs:

- current git branch and recent commits;
- dirty worktree status;
- recent docs changed;
- `docs/PROJECT_STATUS.md`;
- recent lab artifacts;
- recent workflow reports;
- recent failed/passed validation commands if available;
- open todos/goals/session task state;
- user-provided meeting topic, if any.

Postdoc report should include:

- what changed recently;
- what appears complete;
- what is still risky or incomplete;
- code areas touched;
- validation evidence;
- recommended next engineering slices.

Professor review should include:

- whether the project is moving toward the product thesis;
- whether recent work is too local or still strategically valuable;
- missing top-level concerns;
- suggested next direction;
- explicit "do not spend time on this now" guidance.

## User Experience

### CLI/TUI commands

Initial commands:

```text
/lab propose <idea>
/lab approve <proposal_id>
/lab start <goal>
/lab meeting
/lab plan <topic>
/lab integrate [note]
/lab professor-review [note]
/lab draft [instructions]
/lab accept <artifact_id> [note]
/lab revise <artifact_id> <note>
/lab review artifact <artifact_id> [instructions]
/lab task revise <task_id> | <scope_csv> | <validation_csv> | [instructions]
/lab step llm [instructions]
/lab run llm [max_steps] [instructions]
/lab run hybrid [max_steps] [instructions]
/lab status
/lab professor <message>
/lab note <message>
/lab messages
/lab messages <review|meeting|task|reject|apply> <message_id> [note]
/lab open <lab_run_id>
/lab review <lab_run_id>
/lab pause
/lab resume
/lab intervene <message>
/lab closeout <auto|verified|not_verified|partial|blocked|failed> [note]
/lab close
```

Potential later commands:

```text
/lab reject <proposal_id>
/lab delegate <slice_id>
/lab export <lab_run_id>
```

### Desktop / TUI UI

Add a visible "Lab Meeting" action in command palette or status area.

During an active LabRun, the UI should have two separate surfaces:

- main run view: read-only visibility into loop progress, current owner,
  tasks, reports, validation, blockers, and evidence;
- side professor channel: user messages the professor about concerns,
  corrections, or direction changes.

The side professor channel is not a command console for postdoc or graduate
agents. It creates `SponsorMessage` records for professor review.

Meeting panel should show:

- latest lab run status;
- current Lab Mode stage;
- current internal owner;
- pause reason / resume availability;
- last heartbeat age;
- professor verdict;
- postdoc summary;
- cycle count;
- open slices;
- blocked tasks;
- next recommended action.

The UI should not interrupt startup by default. It can show a quiet indicator
when a meeting is recommended. In Lab Mode, a professor-triggered meeting can
surface as a pending meeting request with a short reason, not as a surprise
modal that blocks the user.

## Implementation Plan

### Phase 0: Product spec and templates

Goal: make the workflow concrete before code.

Tasks:

- Add this plan.
- Add `docs/lab/README.md`.
- Add Markdown templates:
  - professor plan;
  - postdoc plan;
  - graduate task report;
  - postdoc integration report;
  - professor review;
  - lab meeting report.

Validation:

```bash
rg -n "lab_run_id|Professor|Postdoc|Graduate" docs/lab
```

### Phase 1: Structured lab model

Goal: introduce typed artifacts without runtime orchestration yet.

Current status:

- Completed: `src/lab/model.rs`, `src/lab/store.rs`, and deterministic
  artifact-gated orchestration now exist.
- Completed: typed JSON artifacts are persisted for the current stage through
  `/lab plan <note>`.
- Completed: `src/lab/report.rs` renders Markdown reports from structured
  stage artifacts.
- Completed: `/lab draft [instructions]` can ask the current provider to draft
  the current-stage artifact body and persist it through the structured
  artifact/report/gate pipeline.
- Completed: `/lab draft` can parse strict role-specific JSON for the core
  professor/postdoc/graduate artifact body types.
- Completed: `/lab accept` and `/lab revise` provide the first artifact
  acceptance/revision flow and wire revision blockers into gate validation.
- Completed: `/lab review artifact` can ask the current provider for a
  strict accept/revise decision and apply it through the deterministic gate
  path.
- Completed: `/lab step llm` chains provider-backed artifact drafting,
  provider-backed review, deterministic accept/revise gates, and advancement
  for non-graduate stages.
- Completed: `/lab run llm` repeats provider-backed non-graduate stage steps in
  a bounded foreground loop and stops at revision/user/terminal/graduate
  boundaries.
- Completed: `/lab run hybrid` repeats provider-backed non-graduate stages and
  hands `graduate_work` to the strict graduate scheduler.
- Completed: `/lab background hybrid` runs provider-backed hybrid planning in
  the current shell process and stops at strict user/blocker/graduate
  boundaries.
- Completed: app lifecycle startup/shutdown checkpoints persist
  `.priority-agent/lab/app_lifecycle.json` and recover interrupted scheduler
  state to `PausedRestart`.
- Completed: persistent daemon policy records enabled/disabled state, strict vs
  hybrid mode, bounded step/interval limits, and optional restart instructions.
- Completed: `pa lab` startup and `/lab daemon start` can consume persisted
  daemon policy and start strict or provider-backed hybrid background execution
  in the current app process.
- Completed: `pa lab-daemon` provides a non-interactive foreground worker that
  consumes daemon policy and runs strict/hybrid Lab execution to a boundary.
- Completed: `/lab daemon launchd [label]` writes a macOS LaunchAgent plist for
  the non-interactive worker under `.priority-agent/lab/launchd/`.
- Completed: `/lab daemon health` provides the read-only health endpoint for
  future desktop/OS supervision of daemon policy, scheduler state, lifecycle
  checkpoint, last start error, and LaunchAgent plist presence.
- Completed: `/lab daemon service [status|install|uninstall|commands] [label]`
  manages LaunchAgent plist installation/removal and renders the exact
  `launchctl` commands without executing them.
- Completed: `/lab daemon service load|unload|restart [label]` executes the
  explicit `launchctl bootstrap`, `bootout`, and `kickstart -k` actions with
  mockable command coverage.
- Completed: `/lab daemon service supervise [label]` checks policy, probes
  `launchctl print`, and repairs a missing service with the same install plus
  bootstrap path.
- Completed: desktop Tauri exposes `lab_daemon_supervise`, returning supervise
  output plus refreshed Lab status for the selected project.
- Completed: desktop Workbench exposes an explicit Lab daemon supervise button
  wired to that Tauri command.
- Completed: Workbench keeps a conservative 120-second daemon supervision timer
  while the drawer is open.
- Completed: `/lab messages classify <message_id|latest> [instructions]` asks
  the Professor provider to classify sponsor side-channel messages into
  review/meeting/task/reject status without directly applying actions.
- Completed: `/lab messages decision <message_id|latest>` writes a durable
  Professor steering decision artifact/report from the persisted SponsorMessage
  status, with the next explicit command but without applying the decision.
- Completed: Lab role agent profiles now expose prompt versions in
  `AgentProfile`, `AgentDefinition`, and agent envelope constraints.
- Completed: `/lab recovery` provides a read-only resume/inspect/keep-paused/
  close options backend for paused or needs-user LabRuns.
- Completed: `/lab report [list|latest|artifact_id]` previews generated
  Markdown reports without creating new artifacts or advancing the LabRun.
- Completed: `/lab open <lab_run_id>` safely switches the active LabRun pointer
  for inspection without resuming work or acquiring a lease.
- Completed: `/lab runs` lists recent LabRuns and marks the current
  active-pointer target for `/lab open` / recovery workflows.
- Completed: `/lab review` shows a current LabRun review summary and concrete
  next review actions instead of returning a placeholder.
- Completed: `pa lab` welcome renders a Lab-only Professor intake hint for
  empty intake, pending proposals, or active LabRuns.
- Completed: accepted `PostdocPlan` artifacts automatically queue scoped
  `GraduateTask` records, with blocked tasks when scope or validation is
  missing.
- Completed: app-scoped desktop timer invokes the active supervision backend
  for the non-interactive worker while preserving daemon-policy opt-in.

Files:

- `src/lab/mod.rs`
- `src/lab/model.rs`
- `src/lab/store.rs`
- `src/lab/report.rs`

Build:

- serde models for `LabRun`, `LabArtifact`, `ProfessorPlan`,
  `PostdocPlan`, `GraduateTask`, `GraduateResult`, `ProfessorReview`;
- JSON schema version field;
- artifact gate metadata: owner, scope, evidence refs, validation status,
  blockers, and next action;
- pause status, pause reason, heartbeat, lease TTL, and resume cursor;
- cost policy fields and per-role budget defaults;
- file-backed store under `.priority-agent/lab/runs/`;
- event append to `events.jsonl`;
- report path registry.

Validation:

```bash
cargo test -q lab
cargo check -q
```

### Phase 1.5: LabRun cost/cache/compression policy

Goal: make Lab Mode safe to run for long projects before broad autonomy.

Current status:

- Completed: `LabCostPolicy` role budgets and cycle/meeting limits exist on
  `LabRun`.
- Completed: `LabCostUsage` ledger records are persisted under each LabRun and
  tagged by `lab_run_id`, `role`, `cycle_id`, and `meeting_id`.
- Completed: `/lab cost` renders total tokens, cache hit/write/miss tokens,
  estimated cost, and per-role summaries.
- Completed: `LabContextPacket` layering and stable/dynamic fingerprints exist
  through `/lab context [role]`.
- Completed: refs-only evidence index exists through `/lab evidence add/list`
  and the `L4 refs-only-evidence-index` context layer.
- Completed: cycle summaries exist through `/lab cycle summary <text>` and are
  written as structured artifacts plus Markdown reports.
- Completed: deterministic compression decisions exist through
  `/lab compression [role]` and `compression_decisions.jsonl`.
- Completed: compression summary execution exists through `/lab compress [role]`
  and writes structured artifacts plus Markdown reports when compression is
  recommended or required.
- Completed: automatic recording from real provider usage mirrors main
  `ConversationLoop` usage into the active LabRun cost ledger.
- Completed: Lab Mode live request construction injects the active
  `LabContextPacket` into provider-bound request messages.
- Completed: live request-time compression decisions are persisted from the
  active `LabContextPacket` into `compression_decisions.jsonl`.
- Completed: persisted live compression decisions automatically write
  deduped compression summary artifacts when policy and thresholds require it.

Build:

- `LabCostPolicy` model with role budgets and meeting/cycle limits;
- `LabContextPacket` builder with L0-L4 context layers;
- stable prefix fingerprint for role profile + project charter;
- dynamic tail fingerprint for current LabRun state;
- `CycleSummary` artifact written after each meaningful cycle;
- refs-only evidence index for large logs, diffs, and file contents;
- cost usage records tagged by `lab_run_id`, `role`, `cycle_id`, and
  `meeting_id`.

Validation:

```bash
cargo test -q lab_cost
cargo test -q lab_context
cargo check -q
```

### Phase 2: Role profiles

Goal: expose professor/postdoc/graduate as product profiles.

Files:

- `src/agent/profiles.rs`
- possibly `.agents/profiles/` examples or built-in profile definitions.
- role prompt templates under a stable built-in profile location.

Build:

- built-in `lab-professor`, `lab-postdoc`, `lab-graduate` profiles;
- versioned prompt/persona templates for professor, postdoc, and graduate;
- explicit input/output contracts for each role prompt;
- tool-surface restrictions per role;
- clear profile descriptions for `/agents` or equivalent surfaces;
- ensure graduate defaults to scoped coding-worker behavior.
- tests that profile prompts preserve role boundaries and required output
  contracts.

Validation:

```bash
cargo test -q agent_profile
cargo test -q agent_tool
```

### Phase 3: Lab command surface

Goal: let user start and inspect lab workflow manually.

Files:

- entry command parsing in `src/main.rs` or `src/entry/mod.rs`;
- `src/entry/lab.rs` for the Lab Mode command entry;
- CLI slash handling in `src/shell/slash.rs` or shared command layer;
- TUI slash handling in `src/tui/slash_handler/`;
- shared lab command module if needed.

Build:

- `pa lab` / `priority-agent lab` as a first-class Lab Mode launcher;
- `/lab propose <idea>`;
- `/lab approve <proposal_id>`;
- `/lab start <goal>`;
- `/lab meeting`;
- `/lab plan <topic>`;
- `/lab integrate [note]`;
- `/lab professor-review [note]`;
- `/lab professor-review llm [instructions]`;
- `/lab draft [instructions]`;
- `/lab accept <artifact_id> [note]`;
- `/lab revise <artifact_id> <note>`;
- `/lab review artifact <artifact_id> [instructions]`;
- `/lab task revise <task_id> | <scope_csv> | <validation_csv> | [instructions]`;
- `/lab step llm [instructions]`;
- `/lab run llm [max_steps] [instructions]`;
- `/lab run hybrid [max_steps] [instructions]`;
- `/lab background hybrid [max_steps] [interval_ms] [instructions]`;
- `/lab status`;
- `/lab professor <message>` or `/lab note <message>` for sponsor-to-professor
  side-channel input;
- `/lab open <id>`;
- `/lab pause`;
- `/lab resume`;
- `/lab intervene`;
- `/lab close`;
- renderer for concise lab run summary.
- `SponsorMessage` append path and professor steering decision renderer.

MVP behavior can generate structured artifacts and Markdown reports with one
current foreground agent turn, without full autonomous delegation. Formal
LabRun creation should require an approved proposal.

Validation:

```bash
cargo test -q lab
cargo test -q slash
cargo check -q
```

### Phase 4: Postdoc review and professor review orchestration

Goal: implement the full two-level review loop before graduate delegation.

Current status:

- Completed: manual artifact accept/revise gates exist and block advancement
  on revision.
- Completed: provider-backed `/lab review artifact` can produce strict
  accept/revise decisions and apply them through the same deterministic gate
  path.
- Completed: foreground `/lab step llm` can run one provider-backed
  draft/review/advance step for non-graduate stages.
- Completed: foreground `/lab run llm` can repeat provider-backed
  draft/review/advance steps across non-graduate stages until a revision,
  user/terminal state, max-step limit, or graduate boundary is reached.
- Completed: foreground `/lab run hybrid` can continue from provider-backed
  non-graduate stages into the strict graduate scheduler boundary.
- Completed: foreground `/lab run hybrid` now also runs deterministic
  `postdoc_review` and `professor_review` bridges instead of provider-drafting
  those review artifacts.
- Completed: strict scheduler steps and process-local background scheduling now
  also run deterministic `postdoc_review` and `professor_review` bridges.
- Completed: provider-backed hybrid process-local background scheduling can run
  bounded professor/postdoc planning while reusing the same strict graduate and
  review boundaries.
- Completed: accepted postdoc plans now create graduate tasks from their
  implementation slices, allowing the strict graduate scheduler to find queued
  work instead of always stopping at an empty `graduate_work` stage.
- Completed: `/lab integrate` creates postdoc-owned integration summaries from
  persisted `GraduateResult` artifacts and blocks the postdoc gate when
  graduate blockers or missing validation remain.
- Completed: `/lab professor-review` creates professor-owned final reviews
  from postdoc integration evidence and gates `user_report` handoff.
- Completed: `/lab professor-review llm` lets the provider Professor make the
  strategic review from postdoc evidence while runtime enforces the final gate
  and evidence boundary.
- Completed: rejected deterministic or provider-backed professor reviews create
  a durable `LabRevisionTask` artifact for postdoc repair.
- Completed: the next `postdoc_plan` creation consumes pending
  `LabRevisionTask` artifacts by adding them to postdoc context/evidence and
  marking the revision task consumed.
- Completed: strict and hybrid scheduler paths now resume from a rejected
  professor review to `postdoc_plan`, so the next postdoc planning pass can
  consume the professor revision task without manual stage editing.
- Not yet complete: live provider validation across supported models for the
  autonomous postdoc repair loop.

Build:

- runtime controller method to run postdoc report from collected context;
- professor review pass over postdoc report and evidence;
- verdict: accepted / needs_revision / rejected / needs_user;
- live-provider validation for autonomous postdoc repair after
  `LabRevisionTask` consumption.

Important boundary:

- professor review must receive evidence summaries and strategic plan artifacts,
  not huge raw diffs by default;
- postdoc remains responsible for code-level correctness.
- professor/postdoc context packets must preserve stable prefix cache by keeping
  rapidly changing LabRun state out of L0/L1.
- professor review can reject "done" claims when validation evidence, changed
  file ownership, or user-impact analysis is missing.

Validation:

```bash
cargo test -q lab_review
cargo test -q lab
cargo check -q
```

### Phase 5: Lab loop orchestration

Goal: support repeated professor/postdoc cycles before adding broad graduate
delegation.

Current status:

- Completed: deterministic stage transitions exist for
  `professor_discussion -> postdoc_plan -> graduate_work -> postdoc_review ->
  professor_review -> user_report`.
- Completed: `/lab tick` runs one runtime-controlled orchestration step by
  creating the current-stage artifact, satisfying the gate, advancing the
  stage, and stopping at `user_report`.
- Completed: artifact gate validation blocks stage advancement when required
  handoff proof is missing.
- Completed: pause/resume, stale lease recovery, and clean shutdown pause
  preserve a resumable cursor.
- Completed: bounded foreground scheduling, process-local background
  scheduling, and restart recovery for interrupted scheduler state.
- Completed: foreground `/lab step llm` runs one provider-backed
  draft/review/advance step for non-graduate stages.
- Completed: `/lab background hybrid [max_steps] [interval_ms] [instructions]`
  runs provider-backed hybrid planning in the current shell process while
  preserving strict stop boundaries.
- Completed: explicit `/lab continue [note]` writes a cycle summary and starts
  the next cycle from `professor_discussion` after `user_report`.
- Completed: retry budget accounting, professor-triggered meeting signals, and
  blocker artifacts exist in the LabRun loop.
- Completed: `pa lab` startup/shutdown records app lifecycle state, startup
  recovery, and shutdown pause through the Lab store.
- Completed: `/lab daemon ...` persists the intended app-owned background
  policy for a future desktop/host process.
- Completed: daemon policy can now be executed automatically when `pa lab`
  starts or manually with `/lab daemon start`.
- Completed: `pa lab-daemon` can run daemon policy without an interactive
  terminal, suitable for a future desktop/launch-agent host.
- Completed: `/lab daemon launchd [label]` can generate the macOS LaunchAgent
  manifest needed by that future desktop/launch-agent host.
- Completed: `/lab daemon health` gives that future host a read-only health
  check over policy, scheduler, lifecycle, start errors, and plist presence.
- Completed: `/lab daemon service [status|install|uninstall|commands] [label]`
  gives that future host a tested service file management surface and exact
  load/unload/kickstart/print command plan.
- Completed: `/lab daemon service load|unload|restart` can execute those
  load/unload/kickstart actions explicitly, with tests replacing `launchctl`.
- Completed: `/lab daemon service supervise` gives that future host a tested
  health-probe and repair action for missing LaunchAgent service state.
- Completed: desktop backend exposes that supervision action through a Tauri
  command for the selected project.
- Completed: Workbench exposes a user-triggered supervise action for that
  desktop backend.
- Completed: Workbench-open sessions periodically call the same supervise
  backend with a local busy guard.
- Completed: Professor-backed `/lab messages classify` can classify sponsor
  side-channel messages while preserving explicit apply boundaries.
- Completed: `/lab messages decision` writes the Professor steering decision
  artifact/report for queued/classified/applied sponsor messages without
  applying meetings or tasks.
- Completed: `/lab meeting open [topic]` provides the read-only open-meeting
  backend for professor-triggered meeting recommendations.
- Completed: Lab professor/postdoc/graduate prompt versions are explicit in the
  runnable profile registry and dispatch constraints.
- Completed: `/lab recovery` exposes the recovery choice surface required
  before desktop startup prompts/buttons can be wired.
- Completed: `/lab report` provides the read-only latest-report inspection
  surface referenced by pause/resume and sponsor-facing workflows.
- Completed: `/lab open <lab_run_id>` supports switching between paused or
  historical LabRuns for inspection while preserving lease boundaries.
- Completed: `/lab runs` gives users a CLI discovery surface for historical
  LabRuns before opening or recovering one.
- Completed: `/lab review` provides the current read-only review panel backend
  for artifact/gate/report/blocker state.
- Not yet complete: real graduate execution validation with live providers.

Build:

- `LabOrchestrator` state transitions for:
  - `professor_discussion`;
  - `professor_plan`;
  - `postdoc_plan`;
  - `postdoc_review`;
  - `professor_review`;
  - `lab_meeting`;
  - `needs_user`;
  - `completed`;
- cycle counter and max-cycle safety budget;
- per-cycle token budget enforcement for explicit foreground hybrid cycle runs;
- Completed: professor-triggered meeting request artifact.
- Completed: postdoc blocker report artifact.
- Completed: read-only meeting summary artifact with recorded decision field.
- Completed: pause/resume hooks that stop scheduling and persist a resumable
  cursor.
- Completed: active lease acquisition/refresh/release around mutating
  orchestration.
- Completed: retry budget accounting and escalation to blocker/user states.
- Completed: artifact gate validation before every stage transition.

Important boundary:

- automatic internal cycles can continue only when permissions, cost budget,
  risk, and validation policy allow it;
- user approval remains required for high-risk mutations, external publication,
  unclear scope changes, or major cost increase.
- losing the active lease must stop scheduling new internal work.
- missing or malformed stage artifacts must block progression instead of
  asking the LLM to remember what happened.

Validation:

```bash
cargo test -q lab_orchestrator
cargo test -q lab
cargo check -q
```

### Phase 5.5: Startup recovery and heartbeat

Goal: make Lab Mode safe across app close, crash, and machine shutdown.

Build:

- project-root lease file for active mutating LabRuns;
- heartbeat writer for active LabRuns;
- startup scanner for active/stale/paused runs;
- stale heartbeat detection that marks `paused_shutdown`;
- "Continue Lab Project" prompt in CLI/TUI/desktop entry surfaces;
- resume context builder that reconstructs project context from LabRun state,
  artifacts, events, reports, and compressed session summaries.

Validation:

```bash
cargo test -q lab_recovery
cargo test -q lab
cargo check -q
```

### Phase 6: Graduate task delegation

Goal: connect lab graduate tasks to the existing subagent machinery.

Current status:

- Completed: `GraduateTask` and `LabTaskStatus` model the postdoc-to-graduate
  task envelope.
- Completed: task records persist under each LabRun and participate in
  resumable `open_task_ids`.
- Completed: manual `/lab task ...` commands cover create/list/start/block/
  complete/cancel lifecycle transitions.
- Completed: deterministic `GraduateTask -> AgentTaskEnvelope` conversion and
  `lab-graduate` agent tool params are available through
  `/lab task envelope <task_id>`.
- Completed: manual graduate result binding writes `GraduateResult` artifacts,
  Markdown reports, task completion state, and artifact-gate evidence.
- Completed: manual result binding rejects reported changed files outside the
  task `allowed_scope`.
- Completed: prepared graduate dispatch records persist the exact
  `AgentTaskEnvelope` and agent tool params for audit/recovery.
- Completed: an async adapter can execute a validated graduate task through the
  existing agent tool when the runtime supplies `ToolContext`.
- Completed: LabRun orchestrator has an async execution hook that records
  dispatch lifecycle and blocks failed graduate executions.
- Completed: shell Lab Mode can invoke the graduate execution hook through
  `/lab task run <task_id>` using runtime `ToolContext`.
- Completed: `/lab step` provides a strict scheduler step that blocks
  `graduate_work` without a queued task instead of producing placeholder work.
- Completed: `/lab run [max_steps]` provides bounded foreground scheduler
  execution and stops on blocked/needs-user/graduate-dispatch states.
- Completed: `/lab background ...` provides a process-local background loop
  with persisted scheduler state and heartbeat refresh.
- Completed: process-local background scheduling can now cross deterministic
  postdoc/professor review bridges and stop at `user_report`.
- Completed: interrupted `Running`/`Stopping` background scheduler state is
  recovered as `PausedRestart` on Lab CLI startup or `/lab background recover`.
- Completed: graduate agent execution is checked against task `allowed_scope`
  using before/after workspace snapshots before dispatch success is accepted.
- Completed: structured graduate agent JSON output can automatically bind a
  `GraduateResult` artifact and dispatch result artifact ID.
- Completed: if a graduate agent succeeds but misses the structured JSON
  contract, the runtime can still bind a `GraduateResult` only after
  parent-side verification proves scoped file changes and required validation.
- Completed: live graduate validation can run with the uncertified-provider
  override for runtime-path evidence without writing a formal
  `graduate passed` provider certification record.
- Completed: the `lab-graduate` profile and generated graduate task prompt now
  require the JSON shape consumed by automatic result binding.
- Completed: `/lab integrate [note]` turns bound `GraduateResult` artifacts
  into postdoc-owned integration summaries and gates professor handoff on
  blockers/validation evidence.
- Completed: `/lab professor-review [note]` turns postdoc integration evidence
  into professor-owned final review artifacts and the `user_report` gate.
- Completed: `/lab meeting [topic]` writes a read-only `LabMeetingSummary`
  artifact and Markdown report.
- Completed: `/lab meeting recommend` reports professor-triggered meeting
  recommendations from blocked tasks, repeated dispatch failures, and failure
  budget exhaustion.
- Completed: graduate execution failures increment LabRun failure accounting and
  escalate to `NeedsUser` when retry budget is exhausted.
- Completed: `/lab blocker report [note]` writes a postdoc-owned
  `LabBlockerReport` artifact and Markdown handoff report.
- Completed: `/lab task retry <task_id> | <validation_summary>` records
  validation retry attempts, creates repair tasks within budget, and escalates
  after budget exhaustion.
- Completed: `/lab task revise <task_id> | <scope_csv> | <validation_csv> |
  [instructions]` repairs blocked graduate tasks with missing or wrong
  scope/validation and requeues them only when the execution boundary is
  complete.
- Completed: validation retry history appears in `/lab context` dynamic tail
  and `/lab blocker status`.
- Completed: `/lab dashboard` provides a read-only status panel summary for the
  future TUI/desktop dashboard.
- Completed: `/lab task worktree <review|merge|cleanup> <task_id> [force]`
  delegates isolated graduate worktree review/merge/cleanup to the existing
  WorktreeTool using the dispatch `agent_id`.
- Completed: Lab graduate dispatch now uses the same stable durable subagent
  task ID scheme as generic subagents, and runtime verification can recover the
  graduate isolated worktree by that task ID if `agent_id` lookup is
  unavailable.
- Completed: `/lab blocker escalate` moves a LabRun with blocker evidence into
  `professor_review` and creates the normal `ProfessorReview` gate.
- Completed: app lifecycle startup/shutdown checkpointing and `/lab lifecycle`
  status expose the owner process, recovered scheduler state, and shutdown
  pause.
- Completed: `/lab daemon status|enable|disable` exposes a restart-surviving
  daemon policy file for desktop/background ownership.
- Completed: `/lab daemon start` and Lab Mode startup consume daemon policy and
  restart strict/hybrid execution in the current process.
- Completed: `pa lab-daemon` is the non-interactive executable needed for
  background execution while the desktop app is closed.
- Completed: `/lab daemon launchd [label]` writes the macOS LaunchAgent plist
  for the daemon worker without installing it into the user's system.
- Completed: `/lab daemon health` exposes a read-only daemon supervision
  backend for desktop/launchd health checks.
- Completed: `/lab daemon service` installs/removes the daemon worker
  LaunchAgent plist and reports exact `launchctl` commands without starting
  the service itself.
- Completed: `/lab daemon service load|unload|restart` executes explicit
  launchctl service actions and has mock-command coverage.
- Completed: `/lab daemon service supervise` can repair missing loaded service
  state when daemon policy is enabled.
- Completed: desktop backend can invoke that supervise repair path and refresh
  Lab status after the action.
- Completed: desktop Workbench can invoke daemon supervision explicitly.
- Completed: desktop Workbench invokes daemon supervision periodically while
  open.
- Completed: `/lab messages classify` provides provider-backed Professor
  classification for sponsor side-channel messages before explicit apply.
- Completed: `/lab messages decision` provides a durable Professor steering
  decision artifact/report for sponsor side-channel messages without applying
  the decision.
- Completed: `/lab meeting open` turns a real professor-triggered meeting
  recommendation into a read-only meeting report without acquiring the mutating
  lease.
- Completed: role profile prompt versions are explicit and tied to persisted
  LabRun role defaults by coverage.
- Completed: `/lab recovery` shows paused LabRun recovery options without
  reacquiring a lease or restarting mutating work.
- Completed: `/lab report` lists and previews generated Markdown reports for
  the latest LabRun.
- Completed: `/lab open` changes the current LabRun inspect target without
  starting mutating work.
- Completed: `/lab runs` lists historical LabRuns without starting mutating
  work.
- Completed: `/lab review` is a working review summary surface rather than a
  planned placeholder.
- Completed: desktop app timer invokes the active supervisor backend without
  requiring the Workbench drawer to be open.
- Completed experimentally: live end-to-end validation of real graduate agent
  execution, runtime verification, worktree review, merge, cleanup, and daemon
  validation has passed under the explicit uncertified-provider override.
- Not yet complete: formal live provider certification that structured result
  emission or runtime-verified fallback is reliable across supported models.
- Not yet complete: live provider validation that sponsor-message
  classification is reliable across supported models.
- Not yet complete: live end-to-end provider validation for Lab meeting and
  graduate execution behavior across supported models.

Build:

- convert `GraduateTask` into existing `AgentTaskEnvelope`;
- preserve `allowed_scope`, `required_validation`, and parent artifact IDs;
- graduate result writes `GraduateResult` plus Markdown report;
- postdoc reads result, reviews diff/evidence, and either accepts or creates a
  correction task.

Hard requirements:

- subagent result is not proof by itself;
- postdoc or parent runtime must bind result to changed files, validation, and
  acceptance criteria;
- failed graduate tasks should become explicit blockers or repair tasks.

Validation:

```bash
cargo test -q subagent
cargo test -q agent_tool
cargo test -q lab
cargo check -q
```

### Phase 7: Lab meeting button and status panel

Goal: make the workflow visible and product-shaped.

Build:

- command palette entry or TUI action for "Lab Meeting";
- desktop/TUI status panel for current lab run;
- quiet "meeting recommended" indicator;
- professor-triggered meeting request display;
- open report action.

Current status:

- Completed: `/lab dashboard` provides the read-only status panel backend for
  a future desktop/TUI dashboard.
- Completed: `/lab meeting recommend` exposes quiet meeting recommendation
  state.
- Completed: `/lab meeting open [topic]` provides the explicit open-meeting
  action backend and refuses no-signal automatic meetings.
- Completed: TUI command palette/help can discover and execute `/lab` through
  the shared Lab command backend.
- Completed: desktop command palette can stage Lab dashboard, meeting,
  recovery, and daemon-health slash commands into the composer for explicit
  user confirmation.
- Completed: desktop Workbench renders a read-only Lab status panel with stage,
  owner, task/blocker counts, meeting recommendation, and latest report path.
- Completed: desktop Workbench renders blocker/retry history and provides
  explicit open-report plus staged closeout/intervention/continue/open-meeting
  actions.
- Completed: TUI `/panel lab` renders a file-backed LabRun status panel with
  task, blocker, retry, meeting, cost, report, and next-action state.
- Completed: desktop startup recovery prompt/buttons surface paused,
  shutdown-paused, or needs-user LabRuns without auto-resuming work; Lab Mode
  shell welcome also points recoverable runs to `/lab resume` and
  `/lab recovery`.
- Not yet complete: live provider validation remains outside Phase 7's UI
  slice.

Meeting recommendation and professor-trigger triggers:

- more than N commits since last lab meeting;
- failed production gate;
- long-running LabRun cycle completed;
- new professor plan accepted but no postdoc plan yet;
- postdoc reports repeated failed graduate task;
- postdoc reports a hard implementation blocker;
- validation fails repeatedly on the same slice;
- professor detects strategic drift from the original plan;
- user-configured interval.

Do not auto-run a meeting on every startup in MVP. In later versions, allow
professor-triggered meetings inside an active Lab Mode run, but record why the
meeting was triggered.

Meetings should not acquire the mutating lease unless the user chooses a
follow-up action that starts or resumes implementation. Read-only meeting
generation may inspect state and evidence without scheduling code work.

Validation:

```bash
cargo test -q tui
cargo test -q shell
cargo check -q
```

### Phase 8: Persistence and dashboard

Goal: make lab runs queryable and durable across sessions.

Current status:

- Completed: a file-backed `runs_index.json` is derived from authoritative
  `runs/*/state.json` records and is refreshable on LabRun state writes.
- Completed: `/lab runs` consumes the rebuilt index for queryable recent-run
  summaries instead of relying only on ad hoc command-local formatting.
- Completed: optional SQLite tables/import now mirror file-backed LabRun state
  into `.priority-agent/lab/lab_index.sqlite3` for future dashboard queries.
- Completed: `/lab status` surfaces both file-backed and SQLite index summaries
  while still reading authoritative run state from `state.json`.
- Completed: `/lab dashboard` consumes SQLite summaries for indexed counts and
  latest Professor/Postdoc artifact state while keeping file-backed runtime
  state authoritative.

Build:

- completed optional SQLite tables:
  - `lab_runs`;
  - `lab_artifacts`;
  - `lab_events`;
  - `lab_tasks`;
- completed import of existing file-backed lab runs, artifacts, events, and
  tasks;
- completed `/lab status` index visibility without replacing state.json as the
  authoritative source;
- completed project dashboard indexed latest Professor/Postdoc state.

Validation:

```bash
cargo test -q migrations
cargo test -q lab
cargo check -q
```

## Runtime Boundaries

The lab workflow must preserve existing priority-agent principles:

- LLM owns semantic judgment.
- Runtime owns deterministic orchestration, storage, permissions, evidence,
  validation, and closeout proof.
- Professor/postdoc/graduate outputs are claims until backed by evidence.
- Review reports should never bypass permission/checkpoint/validation gates.
- Graduate agents cannot self-certify final completion.
- LabRun orchestration cannot bypass active lease ownership.
- LabRun can summarize existing session/tool evidence, but should not invent a
  parallel proof system.

## Risks

### Risk: too much ceremony

Mitigation:

- keep Direct Mode unchanged and low-ceremony;
- make Lab Mode explicit through `pa lab`, professor intake, and a formal
  approval button;
- make single-turn "mini lab meeting" possible;
- do not require professor/postdoc/graduate for small edits.
- allow short/minimal artifacts for small cycles.

### Risk: workflow artifacts are skipped because the LLM thinks they are unnecessary

Mitigation:

- treat required artifacts as runtime stage gates;
- let artifacts be concise, but never absent;
- validate schema, owner, evidence refs, and next action before transition;
- store artifacts on disk so resume/review does not depend on model memory.

### Risk: internal loops run too long without user visibility

Mitigation:

- show `/lab status` with current stage, internal owner, cycle count, and
  blocker summary;
- enforce cycle and cost budgets;
- persist state before scheduling new internal work;
- heartbeat active runs and mark stale runs as `paused_shutdown`;
- require user approval for high-risk scope changes;
- let user pause, resume, or intervene at any point.

### Risk: two processes race on the same project

Mitigation:

- require a project-root LabRun lease for mutating orchestration;
- allow concurrent read-only inspection;
- treat stale leases as paused shutdown only after heartbeat expiry;
- never let a second process silently take over a fresh lease.

### Risk: Lab Mode mutates code without a clear user-approved boundary

Mitigation:

- keep `/lab meeting` read-only in MVP;
- require accepted plan before implementation cycles;
- preserve existing permission/checkpoint gates;
- label user/pre-existing changes separately from LabRun changes;
- require explicit user approval for high-risk or broad-scope mutations.

### Risk: professor becomes vague and ungrounded

Mitigation:

- require evidence refs;
- provide code/doc/status summaries;
- ask professor for strategic verdict, not implementation details.

### Risk: postdoc produces plans but no code

Mitigation:

- every postdoc plan must include implementation slices and validation;
- `/lab status` should show stale plans with no completed slices;
- professor can mark plan as "directionally ok but execution incomplete".

### Risk: graduate agents drift

Mitigation:

- narrow allowed scope;
- explicit validation;
- structured task envelope;
- postdoc review before acceptance.

### Risk: JSON files become a second database

Mitigation:

- start file-backed for iteration speed;
- keep SQLite indexes as a derived mirror once schemas stabilize;
- keep JSON artifacts as exportable source records.

## MVP Definition

MVP is complete when:

- User can run `pa lab` or `priority-agent lab` and enter professor intake.
- Professor intake can produce a `LabProposal` without creating a mutating
  LabRun.
- User can click "Start Project" or run `/lab approve <proposal_id>` to
  formally create a LabRun.
- User can run `/lab start <goal>` as a shortcut into proposal + approval flow.
- Lab Mode routes user-visible conversation through the professor role.
- Runtime records current stage, internal owner, cycle count, and whether user
  input is needed.
- Runtime persists LabRun state to local disk with status, heartbeat, and
  resume cursor.
- Runtime enforces one active mutating LabRun lease per project root.
- Runtime records LabRun cost policy and per-role context budgets.
- Runtime records role profile/prompt versions for professor, postdoc, and
  graduate agents.
- Runtime enforces required artifacts as stage gates before advancing LabRun
  workflow.
- Runtime writes cycle summaries instead of relying on full raw conversation
  history.
- Runtime defaults large evidence to refs-only and pulls raw evidence on demand.
- User can run `/lab pause` and `/lab resume`.
- App startup detects paused or stale active LabRuns and asks before resuming.
- User can run `/lab meeting`.
- Runtime creates a lab run directory with `state.json` and `events.jsonl`.
- Postdoc progress report is generated from real project evidence.
- Professor strategic review is generated from the postdoc report and evidence.
- LabRun closeout distinguishes verified, not verified, partial, blocked,
  failed, and cancelled outcomes.
- A Markdown meeting report appears under `docs/lab/`.
- `/lab status` shows latest lab run, stage, internal owner, cycle count, and
  professor verdict.
- User can send a side-channel message to professor during an active LabRun.
- Side-channel messages cannot directly command postdoc or graduate agents.
- No code mutation happens during lab meeting unless user explicitly asks.
- No postdoc/graduate implementation work starts before formal LabRun approval.

Current MVP caveat:

- `pa lab` now has a Professor intake welcome hint, deterministic proposal
  commands, and provider-backed `/lab propose llm <idea>` for structured
  Professor proposal drafting. The intake flow is still command-backed rather
  than a dedicated guided UI, and approval remains explicit through
  `/lab approve <proposal_id>`.

Post-MVP but required for full Lab Mode:

- professor-triggered meeting requests;
- repeated professor/postdoc/graduate cycles;
- graduate task delegation through subagent infrastructure;
- `/lab intervene` controls wired to active lab runs;
- professor final review after postdoc implementation report.

## First Implementation Slice

Recommended first code slice:

1. Add `src/entry/mod.rs`, `src/entry/direct.rs`, and `src/entry/lab.rs`
   as thin entry controllers while keeping existing default behavior unchanged.
2. Add `pa lab` / `priority-agent lab` dispatch to the Lab Mode entry.
3. Add `LabProposal` and `LabRun` models in `src/lab/model.rs` with tests.
4. Add `src/lab/store.rs` with file-backed proposal and lab run
   create/read/update.
5. Add `/lab propose` and `/lab approve` against file-backed proposal state.
6. Add active lease, heartbeat/resume cursor fields, and atomic file writes.
7. Add closeout status and retry budget fields.
8. Add `LabCostPolicy` defaults and `CycleSummary` model.
9. Add `docs/lab/README.md` and templates.
10. Add `/lab status` returning latest file-backed run summary.
11. Add `/lab pause` and `/lab resume` against file-backed state.
12. Add a no-mutation `/lab meeting` prototype that writes a structured meeting
   run and Markdown report.

Why this first:

- low risk;
- validates the structured protocol;
- creates the foundation for professor/postdoc/graduate without touching core
  conversation loop yet;
- easy to test.

Suggested validation:

```bash
cargo fmt --check
cargo test -q lab
cargo check -q
```

## Open Questions

1. Should lab artifacts live only in `.priority-agent/lab/`, or should accepted
   professor/postdoc artifacts always be mirrored into `docs/lab/`?
2. Should `/lab meeting` be read-only forever, or should it offer an optional
   "create next LabRun cycle" action?
3. Should professor use the strongest configured model by default, or should
   model choice remain explicit?
4. Should graduate tasks always use isolated worktrees for mutating code, or
   only when risk is medium/high?
5. Should professor review be mandatory after every lab implementation run, or
   only for architecture/product slices?
6. What threshold should allow professor-triggered meetings without explicit
   user click?
7. Should Lab Mode continue cycles in the background, or only while the user has
   an active session open?
8. Should professor/postdoc meeting discussion be exposed as a transcript, or
   summarized into one decision report by default?
9. Should LabRun state start as files only, or immediately use `sessions.db`
   migrations for queryable persistence?
10. What heartbeat TTL is safe enough for crash detection without false
   positives during long validation commands?
11. Should LabRun budget be hard-stop, soft warning, or role-dependent?
12. Should professor/postdoc use the same stable project charter prefix, or
    should each role have a separate charter projection?
13. Should the mutating LabRun lease live only in `.priority-agent/lab/`, or
    should it also be mirrored into `sessions.db` once SQLite indexes exist?
14. Should implementation cycles require explicit user approval after
    professor/postdoc planning, or can `/lab start` grant bounded approval for
    low-risk scoped changes?
15. Should `pa lab` always start with professor intake, or should it reopen the
    latest approved LabRun when one is active?

## Recommended Answers For MVP

1. Mirror accepted human-facing reports into `docs/lab/`; keep drafts and raw
   state in `.priority-agent/lab/`.
2. Keep `/lab meeting` read-only in MVP; add "create LabRun cycle from recommendation"
   later.
3. Let professor default to high-reasoning model policy, but make it visible in
   `/lab status`.
4. Use isolated worktrees for graduate tasks that mutate files outside a single
   narrow module; allow direct scoped edits for very small tasks.
5. Require professor review for architecture/product lab runs; make it optional
   for simple implementation-only runs.
6. Allow professor-triggered meetings after repeated validation failure,
   postdoc blocker report, or strategic drift signal, but surface them as
   pending meeting requests before making disruptive changes.
7. Keep MVP foreground/session-bound; add background continuation only after
   pause/resume, budget, and notification behavior are reliable.
8. Store the full structured transcript in `.priority-agent/lab/`, but show a
   concise decision report by default.
9. Start with file-backed JSON/JSONL plus atomic writes; add SQLite indexes
   once the schema stabilizes.
10. Start with a conservative 90 second heartbeat TTL and refresh before/after
   long-running tasks; do not mark active validation as stale if a child process
   is still known to be running.
11. Use soft warnings for professor/postdoc and stricter per-task limits for
    graduate work; require user approval when total LabRun budget is near limit.
12. Use the same versioned project charter fingerprint across roles, but render
    role-specific views in L3 working packets.
13. Start lease state in `.priority-agent/lab/runs/<id>/lease.json` and add a
    project-root active-run pointer; mirror into SQLite only after persistence
    schemas stabilize.
14. Let `/lab start` approve planning and read-only reports only. Require an
    explicit accept/resume/implement action before the first mutating
    implementation cycle. Later, support bounded approval for low-risk scoped
    changes once leases, checkpoints, and status UI are reliable.
15. If there is no active LabRun, `pa lab` starts professor intake. If an
    active or paused LabRun exists, show a chooser: continue, inspect, keep
    paused, or start a new proposal.
