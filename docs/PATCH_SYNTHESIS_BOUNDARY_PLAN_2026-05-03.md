# Patch Synthesis Boundary Plan - 2026-05-03

## Context

The fresh `memory-recall-conflict-precision` capability run failed with
`failure_owner=agent_flow` and `no_code_diff`. The trace showed repeated
inspection, then hidden patch synthesis, then closeout `not_verified`.

This is exactly the boundary risk gex raised: the runtime should guide the LLM
with workflow, evidence, validation, and review, but should not silently encode
task-specific answers for real coding tasks.

## Decision

Default runtime behavior should not use deterministic task-specific patch
synthesis.

Allowed by default:

- focused workflow reminders
- validation requirements
- closeout/acceptance gates
- generic evidence-backed LLM repair prompts

Not allowed by default:

- hard-coded patches for specific benchmark tasks
- hidden source edits based on task id or benchmark phrase

## Steps

- [x] Add an explicit opt-in gate for deterministic patch synthesis.
- [x] Keep existing deterministic helpers available for research/regression
  only, behind that opt-in.
- [x] Record the failed `memory-recall-conflict-precision` run in the capability
  plan as a useful failure, not a product pass.
- [x] Run focused tests for patch synthesis behavior.
- [x] Re-run `memory-recall-conflict-precision` with deterministic patch
  synthesis disabled by default.
- [x] Tighten focused repair tool exposure so bash is available for validation
  only after a file change exists.
- [ ] Commit the evidence and boundary change.

## Validation

```bash
cargo test -q patch_synthesis -- --test-threads=1
cargo test -q code_action_tools -- --test-threads=1
cargo test -q action_checkpoint -- --test-threads=1
```

Result:

- patch synthesis: passed, 19 tests.
- code action tools: passed, 1 test.
- action checkpoint: passed, 5 tests.

## Follow-up Run

`capability-memory-conflict-nosynth-20260503-183836` still failed with
`failure_owner=agent_flow` and no code diff, but it no longer used deterministic
task-specific patch synthesis. The failure shape shifted to focused repair tool
usage: after reading the target file, the agent called bash before any file
change and then failed a file_edit call.

Decision: keep deterministic patch synthesis off by default, and reduce the
tool-surface mismatch in focused repair mode by hiding bash until there is a
file change to validate.
