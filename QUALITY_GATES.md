# Quality Gates

> Last updated: 2026-06-25
> Purpose: Define the validation gates required before merging, release
> candidates, and production-style dogfood runs.

## Gate Overview

| Gate | Scope | Required command or proof |
|------|-------|---------------------------|
| **G0 Format** | Rust formatting and whitespace hygiene | `cargo fmt --check` and `git diff --check` |
| **G1 Build** | Workspace builds across supported feature sets | `cargo check --workspace --all-targets --all-features` plus focused feature checks when touched |
| **G2 Tests** | Unit and integration tests | `cargo test --workspace --all-features -- --test-threads=1` |
| **G3 Lints** | Clippy warnings treated as errors | `cargo clippy --workspace --all-targets --all-features -- -D warnings` |
| **G4 Docs** | Public docs and repo docs stay coherent | `cargo doc --workspace --all-features --no-deps` and `bash scripts/validate_docs.sh` |
| **G5 Runtime Proof** | Agent/tool/closeout behavior is evidence-backed | focused runtime tests or dogfood transcript with commands, diff, and proof status |

## Pull Request Gate

Run this before merging ordinary code changes:

```bash
cargo fmt --check
cargo check --workspace --all-targets --all-features
cargo test --workspace --all-features -- --test-threads=1
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo doc --workspace --all-features --no-deps
bash scripts/validate_docs.sh
git diff --check
```

For narrow changes, run the smallest matching test first, then run the full gate
before closeout if the change touches shared runtime contracts, tools,
permissions, provider behavior, CI, packaging, or public docs. The full
workspace test gate runs with `--test-threads=1` because multiple existing tests
mutate process environment variables through `EnvVarGuard`.

## Release Candidate Gate

Run the pull request gate, then add:

```bash
cargo build --release --workspace --all-features
./target/release/priority-agent --help
cargo check --features experimental-api-server -q
cargo check --features legacy-cli -q
bash scripts/workflow-production-gates.sh
bash scripts/security_dependency_audit.sh
```

Release artifacts must include the built `priority-agent` binary, public setup
docs, and a checksum generated from the packaged archive.

`scripts/security_dependency_audit.sh` requires local `cargo-audit` and
`cargo-deny` installations. If those tools are unavailable on a developer
machine, record the skip explicitly and run the script before tagging a formal
release candidate. The check is intentionally non-noisy: it does not create
dependency-update branches or PRs.

## Runtime And Agent Dogfood Gate

Use this gate when changing routing, tool scope, permission review, validation
proof, closeout, memory, subagents, or recovery:

```bash
cargo test -q route_scoped_tools
cargo test -q closeout
cargo test -q prompt_context
cargo test -q instructions
bash -n scripts/run_live_eval.sh
python3 -m py_compile scripts/live_eval_report_parser.py
```

For release dogfood, keep a record of:

- exact binary or command used;
- model/provider and tool scope;
- requested task;
- commands executed by the agent;
- files changed;
- validation proof;
- final closeout status: `verified`, `partial`, `failed`, or `not_verified`.

Do not mark a runtime run as verified unless proof exists in commands,
artifacts, or trace data. A weak-provider failure should be classified honestly
instead of hidden by weaker gates.

## Desktop And TUI Gate

Desktop packaged validation is an RC-level gate, not the main dogfood surface.
Run it after CLI agent/tool/verification gates are green.

```bash
cargo run -- --tui --help
```

For desktop packaging changes, also run the relevant desktop build or dev-mode
command from `apps/desktop` and capture whether the app can launch, open a
conversation, submit a prompt, display tool progress, and show final proof.

## CI Requirements

CI must not contain placeholder gate reports. A green report line is only valid
when the matching command ran in the same job and exited successfully.

The default CI gate must cover:

- workspace all-features check;
- workspace all-features tests;
- workspace all-targets/all-features clippy with warnings denied;
- docs generation;
- repo docs validation;
- non-noisy dependency and license checks before formal release candidates;
- whitespace diff validation;
- release artifact packaging on `main` and `v*` tags.

## Failure Policy

- Stop release or merge closeout when a required gate fails.
- Fix the failure, narrow it to an owner, or explicitly document why a skipped
  gate does not apply.
- Do not weaken permissions, validation, checkpoint, or high-risk gates to make
  an eval look green.
- If a gate is flaky, rerun once with the same command and record both results.
