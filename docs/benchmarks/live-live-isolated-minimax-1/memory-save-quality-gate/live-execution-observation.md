# Live Execution Observation: live-isolated-minimax-1

- Case: `memory-save-quality-gate`
- Worktree: `target/live-evals/live-isolated-minimax-1/memory-save-quality-gate/worktree`
- Outcome: aborted before code changes

## Result

The isolated runtime state worked, but the model still attempted to inspect the
main checkout through absolute shell paths:

```text
/Users/georgexu/Desktop/rust-agent/src/memory/manager.rs
/Users/georgexu/Desktop/rust-agent/src/memory/quality.rs
```

The eval worktree itself stayed clean, and the main checkout was not modified.
This showed that session/memory isolation is necessary but not sufficient:
tool permissions also need to reject or ask before shell commands reference
paths outside the active workspace.

## Follow-Up Fix

`PermissionContext` now treats bash commands that reference absolute paths
outside the trusted workspace as high risk. In `AutoAll`, those commands require
confirmation instead of silently executing. This closes the specific path leak
seen in this run while keeping normal relative workspace commands fast.

## Validation

Added and passed a focused permission regression:

```text
cargo test -q permissions::tests::test_auto_all_prompts_for_bash_outside_workspace_paths -- --test-threads=1
```

Also passed the full permission test module:

```text
cargo test -q permissions::tests:: -- --test-threads=1
```
