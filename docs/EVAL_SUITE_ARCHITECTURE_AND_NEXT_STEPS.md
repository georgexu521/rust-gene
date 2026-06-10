# Eval Task Suite Architecture & Next Steps
Status: Active

Date: 2026-06-05

## Current State

The project currently has **40+ live eval tasks** covering:

- **Backend tasks**: `backend-todo-api-crud` (Python HTTP API)
- **Frontend tasks**: `frontend-book-notes-localstorage` (JS + localStorage)
- **Core capabilities**: file editing, bash execution, terminal handling, permission recovery
- **Memory & learning**: save/recall, conflict resolution, quality gates, skill promotion
- **MVA (Minimum Viable Agent)**: direct answer, high-risk block, light inspection, loops
- **Runtime spine**: worktree isolation, subagent verification, test failure repair
- **Product maturity**: session resume, dashboard summaries, UI polish

**Observation**: Coverage is comprehensive but lacks a **systematic layering**. Tasks are organized by feature area rather than by verification depth.

---

## Proposed Architecture: 5-Tier Coverage Model

Organize eval tasks by **skill dimension × difficulty dimension**:

```
evalsets/
├── tier-1-foundations/          # Tool health verification
│   ├── tool-file-read.yaml
│   ├── tool-bash-execution.yaml
│   ├── tool-grep-search.yaml
│   └── tool-file-edit-basic.yaml
│
├── tier-2-single-file/          # Single-file modifications
│   ├── fix-bug-python.yaml
│   ├── fix-bug-rust.yaml
│   ├── add-feature-js.yaml
│   └── add-test-rust.yaml
│
├── tier-3-multi-file/           # Cross-file coordination
│   ├── backend-todo-api-crud.yaml
│   ├── frontend-book-notes.yaml
│   └── rust-multi-file-refactor.yaml
│
├── tier-4-integration/          # End-to-end complex tasks
│   ├── fullstack-mini-app.yaml
│   ├── data-processing-pipeline.yaml
│   └── cli-tool-development.yaml
│
└── tier-5-edge-cases/           # Failure modes & boundaries
    ├── permission-recovery.yaml
    ├── memory-conflict.yaml
    └── stale-edit-repair.yaml
```

**Why tiers?**
- Faster feedback loops: run tier-1 for quick sanity, tier-3 for PR gates
- Clearer regression attribution: if tier-1 fails, it's a tool issue; if tier-4 fails, it's planning/coordination
- Easier onboarding: new contributors understand scope by tier, not by hunting 40+ files

---

## Three Immediate Actions

### Action 1: Add Rust Self-Modification Tasks (High Priority)

The product is written in Rust, but existing tasks rarely use Rust as the **subject**. This is a gap: the agent should be able to modify its own codebase.

**Suggested tasks**:

| Task | Description | Validation |
|------|-------------|------------|
| `rust-add-cli-flag` | Add a new CLI argument to `src/main.rs` | `cargo check` passes + flag visible in `--help` |
| `rust-refactor-error-handling` | Extract repeated error patterns into a helper | `cargo clippy` clean + tests pass |
| `rust-add-unit-test` | Write tests for an untested module | `cargo test` passes + coverage increases |
| `rust-update-dependency` | Bump a crate version and fix breakage | `cargo check` passes + lockfile updated |

**Rationale**: These test whether the agent understands its own project structure, build system, and idioms.

### Action 2: Create "Daily Development" Task Group

Tasks that mirror **real developer workflows**:

| Task | Type | Validation |
|------|------|------------|
| `review-pr-diff` | Review a patch and identify issues | Checklist: bugs found, style issues, security concerns |
| `add-documentation` | Write rustdoc for a public API | `cargo doc` passes + examples compile |
| `port-script-to-rust` | Translate a Python script to Rust | Output equivalence + performance check |
| `write-readme` | Generate project README | Contains required sections + markdown lint |
| `debug-test-failure` | Diagnose and fix a failing test | Root cause identified + fix applied + test passes |
| `add-logging-tracing` | Instrument a module with `tracing` | Logs structured + compile passes |

**Rationale**: Eval should measure utility for real work, not just synthetic puzzles.

### Action 3: Implement Progressive Validation

Instead of running all 40+ tasks every time, select by tier:

```bash
# Daily development: ~5 minutes
./scripts/eval-run.sh tier-1-foundations

# Pre-PR gate: ~15 minutes
./scripts/eval-run.sh tier-1-foundations tier-2-single-file

# Release candidate: ~1 hour
./scripts/eval-run.sh all
```

**Implementation**:
- Add a `tier` field to each task YAML
- Create `eval-run.sh` that accepts tier filters
- Generate a summary report: tier pass rates, regression detection

---

## Next Task Candidates (Pick One)

Based on the above, here are four concrete next tasks to implement:

### Option A: Rust Self-Modification
**Task**: `rust-add-cli-flag`
- Add `--verbose` flag to `src/main.rs`
- Update help text
- Validation: `cargo check`, `cargo test`, `--help` shows flag

### Option B: Data Processing
**Task**: `data-csv-analysis`
- Given a CSV file, compute statistics and generate a report
- Validation: output files exist + content matches expected

### Option C: Documentation Generation
**Task**: `doc-generate-readme`
- Read source code and generate project README
- Validation: markdown structure, section completeness

### Option D: Debug Diagnostic
**Task**: `debug-failing-test`
- Given a failing test output, identify and fix the bug
- Validation: test passes after fix

---

## Metrics to Track

For each eval run, collect:

- **Tool call accuracy**: did it use the right tool for the job?
- **Step efficiency**: how many turns to completion?
- **Repair rate**: how often did it need to self-correct?
- **Closeout honesty**: did it report `not_verified` when appropriate?
- **Cost**: token usage per task tier

---

## Appendix: Existing Task Inventory

For reference, the current live_tasks directory contains:

**Backend/Frontend**:
- `backend-todo-api-crud.yaml`
- `frontend-book-notes-localstorage.yaml`

**Core Capabilities**:
- `core-*` tasks (multi-file edit, terminal, provider roundtrip, rollback, stale edit, etc.)
- `cli-scrollback-polish.yaml`

**Memory & Learning**:
- `memory-*` tasks (save, recall, conflict, quality gate, promotion, etc.)
- `skill-promotion-gate.yaml`

**MVA (Minimum Viable Agent)**:
- `minimum-agent-*` tasks (direct answer, high-risk block, light inspection, loop, etc.)

**Runtime Spine**:
- `runtime-spine-p0b-*` tasks (worktree, memory retrieval, permission, route mistake, skill guidance, subagent, test repair)

**Project Partner**:
- `project-partner-*` tasks (failure memory, resume, vague local tool)

**Product Features**:
- `live-eval-dashboard-summary.yaml`
- `desktop-ui-smoke-polish.yaml`
- `resume-session-picker.yaml`

**Permissions & Safety**:
- `permission-default-open-dangerous-guard.yaml`
- `core-permission-rejection-recovery.yaml`
- `memory-save-sensitive-hard-block.yaml`

**Workflow & Verification**:
- `code-change-verification-repair-loop.yaml`
- `core-rollback-product-path.yaml`

---

*This document should be updated as new tiers are implemented and metrics are collected.*
