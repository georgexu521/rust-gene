# Functional Reality Audit

Last updated: 2026-04-26

This audit checks whether visible CLI features are connected to real runtime behavior, rather
than existing only as commands, tools, or status text.

## External Baseline

- Claude Code treats skills as real slash-command capabilities: a `SKILL.md` can become
  `/skill-name`, loads on demand, supports supporting files, scoped allowed tools, dynamic
  context, and subagent execution.
  Source: <https://code.claude.com/docs/en/slash-commands>
- Claude Code MCP is not just a tool list. It covers tools, resources, prompts/commands,
  server health, approval, and authentication flows.
  Source: <https://code.claude.com/docs/en/mcp>
- Claude Code hooks are lifecycle integrations around tool use and other events, with JSON
  payloads and the ability to block or supplement behavior.
  Source: <https://code.claude.com/docs/en/hooks>
- Claude Code subagents have their own context, tool permissions, and task routing.
  Source: <https://code.claude.com/docs/en/sub-agents>
- Codex CLI frames mature behavior around AGENTS.md instructions, approvals/sandboxing,
  slash commands, MCP, images, web search, and feature discoverability.
  Sources: <https://developers.openai.com/codex/cli/features>,
  <https://developers.openai.com/codex/security>

## Current Reality

### Real and Connected

- Core conversation loop: streaming, tool execution, permission prompts, pre/post hooks,
  context compression, session persistence, and UI tool transcript are connected through
  `StreamingQueryEngine`, `ConversationLoop`, and `TuiApp`.
- Local file/search/project tools: `file_read`, `file_write`, `file_edit`, `grep`, `glob`,
  and `project_list` are registered, validated, and available through the model-visible
  tool registry.
- Permission hardening: tool-specific confirmation now participates in the unified approval
  decision, and denied tools are filtered before being shown to the model.
- Skills: bundled skills and `skill_view`/`skills_list` are real. `karpathy-guidelines`
  is loadable and applied through `/karpathy`.
- MCP core path: MCP manager, direct MCP tool calls, resource listing/reading, server
  approval, health diagnostics, and tool search over available MCP tools are implemented
  when MCP servers are configured.
- Session state: CLI and engine session IDs are now bound so `/sessions`, history,
  learning, and trace data refer to the same conversation.
- Hooks: environment-configured pre/post tool hooks can run and can block pre-tool execution.
  `/hooks` now reports both global hooks and per-tool hook environment variables.

### Partially Connected

- Skills are not yet Claude-level. Missing pieces include automatic nested discovery during
  work, live file watching, full frontmatter parity (`allowed-tools`, `model`, `effort`,
  `context: fork`, `user-invocable`), skill-scoped hooks, and automatic compaction carryover.
- MCP lacks full Claude-level command integration. Tools/resources work, but MCP prompts as
  slash commands, OAuth/browser auth, and polished connection repair flows are still thin.
- Agent/swarm execution is present, but subagent type selection, per-agent tool policy,
  inherited vs forked context semantics, and UI task routing are not yet as strong as Claude's
  subagent model.
- Workflow execution has a `NoOpStepExecutor`. It is useful for integration tests, but any
  production path that reaches it will only simulate work.
- ResourcePolicy currently has meaningful tracking/limits in some paths, but does not yet
  uniformly enforce all declared budget dimensions across every real code-change workflow,
  swarm handoff, MCP retrieval, and dashboard route.

### Visible but Weak or Misleading

- `/desktop` is visible but `open`, `close`, and `notify` return "not yet implemented".
  It is now marked as a placeholder in the command registry.
- `/reset all` clears visible message/tool state but does not perform full application,
  persistence, cache, memory, or approval-state reset. `/reset` is now marked as partially
  placeholder.
- `/reload skills` does not rebuild a live skill registry; it only reports that skills can
  be viewed.
- `mcp_auth` is explicitly a simplified placeholder around the current manager auth call.
- Voice is real only if system TTS or `whisper` are available. The base trait still has
  no-op default methods, but the concrete manager does perform availability checks.

## Highest-Value Fix Plan

1. Feature visibility contract:
   hide or mark unavailable entries unless they have a complete command-to-runtime path.
   `/desktop`, partial `/reset`, `mcp_auth`, and `/reload skills` are the first targets.
   Status: implemented for runtime tool availability, `/tools`, model-visible tools, and
   placeholder command markers.

2. Skill parity pass:
   implement a single `SkillRuntime` that handles discovery scopes, live reload, frontmatter
   policy, direct `/skill-name` invocation, automatic model invocation hints, and compaction
   reattachment.
   Status: implemented for unified discovery, reload, direct invocation, and frontmatter
   policy metadata. Compaction reattachment remains a later deepening item.

3. MCP parity pass:
   make MCP prompts appear as command/palette entries, improve auth/repair UX, and route
   MCP resources through the same retrieval context and permission model as project/web
   retrieval.
   Status: implemented for prompt discovery/listing and MCP resource `RetrievalContext`.
   OAuth/browser auth repair remains partial.

4. Subagent parity pass:
   enforce `AgentTaskEnvelope` at every agent/swarm handoff, add named agent profiles,
   per-agent allowed tools, context fork/inherit behavior, and UI visibility for each task.
   Status: implemented for tool/swarm handoff transcript lifecycle and child-agent envelope
   parsing. Named profiles and richer UI controls remain future work.

5. End-to-end feature evals:
   add CLI-level tests that execute each advertised slash command and representative model
   tool call, asserting the feature either succeeds or is clearly marked unavailable. This
   is the guardrail against "feature exists but does not work".
   Status: initial `feature_reality` evalset added for advertised tool availability and
   placeholder command markers.
