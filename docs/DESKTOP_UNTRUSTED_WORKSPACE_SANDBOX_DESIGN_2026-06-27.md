# Desktop Untrusted Workspace Sandbox Design - 2026-06-27

Status: Design boundary for future implementation

Owner: Liz / gex

## Purpose

Priority Agent desktop can safely dogfood trusted local projects with
permission gates, project-scoped trust, controlled validation allowlists, and
redacted diagnostics. That is not the same as running arbitrary untrusted
repositories in a sandbox.

This document defines the release-candidate boundary and the future backend
required before the desktop app should auto-run tests or package scripts in
unknown repositories.

## Current Boundary

Current validation security label:

```text
validation_security=controlled_not_sandboxed
```

That means:

- command strings are parsed and allowlisted;
- obvious shell-injection and path-escape inputs are rejected;
- package-script execution is gated by project-scoped workspace trust;
- diagnostics and LabRun proof label the trust source;
- required-validation child processes can be killed on timeout or cancellation.

It does not mean:

- Rust `build.rs`, proc macros, or tests cannot execute host code;
- `pytest` imports cannot run arbitrary Python;
- `npm test`, `pnpm test`, or `yarn test` cannot run arbitrary package scripts;
- repository shell scripts are safe just because the launcher is allowlisted;
- host credentials, network, filesystem, CPU, memory, or process counts are
  isolated from the repository code.

## Release-Candidate Policy

For unknown or untrusted workspaces:

- package scripts: ask or block;
- shell validation: ask or block;
- LabRun daemon supervision: off;
- Developer Auto: off;
- proof/diagnostics must show `controlled_not_sandboxed`.

For trusted workspaces:

- package scripts may be allowed by explicit project-scoped trust;
- shell validation may be allowed by explicit project-scoped trust;
- LabRun daemon supervision remains opt-in;
- Developer Auto requires explicit acknowledgement.

## Future Sandbox Backend Requirements

A true untrusted-workspace backend should provide:

- isolated process tree with reliable cancellation;
- network disabled by default, with explicit allowlist if ever enabled;
- no inherited provider keys or host credentials;
- read-only host mounts except an isolated workspace scratch copy;
- bounded CPU, memory, process count, output, and wall-clock time;
- deterministic cleanup of scratch state;
- evidence labels recording sandbox backend, limits, image/profile, and exit
  status;
- a clear user prompt before the first sandboxed run in a project.

## Candidate Backends

Potential implementation paths:

- container runtime profile for Linux and CI;
- macOS sandbox profile for local desktop validation;
- lightweight VM/container helper for stronger network/filesystem isolation;
- no-op controlled runner retained only for trusted workspaces.

The first implementation should be a separate backend behind explicit
configuration. It should not silently replace the current controlled runner
without proof labels and RC evidence.

## Acceptance Criteria For Implementation

- Unknown workspace package scripts cannot run outside sandbox or explicit ask.
- Provider keys are absent from sandbox child environment by default.
- Sandbox cancellation kills the child process tree.
- `/lab proof`, desktop diagnostics, and release evidence include sandbox
  backend and limit metadata.
- Tests cover blocked unknown workspace execution, successful trusted
  controlled validation, sandbox cancellation, and redacted environment.
