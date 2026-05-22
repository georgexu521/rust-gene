# Desktop Agent Timeline Plan - 2026-05-22

## Goal

Make the desktop app feel like a coding-agent workbench, not a prettier CLI log.
The main transcript should explain what the agent is doing: tool activity,
permissions, validation, file changes, failures, and final completion.

## Phase 1 - Mainline Timeline Events

- Replace plain `tool` transcript rows with typed timeline events.
- Render run start/completion, tool start/progress/completion, permission
  requests, usage, truncation, and errors as compact event blocks.
- Keep the trace drawer as detailed debug history, but make the main transcript
  useful without opening it.
- Validate with production build and UI smoke screenshots.

Status: implemented. Tool events are grouped by tool id in the main transcript.

## Phase 2 - Tool Grouping And Metadata

- Group events by tool call id so one tool appears as a single live-updating
  card instead of several repeated rows.
- Surface command/path/file metadata when available.
- Add status states: running, waiting, completed, failed, blocked.

Status: implemented. The desktop app now reads real runtime `tool_summary`
metadata, groups events by tool id, shows command/path/validation/file facts as
chips, and renders shell validation, file edits/patches, and failed tools with
specialized summaries.

## Phase 3 - File And Validation Cards

- Promote file edits, patches, tests, and shell commands into specialized cards.
- Add expandable result previews for long outputs.
- Show validation commands and pass/fail outcomes in a scannable format.

Status: implemented for the first desktop pass. File edit/patch tool summaries
now carry a bounded diff preview from the Rust runtime, the desktop timeline
renders it inline, and long failure/output previews collapse behind a native
expand control.

## Phase 4 - Interaction

- Add stop/retry affordances where the runtime supports them.
- Let users approve/reject permissions from the timeline event as well as the
  footer permission card.
- Add jump-to-trace/debug links for each event.

Status: mostly implemented. Permission timeline cards now expose approve and
reject actions in the main transcript and update the original waiting card after
the runtime answers. Timeline cards also expose debug links that open the trace
drawer and highlight the corresponding trace event. Stop/retry remain future
work.

## Current Next Step

Continue Phase 4 only when the runtime exposes explicit cancellation/retry
controls. The next desktop UX work can move to run/session ergonomics: new chat,
session naming, search, and project switching.
