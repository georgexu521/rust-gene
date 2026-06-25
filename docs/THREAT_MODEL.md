# Priority Agent Threat Model
Status: Current

Last updated: 2026-06-25

This document describes the main security assumptions for Priority Agent as a
local programming-agent CLI. It is a practical engineering threat model, not a
formal compliance document.

## Assets

- Provider API keys and local dotenv credential files.
- User projects, source code, git history, and local workspace files.
- LabRun artifacts, proof events, validation logs, and audit evidence.
- Session history, memory files, SQLite stores, and tool traces.
- Release artifacts, CI workflows, and dependency manifests.

## Trust Boundaries

- User prompt to LLM: semantic guidance only; it must not bypass runtime policy.
- LLM to tool execution: all filesystem, shell, MCP, plugin, and package-script
  actions cross a permission and action-review boundary.
- Provider-generated validation to runtime: LabRun validation accepts only direct
  allowlisted commands and blocks suspicious path or shell behavior.
- Graduate work to repository state: Graduate mutation is scoped to approved
  LabRun task paths and must produce verification proof.
- Postdoc audit to durable artifacts: audit text is redacted before persistence;
  sensitive paths store hashes and metadata instead of raw snippets.
- CI to release artifacts: CI can build and upload artifacts, but release
  signing and provenance attestations are not implemented yet.

## Primary Threats And Mitigations

| Threat | Mitigation |
|---|---|
| Provider or model suggests unsafe shell validation. | LabRun validation uses direct allowlisted commands and rejects shell metacharacters, raw shell execution, and suspicious path-bearing flags. |
| Validation escapes the workspace through `--flag=../path`. | Path-bearing validation flags are blocked in both `--flag=value` and `--flag value` forms. |
| Package scripts execute unexpected repository code. | Package-script validation is blocked for unknown or untrusted workspaces and allowed only when workspace trust is explicit. |
| Postdoc audit persists secrets from snippets or diffs. | Audit redaction removes secret-like content, and sensitive paths record hash metadata without raw text. |
| Professor or postdoc role mutates project files. | LabRun role/stage policy overlay blocks professor/postdoc mutation through normal action-review surfaces. |
| Graduate role edits outside assigned scope. | Graduate mutations must match current task allowed scope before normal tool execution proceeds. |
| MCP or plugin tools bypass local policy. | Tool execution remains permission-reviewed; MCP and plugin mutation surfaces are treated as high-risk integration points. |
| Secrets are committed to git or leaked in public reports. | Security and contribution docs require redacted reports; future work should add automated secret scanning. |

## Residual Risks

- Local credential storage is plaintext dotenv with best-effort file permissions.
- Unknown third-party repositories can contain hostile build scripts, tests, or
  package lifecycle hooks.
- CodeQL and Dependabot improve coverage but do not replace human review.
- Windows behavior is not yet validated to the same level as macOS/Linux.
- Release signing, SBOM, dependency license audit, and provenance are future
  release-hardening work.

## Review Cadence

Update this threat model when any of these change:

- tool execution or permission flow;
- LabRun validation, proof, role, or stage policy;
- credential storage;
- MCP/plugin execution;
- CI release automation;
- public release target or platform support.
