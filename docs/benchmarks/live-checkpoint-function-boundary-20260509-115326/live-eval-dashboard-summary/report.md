# Live Eval Report: live-eval-dashboard-summary

- Run id: `checkpoint-function-boundary-20260509-115326`
- Sample: `evalsets/live_tasks/live-eval-dashboard-summary.yaml`
- Worktree: `target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env`
- Test status: `failed`
- Generated: `2026-05-09 11:58:30 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ bash -n scripts/run_live_eval.sh
[exit status: 0]

$ scripts/run_live_eval.sh --list
id                                   type         eval_intent                risk       title
--                                   ----         -----------                ----       -----
backend-todo-api-crud                feature      seeded_code_change         medium     implement a tiny stdlib todo API backend
cli-scrollback-polish                ux           seeded_code_change         medium     interactive CLI should feel smooth and readable
code-change-verification-repair-loop feature      seeded_code_change         high       failed verification should trigger repair before closeout
frontend-book-notes-localstorage     feature      seeded_code_change         medium     build a small book notes frontend with search, tags, and persistence
live-eval-dashboard-summary          feature      seeded_code_change         medium     live eval reports should summarize pass rates and failure modes
memory-recall-conflict-precision     bug_fix      audit_or_regression_check  high       memory recall should demote only relevant conflicts
memory-save-duplicate-demotion       bug_fix      audit_or_regression_check  medium     duplicate memory candidates should not pollute long-term memory
memory-save-quality-gate             bug_fix      seeded_code_change         high       memory_save should respect quality gates
memory-save-sensitive-hard-block     bug_fix      audit_or_regression_check  high       explicit memory saves must not persist sensitive data
permission-default-open-dangerous-guard bug_fix      audit_or_regression_check  high       default-open permissions should still guard destructive operations
persistent-memory-planning-context   bug_fix      seeded_code_change         high       persistent memory should affect workflow planning
resume-session-picker                feature      seeded_code_change         medium     interactive CLI should support Claude-style resume
skill-promotion-gate                 bug_fix      seeded_code_change         medium     skill apply should require promotion evidence
[exit status: 0]

$ scripts/run_live_eval.sh --mode summary --run-id live-summary-smoke
summary mode is not implemented yet
[exit status: 2]

$ cargo test -q -- --test-threads=1
error: could not compile `priority-agent` (bin "priority-agent" test)

Caused by:
  process didn't exit successfully: `/Users/georgexu/.rustup/toolchains/stable-aarch64-apple-darwin/bin/rustc --crate-name priority_agent --edition=2021 src/main.rs --error-format=json --json=diagnostic-rendered-ansi,artifacts,future-incompat --emit=dep-info,link -C embed-bitcode=no -C debuginfo=2 -C split-debuginfo=unpacked --test --cfg 'feature="default"' --check-cfg 'cfg(docsrs,test)' --check-cfg 'cfg(feature, values("default", "experimental-api-server", "experimental-platform", "experimental-priority", "experimental-task-analyzer", "voice"))' -C metadata=94b1db201c3cb5f7 -C extra-filename=-4e8ff203fb792cce --out-dir /Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps -C incremental=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/incremental -L dependency=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps --extern anyhow=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libanyhow-d9b6d410a1361442.rlib --extern arboard=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libarboard-5902728b3a2db187.rlib --extern async_openai=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libasync_openai-602d027a96cd09f9.rlib --extern async_trait=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libasync_trait-4d24afdfaf5578bb.dylib --extern axum=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libaxum-a398193648774880.rlib --extern base64=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libbase64-a2c323877c4533ad.rlib --extern chrono=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libchrono-f3b3ffcecf406e39.rlib --extern clap=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libclap-353c7536cca94d47.rlib --extern colored=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libcolored-22412d2f9f9df7ff.rlib --extern config=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libconfig-5dbc3fa69a0e40bb.rlib --extern crossterm=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libcrossterm-08f3feaa7509038f.rlib --extern diffy=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libdiffy-aeb695d7256e05a9.rlib --extern dirs=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libdirs-3908dcf7fcebfb2c.rlib --extern dotenvy=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libdotenvy-3ea06de9f43f2b91.rlib --extern ed25519_dalek=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libed25519_dalek-e19f41cd5d3dc0af.rlib --extern futures=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libfutures-1e918de705fcb6c7.rlib --extern glob=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libglob-56daeb017c976e69.rlib --extern ignore=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libignore-07748edf382c633b.rlib --extern libc=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/liblibc-33cd76effe6caea2.rlib --extern md5=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libmd5-e1ad22998520b70a.rlib --extern once_cell=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libonce_cell-0c666c58f3a8f5f3.rlib --extern priority_core=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libpriority_core-0735650b93c5c9b1.rlib --extern pulldown_cmark=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libpulldown_cmark-ad611b4d4ef5245a.rlib --extern rand=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/librand-709425f0f99abd1b.rlib --extern ratatui=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libratatui-15da2388b1524278.rlib --extern regex=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libregex-c514e910beb6173f.rlib --extern reqwest=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libreqwest-b1b8aa25ec086c1c.rlib --extern rusqlite=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/librusqlite-5f3b2e616d5a1586.rlib --extern rustyline=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/librustyline-86623963c39baa3a.rlib --extern schemars=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libschemars-d07ffbc39cad48eb.rlib --extern secrecy=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libsecrecy-ea1033fe6bde49f8.rlib --extern serde=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libserde-76adde2a58e75b2c.rlib --extern serde_json=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libserde_json-89b846962c2c390a.rlib --extern serde_yaml=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libserde_yaml-a8f09d028273efda.rlib --extern syntect=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libsyntect-404c7daa13306997.rlib --extern tempfile=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libtempfile-a030ee887a180803.rlib --extern thiserror=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libthiserror-8cfc3d427be4c6d7.rlib --extern tokio=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libtokio-ac05947c3fab3882.rlib --extern tokio_stream=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libtokio_stream-86104e6cd4abf1aa.rlib --extern tokio_tungstenite=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libtokio_tungstenite-b847cc82a27175d6.rlib --extern tokio_util=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libtokio_util-4ea4945cca78b849.rlib --extern toml=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libtoml-455a339677b1ea3f.rlib --extern tower=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libtower-626c852c83210469.rlib --extern tower_http=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libtower_http-a6a79032ad7fb97c.rlib --extern tracing=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libtracing-62e003b56447a530.rlib --extern tracing_subscriber=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libtracing_subscriber-b049434673a04625.rlib --extern tree_sitter=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libtree_sitter-e3ed2d137c35824a.rlib --extern tree_sitter_python=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libtree_sitter_python-d18e77453fbca420.rlib --extern tree_sitter_rust=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libtree_sitter_rust-ae1c2b04e3fd3358.rlib --extern tree_sitter_typescript=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libtree_sitter_typescript-b90cf950f60b42e7.rlib --extern unicode_width=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libunicode_width-db0be247b374c363.rlib --extern uuid=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/deps/libuuid-951e672d8e292831.rlib -L native=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/build/ring-14e42aa85aec03b5/out -L native=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/build/libsqlite3-sys-cf6ca3574372e939/out -L native=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/build/tree-sitter-c84ac39cd1a9cfa0/out -L native=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/build/tree-sitter-python-31c35ba1c4a52548/out -L native=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/build/tree-sitter-rust-2525fdfff140ca44/out -L native=/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/env/cargo-target/debug/build/tree-sitter-typescript-8f79e325afbccec8/out` (signal: 15, SIGTERM: termination signal)
[exit status: 101]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/agent-output.md`
- Events: `docs/benchmarks/live-checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 2
tool_execution_complete: 6
tool_execution_start: 6
trace_summary: 1
```

Quality signals:

```text
output_chars: 1093
diff_chars: 0
tool_executions: 6
first_write_tool_index: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 57
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: not_verified
runtime_diet: prompt=5650 tool_schema=2641 tools=12 workflow=guarded closeout=full validation=not_verified
adaptive_triggers: required_validation,repeated_no_code_progress
trace_event_types: memory.sync,api.start,workflow.fallback,api.done,tool.start,tool.done,workflow.fallback,workflow.fallback,workflow.fallback,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: true
patch_synthesis_no_change: false
eval_intent: seeded_code_change
warning: no_code_diff
warning: action_checkpoint_invalid_tools
warning: required_commands_not_passing
warning: closeout_not_successful
failure_owner: agent_flow
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: false
guided_reasoning_active: false
weighted_planning_active: true
closeout_active: false
adaptive_workflow_active: true
active_specialty_signals: 4/7
memory_sync_events: 3
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 4
required_command_status: failed
validation_events: 0
stage_validation_events: 0
tool_progress_events: 0
guided_debugging_events: 0
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 2
adaptive_triggers: required_validation,repeated_no_code_progress
latest_top_priority: P1
latest_top_importance_score: 0.7549999952316284
latest_top_weight_share: 0.2975369691848755
acceptance_accepted: missing
closeout_status: not_verified
runtime_diet: prompt=5650 tool_schema=2641 tools=12 workflow=guarded
attention: required commands did not pass in the harness
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
2026-05-09T03:55:49.258541Z  WARN priority_agent::engine::conversation_loop: Patch synthesis JSON actions were not directly applicable: synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-function-boundary-20260509-115326/live-eval-dashboard-summary/worktree/scripts/run_live_eval.sh; refusing inexact multi-line replacement; patch synthesis declined without a reason
2026-05-09T03:56:49.751841Z  WARN priority_agent::engine::conversation_loop: Patch synthesis JSON actions were not directly applicable: patch synthesis declined without a reason; patch synthesis declined without a reason
```

## Human Review

- accepted: TODO
- task_success: TODO
- mainline_hit: TODO
- plan_coverage: TODO
- rework_count: TODO
- tool_efficiency: TODO
- diff_discipline: TODO
- closeout_accuracy: TODO
- notes: TODO
