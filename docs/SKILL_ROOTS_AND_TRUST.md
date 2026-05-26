# Skill Roots And Trust

Last updated: 2026-05-26

Priority Agent treats skills as procedural memory: repeatable workflows,
commands, repair loops, provider quirks, and project operating guides belong in
skills rather than durable user memory. Durable memory should keep facts,
preferences, decisions, and stable environment quirks.

## Root Precedence

`SkillRuntime::load(working_dir)` resolves the workspace root from the current
working directory, then loads skills in this precedence order:

1. Workspace `.agents/skills`
2. Workspace `skills`
3. User-configured skill roots
4. Bundled skills

Higher-precedence skills win on name conflict. This means a workspace
`.agents/skills/fix/SKILL.md` can override the bundled `fix` skill, while a
plain workspace `skills/fix/SKILL.md` cannot override `.agents/skills/fix`.

User-configured roots are:

- `~/.priority-agent/skills`
- paths from `PRIORITY_AGENT_SKILLS_PATH`, split by `:`

Remote skill URLs can be configured with `PRIORITY_AGENT_SKILLS_URL`, split by
comma, semicolon, or whitespace. Remote loading is not bundled into the
synchronous per-turn runtime path; when remote loading is used, it goes through
the same third-party scanner as workspace and user-configured files.

## Allowlist

Set `PRIORITY_AGENT_SKILL_ALLOWLIST` to restrict third-party discovery.

The allowlist accepts comma, semicolon, colon, or whitespace separators. Entries
are normalized by lowercasing, trimming a leading slash, and treating `_` as
`-`.

The loader accepts a skill when either the parsed `name` or the containing
directory name is present in the allowlist. Bundled skills remain available so a
bad allowlist does not remove the built-in recovery surface.

Example:

```bash
PRIORITY_AGENT_SKILL_ALLOWLIST="rust-debug,review-pr"
```

## Scanner

Workspace, user-configured, URL-loaded, created, and patched skills are scanned
before they are loaded or written. Bundled skills are treated as trusted build
assets and are not scanned at runtime.

The scanner rejects obvious promptware, secret-like content, private key
material, destructive shell patterns, network-download pipes into shell,
opaque payload patterns such as `base64 -d`, and dynamic `eval`/`exec` usage.

Rejected skills are skipped and logged with a `skills.load` tracing event. The
loader keeps going so one bad skill does not remove the rest of the skill
surface.

## Metadata

Loaded skills carry source, trust, and load-reason metadata:

- `source`: `bundled`, `workspace_agents`, `workspace`, `user_configured`,
  `remote_url`, or `programmatic`
- `trust`: `built_in`, `workspace`, `user_configured`, `remote`, or
  `programmatic`
- `load_reason`: a short explanation of the root or URL that loaded the skill

`skills_list action=explain` includes source/trust in match provenance, and
`skill_view` returns the metadata in structured tool output.

## Memory Boundary

`memory_save` and automatic memory extraction now explicitly exclude task
progress, command history, and repeatable procedures. Those should stay in
traces or become reviewed skills. This keeps long-term memory focused on
durable facts and preferences while giving procedures a first-class home.
