# LabRun Reports

This directory contains human-facing LabRun reports mirrored from structured
state under `.priority-agent/lab/`.

Rules:

- structured JSON/JSONL state is the source of truth;
- Markdown reports are for human review and project history;
- every report should include `lab_run_id`, artifact ID, status, evidence refs,
  and validation status where applicable;
- accepted reports can be mirrored here, while drafts and raw events stay under
  `.priority-agent/lab/`.
