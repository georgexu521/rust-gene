# Personal Agent Product Principles

Date: 2026-05-18

## Core Thesis

Priority Agent should not try to win by becoming a broad, generic clone of
Claude Code, Codex, opencode, or any other general-purpose coding agent.

The project is worth continuing because it can become a narrow, deep,
personalized, and verifiable programming assistant for gex's local machine,
projects, habits, and workflows.

Working principle:

> 值得继续，但不要卷“大而全”。要做窄、深、个人化、可验证。
> 大厂会赢通用入口；我们要赢的是“它是不是最懂 gex、最能在 gex 的机器和项目里稳定工作”。

## What This Means

- Do not compete on generic entrypoint breadth. Large vendors will usually win
  on distribution, model access, default UX, pricing, and ecosystem reach.
- Compete on personal fit: gex's projects, local tools, memory, validation
  habits, risk tolerance, and coding workflow.
- Treat model providers as replaceable backends. The durable value should live
  in the local runtime, evidence system, memory, skills, repair loops, and evals.
- Prefer real coding reliability over feature count. A feature is valuable when
  it helps the agent inspect, edit, validate, repair, explain, or close out real
  code changes more reliably.
- Keep the product verifiable. Use real-project gauntlets, deterministic tests,
  required validation, trace-backed evidence, and durable tool records rather
  than relying on vibes or demo-only success.

## Decision Filter

Before adding or prioritizing a feature, ask:

- Does this make Priority Agent better for gex's real programming work?
- Does this improve local reliability, validation, memory, repair, or evidence?
- Can we test it with deterministic checks or live coding gauntlets?
- Is this provider-neutral, or are we locking ourselves unnecessarily to one
  model/vendor?
- Is this narrow and deep enough to maintain, or is it generic-product bloat?

If the answer is unclear, prefer the smaller slice that strengthens the coding
reliability loop.
