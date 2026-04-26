---
name: karpathy-guidelines
description: "Behavioral guidelines for careful coding: think before coding, keep changes simple, edit surgically, and define verifiable success criteria."
version: "1.0.0"
author: "forrestchang / adapted for Priority Agent"
triggers:
  - code
  - refactor
  - review
  - simplify
  - verify
  - debugging
---

# Karpathy Guidelines

Source: `andrej-karpathy-skills-main`, MIT licensed. These guidelines are adapted as a bundled Priority Agent skill so they can be applied during code writing, review, refactoring, and verification.

## 1. Think Before Coding

Do not assume, hide confusion, or silently choose between ambiguous interpretations.

- State assumptions explicitly.
- Ask when requirements are unclear.
- Surface tradeoffs when more than one approach is plausible.
- Push back when a simpler or safer approach is available.

## 2. Simplicity First

Use the minimum code that solves the problem.

- Do not add features beyond the request.
- Do not create abstractions for single-use code.
- Do not add speculative configurability.
- Prefer the smaller implementation when it is equally correct.
- If a change is much larger than the problem, simplify it before finishing.

## 3. Surgical Changes

Touch only what the task requires.

- Do not rewrite adjacent code as a side effect.
- Do not refactor unrelated modules.
- Match the existing local style.
- Clean up imports, variables, or helpers introduced by your own change.
- Mention unrelated cleanup opportunities instead of silently doing them.

Every changed line should trace back to the user request.

## 4. Goal-Driven Execution

Convert tasks into verifiable goals.

- For bug fixes, reproduce or identify the failing behavior before claiming it is fixed.
- For new behavior, name the acceptance checks.
- For refactors, verify behavior before and after when feasible.
- For multi-step work, pair each step with a check.

Example:

```text
1. Inspect current behavior -> verify by reading the relevant tests or code path.
2. Make the smallest targeted change -> verify with focused tests.
3. Run the final regression check -> verify no unrelated behavior changed.
```

## When To Apply

Use this skill for non-trivial code changes, review, refactoring, debugging, architecture cleanup, and any task where overengineering or unintended edits would be costly.

For trivial one-line fixes, apply the spirit of the rules without adding process overhead.
