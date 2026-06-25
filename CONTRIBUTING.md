# Contributing
Status: Current

Priority Agent is a Rust programming-agent CLI with local tools, memory,
permissions, LabRun orchestration, and a desktop workbench. Contributions should
preserve the hard runtime contracts that keep the agent auditable.

## Setup

```bash
git clone https://github.com/georgexu521/rust-gene.git
cd rust-gene
cargo check -q
```

For desktop work, also install the package manager dependencies under
`apps/desktop/` as described by the desktop package files.

## Development Rules

- Keep source changes focused and consistent with the existing module boundary.
- Keep non-test production Rust files below the project line-size ceiling.
- Prefer deterministic runtime checks, tool contracts, permissions, and evidence
  gates over always-on prompt rules.
- Do not weaken validation, permissions, isolated-worktree checks, scope checks,
  or closeout proof to make a weaker provider pass.
- Do not commit secrets, generated local databases, provider keys, or private
  workspace artifacts.
- Update `docs/PROJECT_STATUS.md` when a change affects release readiness,
  validation posture, LabRun behavior, startup behavior, or public workflow.

## Validation

Use the narrowest relevant test first, then broaden when shared contracts moved:

```bash
cargo fmt --check
cargo check -q
cargo test -q
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

For docs and release-facing changes:

```bash
git diff --check
bash scripts/validate_docs.sh
cargo doc --workspace --all-features --no-deps
```

For LabRun security and workflow changes, include the relevant targeted suites:

```bash
cargo test -q lab::validation --lib -- --test-threads=1
cargo test -q lab::orchestrator --lib -- --test-threads=1
cargo test -q lab::commands --lib -- --test-threads=1
cargo test -q action_review --lib -- --test-threads=1
```

## Pull Requests

Each PR should include:

- a concise description of the behavior change;
- validation commands actually run;
- any known limitations or follow-up work;
- documentation updates when the change affects user-visible behavior.

Security-sensitive PRs should also state whether the change affects secrets,
shell/package execution, MCP/plugin boundaries, permission decisions, LabRun
evidence, or release automation.
