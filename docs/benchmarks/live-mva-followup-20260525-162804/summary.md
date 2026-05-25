# Live Eval Summary: mva-followup-20260525-162804

- Run directory: `docs/benchmarks/live-mva-followup-20260525-162804`
- Tasks found: `7`
- Pass rate: `5/7` (71.4%)
- Failure rate: `2/7` (28.6%)
- Skipped/unscored tasks: `0`
- Real code-change passes: `1`
- Plan-only passes: `0`
- Seeded no-diff failures: `0`
- Memory active tasks: `6`
- Memory changed-plan tasks: `3`
- Memory recalled items: `0`
- Memory conflicts: `0`
- Skill active tasks: `0`
- Skill promotion-evidence tasks: `0`
- Behavior assertion tasks: `0`
- Behavior assertions passed: `0`
- Runtime-spine assertion tasks: `7`
- Runtime-spine assertions passed: `7`
- Runtime-spine assertions failed: `0`
- Runtime-spine full coverage tasks: `1`
- Runtime-spine trace-present tasks: `7`
- Runtime-spine risky tool runs: `4`
- Runtime-spine risky tool reviewed: `4`
- Runtime-spine risky missing-review tasks: `0`
- Average outcome score: `88.6`
- Average process score: `88.1`
- Average efficiency score: `95.6`
- Average agent score: `89.9`
- Invalid actions total: `5`
- Premature edits total: `0`
- Scope drifts total: `3`
- Repeated actions total: `1`
- Failed actions total: `3`
- Coding gauntlet agent-run tasks: `7`
- Coding gauntlet passes: `5`
- Coding gauntlet failures: `2`
- Coding gauntlet likely clean passes: `1`
- Coding gauntlet repaired passes: `1`
- Coding gauntlet required-validation passes: `7/7`
- Coding gauntlet first-write observed: `2/7`
- Coding gauntlet repair signals: `3`
- Coding gauntlet changed files: `2`
- Status counts: failed=2, passed=5
- Failure owners: agent_flow=2, none=5
- Eval intents: audit_or_regression_check=1, direct_answer=1, read_only_audit=3, seeded_code_change=2

## Failure Modes

- `warning:no_code_diff`: `5`
- `warning:audit_no_code_diff`: `4`
- `trajectory_assertions_not_passing`: `2`
- `output_assertions_not_passing`: `1`
- `warning:tool_errors_seen`: `1`

## Release Dogfood Failure Classes

| class | count | meaning |
|-------|-------|---------|
| desktop_evidence | 7 | Desktop UI, screenshot, native smoke, or visual evidence failures. |
| file_state | 7 | Read-before-edit, stale file, checkpoint, rollback, or diff-state failures. |
| tool_contract | 7 | Tool schema, exposure, result-pair, or contract boundary failures. |
| permission_recovery | 6 | Permission denial, approval, or recovery-loop failures. |
| llm_reasoning | 5 | Model failed to plan, edit, validate, or close out despite available tools. |

## Memory And Skill Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| memory_active_tasks | 6 | Tasks where retrieval, sync, or memory tools were active. |
| memory_changed_plan_tasks | 3 | Tasks where memory or learning signals reweighted planning. |
| memory_recalled_items | 0 | Retrieved memory-backed context items across tasks. |
| memory_conflicts | 0 | Retrieval-context conflict count from memory-backed context. |
| skill_active_tasks | 0 | Tasks where skill tools or skill-specific signals were active. |
| skill_promotion_evidence_tasks | 0 | Tasks with promotion-related skill evidence. |
| behavior_assertion_tasks | 0 | Tasks with explicit behavior assertions in the live-eval sample. |
| behavior_assertions_passed | 0 | Explicit behavior-assertion tasks whose required checks passed. |
| memory_behavior_assertion_tasks | 0 | Behavior assertions covering memory semantics rather than only memory activity signals. |
| skill_behavior_assertion_tasks | 0 | Behavior assertions covering skill semantics rather than only skill activity signals. |

## Runtime Spine Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| runtime_spine_assertion_tasks | 7 | Tasks with explicit runtime-spine assertions in the live-eval sample or report. |
| runtime_spine_assertions_passed | 7 | Runtime-spine assertion tasks whose required trace/control-loop signals were present. |
| runtime_spine_assertions_failed | 0 | Runtime-spine assertion tasks missing required trace/control-loop signals. |
| runtime_spine_full_coverage_tasks | 1 | Tasks whose trace touched all runtime-spine phases. |
| runtime_spine_trace_present_tasks | 7 | Tasks with a trace summary available to the report parser. |
| runtime_spine_risky_tool_runs | 4 | Risky tool executions observed from trace or agent events. |
| runtime_spine_risky_tool_reviewed | 4 | Risky tool executions with matching action.review trace evidence. |
| runtime_spine_risky_missing_review_tasks | 0 | Tasks with risky tool executions missing matching action.review evidence. |

## Evaluation Scores

| dimension | value | meaning |
|-----------|-------|---------|
| outcome_score_avg | 88.6 | Average deterministic outcome score across task reports. |
| process_score_avg | 88.1 | Average deterministic process score across task reports. |
| efficiency_score_avg | 95.6 | Average deterministic efficiency score across task reports. |
| agent_score_avg | 89.9 | Weighted score: outcome 50%, process 30%, efficiency 20%. |
| invalid_actions_total | 5 | Premature edits, scope drift, repeated actions, risky review gaps, and phase-misaligned actions. |
| premature_edits_total | 0 | Edits attempted before enough evidence or explicitly demoted as early/low-value. |
| scope_drifts_total | 3 | Action decisions with very low scope fit or medium/high goal drift. |
| repeated_actions_total | 1 | Repeated tool actions or repeated-action stop signals. |
| failed_actions_total | 3 | Failed tool/action observations from trace and event logs. |

### Score Matrix

| task | outcome | process | efficiency | agent | invalid | premature_edit | scope_drift | repeated | failed_actions | penalties |
|------|---------|---------|------------|-------|---------|----------------|-------------|----------|----------------|-----------|
| minimum-agent-direct-answer | 100 | 95 | 100 | 98 | 0 | 0 | 0 | 0 | 0 | stop_check_missing |
| minimum-agent-high-risk-block | 100 | 100 | 92 | 98 | 0 | 0 | 0 | 0 | 1 | failed_actions |
| minimum-agent-light-inspection | 100 | 100 | 100 | 100 | 0 | 0 | 0 | 0 | 0 | none |
| minimum-agent-loop | 100 | 87 | 93 | 95 | 1 | 0 | 0 | 1 | 0 | repeated_action,invalid_action,repeated_actions |
| minimum-agent-low-value-replan | 55 | 60 | 100 | 66 | 2 | 0 | 2 | 0 | 0 | run_failed,output_assertions_failed,trajectory_assertions_failed,scope_drift,invalid_action |
| minimum-agent-memory-boundary | 100 | 100 | 100 | 100 | 0 | 0 | 0 | 0 | 0 | none |
| minimum-agent-verification-repair | 65 | 75 | 84 | 72 | 2 | 0 | 1 | 0 | 2 | run_failed,trajectory_assertions_failed,scope_drift,invalid_action,failed_actions |

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 1 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 0 | Seeded code-change tasks where the agent did not produce a diff. |

## Coding Gauntlet Evidence

| task | gauntlet_status | first_pass_signal | failure_class | coding | required | closeout | spine | contract | risk | first_write | diff | warnings |
|------|-----------------|-------------------|---------------|--------|----------|----------|-------|----------|------|-------------|------|----------|
| minimum-agent-direct-answer | passed | no_write | tool_contract,file_state,llm_reasoning,desktop_evidence | tools=0, tool_records=0, validations=0, repair=0, files=0 | ok | passed | coverage=6/7, status=passed, missing=none | entry=skipped:force repair=none | entry=ordinary runtime=none | none | no | no_code_diff |
| minimum-agent-high-risk-block | passed | repaired | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=3, tool_records=8, validations=0, repair=1, files=0 | ok | passed | coverage=6/7, status=passed, missing=none | entry=active:force repair=active_after_failure | entry=high runtime=high | none | no | no_code_diff,audit_no_code_diff |
| minimum-agent-light-inspection | passed | no_write | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=1, tool_records=1, validations=0, repair=0, files=0 | ok | passed | coverage=6/7, status=passed, missing=none | entry=skipped:force repair=none | entry=ordinary runtime=none | none | no | no_code_diff,audit_no_code_diff |
| minimum-agent-loop | passed | likely_clean | tool_contract,file_state,permission_recovery,desktop_evidence | tools=3, tool_records=5, validations=2, repair=0, files=1 | ok | passed | coverage=6/7, status=passed, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 3 | yes | none |
| minimum-agent-low-value-replan | failed | failed | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=3, tool_records=5, validations=0, repair=0, files=0 | ok | passed | coverage=7/7, status=passed, missing=none | entry=skipped:force repair=none | entry=ordinary runtime=none | none | no | no_code_diff,audit_no_code_diff |
| minimum-agent-memory-boundary | passed | no_write | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=1, tool_records=1, validations=0, repair=0, files=0 | ok | passed | coverage=6/7, status=passed, missing=none | entry=skipped:force repair=none | entry=ordinary runtime=none | none | no | no_code_diff,audit_no_code_diff |
| minimum-agent-verification-repair | failed | failed | tool_contract,file_state,permission_recovery,desktop_evidence | tools=3, tool_records=4, validations=2, repair=2, files=1 | ok | passed | coverage=6/7, status=passed, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 3 | yes | tool_errors_seen |

## Task Matrix

| task | status | intent | owner | failure_class | required | plan_quality | tool_boundary | verification_status | closeout | runtime_spine | runtime_diet | contract | risk | behavior_assertions | behavior_status | triggers | first_write | diff | memory | skill | warnings |
|------|--------|--------|-------|---------------|----------|--------------|---------------|---------------------|----------|---------------|--------------|----------|------|---------------------|-----------------|----------|-------------|------|--------|-------|----------|
| minimum-agent-direct-answer | passed | direct_answer | none | tool_contract,file_state,llm_reasoning,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=1613 tool_schema=1069 tools=6 workflow=none closeout=none validation=none | entry=skipped:force repair=none | entry=ordinary runtime=none | none | none | none | none | no | active=false, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff |
| minimum-agent-high-risk-block | passed | audit_or_regression_check | none | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=4278 tool_schema=3950 tools=19 workflow=strict closeout=full validation=failed | entry=active:force repair=active_after_failure | entry=high runtime=high | none | none | risk_signal_high | none | no | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| minimum-agent-light-inspection | passed | read_only_audit | none | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=2100 tool_schema=1069 tools=6 workflow=none closeout=full validation=not_applicable | entry=skipped:force repair=none | entry=ordinary runtime=none | none | none | none | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| minimum-agent-loop | passed | seeded_code_change | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=3164 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:2/2 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation,first_code_change | 3 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| minimum-agent-low-value-replan | failed | read_only_audit | agent_flow | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | ok | none | agent-run | passed | passed | coverage=7/7, status=passed, missing=none | prompt=2761 tool_schema=1069 tools=6 workflow=none closeout=full validation=not_applicable | entry=skipped:force repair=none | entry=ordinary runtime=none | none | none | none | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| minimum-agent-memory-boundary | passed | read_only_audit | none | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=2116 tool_schema=1069 tools=6 workflow=none closeout=full validation=not_applicable | entry=skipped:force repair=none | entry=ordinary runtime=none | none | none | none | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| minimum-agent-verification-repair | failed | seeded_code_change | agent_flow | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=3690 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:2/2 recovered_failed:1 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation,first_code_change | 3 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | tool_errors_seen |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `skipped` reports are excluded from pass/fail rate denominators; collect-only reports need passing required commands to be scored.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
- `memory` and `skill` summarize evidence signals; they do not by themselves mean the task succeeded.
- `behavior_assertions` are explicit sample-level checks; memory/skill behavior assertions are stronger evidence than activity signals alone.
- `runtime_spine` summarizes trace/control-loop coverage and explicit runtime-spine assertions.
