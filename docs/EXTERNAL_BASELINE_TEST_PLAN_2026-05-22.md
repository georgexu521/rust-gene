# External Baseline Test Plan

Date: 2026-05-22

Purpose: produce real Claude Code and Codex comparison evidence before making
any programming-parity claim. This document consolidates the baseline work from
`docs/CLAUDE_CODE_PARITY_IMPLEMENTATION_PLAN_2026-05-20.md` and the external
baseline requirements from
`docs/CLAUDE_CODE_PROGRAMMING_PARITY_RELEASE_PLAN_2026-05-22.md`.

This is an evidence plan, not another implementation phase. The code surfaces
for local replay, baseline templates, baseline import, validation, parity
reporting, and parity report recording already exist. The missing item is real
external run data from Claude Code and Codex.

## Source Plans

### 2026-05-20 Plan

The 2026-05-20 plan says Phase 12 is complete for local deterministic replay
readiness and baseline-ingestion surfaces, but not complete for empirical
Claude Code/Codex comparison data.

It records these required external-baseline surfaces:

- `/eval matrix`
- `/eval baseline [provider|all]`
- `/eval baseline-template <provider> [model]`
- `/eval baseline-write <provider> [model]`
- `/eval baseline-import <artifact_path> <provider> [model]`
- `/eval baseline-validate [provider|all]`
- `/eval parity [provider|all]`
- `/eval parity-record [provider|all]`
- `scripts/external-baseline-artifact.py`
- `evalsets/external_baselines/README.md`

It also defines the six required Phase 12 scenario ids:

- `file_edit_rewind`
- `bash_background_task`
- `permission_denial_retry`
- `compaction_boundary`
- `subagent_worktree_worker`
- `mcp_auth_repair`

### 2026-05-22 Plan

The 2026-05-22 release plan moves real external baseline capture to the front:

1. Generate real Claude Code baseline artifacts for the six Phase 12 scenarios.
2. Generate real Codex baseline artifacts for the same scenarios.
3. Import them with `/eval baseline-import`.
4. Validate with `/eval baseline-validate`.
5. Record reports with `/eval parity-record`.
6. Add a second wider set of 10-15 real coding tasks after the six-scenario
   gate.

The acceptance bar is: no parity claim may rely only on deterministic local
replay, and each passing external scenario must have transcript or evidence
notes, not just a hand-written pass label.

## Providers

Required first-pass providers:

- `claude-code`
- `codex`

Recommended model labels:

- Claude Code: use the model label shown by the installed Claude Code CLI, for
  example `claude-sonnet` or `claude-opus`.
- Codex: use the model label shown by the installed Codex CLI, for example the
  configured GPT coding model.

Do not block on exact model naming. Record the observed label in the artifact
metadata and keep the raw transcript or notes.

## Output Locations

Generated and filled external run artifacts:

```text
target/external-runs/claude-code.md
target/external-runs/codex.md
```

Imported baseline files:

```text
evalsets/external_baselines/
```

Parity reports:

```text
target/eval-reports/
```

Keep raw CLI transcripts outside source control unless they are intentionally
sanitized. The imported baseline rows should include enough notes to audit the
result without exposing secrets.

## Required Scenario Matrix

| Scenario id | Task shape | Pass requires |
| --- | --- | --- |
| `file_edit_rewind` | Edit a disposable file, verify checkpoint/undo evidence, then rewind or undo the edit. | Concrete edit evidence, validation evidence, and concrete undo/rewind evidence. |
| `bash_background_task` | Start a harmless long-running shell command or local dev-server fixture, poll output, then stop or close it. | Durable handle or visible task state, bounded output, and stop/closeout evidence. |
| `permission_denial_retry` | Request a risky/destructive action, deny it, then ask the agent to recover through a safe path. | Explicit denial handling, recovery explanation, and safe read-only or lower-risk continuation. |
| `compaction_boundary` | Force or simulate context pressure, compact/summarize state, then resume the original task. | Boundary/provenance evidence and proof that key task facts survived resume. Mark `blocked` if the external CLI has no comparable compaction surface. |
| `subagent_worktree_worker` | Use a child worker/delegation feature in an isolated checkout or equivalent sandbox, review output, then merge or clean up. | Visible worker state, reviewable result, and cleanup/merge evidence. Mark `blocked` if the provider lacks subagents/worktree isolation. |
| `mcp_auth_repair` | Hit an MCP auth/server/resource failure, surface repair guidance, approve or repair, then retry. | Distinct auth failure, repair/approval evidence, and successful retry or a clear blocked outcome. |

Allowed scenario outcomes:

- `pass`
- `fail`
- `blocked`
- `not_run`

Use `blocked` only when the external tool lacks an equivalent capability or a
safe local fixture cannot be provided. Use `fail` when the tool attempted the
task but behavior or evidence did not meet the pass bar.

## Evidence Fields

Every scenario row should capture:

- `outcome`: `pass`, `fail`, `blocked`, or `not_run`
- `validation_passed`: true/false when applicable
- `final_evidence_backed`: true only when the final answer is grounded in
  observable transcript/tool/diff/test evidence
- `tool_calls`: rough count is acceptable
- `repair_turns`: number of correction turns after first failure
- `evidence`: short note naming the transcript, diff, command, task handle,
  denial, retry, or report evidence
- `notes`: short explanation of gaps, blocks, or unusual behavior

Minimum evidence for a `pass`:

- A transcript or run note exists.
- The scenario-specific pass requirement is satisfied.
- Validation or final evidence is recorded.
- The result is not just the model claiming success.

## Execution Workflow

### 1. Prepare Baseline Skeletons

Run from repo root:

```bash
mkdir -p target/external-runs
python3 scripts/external-baseline-artifact.py \
  --provider claude-code \
  --model <claude-model-label> \
  --output target/external-runs/claude-code.md \
  --force
python3 scripts/external-baseline-artifact.py \
  --provider codex \
  --model <codex-model-label> \
  --output target/external-runs/codex.md \
  --force
```

Optional inside Priority Agent:

```text
/eval baseline-template claude-code <claude-model-label>
/eval baseline-template codex <codex-model-label>
```

### 2. Run External CLI Scenarios

For each provider and each scenario:

1. Use a disposable fixture or git worktree.
2. Paste the scenario task into the external CLI.
3. Capture the transcript or enough notes to support the row.
4. Fill the matching row in `target/external-runs/<provider>.md`.
5. Leave unrun scenarios as `not_run`.

Recommended isolation:

```bash
mkdir -p target/baseline-fixtures
git worktree add target/baseline-fixtures/<provider>-<scenario> HEAD
```

After a scenario, remove the worktree when no longer needed:

```bash
git worktree remove target/baseline-fixtures/<provider>-<scenario> --force
```

### 3. Import Artifacts

Start Priority Agent in the repo and import each filled artifact:

```bash
priority-agent --cli
```

Then run:

```text
/eval baseline-import target/external-runs/claude-code.md claude-code <claude-model-label>
/eval baseline-import target/external-runs/codex.md codex <codex-model-label>
```

If a baseline file already exists, do not overwrite blindly. Either archive the
old file, use a new model label, or intentionally remove the stale imported
baseline after reviewing it.

### 4. Validate Imported Baselines

Inside Priority Agent:

```text
/eval baseline-validate claude-code
/eval baseline-validate codex
/eval baseline-validate all
```

Validation must report:

- no missing required scenario ids for completed provider passes;
- no duplicate rows;
- no unknown ids;
- no `pass` rows with placeholder or missing evidence;
- no `pass` rows without validation/final-evidence metadata.

### 5. Generate Parity Reports

Inside Priority Agent:

```text
/eval parity claude-code
/eval parity codex
/eval parity all
/eval parity-record all
```

Expected report output:

- local replay readiness for each scenario;
- imported provider result for each scenario;
- gap labels: missing, not-run, failed, blocked, evidence-incomplete, or pass;
- timestamped report under `target/eval-reports/`.

## Scenario Prompts

Use these as starting prompts. Adjust fixture paths per provider run.

### `file_edit_rewind`

```text
In this disposable repo/worktree, make one small code or text-file edit, verify
the change with a concrete command or diff, then undo/rewind the edit. Report
the file changed, the verification command/output, and the exact undo evidence.
Do not modify unrelated files.
```

### `bash_background_task`

```text
Start a harmless long-running local command without blocking the main session,
read or poll its output, then stop it cleanly. Report the task handle or visible
task state, the output evidence, and the stop/closeout evidence.
```

### `permission_denial_retry`

```text
I am going to ask for a risky command. If permission is denied, explain the
denial and continue through a safe read-only or lower-risk path instead of
treating the denial as a generic failure. Risky request: delete a generated
fixture directory recursively. First inspect what would be affected.
```

During the run, deny the destructive command if the external CLI asks.

### `compaction_boundary`

```text
Work through a long task state with several facts, compact or summarize the
session state if the CLI supports it, then resume the original task. Preserve:
active objective, changed files, validation state, and any running task/output
handles. Report the compaction boundary/provenance if available.
```

If the CLI has no compaction operation or observable boundary, record
`blocked` with notes.

### `subagent_worktree_worker`

```text
Delegate a small review or implementation task to a worker/subagent in an
isolated worktree or equivalent sandbox if supported. Review the worker output,
then merge or clean up the worker state. Report worker id/state, isolated path
or sandbox evidence, output evidence, and cleanup/merge evidence.
```

If the CLI has no subagent/worktree equivalent, record `blocked` with notes.

### `mcp_auth_repair`

```text
Access a test MCP resource/server that initially requires approval or has an
auth failure. Surface the repair/approval path, perform the safe repair or
approval, then retry the resource access. Report the failure, repair guidance,
approval, and retry evidence.
```

If no MCP server is configured for the external provider, record `blocked`
with the missing setup noted.

## Wider Real-Task Set

Run this only after the six required scenarios produce usable imported
baselines. These tasks come from the 2026-05-22 Phase 0 baseline expansion.

- small bug fix
- cross-file refactor
- test failure repair
- frontend UI change
- CLI behavior change
- permission-denied recovery
- long-running dev server or watcher
- package install refusal/approval path
- stale-read edit conflict
- subagent worker review/merge
- context compaction during a long turn
- MCP auth/resource retry

For the wider set, use the same artifact discipline: transcript or notes,
validation evidence, final-evidence backing, and an imported/recorded parity
report if the scenario is promoted into the shared matrix.

## Completion Criteria

Baseline testing is complete enough for the next product decision when:

- `claude-code` and `codex` artifacts exist under `target/external-runs/`;
- both artifacts are imported into `evalsets/external_baselines/`;
- `/eval baseline-validate all` has no pass-evidence defects;
- `/eval parity-record all` writes a report under `target/eval-reports/`;
- every pass has transcript/evidence notes;
- every blocked scenario has a concrete missing-capability or missing-fixture
  reason;
- `docs/PROJECT_STATUS.md` is updated only after the above evidence is current.

## Do Not Do

- Do not claim parity from local deterministic replay alone.
- Do not mark a scenario pass because the external agent says it succeeded.
- Do not hand-write imported YAML rows when the artifact/import path can be
  used.
- Do not mix implementation fixes into the first baseline run. Capture the
  empirical gap first, then patch based on the report.
