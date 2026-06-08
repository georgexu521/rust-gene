#!/usr/bin/env bash
set -euo pipefail

# Focused regression gate for the weighting-system audit follow-ups.
# This is intentionally narrow: it proves the executable coverage that exists
# today without pretending the full 15-case live-eval calibration suite exists.

cargo test -q candidate_action -- --test-threads=1
cargo test -q task_guidance -- --test-threads=1
cargo test -q memory::manager -- --test-threads=1
cargo test -q memory_tool -- --test-threads=1
cargo test -q retrieval_context -- --test-threads=1
cargo test -q trace_summary_includes_scoring_summary -- --test-threads=1
