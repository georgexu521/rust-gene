# Test Lanes And Slow-Test Baseline

Date: 2026-05-29

## Initial Baseline

Measured on the local `claude-test` checkout after the streaming integration
test fix.

| Lane | Command | Result | Wall time |
|---|---|---:|---:|
| workflow | `cargo test -q workflow` | 146 passed | 1.22s |
| memory | `cargo test -q memory` | 216 passed | 162.53s |
| streaming | `cargo test -q streaming` | 16 lib tests + 2 integration tests passed | 159.35s |
| full | `cargo test -q` | 2079 + 3 + 2 passed | 634.71s |

Representative exact-test probes:

| Test | Command | Wall time |
|---|---|---:|
| memory flush async | `cargo test -q memory::manager::tests::test_flush_with_reason_async_records_completed -- --exact` | 0.84s |
| memory doctor JSON | `cargo test -q tools::memory_tool::tests::test_memory_doctor_json_includes_calibration_and_gates -- --exact` | 60.72s |
| streaming history persistence | `cargo test -q engine::streaming::tests::streaming_history_does_not_persist_completed_tool_calls_as_final_assistant_calls -- --exact` | 141.12s |

## Optimized Baseline

After isolating memory-doctor formatting tests from live memory-store scans,
changing the streaming history test to use a read-only file tool path, and
isolating the project-partner closeout test from real local progress/proposal
stores:

| Lane | Command | Result | Wall time |
|---|---|---:|---:|
| memory | `cargo test -q memory` | 216 passed | 0.80s |
| streaming | `cargo test -q streaming` | 16 lib tests + 2 integration tests passed | 0.44s |

Representative exact-test probes:

| Test | Command | Wall time |
|---|---|---:|
| memory doctor text | `cargo test -q tools::memory_tool::tests::test_format_memory_doctor_includes_conflicts_and_counts -- --exact` | 0.69s |
| memory doctor JSON | `cargo test -q tools::memory_tool::tests::test_memory_doctor_json_includes_calibration_and_gates -- --exact` | 0.23s |
| project-partner closeout memory proposal | `cargo test -q engine::conversation_loop::closeout_controller::tests::project_partner_profile_surfaces_review_only_memory_proposal -- --exact` | 0.69s |
| streaming history persistence | `cargo test -q engine::streaming::tests::streaming_history_does_not_persist_completed_tool_calls_as_final_assistant_calls -- --exact` | 0.29s |

Interpretation: the targeted exact slow tests are now fast-lane compatible.
`streaming` is no longer a slow lane, and `memory` is now a practical
touched-module gate. The root cause of the remaining memory slow tail was one
closeout test reading/writing real local project progress and memory proposal
stores; tests should use path-specific store overrides instead of changing
`HOME`.

## Daily Fast Lane

Use:

```bash
bash scripts/test-fast-lane.sh
```

This runs:

```bash
cargo fmt --check
cargo check -q
cargo clippy --all-features -- -D warnings
cargo test -q workflow
cargo test -q --test streaming_query
cargo check --features experimental-api-server -q
```

The intent is to catch formatting, type, lint, workflow-runtime, streaming
integration, and API feature regressions without paying the full 10-minute test
cost on every small edit.

## Slow Lane Profile

Use:

```bash
bash scripts/profile-test-lanes.sh
```

The script writes a timestamped report under `target/test-lane-profiles/` and
profiles:

```bash
cargo test -q workflow
cargo test -q memory
cargo test -q streaming
cargo test -q tools::memory_tool::tests::test_format_memory_doctor_includes_conflicts_and_counts -- --exact
cargo test -q tools::memory_tool::tests::test_memory_doctor_json_includes_calibration_and_gates -- --exact
cargo test -q engine::conversation_loop::closeout_controller::tests::project_partner_profile_surfaces_review_only_memory_proposal -- --exact
cargo test -q engine::streaming::tests::streaming_history_does_not_persist_completed_tool_calls_as_final_assistant_calls -- --exact
```

Refresh this baseline before changing test architecture, timeouts, memory
doctor behavior, or streaming history persistence.

## Refactor Gate Policy

For `memory/manager.rs` extraction:

1. Run `bash scripts/test-fast-lane.sh` before the first extraction.
2. Extract one low-coupling slice at a time, starting with pure data types or
   standalone ranking/scoring helpers.
3. After each slice, run the narrowest relevant tests:
   `cargo test -q memory::manager::<test_name> -- --exact` for moved exact
   tests, then `cargo test -q memory` before widening.
4. Keep `cargo test -q memory` as the touched-module gate; it is now fast enough
   to run after each memory extraction.

For `tools/file_tool/mod.rs` extraction:

1. Start with state/path/diff helper modules that have clear boundaries.
2. Keep the `Tool` trait impls as the orchestration layer until helper
   boundaries are stable.
3. Use targeted tests such as `cargo test -q file_tool`, then `bash
   scripts/test-fast-lane.sh`.

Avoid adding a new high-level conversation-loop director until a concrete
controller boundary proves it will remove state coupling. The current bottleneck
is file size and slow-test feedback, not the absence of another orchestration
abstraction.
