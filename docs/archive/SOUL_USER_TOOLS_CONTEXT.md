# Root Context Files
Status: Reference

Priority Agent supports three optional project-root context files beside
`AGENTS.md`:

- `SOUL.md`
- `USER.md`
- `TOOLS.md`

These files are loaded after `AGENTS.md` and are supplemental context only.
They cannot override runtime, sandbox, permission, validation, checkpoint, or
tool-safety rules.

## File Roles

`AGENTS.md` remains the place for runtime and project operating constraints.
Only the `## Agent Runtime Guidance` section is prompt-injected by default when
that section exists.

`SOUL.md` is for assistant voice, tone, communication style, judgment posture,
and personality. Keep it short. It should not contain security policy, project
status, test gates, tool contracts, or long history.

`USER.md` is for compact, stable user-profile facts and collaboration
preferences. It is not a replacement for memory retrieval. Evolving facts,
project-specific observations, and evidence-backed preferences should still go
through the memory system.

`TOOLS.md` is for stable project-local tool hints and command conventions.
Avoid volatile status, one-off command output, and stale environment notes.

## Load Order

The rendered prompt order is:

1. Base system prompt
2. Workspace boundary
3. Layered `AGENTS.md` instructions
4. Supplemental `SOUL.md`
5. Supplemental `USER.md`
6. Supplemental `TOOLS.md`

If `AGENTS.md` is absent, the supplemental files can still be loaded, but they
remain contextual and non-authoritative.

## Budgets

Each supplemental file is capped at a compact per-file budget. The supplemental
section also has a total budget across all three files. Oversized files are
truncated and reported in prompt-context diagnostics.

## Recommended Shape

Good `SOUL.md` content:

```markdown
# SOUL

- Be direct and concrete.
- Prefer execution over advice-only replies when intent is clear.
- Surface uncertainty and validation gaps plainly.
```

Good `USER.md` content:

```markdown
# USER

- gex prefers staged implementation with visible progress.
- gex wants durable repo docs for broad project comparisons.
```

Good `TOOLS.md` content:

```markdown
# TOOLS

- Use `cargo test -q instructions` for instruction-loader changes.
- Use `cargo test -q prompt_context` for prompt assembly changes.
```

Bad fits:

- One-off task progress
- Long product history
- Secrets or credentials
- Prompt-injection-like instructions
- Runtime safety policy that belongs in `AGENTS.md`
