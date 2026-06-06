# Eval Task Registry

This directory contains the tiered evaluation task suite for Priority Agent.

## Task Tiers

### tier-1-foundations (7 tasks)
Tool verification and basic capability tests.

- `tool-file-read` — Verify file_read tool works correctly
- `tool-bash-execution` — Verify bash tool executes commands correctly
- `core-inspection-grounding` — Check inspection and grounding capabilities
- `core-provider-roundtrip` — Test provider API roundtrip
- `core-terminal-install-run` — Test terminal installation and execution
- `minimum-agent-direct-answer` — Minimum agent direct answer without tool use
- `minimum-agent-light-inspection` — Light inspection task

### tier-2-single-file (6 tasks)
Single-file modification tasks.

- `fix-bug-rust` — Fix a simple Rust compilation error
- `rust-add-cli-flag` — Add --verbose flag to CLI
- `core-simple-stale-edit` — Read before focused single-file edit
- `core-multi-file-edit` — Multi-file coordinated edit
- `cli-scrollback-polish` — CLI scrollback polish
- `desktop-ui-smoke-polish` — Desktop UI smoke test polish

### tier-3-multi-file (6 tasks)
Cross-file coordination and integration tasks.

- `backend-todo-api-crud` — Implement a tiny stdlib todo API backend
- `frontend-book-notes-localstorage` — Build a small book notes frontend
- `core-rust-multi-file-refactor` — Rust multi-file refactoring
- `code-change-verification-repair-loop` — Code change verification repair loop
- `live-eval-dashboard-summary` — Live eval dashboard summary
- `persistent-memory-planning-context` — Persistent memory planning context

### tier-4-integration (0 tasks)
End-to-end complex tasks requiring multiple subsystem coordination.

*No tasks yet. Candidates:*
- Full-stack web application creation
- Multi-service system setup
- Complex data pipeline implementation

### tier-5-edge-cases (27 tasks)
Boundary conditions, failure modes, and advanced runtime scenarios.

**Memory Management:**
- `memory-save-quality-gate` — Memory save respects quality gates
- `memory-save-sensitive-hard-block` — Memory save blocks sensitive content
- `memory-save-duplicate-demotion` — Duplicate memory demotion
- `memory-recall-conflict-precision` — Memory recall conflict resolution
- `memory-stale-project-fact-demotion` — Stale project fact demotion
- `memory-failure-lesson-promotion` — Failure lesson promotion to memory

**Permissions & Security:**
- `core-permission-rejection-recovery` — Permission rejection recovery
- `permission-default-open-dangerous-guard` — Dangerous guard detection
- `core-rollback-product-path` — Rollback product path

**MVA (Minimum Viable Agent):**
- `minimum-agent-loop` — MVA loop behavior
- `minimum-agent-high-risk-block` — High-risk block handling
- `minimum-agent-low-value-replan` — Low-value replan avoidance
- `minimum-agent-memory-boundary` — Memory boundary checks
- `minimum-agent-verification-repair` — Verification repair

**Runtime Spine:**
- `runtime-spine-p0b-isolated-worktree-implementer` — Isolated worktree implementer
- `runtime-spine-p0b-memory-retrieval-conflict` — Memory retrieval conflict
- `runtime-spine-p0b-permission-required` — Permission required scenarios
- `runtime-spine-p0b-route-mistake-recovery` — Route mistake recovery
- `runtime-spine-p0b-skill-guidance` — Skill guidance
- `runtime-spine-p0b-subagent-verifier` — Subagent verifier
- `runtime-spine-p0b-test-failure-repair` — Test failure repair

**Project Partner:**
- `project-partner-failure-memory-proposal` — Failure memory proposal
- `project-partner-resume-with-memory` — Resume with memory
- `project-partner-vague-local-tool` — Vague local tool handling

**Other:**
- `core-long-output-artifact` — Long output artifact handling
- `skill-promotion-gate` — Skill promotion gating
- `resume-session-picker` — Session picker resume

## Running Tasks

```bash
# Run a specific tier
./scripts/eval-run.sh tier-1
./scripts/eval-run.sh tier-2
./scripts/eval-run.sh tier-3
./scripts/eval-run.sh tier-5

# Run all tiers
./scripts/eval-run.sh all

# List all tasks
./scripts/eval-run.sh list
```

## Task Structure

Each task is a YAML file with:
- `id`: Unique task identifier
- `title`: Human-readable title
- `type`: Task type (feature, bug_fix, audit, etc.)
- `tier`: Classification tier
- `eval_intent`: Evaluation intent (seeded_code_change, direct_answer, etc.)
- `complexity`: low, medium, high
- `risk`: low, medium, high
- `repo`: Repository fixture and preparation commands
- `prompt`: Task description for the agent
- `allowed_tools`: Permitted tools
- `forbidden_tools`: Prohibited tools
- `acceptance`: Required validation commands

## Adding New Tasks

1. Create task YAML in appropriate `tier-X-*/` directory
2. Include `tier` field in YAML
3. Add task description to this README
4. Test with `./scripts/eval-run.sh tier-N`

## Legacy Tasks

Original live tasks are preserved in `evalsets/live_tasks/` but are now
shadow-copied into tier directories with added `tier` field.
