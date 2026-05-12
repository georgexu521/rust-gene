# Priority Agent 核心编码质量下一阶段计划

日期：2026-05-11

本文档承接：

- `docs/NEXT_AGENT_PRODUCTIZATION_PLAN_2026-05-10.md`
- `docs/LLM_RUNTIME_SIMPLIFICATION_PLAN_2026-05-08.md`
- `docs/PROJECT_STATUS.md`
- 本地 Claude Code 源码：`/Users/georgexu/Desktop/claude`
- 本地 opencode 源码：`/Users/georgexu/Desktop/opencode-dev`

这份计划不是再写一个总路线，而是把下一阶段执行顺序收敛为三条主线：

1. 先拆分主循环，降低行为副作用。
2. 再把 shell / terminal 做成一等编码能力。
3. 最后补齐文件编辑质量，尤其是 stale read、编码、换行、锁、diff、LSP 和回滚。

## Implementation Progress

- 2026-05-11: Phase 1 Batch 1.1 started. Added
  `docs/CONVERSATION_LOOP_RESPONSIBILITY_MAP_2026-05-11.md` as the current
  `ConversationLoop::run_inner` responsibility map and extraction boundary.
- 2026-05-11: Phase 1 Batch 1.2 started. Added
  `src/engine/conversation_loop/turn_runtime_state.rs` and moved the first
  turn-owned mutable state into it: `EvidenceLedger`, `RuntimeDietSnapshot`,
  iteration counters, and repair counters. This is a behavior-preserving first
  slice; checkpoint state and tool-call lifecycle state remain in `run_inner`
  until the next split.
- 2026-05-11: Phase 1 Batch 1.3 started. Replaced the anonymous
  `(content, tool_calls, pre_executed_results)` session processor tuple with
  `SessionStepResult`, giving the model request / streaming step an explicit
  output boundary before deeper lifecycle migration.
- 2026-05-11: Phase 1 Batch 1.3 continued. Extended `SessionStepResult` with
  `usage`, `finish_reason`, and `source` so the future `SessionProcessor`
  state machine can distinguish normal non-streaming, successful streaming,
  and streaming fallback paths without depending on tuple position or ad hoc
  log text.
- 2026-05-11: Phase 1 Batch 1.3 continued. Added `ToolCallLifecycle` with
  explicit pending/running/completed/failed/denied/provider-executed states and
  connected it to the current tool execution path through `TurnRuntimeState`.
  This keeps the existing `Vec<(ToolCall, ToolResult)>` return shape while
  giving the future `SessionProcessor` state machine a real lifecycle boundary.
- 2026-05-11: Phase 1 Batch 1.3 continued. Wrapped the tool execution return
  value in `ToolExecutionBatch`, so the main loop now consumes a named batch
  object instead of a naked `Vec<(ToolCall, ToolResult)>`. The first slice keeps
  existing result ordering and exposes lifecycle-derived denied / failed /
  pre-executed counts for future state-machine routing.
- 2026-05-11: Phase 1 Batch 1.3 continued. Added result-derived
  `ToolExecutionBatch` accessors (`any_success`, `unsuccessful_count`,
  `result_successes`) and wired the main loop's low-risk retry/guard checks to
  those structured facts instead of rescanning raw tuples.
- 2026-05-11: Phase 1 Batch 1.3 continued. Wrapped the
  `execute_tools_parallel` input context in `ToolExecutionRequest`, replacing
  the long anonymous argument list with a named execution boundary for tool
  calls, streaming, pre-executed results, route policy, exposed tools,
  checkpoint facts, destructive scope, and lifecycle state.
- 2026-05-11: Phase 1 Batch 1.3 continued. Introduced
  `ToolExecutionController` as the owner of `execute_tools_parallel`; the main
  loop now constructs the controller and passes a `ToolExecutionRequest`,
  keeping execution behavior unchanged while moving tool orchestration out of
  `impl ConversationLoop`.
- 2026-05-11: Phase 1 Batch 1.3 continued. Added explicit
  `ToolExecutionContext`, so `ToolExecutionController` no longer borrows the
  whole `ConversationLoop` during execution. The context snapshots the concrete
  execution dependencies: tool registry, cost tracker, session persistence,
  hooks, approval channel, allowed tools, audit/denial trackers, active goal,
  and the base `ToolContext`.
- 2026-05-11: Phase 1 Batch 1.3 continued. Added `ToolExecutionGate` for
  pre-execution checks: exposed-tool enforcement, resource call budget,
  goal-drift trace, allowed-tools isolation, destructive scope, and action
  checkpoint bash/file_edit guards. The gate only decides allow/deny and
  prepares denial results; `ToolExecutionController` still owns persistence,
  lifecycle updates, scheduling, and execution order.
- 2026-05-11: Phase 1 Batch 1.3 continued. Split
  `ToolExecutionController::execute_tools_parallel` internally: read-only tool
  job creation now lives in `read_only_job`, read-only result collection in
  `collect_read_only_results`, and sequential read-write execution in
  `execute_read_write_calls`. This keeps the batch/gate/context contract stable
  while reducing the main execution method to orchestration.
- Validation after the read-only/read-write execution split:
  `cargo fmt --check`, `git diff --check`, targeted `tool_call_lifecycle`,
  `batch_summarizes_results_and_lifecycle_statuses`, `route_scoped_tools`,
  `tool_result`, `runtime_diet`, and `patch_synthesis` tests,
  `cargo check -q`, `cargo clippy --all-features -- -D warnings`, and full
  `cargo test -q` all passed (`1208 passed; 0 failed`).
- Validation after the `ToolExecutionGate` split: `cargo fmt --check`,
  `git diff --check`, targeted `tool_call_lifecycle`,
  `batch_summarizes_results_and_lifecycle_statuses`, `route_scoped_tools`,
  `tool_result`, `runtime_diet`, and `patch_synthesis` tests,
  `cargo check -q`, `cargo clippy --all-features -- -D warnings`, and full
  `cargo test -q` all passed (`1208 passed; 0 failed`).
- Validation after the `ToolExecutionContext` split: `cargo fmt --check`,
  `git diff --check`, targeted `tool_call_lifecycle`,
  `batch_summarizes_results_and_lifecycle_statuses`, `route_scoped_tools`,
  `tool_result`, `runtime_diet`, and `patch_synthesis` tests,
  `cargo check -q`, `cargo clippy --all-features -- -D warnings`, and full
  `cargo test -q` all passed (`1208 passed; 0 failed`).
- Validation after the `ToolExecutionController` ownership split:
  `cargo fmt --check`, `git diff --check`, targeted `tool_call_lifecycle`,
  `batch_summarizes_results_and_lifecycle_statuses`, `route_scoped_tools`,
  `tool_result`, `runtime_diet`, and `patch_synthesis` tests,
  `cargo check -q`, `cargo clippy --all-features -- -D warnings`, and full
  `cargo test -q` all passed (`1208 passed; 0 failed`).
- Validation after the `ToolExecutionRequest` slice: `cargo fmt --check`,
  `git diff --check`, targeted `tool_call_lifecycle`,
  `batch_summarizes_results_and_lifecycle_statuses`, `tool_result`,
  `route_scoped_tools`, `runtime_diet`, and `patch_synthesis` tests,
  `cargo check -q`, `cargo clippy --all-features -- -D warnings`, and full
  `cargo test -q` all passed (`1208 passed; 0 failed`).
- Validation after the Batch 1.3 continuation: `cargo fmt --check`,
  `git diff --check`, targeted `runtime_diet`, `route_scoped_tools`,
  `prompt_context`, `tool_result`, and `patch_synthesis` tests,
  `cargo check -q`, `cargo clippy --all-features -- -D warnings`, and full
  `cargo test -q` all passed (`1205 passed; 0 failed`).
- Validation after the `ToolCallLifecycle` slice: `cargo fmt --check`,
  `git diff --check`, targeted `tool_call_lifecycle`, `tool_result`,
  `route_scoped_tools`, `runtime_diet`, and `patch_synthesis` tests,
  `cargo check -q`, `cargo clippy --all-features -- -D warnings`, and full
  `cargo test -q` all passed (`1207 passed; 0 failed`).
- Validation after the `ToolExecutionBatch` slice: `cargo fmt --check`,
  `git diff --check`, targeted `tool_call_lifecycle`,
  `batch_counts_lifecycle_statuses`, `tool_result`, `route_scoped_tools`,
  `runtime_diet`, and `patch_synthesis` tests, `cargo check -q`,
  `cargo clippy --all-features -- -D warnings`, and full `cargo test -q` all
  passed (`1208 passed; 0 failed`).
- Validation after wiring batch summaries into low-risk main-loop checks:
  `cargo fmt --check`, `git diff --check`, targeted
  `batch_summarizes_results_and_lifecycle_statuses`, `tool_call_lifecycle`,
  `tool_result`, `route_scoped_tools`, `runtime_diet`, and `patch_synthesis`
  tests, `cargo check -q`, `cargo clippy --all-features -- -D warnings`, and
  full `cargo test -q` all passed (`1208 passed; 0 failed`).
- 2026-05-11: Phase 1 Batch 1.4 started. Added the first
  `ToolResultNormalizer` boundary and routed provider-facing tool result
  content through it. The first slice preserves the exact existing model
  content while creating the owner for later UI content / metadata / evidence
  separation.
- Validation after this slice: `cargo fmt --check`, `git diff --check`,
  targeted `runtime_diet`, `route_scoped_tools`, `prompt_context`,
  `evidence_ledger`, `closeout`, `tool_result`, and `patch_synthesis` tests,
  `cargo check -q`, `cargo clippy --all-features -- -D warnings`, and full
  `cargo test -q` all passed (`1205 passed; 0 failed`).
- 2026-05-11: Phase 1 Batch 1.4 continued. Expanded
  `ToolResultNormalizer` from a provider-content wrapper into the explicit
  tool-result boundary for `model_content`, `ui_content`,
  `structured_metadata`, and `evidence_facts`. The append path now records
  evidence through the normalized result, and streaming completion events use
  the normalized UI content instead of calling provider formatting directly.
- Validation after the normalized tool-result split: `cargo fmt --check`,
  `git diff --check`, targeted `tool_result`, `evidence_ledger`, `closeout`,
  `route_scoped_tools`, `runtime_diet`, and `patch_synthesis` tests,
  `cargo check -q`, `cargo clippy --all-features -- -D warnings`, and full
  `cargo test -q` all passed (`1209 passed; 0 failed`).
- 2026-05-12: Phase 1 Batch 1.4 continued. Moved large tool-output
  preparation behind `ToolResultNormalizer::normalize_after_execution`, so
  `run_inner` no longer calls `truncate_tool_result` directly before appending
  results. Large output truncation now also records structured
  `output_truncation` metadata with original size, preview size, threshold, and
  stored artifact path.
- Validation after moving truncation behind the normalizer: `cargo fmt --check`,
  `git diff --check`, targeted `tool_result`, `tool_execution`,
  `evidence_ledger`, `closeout`, `route_scoped_tools`, `runtime_diet`, and
  `patch_synthesis` tests, `cargo check -q`,
  `cargo clippy --all-features -- -D warnings`, and full `cargo test -q` all
  passed (`1210 passed; 0 failed`).
- 2026-05-12: Phase 1 Batch 1.4 continued. Added a pre-execution schema gate
  for normal tool execution, so invalid tool arguments are rejected before
  hooks/approval/tool execution and returned as standard `invalid_params`
  `ToolResult`s. The normalized metadata now carries `error_code` plus
  `schema_validation` details for these failures.
- Validation after the schema gate: `cargo fmt --check`, `git diff --check`,
  targeted `tool_result`, `invalid_tool_params_are_rejected_before_execution`,
  `batch_summarizes_results_and_lifecycle_statuses`, `evidence_ledger`,
  `closeout`, `route_scoped_tools`, `runtime_diet`, and `patch_synthesis`
  tests, `cargo check -q`, `cargo clippy --all-features -- -D warnings`, and
  full `cargo test -q` all passed (`1212 passed; 0 failed`).
- 2026-05-12: Phase 1 Batch 1.5 started. Added
  `ContextBudgetController` as the first explicit request-budget boundary. It
  now owns request token observation, preflight compaction decisions, exposed
  tool counts, total request tokens, and remaining/max context tracking for the
  runtime diet report. The first slice preserves the existing compression
  behavior while making the model-context budget visible in traces.
- Validation after the first context-budget slice: `cargo fmt --check`,
  `git diff --check`, targeted `context_budget`, `runtime_diet`,
  `prompt_context`, and `trace_summary_includes_runtime_diet_report` tests,
  `cargo check -q`, `cargo clippy --all-features -- -D warnings`, and full
  `cargo test -q` all passed (`1216 passed; 0 failed`).
- 2026-05-12: Phase 1 Batch 1.5 continued. Extended
  `ContextBudgetController` to observe model-facing tool result aggregate size
  and large-output truncation/artifact records from `NormalizedToolResult`.
  `RuntimeDietReport` now includes tool-result chars/tokens, truncated result
  count, and artifact count, with a real tool-turn trace regression.
- Validation after tool-result budget observation: `cargo fmt --check`,
  `git diff --check`, targeted `context_budget`, `runtime_diet`,
  `tool_result`, `prompt_context`, `trace_summary_includes_runtime_diet_report`,
  and `evidence_ledger` tests, `cargo check -q`,
  `cargo clippy --all-features -- -D warnings`, and full `cargo test -q` all
  passed (`1219 passed; 0 failed`).
- 2026-05-12: Phase 1 Batch 1.6 started. Added
  `PermissionController` as the first explicit runtime permission boundary for
  read-write tool execution. It now owns permission request records, approval
  prompt construction, user approval submission, once-mode approval grants,
  structured permission-denied `ToolResult` metadata, and permission-denied
  classification. The existing tool execution order and approval semantics are
  unchanged, but denied permission results now carry `permission_request`
  metadata through `ToolResultNormalizer` and permission facts into
  `EvidenceLedger`.
- Validation after the first permission-controller slice: `cargo fmt --check`,
  `git diff --check`, targeted `permission_controller`, `permissions`,
  `tool_exposure`, `bash_tool`, `tool_result`, `evidence_ledger`, and
  `test_tool_specific_confirmation_blocks_git_push_without_approval` tests,
  `cargo check -q`, `cargo clippy --all-features -- -D warnings`, and full
  `cargo test -q` all passed (`1223 passed; 0 failed`).
- 2026-05-12: Phase 1 Batch 1.6 continued. Extended permission request
  metadata with a `permission_family` classification so the same
  `PermissionController` path can report shell, file, external-directory, task,
  and subagent permission decisions without adding prompt rules. This keeps the
  default approval behavior unchanged while making permission recovery facts
  clearer for the model and traces.
- Validation after permission-family metadata: `cargo fmt --check`,
  `git diff --check`, targeted `permission_controller`, `permissions`,
  `tool_result`, `evidence_ledger`, `tool_exposure`, `bash_tool`, and
  `test_tool_specific_confirmation_blocks_git_push_without_approval` tests,
  `cargo check -q`, `cargo clippy --all-features -- -D warnings`, and full
  `cargo test -q` all passed (`1226 passed; 0 failed`).
- 2026-05-12: Phase 1 Batch 1.6 continued. Added explicit
  `recovery_feedback` to permission request records and permission-denied tool
  results. Denied tools now tell the model not to claim execution succeeded and
  how to recover: ask for approval, choose a lower-risk/read-only alternative,
  narrow the path/scope, continue locally, or confirm goal scope depending on
  the permission family.
- Validation after permission recovery feedback: `cargo fmt --check`,
  `git diff --check`, targeted `permission_controller`, `permissions`,
  `tool_result`, `evidence_ledger`, `tool_exposure`, `bash_tool`, and
  `test_tool_specific_confirmation_blocks_git_push_without_approval` tests,
  `cargo check -q`, `cargo clippy --all-features -- -D warnings`, and full
  `cargo test -q` all passed (`1226 passed; 0 failed`).
- 2026-05-12: Phase 1 Batch 1.7 started. Added
  `src/services/api/provider_protocol.rs` as the provider-bound message
  protocol matrix for OpenAI-compatible, MiniMax, Kimi, Anthropic-like, and
  reasoning-capable families. OpenAI-compatible, MiniMax, and Kimi request
  conversion now share this normalization boundary; empty assistant
  `tool_calls` are not serialized, orphan/aborted tool results are dropped
  before provider requests, and MiniMax keeps its system-message merge without
  breaking tool-call adjacency. Provider 400 errors that mention tool-result
  ordering are now classified as `provider_protocol`, while generic invalid
  params remain `schema`.
- Validation after the first provider-protocol matrix slice:
  `cargo fmt --check`, `git diff --check`, targeted `provider`,
  `openai_compat`, `minimax`, `kimi`, `error_classifier`, and
  `provider_health` tests, `cargo check -q`,
  `cargo clippy --all-features -- -D warnings`, and full `cargo test -q` all
  passed (`1241 passed; 0 failed`).
- 2026-05-12: Phase 1 Batch 1.8 started. Added a structured
  `FocusedRepairActionRequest -> FocusedRepairActionProposal` boundary for the
  action-checkpoint repair path. The main loop now receives explicit failure
  evidence, allowed lookup budget, exposed tools, fallback owner, and fallback
  reason before entering patch synthesis; deterministic and model-led patch
  synthesis traces now record `owner=action_checkpoint reason=...`.
- Validation after the first focused-repair proposal slice:
  `cargo fmt --check`, `git diff --check`, targeted `focused_repair`,
  `action_checkpoint`, `patch_synthesis`, and `closeout` tests,
  `cargo check -q`, `cargo clippy --all-features -- -D warnings`,
  `bash scripts/workflow-production-gates.sh`, and full `cargo test -q` all
  passed (`1243 passed; 0 failed`).
- 2026-05-12: Phase 1 Batch 1.8 second slice tightened the patch repair
  boundary. `synthesize_patch_tool_calls` now returns a structured outcome with
  `source=model_json|model_tool_fallback|deterministic_fallback`; deterministic
  patch synthesis no longer runs before model synthesis when usable evidence
  exists, and trace output records the source plus fallback reason.
- Validation after the explicit patch-synthesis fallback slice:
  `cargo fmt --check`, `git diff --check`, targeted `patch_synthesis`,
  `focused_repair`, and `action_checkpoint` tests, `cargo check -q`,
  `cargo clippy --all-features -- -D warnings`,
  `bash scripts/workflow-production-gates.sh`, and full `cargo test -q` all
  passed (`1244 passed; 0 failed`).
- 2026-05-12: Phase 2 Batch 2.1 started. `tool_exposure` diagnostics now also
  report provider-facing tool schema compatibility, and `/status` / `/doctor`
  bash exposure output includes `schema=ok` or a concrete schema reason. This
  keeps terminal availability debugging in runtime diagnostics instead of
  pushing more rules into prompts.
- Validation after the provider-schema exposure diagnostic slice:
  `cargo fmt --check`, `git diff --check`, targeted `tool_exposure`,
  `bash_exposure`, `doctor_route_summary`, and `intent_router` tests,
  `cargo check -q`, `cargo clippy --all-features -- -D warnings`, and full
  `cargo test -q` all passed (`1245 passed; 0 failed`).
- 2026-05-12: Phase 2 Batch 2.2 started. Added a finer
  `ShellCommandCategory` beside the legacy `CommandKind`, with categories for
  read/list/search, validation, test runs, package installs, dev servers, file
  mutation, git mutation, destructive commands, and unknown commands. Bash tool
  metadata, `EvidenceLedger`, tool execution summaries, and shell progress
  labels now use the shared classifier. Plain `rg ...` is no longer treated as
  required validation; only explicit `! rg ...` assertions are.
- Validation after the shell-command category slice: `cargo fmt --check`,
  `git diff --check`, targeted `command_classifier`, `bash_tool`,
  `evidence_ledger`, `tool_result`, `progress`, and live-eval required-command
  tests, `cargo check -q`, `cargo clippy --all-features -- -D warnings`, and
  full `cargo test -q` all passed (`1246 passed; 0 failed`).
- 2026-05-12: Phase 2 Batch 2.2 continued. `PermissionContext` now uses the
  shared shell command category for bash risk: read/list/search/validation/test
  commands are low risk, package installs/dev servers/file mutations/git
  mutations remain medium or higher, and high-risk/network/outside-workspace
  checks still override to high. Permission explanations include the shell
  category so approval prompts and diagnostics share the same semantic source.
- Validation after the permission-risk category slice: `cargo fmt --check`,
  targeted `permissions` and `command_classifier` tests, `cargo check -q`,
  `cargo clippy --all-features -- -D warnings`, and full `cargo test -q` all
  passed (`1247 passed; 0 failed`).
- 2026-05-12: Phase 2 Batch 2.2 continued. The TUI bash tool summary now uses
  the shared shell classifier instead of its own string-prefix checks, so
  package installs, dev servers, search, listing, validation, git mutation, and
  file mutation use the same semantics in UI, evidence, permissions, and
  closeout metadata.
- Validation after the TUI shell-summary slice: `cargo fmt --check`, targeted
  `tool_view` and `command_classifier` tests, `cargo check -q`,
  `cargo clippy --all-features -- -D warnings`, and full `cargo test -q` all
  passed (`1248 passed; 0 failed`).
- 2026-05-12: Phase 2 Batch 2.3 started. Bash results now include a structured
  `shell_result` payload with command, cwd, exit code, stdout/stderr previews,
  truncation status, output artifact path, classifier data, and evidence
  status. Long combined output is written under
  `.priority-agent/tool-results/<session>/...`, while model-facing content keeps
  a bounded preview.
- Validation after the shell-result schema/artifact slice: `cargo fmt --check`,
  targeted `bash_tool`, `tool_result`, and `evidence_ledger` tests,
  `cargo check -q`, `cargo clippy --all-features -- -D warnings`, and full
  `cargo test -q` all passed (`1249 passed; 0 failed`).
- 2026-05-12: Phase 2 Batch 2.3 continued. Tool execution metadata now writes
  the measured runtime duration back into `shell_result.duration_ms`, so the
  structured shell schema no longer leaves duration as a placeholder after the
  controller records elapsed time.
- Validation after the shell-result duration slice: `cargo fmt --check`,
  targeted `tool_result`, `bash_tool`, and shell-result duration tests,
  `cargo check -q`, `cargo clippy --all-features -- -D warnings`, and full
  `cargo test -q` all passed (`1250 passed; 0 failed`).

## 当前判断

Priority Agent 的基础编码能力已经不再是空白：

- 有 `file_read`、`grep`、`glob`、`file_edit`、`file_write`、`bash`、`git`、`format`、`lsp`。
- 有 route-scoped tools、权限上下文、closeout、EvidenceLedger、live eval、provider retry 和 provider-safe tool result work。
- 最近全量本地测试基线是 `1243 passed; 0 failed`。

但还没有完全赶上 Claude Code / opencode 的核心编码质量。差距主要不是功能数量，而是运行时产品化程度：

- 主循环仍然过重，`src/engine/conversation_loop/mod.rs` 还有 5600+ 行。
- shell 仍是普通工具，不是完整终端运行时。
- 文件编辑工具已经有 stale-read 检测和路径身份修复，但还缺成熟产品里的编码、换行、锁、diff、LSP、历史恢复等细节。

## 参考结论

### Claude Code 值得借鉴的语义

参考文件：

- `/Users/georgexu/Desktop/claude/src/query.ts`
- `/Users/georgexu/Desktop/claude/src/Tool.ts`
- `/Users/georgexu/Desktop/claude/src/tools/BashTool/BashTool.tsx`
- `/Users/georgexu/Desktop/claude/src/tasks/LocalShellTask/`
- `/Users/georgexu/Desktop/claude/src/tools/FileEditTool/FileEditTool.ts`

借鉴点：

- query loop 负责会话推进和 context budget，不把所有工具细节塞在主循环里。
- `ToolUseContext` 和 `ToolPermissionContext` 把工具执行、权限、文件读取状态、UI 状态、agent 状态分开。
- BashTool 不只是执行命令，还处理命令语义、权限、timeout、background、sandbox、输出展示和任务状态。
- FileEditTool 强制 read-before-edit，检查外部修改，保留 encoding / line endings，更新 file history 和 LSP。

### opencode 值得借鉴的语义

参考文件：

- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/session/processor.ts`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/session/prompt.ts`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/tool/tool.ts`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/tool/registry.ts`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/tool/shell.ts`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/tool/truncate.ts`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/tool/edit.ts`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/permission/`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/pty/`

借鉴点：

- `SessionProcessor` 管 tool call lifecycle：pending、running、completed、error、cleanup、snapshot。
- `Tool.define` 统一 schema decode、执行、truncation 和 metadata。
- shell 会扫描命令、路径和权限，输出过长时落盘并给模型可继续读取的路径。
- edit tool 有 per-file lock、BOM、换行、format、LSP diagnostics 和 snapshot diff。
- permission 是 ruleset 和 runtime ask，不是提示词里的提醒。

## 二次对照后的补充结论

重新看 Claude Code 和 opencode 源码后，这份计划需要补四个横切主题。它们不改变“三条主线”的顺序，但必须明确写进计划，否则后面容易继续靠 prompt 或局部补丁解决系统问题。

### 1. Context / Tool Output Budget 是主循环拆分的一部分

Claude Code 在 query loop 里把 tool result budget、snip、microcompact、autocompact、context collapse、memory prefetch、skill prefetch 放在模型请求前后的固定位置。opencode 也有 session compaction、overflow、summary 和 truncation service。

这说明上下文控制不是“优化项”，而是编码 agent 的主循环职责。Priority Agent 后续拆主循环时，必须把下面能力归到明确模块：

- tool result 过长时替换为 artifact 引用。
- 历史工具结果能被压缩，但最近关键证据不能丢。
- memory/skill/retrieval 是后台上下文，不应该阻塞简单任务。
- context overflow 是可恢复状态，不是普通 API 失败。

### 2. Permission / Ask / Reply 要成为产品路径

opencode 的 permission service 有 pending requests、approved ruleset、reject/corrected error、session permission persistence。Claude Code 的 `ToolPermissionContext` 也区分 allow/deny/ask、mode、additional working dirs、avoid prompts、automated checks。

这说明权限不是“是否展示工具”的布尔值。Priority Agent 需要把权限拆成：

- model-visible tool exposure。
- runtime permission evaluation。
- user approval / rejection / correction。
- session-scoped allow rules。
- permission denial 的可恢复提示。

### 3. Provider Protocol Transform 要有矩阵

opencode 的 provider transform 会按 Anthropic、Bedrock、Claude、DeepSeek、OpenAI-compatible 等模型处理空内容、tool id、tool result、reasoning、interleaved fields。我们最近遇到的 MiniMax 400 本质上就是 provider protocol transform 不够系统。

后续不能只在某个 provider 出错时修一次。需要建立 provider/tool-call roundtrip 矩阵，覆盖：

- assistant pure tool-call message。
- tool result follows tool call。
- empty assistant content。
- reasoning content 保留、隐藏或转换。
- streaming delta 合并。
- aborted / missing tool result 的 synthetic result。
- provider-specific bad request 的归因。

### 4. Todo / Task / Subagent 不是第一主线，但边界要固定

opencode 的 build、plan、general、explore agent 都是 permission ruleset 驱动；task tool 可以恢复 subagent session，truncate service 甚至会建议让 explore agent 处理长输出。Claude Code 也有 todo/task/agent 相关工具。

但对 Priority Agent 下一阶段来说，多 agent 不能抢主线。正确边界是：

- 单 agent coding loop 先稳定。
- todo 是用户可见任务状态，不是强制 planning 框架。
- subagent 只在长输出、宽代码搜索、独立审查时作为辅助。
- route/permission 决定 subagent 能做什么，不靠 prompt 约束。

## 第一性原则

1. LLM 负责理解、判断、写代码和解释。
2. runtime 负责工具、终端、权限、文件事实、证据、回滚和验收。
3. 硬约束必须放到 tool schema、permission、file state、terminal state、EvidenceLedger 和 tests。
4. 不用长 prompt 修补工具契约问题。
5. 简单任务不要被 workflow 框架绑住，复杂任务才启用 repair、validation、closeout。

## 非目标

- 不新增一个更大的 coordinator。
- 不把所有能力都默认暴露给模型。
- 不为了单个 live eval case 写特殊分支。
- 不把 Claude/opencode 的 TypeScript/React/Effect 结构机械翻译到 Rust。
- 不把“功能数量多”当作赶上 Claude Code 的证明。

## Phase 1：拆分主循环

目标：让 `ConversationLoop::run_inner` 退回到会话编排层，把具体职责移动到可测试的小模块。

当前已有拆分：

- `session_processor.rs`
- `tool_execution_controller.rs`
- `tool_result_controller.rs`
- `tool_orchestrator.rs`
- `repair_controller.rs`
- `closeout_controller.rs`
- `validation_runner.rs`
- `action_checkpoint.rs`
- `patch_recovery.rs`
- `patch_repair_rules.rs`
- `runtime_diet.rs`
- `runtime_timeouts.rs`
- `turn_recording.rs`

问题是这些模块很多仍然是 `impl ConversationLoop` 的横向切片，状态和职责还耦合在 `ConversationLoop` 上。下一步不是继续随便抽函数，而是建立清晰边界。

### Batch 1.1：主循环职责地图和行为冻结

任务：

- 给 `conversation_loop/mod.rs` 当前职责分区：
  - prompt/context assembly
  - intent route 和 resource policy
  - model request/streaming
  - tool exposure
  - tool call lifecycle
  - tool result normalization
  - evidence ledger
  - repair/action checkpoint
  - validation
  - closeout
  - memory/learning persistence
  - trace/session store
- 给每个职责标注目标模块和迁移状态。
- 写一个很小的 architecture note 或直接补到本文件的进度区。

验收：

```bash
cargo fmt --check
git diff --check
```

完成标准：

- 后续每个拆分 commit 都能说明“从哪个职责区搬到哪个模块”。
- 不再通过猜测判断主循环能不能继续拆。

### Batch 1.2：建立 `TurnRuntimeState`

参考：

- Claude `query.ts` 在每轮顶部显式拆出 state。
- opencode `ProcessorContext` 明确保存 toolcalls、snapshot、blocked、needsCompaction、currentText、reasoningMap。

任务：

- 新增或完善 `TurnRuntimeState`，承载一轮运行中状态：
  - route
  - resource policy
  - exposed tools
  - pending tool calls
  - pre-executed read-only results
  - changed files before/after
  - validation labels
  - closeout visibility
  - trace handles
- 把散落在 `run_inner` 的局部变量逐步收进 state。
- 第一批只移动数据承载，不改变行为。

验收：

```bash
cargo test -q route_scoped_tools -- --test-threads=1
cargo test -q runtime_diet -- --test-threads=1
cargo check -q
```

完成标准：

- `run_inner` 的局部状态数量明显减少。
- tool exposure、repair、closeout 读取 state，而不是互相传长参数串。

### Batch 1.3：让 `SessionProcessor` 成为状态机，而不是 helper 文件

参考：

- opencode `session/processor.ts` 用事件处理 `tool-input-start`、`tool-call`、`tool-result`、`finish-step`、cleanup。
- Claude `query.ts` 不依赖 `stop_reason` 判断工具调用，而是看实际 streamed tool_use block。

任务：

- 定义 `SessionProcessor` 的输入和输出：
  - input：messages、tools、route、runtime state、provider handle。
  - output：assistant text、tool calls/results、usage、finish reason、evidence events。
- 把 provider request、stream handling、tool-call collection 迁出 `mod.rs`。
- 给 tool call lifecycle 建立状态枚举：
  - pending
  - running
  - completed
  - failed
  - denied
  - provider_executed
- 把 streaming fallback、pre-executed read-only tool result、tool result attach 统一进状态机。

验收：

```bash
cargo test -q prompt_context -- --test-threads=1
cargo test -q route_scoped_tools -- --test-threads=1
cargo test -q tool_result -- --test-threads=1
cargo check -q
```

完成标准：

- `run_inner` 不再直接管理 streamed tool call 的内部生命周期。
- provider 400 类 tool result schema 问题有固定归属，不再散落在主循环。

### Batch 1.4：抽出 `ToolCallLifecycle` 和 `ToolResultNormalizer`

参考：

- opencode `Tool.define` 统一 decode、execute、truncate、metadata。
- Claude `ToolDef` 有 strict、max result size、validateInput、outputSchema。

任务：

- 从 `tool_execution_controller.rs` 里拆出：
  - 参数 schema 校验和错误格式化。
  - execution metadata。
  - provider-facing tool result content。
  - user-facing tool result summary。
  - truncation/large output handling。
- 让 bash/file/edit/git 的 result schema 进入统一 normalizer。
- 所有 tool result 都必须区分：
  - model content
  - UI content
  - structured metadata
  - evidence facts

验收：

```bash
cargo test -q provider -- --test-threads=1
cargo test -q evidence_ledger -- --test-threads=1
cargo test -q closeout -- --test-threads=1
cargo check -q
```

完成标准：

- provider-safe serialization 是 normalizer 的职责。
- closeout 不再从原始 stdout/stderr 里临时猜事实。

### Batch 1.5：建立 `ContextBudgetController`

参考：

- Claude `query.ts` 的 `applyToolResultBudget`、snip、microcompact、autocompact、context collapse。
- Claude memory / skill prefetch。
- opencode `tool/truncate.ts`、`session/compaction`、`session/overflow`、`session/summary`。

任务：

- 把上下文预算从主循环散点逻辑收敛为一个控制器：
  - model context used / remaining。
  - tool result aggregate size。
  - large output replacement records。
  - compaction boundary。
  - retained evidence window。
- 工具长输出不直接进入 messages；先进入 artifact，再给模型 preview + path。
- memory/skill/retrieval 改成可跳过、可延迟、可追踪的背景上下文。
- context overflow 触发可恢复路径，而不是普通 provider error。

验收：

```bash
cargo test -q runtime_diet -- --test-threads=1
cargo test -q prompt_context -- --test-threads=1
cargo test -q evidence_ledger -- --test-threads=1
cargo check -q
```

完成标准：

- 长工具输出不会挤掉最近代码和验证证据。
- compaction 后仍能解释关键事实来自哪里。
- 简单任务不会因为 memory/skill 检索而变慢或变复杂。

### Batch 1.6：建立 `PermissionController` 边界

参考：

- Claude `ToolPermissionContext`。
- opencode `permission/index.ts`、`permission/evaluate.ts`、`agent/agent.ts`。

任务：

- 把权限拆成四层：
  - registry availability。
  - route/role tool exposure。
  - runtime permission evaluation。
  - user ask/reply/persisted session rules。
- 建立 permission request 数据结构：
  - id
  - session id
  - permission kind
  - patterns
  - metadata
  - allowed always rules
  - rejection/correction feedback
- permission denied / rejected / corrected 都进入 ToolResultNormalizer 和 EvidenceLedger。
- 对 shell、file edit、external directory、task/subagent 使用同一套 permission path。

验收：

```bash
cargo test -q permissions -- --test-threads=1
cargo test -q tool_exposure -- --test-threads=1
cargo test -q bash_tool -- --test-threads=1
cargo check -q
```

完成标准：

- “工具不可用”能区分 registry、route、permission、platform、provider。
- 用户拒绝或修正权限后，模型得到可恢复信息，而不是泛化失败。
- 权限规则是 runtime contract，不靠长 prompt 约束。

### Batch 1.7：建立 provider protocol regression matrix

参考：

- opencode `provider/transform.ts`。
- opencode `session/llm.ts`。
- Claude query loop 的 missing tool result / abort synthetic result 处理。

任务：

- 为每个 provider family 建立消息转换用例：
  - OpenAI-compatible。
  - MiniMax。
  - Kimi。
  - Anthropic-like。
  - reasoning / thinking capable。
- 固定 tool-call roundtrip 场景：
  - assistant pure tool call。
  - assistant text + tool call。
  - empty content。
  - multiple tool calls。
  - tool result after abort。
  - tool result error。
  - reasoning content with tool calls。
- provider bad request 必须被归因为 schema/protocol/provider，而不是泛化成 LLM 失败。

验收：

```bash
cargo test -q provider -- --test-threads=1
cargo test -q openai_compat -- --test-threads=1
cargo test -q minimax -- --test-threads=1
cargo test -q kimi -- --test-threads=1
```

完成标准：

- MiniMax/Kimi/OpenAI-compatible 的 tool result 形状有固定回归测试。
- provider 400 不再通过人工看截图判断。

### Batch 1.8：拆 `RepairController` 和 deterministic repair 边界

任务：

- 把 repair 入口统一成：
  - failure evidence in
  - allowed repair budget in
  - proposed next action out
- deterministic patch synthesis 只能作为明确 fallback，并记录 owner/reason。
- action checkpoint 只约束“此刻允许哪些工具”，不要注入大量模型规则。

验收：

```bash
cargo test -q focused_repair -- --test-threads=1
cargo test -q action_checkpoint -- --test-threads=1
bash scripts/workflow-production-gates.sh
```

完成标准：

- repair 失败时能说明是 tool boundary、model reasoning、validation、provider 还是 harness。
- 不再通过增加 prompt 段落修 repair 行为。

### Phase 1 完成标准

- `conversation_loop/mod.rs` 从 5600+ 行降到 3500 行以内。
- 后续目标是 2500 行以内，但第一阶段不为行数破坏清晰度。
- 主循环只负责高层顺序：route、prompt、session processor、tool lifecycle、closeout。
- context budget、permission、provider protocol 都有独立边界，不再和主循环互相穿插。
- 每个核心行为都有独立测试入口。

## Phase 2：shell / terminal 一等化

目标：让 Priority Agent 在基本编程任务上像 Claude Code / opencode 一样可靠使用终端。

当前问题：

- `bash` 已可执行命令，但仍像普通工具。
- 长命令、后台任务、输出继续读取、交互式 PTY、取消、输出落盘还不完整。
- route/permission 隐藏 bash 时，模型有时只能给命令文本，用户体验会倒退。
- shell 权限、命令语义、输出 artifact、EvidenceLedger 还没有形成完整闭环。

### Batch 2.1：终端可见性和诊断

任务：

- 完善 `tool_exposure` 诊断：
  - registry 是否注册
  - tool 是否 available
  - permission 是否暴露
  - route 是否允许
  - provider/tool schema 是否兼容
- 在 `/status` 或 `/doctor` 暴露当前 bash 状态。
- 对用户问题“检查/安装/运行/测试/启动/默认 python/package”强制走 terminal-capable route。
- 把 terminal route 的诊断输出接入 PermissionController，而不是单独写一套判断。

验收：

```bash
cargo test -q tool_exposure -- --test-threads=1
cargo test -q intent_router -- --test-threads=1
```

完成标准：

- 用户问“帮我看看默认 python 有没有安装 pygame，帮我安装一下”时，模型能看到 `bash`。
- 如果不能看到，UI/诊断能说清楚具体原因。

### Batch 2.2：统一 `ShellCommandClassification`

参考：

- Claude `BashTool/commandSemantics.ts`
- Claude `BashTool/bashPermissions.ts`
- opencode `tool/shell.ts`

任务：

- 把现有 bash classifier 和 destructive scope 共享到一个语义层。
- 命令分类至少包括：
  - read
  - list
  - search
  - validation
  - package_install
  - dev_server
  - test_run
  - file_mutation
  - git_mutation
  - destructive
  - unknown
- 分类结果进入：
  - permission
  - progress label
  - EvidenceLedger
  - closeout
  - UI summary
- 分类器要输出 path patterns，供 permission ask/reply 使用。

验收：

```bash
cargo test -q bash_tool -- --test-threads=1
cargo test -q destructive_scope -- --test-threads=1
cargo test -q progress -- --test-threads=1
```

完成标准：

- shell 语义只维护一份，不在 bash tool、permission、trace、closeout 各写一套。

### Batch 2.3：Shell result schema 和输出落盘

参考：

- opencode `tool/truncate.ts`
- Claude tool result storage / BashTool output handling

任务：

- 标准化 shell result：
  - command
  - cwd
  - exit_code
  - stdout_preview
  - stderr_preview
  - output_path
  - duration_ms
  - timed_out
  - truncated
  - classification
  - evidence_status
- 超过阈值的 stdout/stderr 写入 `.priority-agent/tool-results/` 或 session artifact 目录。
- tool result 给模型的是预览和可继续读取路径，不直接塞完整长输出。
- `file_read` / `grep` 可以读取 output artifact。

验收：

```bash
cargo test -q bash_tool -- --test-threads=1
cargo test -q evidence_ledger -- --test-threads=1
cargo test -q closeout -- --test-threads=1
```

完成标准：

- 大输出不会污染上下文。
- 模型可以用工具继续检查完整输出。
- closeout 只引用结构化 evidence，不从截断文本猜。

### Batch 2.4：前台、后台、取消和继续读取

参考：

- Claude `LocalShellTask`
- opencode `pty/` 和 `shell/shell.ts`

任务：

- 新增 terminal task abstraction：
  - task id
  - command
  - cwd
  - status
  - started_at / ended_at
  - output artifact
  - cancel handle
- 支持：
  - foreground command
  - background command
  - read output by task id
  - stop task
  - timeout kill process group
- UI 显示 active shell task。

验收：

```bash
cargo test -q bash_tool -- --test-threads=1
cargo test -q terminal -- --test-threads=1
cargo check -q
```

完成标准：

- dev server、watch test、长安装命令不再卡死主 loop。
- 用户可以让 agent 启动服务，再继续读输出或停止。

### Batch 2.5：PTY 能力和交互式终端边界

任务：

- 先做非交互式 PTY smoke，避免一开始就扩大范围。
- 明确哪些命令应该走普通 `bash`，哪些应该走 PTY：
  - 普通测试、安装、脚本运行：bash
  - REPL、交互式 CLI、需要持续读取屏幕：PTY
- 如果 PTY 不可用，给出可诊断原因。

验收：

```bash
cargo test -q terminal -- --test-threads=1
cargo check -q
```

完成标准：

- terminal 能力有明确边界，不再出现“bash 工具不可用，只能让用户手动运行”的退化。

### Phase 2 完成标准

- 用户要求检查环境、安装包、运行脚本、启动项目时，agent 默认能实际执行。
- 长输出可落盘并继续读取。
- 长命令可后台运行、取消、读取输出。
- bash 结果进入 EvidenceLedger，最终回答不和命令事实矛盾。
- shell command permission、output artifact、task state 和 closeout 使用同一份结构化事实。

## Phase 3：文件编辑质量追上成熟编码 agent

目标：把文件编辑从“能替换文本”提升到“长期安全写代码”的产品级能力。

当前已有能力：

- 路径边界和只读根。
- 文件大小限制。
- stale-read 检测。
- line_start / line_end 编辑。
- 多 occurrence guard。
- checkpoint。
- 最近新增：read/edit 状态使用解析后的规范路径。

缺口：

- encoding / BOM / line ending 保真。
- per-file edit lock。
- atomic write。
- read-before-edit 默认策略还不够清晰。
- LSP/format feedback 和 edit result 没有深度集成。
- file history / rollback 和用户可见 diff 还没有达到 Claude/opencode 级别。
- read/search 工具输出和 file edit 输入之间仍需更强的 display/content 边界，避免把行号、截断提示、highlight 当成文件内容。

### Batch 3.0：读文件 / 搜索输出保真

参考：

- Claude Read/Grep/Glob 的 bounded output 和 file read state。
- opencode read/grep/glob 的 truncation metadata。

任务：

- 明确区分：
  - raw file content。
  - displayed content with line numbers。
  - search output with match context。
  - truncated output hint。
- `file_edit` 不能接受 display prefixes、grep decoration、truncation hints 当作真实内容。
- `file_read` 和 `grep` 输出进入 EvidenceLedger 时保留 raw fact metadata：
  - path
  - line range
  - total lines
  - displayed lines
  - truncated
  - content hash when available

验收：

```bash
cargo test -q file_tool -- --test-threads=1
cargo test -q grep -- --test-threads=1
cargo test -q evidence_ledger -- --test-threads=1
```

完成标准：

- 模型可以看到行号，但工具层不会把行号当成文件内容。
- search evidence 可以驱动 line-range edit，而不污染 patch anchor。

### Batch 3.1：文件身份和 read state 整理

任务：

- 把 `file_state_key`、read state、file cache、checkpoint 统一到一个 `FileStateTracker`。
- 明确 path identity：
  - lexical path
  - resolved path
  - canonical path
  - display path
- read state 记录：
  - full read vs partial read
  - content hash
  - mtime
  - line range
  - session id

验收：

```bash
cargo test -q file_tool -- --test-threads=1
```

完成标准：

- `./a.rs`、`a.rs`、`/abs/a.rs` 不再绕过 stale-read 检测。
- partial read 的编辑策略明确，不误认为完整上下文已经读过。

### Batch 3.2：encoding、BOM、line ending 保真

参考：

- Claude `FileEditTool` 的 encoding / line endings 处理。
- opencode `Bom.readFile`、`detectLineEnding`、`convertToLineEnding`。

任务：

- 读取文件时记录：
  - utf8 / utf16le / unknown
  - BOM
  - LF / CRLF
- 编辑写回时保留原编码和换行。
- 对 binary/unknown encoding 给出清晰错误。

验收：

```bash
cargo test -q file_tool -- --test-threads=1
```

测试用例：

- CRLF 文件编辑后仍是 CRLF。
- UTF-8 BOM 文件编辑后仍保留 BOM。
- binary 文件拒绝文本编辑。

### Batch 3.3：per-file lock 和 atomic edit

参考：

- opencode `edit.ts` 的 file lock。
- Claude FileEditTool 的读写临界区。

任务：

- 为每个 canonical path 建立 async lock。
- staleness check 和 write 在同一临界区完成。
- 写文件用临时文件加 rename，避免半写入。
- checkpoint 在写前创建，失败时不污染 read state。

验收：

```bash
cargo test -q file_tool -- --test-threads=1
cargo test -q checkpoint -- --test-threads=1
```

完成标准：

- 并发编辑同一文件不会互相覆盖。
- 写失败不会把文件状态标成成功编辑。

### Batch 3.4：diff、format、LSP diagnostics 进入 edit result

参考：

- Claude FileEditTool 的 patch、LSP notify、diagnostics。
- opencode edit tool 的 diff、format、LSP diagnostic report。

任务：

- `file_edit` result 返回：
  - file path
  - replacements
  - changed line range
  - additions/deletions
  - unified diff preview
  - diagnostics summary
- 若项目有 formatter，可按配置或 route 运行。
- LSP diagnostics 不阻塞所有编辑，但必须进入 evidence。

验收：

```bash
cargo test -q file_tool -- --test-threads=1
cargo test -q lsp -- --test-threads=1
cargo test -q closeout -- --test-threads=1
```

完成标准：

- 模型和最终回答都能引用真实 diff / diagnostics，而不是猜代码是否正确。

### Batch 3.5：文件历史和 rollback 产品化

任务：

- 把 checkpoint、file history、diff viewer、rollback 统一。
- 每次 edit/write 记录：
  - before hash
  - after hash
  - diff
  - tool call id
  - user/session id
  - timestamp
- `/rollback` 能按最近 edit/write 选择恢复。

验收：

```bash
cargo test -q checkpoint -- --test-threads=1
cargo test -q rollback -- --test-threads=1
cargo check -q
```

完成标准：

- 用户可以信任 agent 写代码，因为每次修改都有可解释、可恢复路径。

### Batch 3.6：多文件 patch / apply-patch 边界

参考：

- opencode `tool/apply_patch.ts`。
- Claude FileEditTool 的 per-file diff 和 permission path。

任务：

- 明确 `file_edit`、`file_write`、patch/apply-patch 的边界：
  - 单点替换：`file_edit`。
  - 新文件或完整替换：`file_write`。
  - 多文件原子 patch：专门 patch path。
- 多文件 patch 必须：
  - 逐文件 permission check。
  - 逐文件 stale-read check。
  - 生成统一 diff summary。
  - 失败时不产生半应用状态，或明确记录 partial failure。
- patch fallback 不能绕过文件编辑质量约束。

验收：

```bash
cargo test -q file_tool -- --test-threads=1
cargo test -q patch_recovery -- --test-threads=1
cargo test -q permissions -- --test-threads=1
cargo check -q
```

完成标准：

- 多文件修改有明确工具路径，不靠 bash heredoc 或 deterministic patch synthesis 偷偷绕过权限和 stale-read。

### Phase 3 完成标准

- file edit 对编码、换行、并发和外部修改安全。
- 文件修改结果有 diff、diagnostics 和 evidence。
- rollback 是正常产品路径，不是 debug fallback。
- 多文件 patch 和单文件 edit 共享 permission、state、diff、rollback 语义。

## Phase 4：基本编码质量回归集

目标：用少量稳定场景证明“基本编程质量”是否真的接近 Claude Code / opencode，而不是只看单次 live eval。

任务：

- 建立 `core-coding-quality` eval group，至少包含：
  - inspection-only：查看目录/文件，不编造大小、时间、数量。
  - simple edit：读文件后单点编辑，验证 stale-read。
  - multi-file edit：修改两个相关文件，验证 diff 和 tests。
  - terminal install/run：检查包、安装或解释不能安装的具体原因、运行脚本。
  - long output：命令输出过长，落盘后继续读取关键段。
  - provider roundtrip：pure tool call + tool result 在 MiniMax/Kimi/OpenAI-compatible 下不 400。
  - permission rejection：用户拒绝/修正后，模型按反馈恢复。
  - rollback：编辑后回滚并验证文件恢复。
- 每个 case 标注 failure_owner：
  - model_reasoning
  - tool_contract
  - permission
  - provider_protocol
  - terminal_runtime
  - file_state
  - harness
- 每个 case 都要有“Claude/opencode 借鉴点”和“Priority Agent 验收事实”。

验收：

```bash
bash -n scripts/run_live_eval.sh
python3 -m py_compile scripts/live_eval_report_parser.py
cargo test -q evidence_ledger -- --test-threads=1
```

完成标准：

- 每个阶段改动后都能跑同一组核心场景。
- 失败能定位到产品层，不再靠截图猜原因。

## 推荐执行顺序

严格按下面顺序推进：

1. Phase 1 Batch 1.1 到 1.4：先把主循环和工具生命周期边界稳住。
2. Phase 1 Batch 1.5 到 1.7：补 context budget、permission、provider protocol 这三个横切层。
3. Phase 1 Batch 1.8：最后再收 repair，避免 repair 继续绕过工具契约。
4. Phase 2 Batch 2.1 到 2.3：先让 bash 可见、可诊断、结果可靠。
5. Phase 2 Batch 2.4 到 2.5：再做后台任务和 PTY。
6. Phase 3 Batch 3.0 到 3.3：先做 read/search 保真、文件身份、编码、锁。
7. Phase 3 Batch 3.4 到 3.6：再做 diagnostics、history、rollback、多文件 patch。
8. Phase 4：用核心编码质量回归集验证每个阶段是否真的改善体验。

原因：

- 不先拆主循环，后面 terminal 和 file edit 会继续往上叠补丁。
- 不先让 terminal 可靠，基本编程任务还是会退化成“给用户命令”。
- 文件编辑质量很重要，但它依赖更清晰的 tool result、evidence、rollback 路径。
- context budget、permission 和 provider protocol 是三条主线的共同底座，缺它们会导致同类问题反复以不同形式出现。

## 每批通用验收

每个 batch 至少跑：

```bash
cargo fmt --check
cargo check -q
```

涉及工具、文件、终端、closeout 时补充：

```bash
cargo test -q file_tool -- --test-threads=1
cargo test -q bash_tool -- --test-threads=1
cargo test -q provider -- --test-threads=1
cargo test -q evidence_ledger -- --test-threads=1
cargo test -q closeout -- --test-threads=1
```

涉及 workflow / live eval 时补充：

```bash
bash -n scripts/run_live_eval.sh
bash scripts/workflow-production-gates.sh
```

涉及 provider/tool-call 协议时补充：

```bash
cargo test -q provider -- --test-threads=1
cargo test -q openai_compat -- --test-threads=1
```

大批次完成后跑：

```bash
cargo clippy --all-features -- -D warnings
cargo test -q
```

## 风险控制

- 每次只拆一个职责边界。
- 先移动代码，再改行为。
- 每个行为变化都要有回归测试。
- 如果某个改动需要新增大量 prompt 规则，先暂停，改成 tool/runtime contract。
- 如果某个 eval case 只能靠 special-case 通过，先记录为产品差距，不直接编码分支。

## 成功标准

下一阶段完成后，Priority Agent 应该达到下面状态：

- 主循环足够薄，新增工具或修复 closeout 不会误伤 streaming/provider/repair。
- shell 是可靠的一等能力，能运行、后台、取消、读输出、解释失败。
- file edit 具备成熟编码 agent 的基本安全性：read state、stale check、encoding、line ending、lock、diff、diagnostics、rollback。
- provider/tool-call 协议有矩阵测试，不再靠线上 400 才发现。
- context budget 和 tool output artifact 能保护长任务上下文。
- EvidenceLedger 从“评测辅助”变成日常回答和 closeout 的事实来源。
- 用户看到的是自然的编码 agent，而不是被规则和框架牵着走的模型。
