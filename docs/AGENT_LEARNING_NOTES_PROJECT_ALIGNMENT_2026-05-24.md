# Agent Learning Notes Project Alignment Review

Date: 2026-05-24

Source note: `/Users/georgexu/Downloads/agent_learning_notes.md`

Scope: current `priority-agent` checkout in `/Users/georgexu/Desktop/rust-agent`.
This document started as an analysis review and now also records the
implementation batches landed from that review.

## 0. Implementation Progress

2026-05-24 first implementation batch:

- Added typed five-zone context assembly in `src/engine/context_assembly.rs`.
- Connected prompt context reporting to the five-zone plan.
- Changed context usage stable-prefix fingerprinting to use the stable prompt
  zone, not the full dynamic prompt.
- Added minimal `AgentTaskState` inside `src/engine/task_context.rs`.
- Synchronized `AgentTaskState` from `TaskContextBundle` for active files,
  risks, and acceptance checks.
- Injected `<task-state>` into prepared model requests immediately after the
  stable system prompt, before ledger/memory/retrieval-style dynamic material.
- Added phase-aware tool exposure for programming workflows:
  `Understand`, `Edit`, `Validate`, `Repair`, and `Closeout` now expose
  different tool subsets.
- Added task-stage transitions after tool rounds: successful inspection moves
  toward `Edit`, writes move toward `Validate`, successful validation moves
  toward `Closeout`, and failures move toward `Repair`.
- Added runtime-owned action-level scoring in `src/engine/action_decision.rs`.
- Attached `action_decision` metadata to tool results and trace events for
  high-risk, repair, mutation, broad-shell, phase-misaligned, and no-progress
  actions.
- Added typed verification proof semantics in
  `src/engine/verification_proof.rs`.
- Made `EvidenceLedger` derive an evidence-backed proof status from required
  validation commands, current validation facts, and task verification state.
- Connected closeout evaluation to the proof object so final closeout can
  distinguish `verified`, `failed`, `not_run`, `not_applicable`, `blocked`,
  `user_deferred`, and `unavailable`.
- Added proof status and proof summary to final closeout trace events so evals
  can separate real validation success from closeout/reporting mistakes.
- Added state-backed stop-check semantics in `src/engine/stop_checker.rs`.
- Extended `AgentTaskState` with bounded `StopCheckRecord` history so
  no-progress, focused-repair stalls, duplicate read-only loops, repeated tool
  failures, and validation-ready states are visible in the task recordbook.
- Connected stop-check evaluation to turn iteration after tool-round
  observation and after focused-repair state updates.
- Added `stop.check` trace events and behavior tests for no-progress,
  duplicate-read-only, repeated failure, and turn-iteration trace recording.
- Expanded `src/engine/context_ledger.rs` beyond read history into structured
  edit, diff, validation, and user-confirmation evidence.
- Connected the new evidence records to the shared tool outcome persistence
  path, so successful file mutations, diff inspections, safe validation
  commands, and permission confirmations are recorded without each caller
  owning a separate ledger path.
- Extended request preparation so the next model call sees recent recorded
  reads, edits, diffs, validations, and confirmations as compact session
  evidence instead of relying only on visible transcript text.
- Exposed the ledger extraction as a shared runtime fact source and connected it
  to `AgentTaskState`, so tool results now update bounded observations,
  completed steps, active files, verification status, and user-deferred
  permission outcomes.
- Rendered recent completed steps and recent observations inside the
  `<task-state>` context zone, keeping task state separate from transcript
  prose and long-term memory.
- Made goal-drift checks consult `AgentTaskState.allowed_scope` and
  `AgentTaskState.forbidden_actions`, even when no explicit `SessionGoal` is
  active.
- Passed the current task state through tool-round execution into the shared
  tool permission path, so scope drift and forbidden mutations can become
  approval evidence before a tool runs.
- Added state-backed drift tests for forbidden local mutation, mutation outside
  the task working scope, and allowed in-scope file edits.
- Added direct-task behavior regressions that pin simple factual answers,
  rewriting, concept explanation, and error explanation to direct routing,
  empty tool exposure, ordinary risk signals, no entry workflow contract, and
  direct task-state semantics.
- Added bounded edit-state snapshots to `AgentTaskState`, recording recent
  edit, failed-edit, failed-validation, and repair-entry surfaces so repair
  turns can reference the current change surface from task state instead of
  relying only on transcript text.
- Rendered recent edit snapshots inside `<task-state>` with stage,
  verification status, and active files.
- Added a runtime control-loop diagnostic derived from trace events, grouping
  each turn into context, decision, permission, tool execution, state update,
  verification, and closeout phases.
- Added the control-loop diagnostic to `/trace` summaries without changing
  user-facing final answers.
- Refreshed `docs/PROJECT_STATUS.md` so the canonical status doc records the
  active 2026-05-24 agent-runtime alignment line separately from older shipped
  deterministic/live baselines.
- Added a cross-module runtime-spine behavior regression that ties together
  context-zone order, task-stage transitions, action scoring, no-progress stop
  checks, and verification-proof semantics in one control-loop contract.
- Added runtime-spine live-eval/report assertions in
  `scripts/live_eval_report_parser.py`, including normalized sample assertions,
  phase coverage, required trace-event checks, verification-proof checks, and
  `runtime_spine` failure classification.
- Extended `scripts/run_live_eval.sh` reports to print `runtime_spine`,
  `runtime_spine_detail`, phase coverage, missing assertions, proof status, and
  proof summary; missing runtime-spine assertions now become quality failures
  with `failure_owner=agent_flow`.
- Extended live-eval run summaries and aggregate summaries with
  runtime-spine assertion counts, pass/fail counts, full-coverage counts, and
  trace-present counts.
- Added runtime-spine assertions to representative live tasks:
  `core-simple-stale-edit`, `core-multi-file-edit`,
  `core-inspection-grounding`, and `code-change-verification-repair-loop`.
- Updated `scripts/live-eval-summary-smoke.sh` so summary/aggregate report
  tests cover both passing and failing runtime-spine assertions.
- Added a read-only `runtime_diagnostic` stream event that packages current
  task state, verification proof, and control-loop phase coverage for clients.
- Mapped the diagnostic through `DesktopRunEvent` so the Tauri app receives it
  as a stable desktop event instead of reaching into conversation-loop internals.
- Rendered runtime diagnostics in the desktop run summary and trace drawer,
  including stage, verification, proof status, control-loop coverage, active
  files, proof summary, and per-phase latest labels.
- Updated web-preview and native-smoke fixtures so desktop smoke tests exercise
  the runtime-spine visibility surface.
- Added runtime-owned `TaskModeScore` diagnostics in
  `src/engine/task_mode_score.rs`, scoring complexity, risk, uncertainty, tool
  need, and user impact before assigning direct/light/full/high-risk mode.
- Added a compact `LightweightPlan` in `src/engine/lightweight_planner.rs` for
  tool-assisted light tasks, giving them bounded scope/observe/respond steps
  without invoking the heavy workflow contract.
- Attached mode score and lightweight plan state to `AgentTaskState`,
  `<task-state>`, and desktop runtime diagnostics so light-vs-full decisions
  are inspectable.

Validated with:

```bash
cargo fmt --check
cargo test -q context_assembly -- --test-threads=1
cargo test -q prompt_context -- --test-threads=1
cargo test -q context_usage -- --test-threads=1
cargo test -q task_context -- --test-threads=1
cargo test -q request_preparation_controller -- --test-threads=1
cargo test -q turn_model_step_controller -- --test-threads=1
cargo test -q tool_exposure_plan -- --test-threads=1
cargo test -q turn_iteration_setup_controller -- --test-threads=1
cargo test -q action_decision -- --test-threads=1
cargo test -q tool_execution_controller -- --test-threads=1
cargo test -q turn_runtime_context -- --test-threads=1
cargo test -q verification_proof -- --test-threads=1
cargo test -q evidence_ledger -- --test-threads=1
cargo test -q closeout_controller -- --test-threads=1
cargo test -q trace_summary_includes_closeout_tool_record_count -- --test-threads=1
cargo test -q stop_check -- --test-threads=1
cargo test -q turn_iteration_controller -- --test-threads=1
cargo test -q focused_repair_state_controller -- --test-threads=1
cargo test -q turn_focused_repair_flow_controller -- --test-threads=1
cargo test -q tool_failure_stop_controller -- --test-threads=1
cargo test -q trace -- --test-threads=1
cargo test -q context_ledger -- --test-threads=1
cargo test -q request_preparation_controller -- --test-threads=1
cargo test -q tool_metadata -- --test-threads=1
cargo test -q tool_batch_result_processor -- --test-threads=1
cargo test -q turn_tool_round_step_controller -- --test-threads=1
cargo test -q turn_iteration_controller -- --test-threads=1
cargo test -q stop_check -- --test-threads=1
cargo test -q goal_drift -- --test-threads=1
cargo test -q turn_recording -- --test-threads=1
cargo test -q tool_execution_controller -- --test-threads=1
cargo test -q tool_round_controller -- --test-threads=1
cargo test -q permission_controller -- --test-threads=1
cargo test -q direct_task_behavior -- --test-threads=1
cargo test -q intent_router -- --test-threads=1
cargo test -q route_scoped_tools -- --test-threads=1
cargo test -q risk_signal_controller -- --test-threads=1
cargo test -q workflow_runtime -- --test-threads=1
cargo test -q task_context -- --test-threads=1
cargo test -q tool_batch_result_processor -- --test-threads=1
cargo test -q turn_iteration_controller -- --test-threads=1
cargo test -q request_preparation_controller -- --test-threads=1
cargo test -q trace -- --test-threads=1
cargo test -q runtime_spine_behavior -- --test-threads=1
cargo test -q action_decision -- --test-threads=1
cargo test -q verification_proof -- --test-threads=1
cargo test -q evidence_ledger -- --test-threads=1
cargo check -q
python3 -m py_compile scripts/live_eval_report_parser.py
bash -n scripts/run_live_eval.sh
bash -n scripts/live-eval-aggregate-summary.sh
scripts/run_live_eval.sh --list
bash scripts/live-eval-summary-smoke.sh
cargo test -q turn_completion_controller -- --test-threads=1
cargo test -q desktop_runtime -- --test-threads=1
corepack pnpm --dir apps/desktop exec playwright test tests/run-event-state.spec.ts
corepack pnpm --dir apps/desktop exec tsc --noEmit
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
cargo test -q task_mode_score -- --test-threads=1
cargo test -q lightweight_planner -- --test-threads=1
cargo test -q direct_task_behavior -- --test-threads=1
cargo test -q runtime_spine_behavior -- --test-threads=1
```

Final cleanup validation before committing the combined uncommitted batch:

```bash
cargo test -q -- --test-threads=1
cargo clippy --all-features -- -D warnings
cargo check -q
cargo check --features experimental-api-server -q
cargo fmt --check
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
corepack pnpm --dir apps/desktop exec tsc --noEmit
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
git diff --check
```

Next implementation target: use real desktop dogfood runs to tune the
runtime-diagnostic surface, especially where proof/state/trace disagree or where
the UI needs a clearer normal-run summary without leaking debug noise into the
main transcript.

### 0.1 Current Completion Readout

The point-by-point table below was written before implementation started, so
some "mostly missing" cells are now historical baseline rather than current
status. Current state:

- Core P0 architecture from this review is implemented: five-zone context,
  task state, phase-aware tools, action scoring, evidence-backed closeout,
  stop checks, control-loop trace, and runtime-spine eval assertions.
- The former light/full ambiguity is now partially closed by `TaskModeScore`
  and `LightweightPlan`: tool-assisted direct tasks become `Light`, pure
  factual direct tasks remain `Direct`, and real code-change/bug-fix work stays
  `Full` or `HighRisk`.
- Desktop observability is implemented for the runtime spine through
  `runtime_diagnostic`.
- Remaining work is no longer broad skeleton building. It is tuning and proof:
  real desktop dogfood runs, live-eval reruns with the new assertions, and
  follow-up refinements where state/proof/trace disagree.

## 1. Short Conclusion

The note is useful for the project. It is not just conceptual learning material;
it describes the next runtime maturity layer that `priority-agent` still needs:
an agent should not be a bigger prompt, and should not be an unrestricted LLM
loop. The system should own routing, context shape, permissions, action
contracts, state updates, verification, and traceability, while the LLM owns
semantic judgment, tradeoff analysis, and natural-language reasoning.

The current project is already aligned with this direction in several important
areas:

- Task routing exists in `src/engine/intent_router.rs`.
- Route-level resource policy exists in `src/engine/resource_policy.rs`.
- Route-scoped tools exist in `src/engine/conversation_loop/tool_orchestrator.rs`.
- Permission and destructive-scope checks exist in
  `src/engine/conversation_loop/permission_controller.rs`,
  `src/engine/conversation_loop/tool_execution_controller.rs`, and
  `src/engine/destructive_scope.rs`.
- Risk-sensitive workflow control exists in
  `src/engine/conversation_loop/risk_signal_controller.rs` and
  `src/engine/code_change_workflow.rs`.
- Prompt diet and AGENTS runtime section extraction exist in
  `src/instructions/mod.rs`.
- Context management work has started through `src/engine/model_context.rs`,
  `src/engine/context_usage.rs`, `src/engine/context_ledger.rs`, and the
  request-preparation controllers.

The largest original gaps have mostly moved from skeleton work to tuning work:

1. `AgentTaskState` now records the current goal, mode, mode score, light plan,
   stage, scope, observations, edit snapshots, stop checks, risks, verification,
   and done condition.
2. `ActionDecision` now records action-level value/risk/uncertainty/cost/
   reversibility signals for meaningful tool boundaries.
3. `ContextAssemblyPlan` now owns the five zones from the note and gives the
   stable prefix its own fingerprint.
4. Tool exposure is now phase-aware for programming work, driven by task stage.
5. Context ledger evidence now updates task state and closeout proof.

The remaining gaps are narrower: real-run calibration, live-eval reruns against
the new runtime-spine assertions, and continued refinement of light/full route
scoring as more desktop dogfood evidence arrives.

## 2. Current Baseline Map

| Area | Current support | Main evidence | What this means |
| --- | --- | --- | --- |
| Product direction | Strong | `docs/PERSONAL_AGENT_PRODUCT_PRINCIPLES_2026-05-18.md`; `docs/PROJECT_STATUS.md` | The project already chose narrow, deep, personal, verifiable instead of generic clone. |
| Runtime ownership | Medium-strong | `docs/LLM_RUNTIME_SIMPLIFICATION_PLAN_2026-05-08.md`; `src/instructions/mod.rs` | The repo is already reducing prompt bloat and moving hard constraints into runtime/tool contracts. |
| Task router | Medium | `src/engine/intent_router.rs` | Good rule-based route foundation, but no structured LLM-plus-formula route scoring yet. |
| Resource policy | Medium | `src/engine/resource_policy.rs` | Per-route budgets exist, but not adaptive per-action value/risk/cost decisions. |
| Tool exposure | Medium | `src/engine/conversation_loop/tool_orchestrator.rs` | Tools are scoped by route, but broad routes still expose many capabilities too early. |
| Permissions | Strong | `src/engine/conversation_loop/permission_controller.rs`; `src/engine/destructive_scope.rs` | Good system-owned boundary for confirmation, drift, and destructive scope. |
| Risk control | Strong | `src/engine/conversation_loop/risk_signal_controller.rs`; `src/engine/code_change_workflow.rs` | High-risk and repair paths are already runtime-controlled. |
| Workflow contract | Medium | `src/engine/workflow_contract.rs` | Weight factors exist for workflow plan steps, but not for each action/tool decision. |
| Task state | Weak-medium | `src/engine/task_context.rs`; `src/engine/conversation_loop/turn_runtime_state.rs` | Useful fields exist, but no unified task recordbook. |
| Context builder | Weak-medium | `src/engine/prompt_context.rs`; `src/engine/conversation_loop/request_preparation_controller.rs`; `src/engine/context_ledger.rs` | Many pieces exist, but the builder lacks one typed assembly plan and stable zone contract. |
| Memory/ledger | Medium, in progress | `src/memory/manager.rs`; `src/engine/context_ledger.rs` | Memory retrieval and session ledger exist, but should become more stateful and evidence-aware. |
| Closeout/checking | Medium-strong | `src/engine/code_change_workflow.rs` | Verification closeout is real, but proof semantics and no-op/not-run reporting should stay explicit. |

### 2.1 Coverage Snapshot

This section answers the direct question: "Do we already have the other things
from the source note?"

| Coverage | Source-note content | Project status | Verdict |
| --- | --- | --- | --- |
| Mostly present | Product positioning: agent is not only prompt, not only LLM, and should use runtime constraints. | Runtime simplification, route-scoped tools, permission checks, and project principles already align. | Keep the direction. Do not turn the note into a larger prompt. |
| Mostly present | Direct-answer vs high-risk distinction. | Router and risk controller already separate simple answers from risky coding work. | Add tests to keep simple tasks lightweight. |
| Mostly present | Tool schemas, permission checks, destructive-operation boundaries. | Tool execution gate, permission controller, and destructive-scope contract are real system-owned controls. | Continue tightening edge cases. |
| Partially present | Five agent elements: goal, decomposition, decision, tools, feedback. | Goal signals, workflow planning, routing, tools, and feedback exist, but not as one integrated state loop. | Needs `AgentTaskState` plus observation updates. |
| Partially present | Four brain parts: understanding, planning, execution, checking. | Understanding/router, execution/tool gate, and checking/closeout exist; planning is split between prompt conventions and workflow contract. | Add a lightweight planner for medium tasks. |
| Partially present | Task levels: direct, light, full, high-risk. | Direct and high-risk are clear; light/full boundaries are less explicit. | Introduce explicit task mode labels and diagnostics. |
| Partially present | LLM judges; system controls. | This is the architecture direction, but some decisions are still implicit in model tool calls. | Add structured action decisions at selected boundaries. |
| Partially present | Mature-agent modules: router, context builder, planner, tool caller, permission, state/rollback, checker, memory. | Most modules have current equivalents. | State/rollback and context builder are the weakest parts. |
| Partially present | Context/state/memory/prompt-cache distinction. | Separate code paths exist, especially in context-management WIP. | Needs a typed five-zone context assembly plan. |
| Mostly missing | Unified task state as a task recordbook. | Current state is split across task context, turn runtime state, goal drift, ledger, and closeout. | Add small serializable `AgentTaskState`. |
| Mostly missing | Action-level weighted decision. | Weighting exists for workflow plan steps, not for every meaningful action. | Add `ActionDecision` for high-risk, repair, mutation, and no-progress paths first. |
| Mostly missing | Per-action output: reason summary, action, risk, expected observation. | Raw tool calls and traces exist, but not this contract. | Add internally before exposing anything user-facing. |
| Mostly missing | Phase-aware tools. | Tools are route-scoped, not stage-scoped. | Gate tools by `Understand`, `Plan`, `Edit`, `Validate`, `Repair`, `Closeout`. |
| Mostly missing | Stop checker and no-progress detector. | Some tool limits and repair counters exist. | Add state-backed loop/progress detection. |
| Mostly missing | Stable-prefix and prompt-cache regression tests. | Prompt diet and model-context work help, but tests should pin ordering and cache-sensitive zones. | Add context-zone ordering and stable-prefix invariance tests. |
| Should not be literalized | Learning explanations and repeated reminders in the note. | The note is written for human learning. | Convert into code contracts, tests, and concise docs, not runtime prompt prose. |

## 3. Point-By-Point Comparison

| Note section | Meaning for this project | Current state | Gap or optimization |
| --- | --- | --- | --- |
| 0. 核心结论 | Agent value is in organizing, constraining, recording, and checking LLM work. | The current direction matches this: prompt diet, route-scoped tools, permissions, risk controller, workflow closeout. | Continue moving durable rules into runtime/tool contracts. Avoid re-growing the giant prompt. |
| 1. Agent 是什么 | Agent should advance work step by step toward a goal, not only answer. | The conversation loop and tool execution path do this for coding tasks. | The loop still lacks a single state object that makes every next-step decision auditable. |
| 2.1 目标 | Every task needs a current goal. | `TaskContextBundle` has goal-like signals; session goals exist in status docs. | Add explicit `main_goal`, `allowed_scope`, `forbidden_actions`, and `done_condition` to task state. |
| 2.2 拆分 | The system should decompose when useful, not always. | Workflow contracts and code-change workflow support planned steps for higher-risk tasks. | Add light decomposition for medium tasks without invoking the full heavy workflow contract. |
| 2.3 决策 | Agent should choose the next action based on value, risk, uncertainty, and cost. | Router/resource policy decide at route level; workflow contract has plan-step factors. | Add action-level decision scoring before meaningful tool calls or phase transitions. |
| 2.4 工具 | Tools should be available through controlled contracts. | Tool registry, route-scoped allowlists, parameter validation, destructive-scope checks exist. | Split broad coding routes into phase-aware tool sets: inspect, plan, edit, validate, closeout. |
| 2.5 反馈 | Observations should update state and affect the next step. | Tool outputs flow back to the model; ledger records some reads; closeout uses evidence. | Add structured observation summaries into task state so the next decision is not only prompt-text dependent. |
| 3.1 理解器 | Understand user intent and task type. | `intent_router.rs` covers direct answer, planning, code change, bug fix, local inspection, terminal tasks, memory tasks. | Add a structured confidence/reason scoring path and tests for ambiguous Chinese mixed-intent requests. |
| 3.2 规划器 | Produce enough plan for the task level. | Workflow contract and task context provide planning for selected paths. | Create a lightweight planner object for `LightAgent` tasks, separate from high-risk workflow contracts. |
| 3.3 执行器 | Execute actions through tools, not free-form side effects. | `tool_execution_controller.rs` owns gates, max calls, exposed tools, permission checks, and execution. | Make execution consume an `ActionDecision` object, not just raw model tool calls. |
| 3.4 检查器 | Verify whether the work is done. | Closeout and validation logic exist, especially in coding workflow. | Strengthen proof taxonomy: verified, failed, not run, not applicable, blocked, user-deferred. |
| 4. 简单问题不要完整四步 | Do not over-control simple tasks. | Runtime simplification plan and router direct-answer paths match this. | Keep all new state/action machinery gated by task level so simple answers stay lightweight. |
| 5.1 直接回答模式 | Answer directly when no tool/process is needed. | `IntentClass::DirectAnswer` exists and tool exposure can be empty/minimal. | Add regression tests that simple factual/style questions do not trigger workflow contracts or broad tools. |
| 5.2 轻量 Agent 模式 | Use a small loop for small tool-assisted tasks. | Some local inspection and terminal routes approximate this. | Make this an explicit mode with small state, small tool budget, and no heavy closeout unless mutation happens. |
| 5.3 完整 Agent 模式 | Multi-step work needs planning, tools, feedback, checking. | Code-change and bug-fix paths mostly support this. | The transition between inspect, edit, and verify should be state-driven instead of prompt-guided only. |
| 5.4 高风险 Agent 模式 | Risky changes require stricter confirmation and verification. | Risk signal controller, destructive scope, permissions, and workflow contract are strong here. | Keep improving high-risk tests around config/schema/provider/memory/tool/permission surfaces. |
| 6. 启动完整流程标准 | Use complexity, risk, uncertainty, tool need, and user impact to choose mode. | Router has many heuristics; resource policy maps route to budgets. | Introduce a visible `TaskModeScore` or equivalent diagnostic so routing is explainable and tunable. |
| 7. LLM 判断 vs Agent 控制 | LLM can judge semantics, but system controls boundaries. | Current architecture broadly follows this. | Make the boundary concrete in code: LLM proposes `ActionDecision`; runtime validates/gates/records it. |
| 8. 不能完全让 LLM 决定 | Pure LLM overthinks, drifts, forgets goals, overuses tools. | Tool-call limits, route allowlists, permission checks, and risk gates reduce this. | Add no-progress and repeated-tool detection tied to state, not only per-turn counters. |
| 9. LLM 和系统职责 | LLM: understand/trade off/explain. System: route/tool/permission/state/check. | Many system-owned pieces already exist. | State update and context-zone construction should move further into system-owned code. |
| 10. 成熟 Agent 的双循环 | Need intelligent loop plus control loop. | Conversation loop is the control spine; LLM supplies reasoning/tool calls. | Make control-loop steps explicit in code and trace: build_context, decide, parse, allow, run, update, check. |
| 11.1 任务路由器 | Router picks task level and capability set. | Implemented, mostly rule-based. | Add route scoring and clearer mode labels: direct, light, full, high-risk. |
| 11.2 上下文构建器 | Context must be assembled deliberately. | Prompt assembler, memory snapshot, retrieval injection, context budget, context ledger all exist. | Build one `ContextAssemblyPlan` with five zones and token/cache accounting. |
| 11.3 计划器 | Plan only as much as the task needs. | Workflow contract is available for heavier paths. | Add a smaller planner for medium work; avoid forcing high-risk workflow language into normal coding. |
| 11.4 工具调用器 | Tool calls need schema, validation, and observation. | Tool registry, controllers, and parameter validation are solid. | Add expected observation and value/risk metadata to planned tool calls. |
| 11.5 权限系统 | Permission must be system-owned. | Strong support through permission controller and destructive scope. | Tie permission decisions to task state scope and action reversibility score. |
| 11.6 状态与回滚 | State and rollback prevent drift and unsafe changes. | Turn state, task context, destructive scope, and git/diff tools exist. | Add state snapshots around edit phases and explicit rollback/repair strategy for failed validations. |
| 11.7 检查器 | Checker validates completion. | Closeout builder and validation policies exist. | Make checker consume task state and evidence ledger, not just recent messages/tool results. |
| 11.8 记忆经验 | Memory should provide relevant durable context, fenced from current state. | Memory manager and retrieval context exist; context ledger WIP helps local session memory. | Separate long-term memory, session ledger, and current task state in context zones and code types. |
| 12. Agent 系统有没有智能 | System is a workbench, not the intelligence. | Product principles and simplification plan align. | Avoid over-fitting logic to imitate intelligence; keep system logic as constraints, records, and checks. |
| 13. Agent 分配任务 | LLM judges structured task; Agent executes judgment. | Partly implemented by route plus workflow. | Standardize structured decisions so runtime can execute them without guessing model intent. |
| 14. Agent 不只是提示词 | Prompt alone is fragile. | Repo has already moved many constraints into runtime. | Continue prompt diet; do not add this note as a giant injected instruction. |
| 15. 权重分配 | Best path is LLM plus rules plus formula. | `workflow_contract.rs` has `WeightFactors` and importance calculation for plan steps. | Reuse the idea at action level: value, risk, uncertainty reduction, cost, reversibility. |
| 16. 结构化输出 | JSON is a carrier, not the whole design. | Tool calls and workflow prompt use structured formats. | Use small typed structures only at decision boundaries, not for all thinking. |
| 17. 不能只用提示词 | Prompt-only cannot reliably enforce behavior. | Current direction already agrees. | Any new behavior from the note should land in code/tests first, prompt second. |
| 18. 不能全 JSON | Full JSON can harm reasoning and UX. | Current system still lets the model answer naturally. | Keep free-form reasoning/answering, but structure route/action/tool/permission/state/check boundaries. |
| 19. 自由思考 + 结构化决策 | Natural thought plus structured handoff is the target. | Tool-calling interface is close to this. | Add a compact `reason_summary` plus typed action fields for high-value decisions. |
| 20. 适合结构化的地方 | Route, tool, permission, state, final check should be structured. | Route/tool/permission/final check mostly exist; state is weakest. | Prioritize structured state and structured context assembly before adding new product features. |
| 21. 不适合强结构化的地方 | Analysis and explanation should remain natural language. | Final answers are still natural. | Keep final-answer rules concise; avoid leaking internal state schema into user-facing output. |
| 22. 第一版思路 | Each action should report goal, stage, value, risk, confirmation, verification. | Some of these are present in workflow traces and closeout. | Implement as internal action trace first, surfaced only when debugging or high-risk. |
| 23. 核心定位 | Demand decomposition, weights, active decision, self-check/repair. | Product direction and risk workflow match. | This is the right next architecture theme after context-management WIP stabilizes. |
| 24. 记忆点 | Avoid max-power always-on, prompt-only, rule-only, all-structured designs. | Runtime simplification plan is consistent. | Use the note as a guardrail against both over-control and under-control. |
| 25. 下一步学习控制循环 | Control loop is the next useful learning object. | The repo already has a real loop but it is split across many controllers. | Add a high-level control-loop map in code/docs after the state/context work lands. |
| 26.1 最简单控制循环 | build_context -> decide -> parse -> allow -> run -> update_state -> check_done. | These responsibilities exist across controllers. | Make the sequence traceable through one per-turn diagnostic object. |
| 26.2 不让 LLM 瞎跑 | The loop should limit choices and update from observations. | Tool gates and route allowlists help. | Add repeated-action/no-progress detection and phase transitions. |
| 26.3 可用骨架 | Need concrete modules for state, planner, executor, permission, observer, stop checker. | Existing modules cover executor/permission/checker partially. | Missing cohesive observer and stop-checker tied to task state. |
| 26.4 谁负责什么 | System owns context/parse/allow/run/update; LLM owns semantic decision. | Mostly aligned. | Move memory/retrieval/context insertion into a single system-owned builder. |
| 26.5 核心状态 | State is task recordbook, not context or memory. | Current `TaskContextBundle` and `TurnRuntimeState` are partial. | Add `AgentTaskState` and keep it small, serializable, and testable. |
| 26.6 五个恐惧 | Loops, tool overuse, privilege drift, goal drift, context explosion. | Tool limits, permission, goal drift detector, context budget exist. | Connect all five to the same state/control-loop telemetry. |
| 26.7 项目第一版控制循环 | State/planner/tool executor/permission checker/observer/stop checker. | Several parts exist under different names. | Introduce only the missing contract layer, not a large rewrite. |
| 26.8 每步输出 | reason_summary, action, risk, expected_observation. | Current tool calls do not require these fields. | Start requiring this only for high-risk, focused repair, and repeated-failure paths. |
| 26.9 权重判断 | value/risk/uncertainty/cost/reversibility drive choices. | Plan-step weighting exists in workflow contract. | Add action-level scoring and use it to decide ask-user vs inspect vs edit vs validate. |
| 26.10 控制循环总图 | Architecture should be understandable as a loop. | Docs map responsibilities, but code remains spread out. | Add an updated runtime control-loop doc after implementing state/context builder. |
| 27. 上下文是什么 | Context is what the model sees now. | Context budget, model context profile, prompt assembly, memory injection exist. | Make exact context shape inspectable per request. |
| 28. context/state/memory/cache 区别 | These four must not be mixed. | The repo has separate pieces, but some insertion still blurs roles. | Enforce type separation: current context material, task state, long-term memory, provider cache metadata. |
| 29. 缓存命中 | Stable prefix improves provider cache behavior. | AGENTS runtime diet and model-context WIP help. | Add regression tests that stable prefix remains stable while dynamic tail changes. |
| 30. 稳定前缀 + 变化尾部 | Stable rules first, changing evidence later. | Prompt assembler has layered prompt reporting, but not a full zone contract. | Design the request builder around stable prefix and dynamic tail zones. |
| 31. 五区结构 | Stable prefix, task state, relevant material, recent observation, current decision request. | Current implementation is close in pieces but not in one builder. | Highest-leverage next change: typed five-zone `ContextAssemblyPlan`. |

## 4. Main Gaps To Optimize

### P0. Unified Agent Task State

Current evidence:

- `src/engine/task_context.rs` has `TaskContextBundle` with useful task facts,
  constraints, risks, and acceptance checks.
- `src/engine/conversation_loop/turn_runtime_state.rs` tracks per-turn controller
  state such as repair attempts and tool counts.
- `src/engine/goal_drift.rs` is still described as advisory for V1, with only
  some high-risk cases forcing approval through other controllers.

Gap:

These pieces are useful, but they are not one task recordbook. The note's
`state` idea is not "more prompt"; it is a compact runtime-owned object that
survives the loop and tells the next step what has already happened.

Suggested shape:

```rust
pub struct AgentTaskState {
    pub main_goal: String,
    pub mode: TaskMode,
    pub stage: TaskStage,
    pub allowed_scope: Vec<ScopeItem>,
    pub forbidden_actions: Vec<ForbiddenAction>,
    pub completed_steps: Vec<CompletedStep>,
    pub observations: Vec<ObservationSummary>,
    pub active_files: Vec<PathBuf>,
    pub risks: Vec<RiskSignal>,
    pub verification_plan: VerificationPlan,
    pub done_condition: DoneCondition,
}
```

Start small. Do not try to encode everything. The first version should support:
goal, mode, stage, allowed scope, recent observations, edits made, validations
run, and current done status.

### P0. Action-Level Weighted Decision

Current evidence:

- `src/engine/workflow_contract.rs` already has `WeightFactors` and plan-step
  importance calculation.
- `src/engine/resource_policy.rs` provides route-level latency/cost/tool budgets.
- `src/engine/conversation_loop/tool_execution_controller.rs` gates raw tool
  calls by exposure, schema, permission, destructive scope, and tool-call limits.

Gap:

The project can control whether a model is allowed to call a tool, but it does
not yet require the model/runtime to explain why this action is worth doing now.
The note's weight idea should land between planning and tool execution.

Suggested first contract:

```rust
pub struct ActionDecision {
    pub reason_summary: String,
    pub action: ProposedAction,
    pub expected_observation: String,
    pub scores: ActionScores,
    pub requires_confirmation: bool,
    pub verification_after: Option<VerificationNeed>,
}

pub struct ActionScores {
    pub value: u8,
    pub risk: u8,
    pub uncertainty_reduction: u8,
    pub cost: u8,
    pub reversibility: u8,
}
```

Do not enable this everywhere at first. Use it for:

- high-risk tasks;
- focused repair loops;
- no-progress or repeated-failure turns;
- tool calls that mutate files, run broad shell commands, or touch config/schema.

### P0. Five-Zone Context Builder

Current evidence:

- `src/engine/prompt_context.rs` assembles base prompt and prompt-layer reports.
- `src/instructions/mod.rs` extracts only runtime guidance from `AGENTS.md`.
- `src/engine/conversation_loop/memory_snapshot_controller.rs` injects memory
  snapshots.
- `src/engine/conversation_loop/turn_request_bootstrap_controller.rs` performs
  memory snapshot, preflight compaction, and retrieval prompt injection.
- `src/engine/conversation_loop/request_preparation_controller.rs` injects
  ledger hints and memory prefetch near the final request.
- `src/engine/context_ledger.rs` records some file/bash read facts.
- `src/engine/model_context.rs` and `src/engine/context_usage.rs` begin model
  context-window and cached-token accounting.

Gap:

Context assembly is real, but distributed. Because several controllers append
or inject material independently, it is hard to guarantee:

- stable prefix remains stable;
- dynamic tail stays late;
- task state is separate from memory;
- retrieved material is separated from recent observations;
- provider prompt cache behavior is inspectable.

Suggested target:

```rust
pub struct ContextAssemblyPlan {
    pub stable_prefix: ContextZone,
    pub task_state: ContextZone,
    pub relevant_material: ContextZone,
    pub recent_observation: ContextZone,
    pub current_decision_request: ContextZone,
    pub token_report: ContextTokenReport,
    pub cache_report: ContextCacheReport,
}
```

The order should match the note:

1. Stable rules and tool contracts.
2. Current task state.
3. Relevant files, memory, retrieved docs, and ledger facts.
4. Recent tool/model observations.
5. The current user request and decision prompt.

This should be the next highest-leverage architecture improvement, especially
because the repo already has context-window WIP.

### P0. Phase-Aware Tool Exposure

Current evidence:

- `src/engine/conversation_loop/tool_orchestrator.rs` scopes tools by route.
- `CodeChange` and `BugFix` still expose a broad set including read/search,
  write/edit/patch, bash, diff, git, format, todo, and ask-user tools.

Gap:

Route-level scoping is better than one global tool surface, but it still gives
the model too many choices inside a route. A debugging task should not get the
same effective action surface during first inspection as during a confirmed
patch phase.

Suggested phases:

| Phase | Typical tools |
| --- | --- |
| `Understand` | list, glob, grep, file_read, maybe safe bash read-only |
| `Plan` | todo/plan trace, ask_user if ambiguity blocks progress |
| `Edit` | file_edit/file_patch/file_write only when scope is clear |
| `Validate` | bash, format, test, diff |
| `Repair` | narrow read/edit/validate tools only |
| `Closeout` | diff/status/evidence summary tools |

This should be controlled by `AgentTaskState.stage`, not only by intent route.

### P0. Evidence And Closeout Semantics

Current evidence:

- `src/engine/code_change_workflow.rs` has risk-sensitive policies and closeout
  rendering.
- `docs/PROJECT_STATUS.md` records strong validation baselines.

Gap:

The checker should consume structured evidence from task state and ledger. Final
answers should stay concise, but internal trace should know the difference
between:

- verified;
- failed;
- not run;
- not applicable;
- unavailable;
- user-deferred.

This matters because live evals and dogfood runs need to separate real product
success from agent closeout mistakes.

## 5. P1 Improvements

### P1. Memory And Ledger Should Become State Evidence

`src/engine/context_ledger.rs` is a strong start, but it should grow from read
history into session evidence:

- file read facts;
- bash observation facts;
- edits made;
- diffs produced;
- tests run;
- failures observed;
- fixes attempted;
- user confirmations.

The important distinction from the note:

- context is what the model sees now;
- state is the current task recordbook;
- memory is long-term experience;
- prompt cache is provider-side reuse of stable prefix tokens.

The repo should keep these separate in both types and rendered prompt zones.

### P1. Goal Drift Should Be State-Backed

Status: implemented in the current batch.

`src/engine/goal_drift.rs` now evaluates tool calls against
`AgentTaskState.allowed_scope` and `AgentTaskState.forbidden_actions`, not just
the latest user text and tool name. The current task state is passed through the
tool-round execution path into permission evaluation, so drift findings can
become approval evidence before a risky or out-of-scope action runs.

### P1. Workflow Contract Prompt Diet

`src/engine/workflow_contract.rs` has useful planning logic, but the rendered
contract should stay compact and reserved for tasks that need it. The note is
clear that the project should not force max-power workflow onto simple tasks.

Keep:

- strict contract for high-risk work;
- focused contract for repair/no-progress loops;
- light state/action trace for normal multi-step coding.

Avoid:

- always-on long planning prose;
- forcing every task through the same heavyweight template.

### P1. Context Cache Regression Tests

The context-management WIP should gain tests that prove:

- stable prefix hash/fingerprint does not change when only recent observations
  change;
- task state appears before relevant material;
- recent observation appears after relevant material;
- current decision request is last;
- memory retrieval does not mutate the stable prefix;
- context budget pressure shrinks dynamic zones before stable instructions.

## 6. P2 Improvements

### P2. Behavior Evals For Control-Loop Quality

Add or extend evals that assert behavior, not only final code diff:

- simple direct answer does not invoke workflow contract;
- debugging first inspects before editing;
- repeated failed test changes stage to repair;
- broad shell command requires confirmation or narrower alternative;
- no-progress loop asks user or changes strategy;
- task state records validation evidence before final closeout;
- context builder preserves zone order.

Status: started. Direct factual, rewrite, concept-explanation, and
error-explanation prompts now have regression coverage for direct routing,
empty scoped tool exposure, ordinary risk, skipped workflow contract, and direct
task-state verification semantics.

### P2. Docs And Status Alignment

`docs/PROJECT_STATUS.md` is the canonical status, but there are newer context
management WIP docs from 2026-05-23/24. After the current implementation line
lands, refresh status docs so they clearly distinguish:

- shipped baseline;
- active WIP;
- proposed next architecture work from this review.

## 7. Suggested Implementation Order

Completed in the current implementation line:

1. Add `ContextAssemblyPlan` and five-zone rendering around existing prompt,
   memory, retrieval, ledger, and request-preparation code.
2. Add minimal `AgentTaskState` and populate it for coding tasks without
   changing model behavior yet.
3. Feed task state into context zone 2 and closeout/checker logic.
4. Add phase-aware tool exposure driven by task state stage.
5. Add targeted `ActionDecision` scoring for high-risk, repair, and mutation
   actions.
6. Expand context ledger into edit/diff/validation/user-confirmation evidence.
7. Make goal drift and scope checks consult task state allowed scope and
   forbidden actions.

Remaining implementation queue:

1. Surface the new task-state, proof, and control-loop diagnostics in desktop
   app panels where useful.
2. Use the first runtime-spine live-eval reruns to tune which assertions should
   be required for audit/no-diff tasks versus mutating code-change tasks.

This order matters. If action scoring is added before context/state is clear,
the scoring becomes another prompt convention. If context/state land first,
weighted action decisions can be small, testable, and useful.

## 8. Concrete First PR Shape

Original recommended first PR:

Title: `Add five-zone context assembly plan`

Scope:

- Introduce a small `ContextAssemblyPlan` type.
- Move current stable instructions, task state placeholder, relevant material,
  recent observations, and current decision request into named zones.
- Preserve existing rendered prompt behavior as much as possible.
- Emit a debug report with zone token estimates and stable-prefix fingerprint.
- Add tests for zone order and stable-prefix invariance.

Why this first:

- It directly addresses sections 27-31 of the note.
- It builds on the repo's current context-management WIP.
- It creates the slot where future `AgentTaskState` and action decisions can
  live without inflating the always-on prompt.

Status: completed as part of the current implementation line. The next concrete
PR-sized slice should be behavior regressions for direct/lightweight tasks,
because most of the runtime contracts from this review now exist and need guard
tests that prevent over-control from returning.

Suggested validation:

```bash
cargo fmt --check
cargo test -q prompt_context
cargo test -q instructions
cargo test -q route_scoped_tools
cargo test -q context_usage
cargo test -q context_ledger
```

Then broaden only if shared request-building behavior moved:

```bash
cargo test -q
cargo clippy --all-features -- -D warnings
```

## 9. Final Assessment

The note is valuable because it points to a precise next step for
`priority-agent`: not "add more agent features", but make the runtime loop more
explicit and inspectable. The project already has many right components. The
missing work is integration:

- one task state recordbook;
- one context assembly contract;
- one structured action decision boundary;
- phase-aware tool exposure;
- evidence-backed checking.

That is consistent with the product principle of narrow, deep, personal, and
verifiable. It also avoids competing with generic agents by making this project
better at gex's actual local workflow.

## 10. Source Note Quality Review

The source note is high-value for this project. Its strongest point is that it
does not confuse "agent" with "a bigger prompt". It repeatedly separates:

- LLM semantic judgment from system-owned control;
- context from state;
- state from memory;
- structured decisions from natural-language thinking;
- high-risk workflow from simple direct answers.

That maps well to the current `priority-agent` direction. It is especially
useful because it names the exact runtime shape the project still needs:
controlled loop, state recordbook, weighted action choice, feedback updates,
and context zones.

The note is also balanced. It avoids three bad extremes:

- pure LLM autonomy;
- pure rules and formulas;
- all-JSON structured thinking.

The parts that need engineering refinement are:

- It repeats several principles because it is a learning note. In project docs,
  those should be compressed into a smaller set of design rules.
- It does not define exact Rust interfaces for state, action decisions,
  observations, or checkers.
- It does not define validation gates. The project should translate the note
  into tests such as route simplicity, phase transitions, stable context zones,
  no-progress detection, and evidence-backed closeout.
- It does not distinguish enough between "good internal trace" and "good
  user-facing answer". The runtime can be structured internally while final
  replies stay concise.

The right way to use the note is not to paste it into the system prompt. The
right way is to mine it for runtime contracts:

1. `ContextAssemblyPlan`
2. `AgentTaskState`
3. `ActionDecision`
4. phase-aware tool exposure
5. evidence-backed checker
6. control-loop behavior evals
