# External Baseline Files

Place Claude Code, Codex, or other external-agent baseline results here as
YAML or JSON files. `/eval baseline [provider|all]` loads this directory and
compares each provider against the Phase 12 deterministic scenario ids.
`/eval baseline-validate [provider|all]` checks the same files for missing
required scenarios, duplicate/unknown ids, placeholder evidence, and pass
records that lack validation/evidence-backed fields.
`/eval parity [provider|all]` combines local replay readiness with imported
external outcomes so each Phase 12 scenario shows its provider-specific gap.
`/eval parity-record [provider|all]` writes that same report to
`target/eval-reports/` as a timestamped artifact.

Create a not-run template with:

```text
/eval baseline-template claude-code claude-opus
/eval baseline-write claude-code claude-opus
```

Import a run artifact without hand-writing YAML:

```text
python3 scripts/external-baseline-artifact.py --provider claude-code --model claude-opus
/eval baseline-import target/external-runs/claude-code.md claude-code claude-opus
```

`scripts/external-baseline-artifact.py` creates a Markdown run-record skeleton
for the six Phase 12 scenarios, including per-scenario run cards and minimum
evidence notes. Fill the table from a real external-agent transcript, then
import it. The importer also accepts an existing baseline YAML/JSON file or any
Markdown table with `id`/`scenario` and `outcome`/`result` columns. Optional
columns include `validation`, `evidence_backed`, `tool_calls`, `repair_turns`,
`notes`, and `evidence`.

Minimal shape:

```yaml
provider: claude-code
generated_at: "2026-05-21T12:00:00Z"
model: claude-opus
source: manual run notes or artifact path
scenarios:
  - id: file_edit_rewind
    outcome: pass # pass | fail | blocked | not_run
    validation_passed: true
    final_evidence_backed: true
    tool_calls: 4
    repair_turns: 0
    evidence: "edited, tested, rewound"
```

Required Phase 12 ids:

- `file_edit_rewind`
- `bash_background_task`
- `permission_denial_retry`
- `compaction_boundary`
- `subagent_worktree_worker`
- `mcp_auth_repair`
