# Live Eval Summary: project-partner-demo-latest-20260525-233509

- Run directory: `docs/benchmarks/live-project-partner-demo-latest-20260525-233509`
- Tasks found: `3`
- Pass rate: `3/3` (100.0%)
- Failure rate: `0/3` (0.0%)
- Skipped/unscored tasks: `0`
- Real code-change passes: `2`
- Plan-only passes: `0`
- Seeded no-diff failures: `0`
- Memory active tasks: `3`
- Memory changed-plan tasks: `2`
- Memory recalled items: `0`
- Memory conflicts: `0`
- Memory typed-candidate tasks: `2`
- Memory evidence-backed candidate tasks: `2`
- Memory proposal tasks: `3`
- Memory proposal candidates: `2`
- Memory proposal evidence items: `17`
- Memory proposal review-required tasks: `3`
- Skill active tasks: `0`
- Skill promotion-evidence tasks: `0`
- Behavior assertion tasks: `0`
- Behavior assertions passed: `0`
- Runtime-spine assertion tasks: `3`
- Runtime-spine assertions passed: `3`
- Runtime-spine assertions failed: `0`
- Runtime-spine full coverage tasks: `0`
- Runtime-spine trace-present tasks: `3`
- Runtime-spine risky tool runs: `4`
- Runtime-spine risky tool reviewed: `4`
- Runtime-spine risky missing-review tasks: `0`
- Average outcome score: `100.0`
- Average process score: `100.0`
- Average efficiency score: `97.3`
- Average agent score: `99.3`
- Invalid actions total: `0`
- Premature edits total: `0`
- Scope drifts total: `0`
- Repeated actions total: `0`
- Failed actions total: `1`
- Coding gauntlet agent-run tasks: `3`
- Coding gauntlet passes: `3`
- Coding gauntlet failures: `0`
- Coding gauntlet likely clean passes: `1`
- Coding gauntlet repaired passes: `1`
- Coding gauntlet required-validation passes: `3/3`
- Coding gauntlet first-write observed: `2/3`
- Coding gauntlet repair signals: `2`
- Coding gauntlet changed files: `2`
- Status counts: passed=3
- Failure owners: none=3
- Eval intents: read_only_audit=1, seeded_code_change=2

## Failure Modes

- `warning:audit_no_code_diff`: `1`
- `warning:no_code_diff`: `1`

## Release Dogfood Failure Classes

| class | count | meaning |
|-------|-------|---------|
| desktop_evidence | 3 | Desktop UI, screenshot, native smoke, or visual evidence failures. |
| file_state | 3 | Read-before-edit, stale file, checkpoint, rollback, or diff-state failures. |
| permission_recovery | 3 | Permission denial, approval, or recovery-loop failures. |
| tool_contract | 3 | Tool schema, exposure, result-pair, or contract boundary failures. |
| llm_reasoning | 1 | Model failed to plan, edit, validate, or close out despite available tools. |

## Memory And Skill Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| memory_active_tasks | 3 | Tasks where retrieval, sync, or memory tools were active. |
| memory_changed_plan_tasks | 2 | Tasks where memory or learning signals reweighted planning. |
| memory_recalled_items | 0 | Retrieved memory-backed context items across tasks. |
| memory_conflicts | 0 | Retrieval-context conflict count from memory-backed context. |
| memory_candidate_typed_tasks | 2 | Tasks with typed memory candidates, including review-only MemoryProposal candidates. |
| memory_candidate_evidence_tasks | 2 | Tasks with evidence-backed memory candidates, including review-only MemoryProposal evidence. |
| memory_proposal_tasks | 3 | Tasks that emitted a review-only MemoryProposal trace event. |
| memory_proposal_candidates | 2 | Review-only MemoryProposal candidates proposed across tasks. |
| memory_proposal_evidence_items | 17 | Evidence items attached to review-only MemoryProposal candidates. |
| memory_proposal_review_required_tasks | 3 | MemoryProposal tasks that require review before persistence. |
| skill_active_tasks | 0 | Tasks where skill tools or skill-specific signals were active. |
| skill_promotion_evidence_tasks | 0 | Tasks with promotion-related skill evidence. |
| behavior_assertion_tasks | 0 | Tasks with explicit behavior assertions in the live-eval sample. |
| behavior_assertions_passed | 0 | Explicit behavior-assertion tasks whose required checks passed. |
| memory_behavior_assertion_tasks | 0 | Behavior assertions covering memory semantics rather than only memory activity signals. |
| skill_behavior_assertion_tasks | 0 | Behavior assertions covering skill semantics rather than only skill activity signals. |

## Runtime Spine Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| runtime_spine_assertion_tasks | 3 | Tasks with explicit runtime-spine assertions in the live-eval sample or report. |
| runtime_spine_assertions_passed | 3 | Runtime-spine assertion tasks whose required trace/control-loop signals were present. |
| runtime_spine_assertions_failed | 0 | Runtime-spine assertion tasks missing required trace/control-loop signals. |
| runtime_spine_full_coverage_tasks | 0 | Tasks whose trace touched all runtime-spine phases. |
| runtime_spine_trace_present_tasks | 3 | Tasks with a trace summary available to the report parser. |
| runtime_spine_risky_tool_runs | 4 | Risky tool executions observed from trace or agent events. |
| runtime_spine_risky_tool_reviewed | 4 | Risky tool executions with matching action.review trace evidence. |
| runtime_spine_risky_missing_review_tasks | 0 | Tasks with risky tool executions missing matching action.review evidence. |

## Evaluation Scores

| dimension | value | meaning |
|-----------|-------|---------|
| outcome_score_avg | 100.0 | Average deterministic outcome score across task reports. |
| process_score_avg | 100.0 | Average deterministic process score across task reports. |
| efficiency_score_avg | 97.3 | Average deterministic efficiency score across task reports. |
| agent_score_avg | 99.3 | Weighted score: outcome 50%, process 30%, efficiency 20%. |
| invalid_actions_total | 0 | Premature edits, scope drift, repeated actions, risky review gaps, and phase-misaligned actions. |
| premature_edits_total | 0 | Edits attempted before enough evidence or explicitly demoted as early/low-value. |
| scope_drifts_total | 0 | Action decisions with very low scope fit or medium/high goal drift. |
| repeated_actions_total | 0 | Repeated tool actions or repeated-action stop signals. |
| failed_actions_total | 1 | Failed tool/action observations from trace and event logs. |

### Score Matrix

| task | outcome | process | efficiency | agent | invalid | premature_edit | scope_drift | repeated | failed_actions | penalties |
|------|---------|---------|------------|-------|---------|----------------|-------------|----------|----------------|-----------|
| project-partner-failure-memory-proposal | 100 | 100 | 100 | 100 | 0 | 0 | 0 | 0 | 0 | none |
| project-partner-resume-with-memory | 100 | 100 | 100 | 100 | 0 | 0 | 0 | 0 | 0 | none |
| project-partner-vague-local-tool | 100 | 100 | 92 | 98 | 0 | 0 | 0 | 0 | 1 | failed_actions |

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 2 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 0 | Seeded code-change tasks where the agent did not produce a diff. |

## Coding Gauntlet Evidence

| task | gauntlet_status | first_pass_signal | failure_class | coding | required | closeout | spine | contract | risk | first_write | diff | warnings |
|------|-----------------|-------------------|---------------|--------|----------|----------|-------|----------|------|-------------|------|----------|
| project-partner-failure-memory-proposal | passed | likely_clean | tool_contract,file_state,permission_recovery,desktop_evidence | tools=3, tool_records=5, validations=2, repair=0, files=1 | ok | passed | coverage=6/7, status=passed, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 3 | yes | none |
| project-partner-resume-with-memory | passed | no_write | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=5, tool_records=9, validations=0, repair=0, files=0 | ok | passed | coverage=6/7, status=passed, missing=none | entry=skipped:force repair=none | entry=ordinary runtime=none | none | no | no_code_diff,audit_no_code_diff |
| project-partner-vague-local-tool | passed | repaired | tool_contract,file_state,permission_recovery,desktop_evidence | tools=3, tool_records=6, validations=2, repair=2, files=1 | ok | passed | coverage=6/7, status=passed, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 3 | yes | none |

## Task Matrix

| task | status | intent | owner | failure_class | required | plan_quality | tool_boundary | verification_status | closeout | runtime_spine | runtime_diet | contract | risk | behavior_assertions | behavior_status | triggers | first_write | diff | memory | skill | warnings |
|------|--------|--------|-------|---------------|----------|--------------|---------------|---------------------|----------|---------------|--------------|----------|------|---------------------|-----------------|----------|-------------|------|--------|-------|----------|
| project-partner-failure-memory-proposal | passed | seeded_code_change | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=4983 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:3/3 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation,first_code_change | 3 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| project-partner-resume-with-memory | passed | read_only_audit | none | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=4097 tool_schema=1069 tools=6 workflow=none closeout=full validation=not_applicable | entry=skipped:force repair=none | entry=ordinary runtime=none | none | none | none | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| project-partner-vague-local-tool | passed | seeded_code_change | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=5386 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:1/1 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation,first_code_change | 3 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `skipped` reports are excluded from pass/fail rate denominators; collect-only reports need passing required commands to be scored.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
- `memory` and `skill` summarize evidence signals; they do not by themselves mean the task succeeded.
- `behavior_assertions` are explicit sample-level checks; memory/skill behavior assertions are stronger evidence than activity signals alone.
- `runtime_spine` summarizes trace/control-loop coverage and explicit runtime-spine assertions.
