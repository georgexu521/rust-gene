# External Baseline Files

Place Claude Code, Codex, or other external-agent baseline results here as
YAML or JSON files. `/eval baseline [provider|all]` loads this directory and
compares each provider against the Phase 12 deterministic scenario ids.

Create a not-run template with:

```text
/eval baseline-template claude-code claude-opus
/eval baseline-write claude-code claude-opus
```

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
