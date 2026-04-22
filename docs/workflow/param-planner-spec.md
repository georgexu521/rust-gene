# Workflow Param Planner Spec (W1)

## 1. Goal
Stabilize tool parameter synthesis in workflow execution so that fallback behavior is deterministic, safe, and testable.

## 2. Scope (Top-10 tools)

| Tool | Priority | v1 Status | Strategy |
|---|---:|---|---|
| `file_read` | P0 | Implemented | path extraction + required fill |
| `file_edit` | P0 | Implemented | explicit path + replace(old,new) constraints |
| `bash` | P0 | Implemented | command extraction + timeout default |
| `grep` | P0 | Implemented | pattern/path extraction + required fill |
| `file_write` | P1 | Implemented | path + content inference (`backtick` > quoted > template) |
| `glob` | P1 | Implemented | glob pattern inference + safe path default |
| `project_list` | P1 | Implemented | action/query inference (`summary/list/search/dir/refresh`) |
| `memory_save` | P2 | Implemented | content extraction + category inference |
| `todo_write` | P2 | Implemented | todo array synthesis (split + status/priority inference) |
| `json_query` | P2 | Implemented | action/json/path inference + safe defaults |

## 3. Execution Pipeline
1. Tool-specific planner first.
2. LLM schema synthesis second.
3. Schema fallback third.
4. Normalize/repair required fields and required field types.
5. Tool-level `validate_params` as final guard.

## 4. Safety rules
1. Never fabricate destructive file/system paths as defaults.
2. For command-like tools, fallback command must be explicit and auditable.
3. Missing required params must map to deterministic defaults or explicit error.
4. All fallback reasons should be machine-searchable in logs.

## 5. Current implementation baseline
- Added required-field completion for generated params.
- Added required-field type coercion (`string/integer/boolean/array/object`).
- Added rejection for non-object synthesized payload.
- Added unit tests for normalize behavior.
- Added tool-specific planner implementations for `file_write/glob/project_list`.
- Added param replay dataset + script + report generation.

## 6. Acceptance criteria (W1/W2)
- W1: replay success rate >= 90% on param sample set.
- W2: replay success rate >= 95%.
- Dangerous default parameter count = 0.
- CI includes parameter replay checks.

## 7. Next implementation tasks
1. Add tool-specific planners for `file_write/glob/project_list`.
2. Add structured error codes for planner fallback path.
3. Build replay dataset (`param-replay-samples.json`) and report (`param-replay-report.md`).
