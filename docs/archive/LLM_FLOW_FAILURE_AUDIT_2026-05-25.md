# LLM Flow Failure Audit - 2026-05-25

## Purpose

This audit reviews the recent MVA and weighted-agent live-eval records with one
specific question: did failures come from Priority Agent's control flow, or from
model variability in MiniMax-M2.7?

The operating assumption is that a normal LLM can make task mistakes. The
runtime should not try to prompt those mistakes away. The runtime should:

- pass tool failures and validation output back into the next model turn;
- block verified closeout when required evidence is missing;
- stop bounded no-progress loops honestly;
- classify model task-completion failures separately from agent-flow failures.

## Evidence Reviewed

- `docs/benchmarks/mvp-weighted-agent-ab-20260525-155452.md`
- `docs/benchmarks/live-mva-followup-20260525-162804/`
- `docs/benchmarks/live-mva-followup-lowvalue2-20260525-164214/`
- `docs/benchmarks/live-mva-followup-lowvalue3-20260525-164512/`
- `docs/benchmarks/live-mva-followup-verification-20260525-165019/`
- `docs/benchmarks/live-mva-followup-full-20260525-165257/`
- `docs/benchmarks/live-mva-prompt-policy-verification-20260525-llm-flow-audit/`

## Classification

| Area | Classification | Evidence | Current status |
| --- | --- | --- | --- |
| Weighted vs baseline A/B | Process improvement, not outcome improvement | A/B score moved from `72.4` to `75.4`; process improved `+8.4`; outcome stayed flat. | Good signal, but not enough to claim broad capability improvement. |
| High-risk block | Eval/reporting false negative, not core runtime failure | Follow-up reports show protected no-diff task passed with runtime-spine assertions and required command status ok. | Fixed. |
| Loop output assertion | Eval assertion too brittle | Semantic `contains_any` assertions now accept Chinese and English verification wording. | Fixed. |
| Low-value replan | Real process issue plus assertion issue | Earlier run passed required commands but failed output/trajectory assertions; final low-value run passed with `agent_score=95`, no scope drift, and bounded duplicate read handling. | Fixed enough for MVA. |
| Verification repair targeted run | Model can recover when evidence is fed back | `mva-followup-verification-20260525-165019` passed after failed validation and repair, but with noisy process score. | Flow works; efficiency still noisy. |
| Verification repair full-suite failure | Mostly model task-completion failure, with one prompt-policy risk found | `mva-followup-full-20260525-165257` produced no diff, failed required commands, and closeout was `not_verified`; runtime did not falsely pass. However the trace also showed a misleading code-write-forbidden checkpoint. | Flow did not falsely succeed; prompt-policy risk fixed in this audit. |
| Prompt/tool policy over-control | Agent-flow risk | `WorkflowPromptPolicy::forbids_code_write_tools` treated a prompt that forbade `file_write` and `file_patch` as if all code-write tools were forbidden, even when `file_edit` was explicitly allowed. | Fixed with tests. |

## Prompt Over-Tuning Assessment

The current default prompt is not the main source of the recent failures.

The base prompt in `src/engine/mod.rs` is now relatively small and stable:

- core conduct;
- model-led programming workflow;
- verification and honest reporting;
- actions with care;
- response format.

The project also avoids loading the whole `AGENTS.md` doctrine by default:
`src/instructions/mod.rs` prefers only the `## Agent Runtime Guidance` section,
caps each layer, and records prompt-layer reports.

The remaining over-control risk is not the base prompt. It is targeted runtime
repair text and prompt-derived policy. These are acceptable only when they are
gated by actual failures, required validation, or high risk. The specific
prompt-derived policy bug found here was fixed by making code-write forbiddance
respect explicit `## Allowed tools` entries.

## Repair Feedback Chain

The current runtime does feed LLM mistakes back into the next turn:

1. Failed validation commands become `ToolObservation` records with diagnostics,
   key findings, next attention, and recovery kind.
2. Those observations are stored in task state and recent observation context.
3. The next model request includes the failed evidence and can call a repair
   tool.
4. Successful edits trigger required validation and completion proof.
5. Completion proof gates closeout; missing or failed proof produces
   `not_verified`, `failed`, or `partial` status instead of a false green
   result.
6. Repeated no-progress or repeated failed tools stop the loop with an honest
   blocker rather than spinning indefinitely.

This is the right architecture for MiniMax-level variability: let the model try,
let runtime verify, send failures back, and stop honestly when the model cannot
recover.

## Fix Applied During This Audit

File:

- `src/engine/conversation_loop/workflow_prompt_policy.rs`

Change:

- `forbids_code_write_tools` now parses both `## Allowed tools` and
  `## Forbidden tools`.
- If any code-write tool such as `file_edit` is explicitly allowed, the prompt
  is not treated as globally write-forbidden.
- Read-only tasks with an allowed-tool section that has no code-write tools are
  still treated as write-forbidden.
- Natural-language constraints such as `Do not edit files` still block writes.

Why this matters:

- `minimum-agent-verification-repair` allows `file_edit` but forbids
  `file_write` and `file_patch`.
- Before the fix, the runtime could inject misleading code-write-forbidden
  recovery text.
- After the fix, targeted rerun
  `mva-prompt-policy-verification-20260525-llm-flow-audit` passed with only
  three tool executions, one changed file, required commands ok, and
  `closeout_status=passed`.

## Current Verdict

Priority Agent's LLM/runtime flow is now substantially healthier than the raw
failure count suggests.

The latest evidence supports this split:

- True flow or eval issues found and fixed: brittle semantic assertions,
  blocked-task closeout normalization, duplicate read-only behavior, stale
  edit-repair guidance, and prompt-derived write-policy over-blocking.
- Expected model variability: MiniMax sometimes fails to move from evidence to
  the right edit, repeats reads, or proposes stale edit anchors.
- Runtime success criterion: when the model fails, the system should return
  `not_verified` or `failed` with required-command evidence. The latest failed
  full-suite verification run did that.

Do not respond to individual MiniMax failures by adding more always-on prompt
rules. Prefer small runtime checks, tighter tool contracts, better failure-owner
classification, and targeted repair feedback that activates only after concrete
evidence.

## Recommended Next Steps

1. Treat full-suite failures as actionable only after checking whether required
   commands, closeout, completion proof, and failure owner agree.
2. Keep the base prompt short. Add new guidance only as a gated repair message
   after a concrete tool or validation failure.
3. Improve failure-owner classification for `not_verified` seeded code-change
   failures so they land under `llm_reasoning` unless runtime proof is missing
   or contradictory.
4. Add a report assertion that flags misleading workflow fallback messages such
   as code-write-forbidden when `file_edit` is explicitly allowed.
5. Track `recovered_failed` validation evidence as a positive repair-loop
   signal, not only as a process penalty.
