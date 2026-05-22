# Priority Agent Release Readiness Guide

This guide is the operator checklist for shipping a local release candidate.

## Quick Start

```bash
scripts/install.sh --dry-run --release
scripts/install.sh --release
pa --cli
```

Use `pa --tui` only for the compatibility full-screen interface. The normal
interactive path is `priority-agent` or `pa` with the CLI shell.

## Configuration

Inspect configuration:

```text
/config list
/config schema
/config paths
/config doctor
/config export
```

`/config export` returns a redacted JSON payload suitable for debugging and bug
reports. API keys, tokens, and secrets are replaced with `[redacted]`.

Common release keys:

- `api.base_url`
- `api.model`
- `features.plugin_trust_mode`
- `engine.max_iterations`

## Doctor

Run:

```text
/doctor
/doctor json
```

Release doctor output should cover:

- provider protocol family and tool-call compatibility;
- API key and network readiness;
- permission config presence;
- writable state directories;
- git worktree availability;
- MCP/plugin runtime health;
- bridge and remote-session runtime state.

Treat `Error` as release-blocking. Treat `Warning` as release-blocking only when
it affects the target workflow, for example a blocked plugin needed by the task.

## Safety Model

Priority Agent should default to local, inspectable, reversible work:

- run from a git worktree for code edits;
- keep permission rules in `.priority-agent/permissions.toml` or the global
  priority-agent permissions file;
- use `/permissions explain <tool_name>` when a tool decision is surprising;
- use `/config export` rather than copying raw config files into issues;
- use `scripts/release-gates.sh quick` before publishing a local build.

## Release Gates

Fast local gate:

```bash
scripts/release-gates.sh quick
```

Full local gate:

```bash
scripts/release-gates.sh full
```

Package dry-run:

```bash
scripts/package-release.sh --features experimental-api-server --dry-run
```

Package release:

```bash
scripts/package-release.sh --features experimental-api-server
```

The package script writes:

- `target/dist/priority-agent-<version>-<target>.tar.gz`
- `target/dist/priority-agent-<version>-<target>.tar.gz.sha256`

## Known Gaps

- External Claude Code/Codex parity reports still need to be generated from real
  artifacts before claiming product-level equivalence.
- Live LLM replay remains opt-in because it depends on provider keys, latency,
  and quota.
- Release packaging is local tarball/checksum packaging; notarized macOS
  distribution and multi-platform release publishing are not yet automated.
