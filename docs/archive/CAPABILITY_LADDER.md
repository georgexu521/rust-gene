# Capability Ladder

Date: 2026-06-02

Status: implemented

## Overview

The capability ladder defines 6 levels of coding agent capability. Each level
builds on the previous one. Live evals are classified by level to track
progress.

## Levels

| Level | Name | Description | Example Tasks |
|-------|------|-------------|---------------|
| 1 | Inspect and Explain | Read-only inspection of codebase | `core-inspection-grounding` |
| 2 | Project Memory Grounding | Resume from local project memory and prior execution evidence | `project-partner-resume-with-memory`, `memory-recall-conflict-precision` |
| 3 | Stale Edit Repair | Recover from stale read conflicts | `core-simple-stale-edit` |
| 4 | Multi-File Refactor | Change multiple files coherently | `core-multi-file-edit`, `core-rust-multi-file-refactor` |
| 5 | Validation Failure Repair | Recover from failed validation | `code-change-verification-repair-loop` |
| 6 | Long Task with Honest Closeout | Complex task with partial/failure states | `minimum-agent-verification-repair` |

## Classification

Curated daily-gate live task YAML should include `capability_level` field:

```yaml
id: core-inspection-grounding
capability_level: 1
```

## Acceptance

- Each level has at least one stable task
- Level 3+ tasks must have stale-read recovery evidence
- Level 5+ tasks must have validation failure repair evidence
- Level 6 tasks must have honest partial/failure closeout

## Validation

```bash
cargo test -q scenario_matrix
bash scripts/product-daily-gate.sh --dry-run
```
