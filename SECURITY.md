# Security Policy
Status: Current

Priority Agent is pre-1.0 software. Treat the current public repository as a
development and dogfood project, not as a hardened security product.

## Supported Versions

Security fixes target the `main` branch until formal versioned releases are
published. No older release line is currently supported.

## Reporting A Vulnerability

Do not open a public issue that contains secrets, exploit details, private
workspace paths, or provider credentials.

Preferred reporting path:

1. Open a private GitHub Security Advisory for this repository when available.
2. If private advisories are not available to you, contact the repository owner
   directly and include only enough detail to reproduce the issue safely.
3. Use redacted examples for API keys, bearer tokens, private keys, and local
   credential files.

Please include:

- affected commit or branch;
- operating system and shell;
- exact Priority Agent command or LabRun path involved;
- expected behavior and observed behavior;
- whether any secret, credential, or private file may have been exposed.

## Current Security Boundaries

Priority Agent is designed as a local coding agent with explicit runtime
boundaries:

- tool calls are reviewed through action and permission policy;
- LabRun graduate work is scoped by allowed paths and validation proof;
- LabRun validation uses direct allowlisted commands instead of raw shell
  execution for provider-authored validation strings;
- Postdoc audit evidence redacts secret-like snippets and suppresses raw text
  for sensitive paths;
- package-script validation is not treated as verified LabRun proof unless the
  workspace is explicitly trusted.

## Known Limitations

- `/connect` stores local provider keys in `~/.priority-agent/.env` as plaintext
  dotenv content. Unix-like systems attempt `0600` permissions, but macOS
  Keychain, Secret Service, and Windows Credential Manager are not implemented.
- The current release target is macOS and Linux. Windows remains best-effort
  until shell defaults, installer behavior, and credential paths are validated.
- MCP servers, plugins, package scripts, and repository-local scripts can execute
  code. Use trusted workspaces and review permission prompts carefully.
- Release signing, provenance attestations, SBOM generation, and automated
  secret scanning are future release-hardening work. Dependency vulnerability
  and license checks are available through
  `scripts/security_dependency_audit.sh`.

## Security-Sensitive Changes

Changes touching these areas require extra review and targeted tests:

- `src/lab/`, especially validation, artifact gates, audit evidence, and role
  policy;
- `src/permissions/` and `src/engine/action_review.rs`;
- provider credential handling and `.env` persistence;
- MCP, plugin, shell, filesystem, and package-script execution;
- `.github/workflows/`, release scripts, and CI security configuration.
