# Opencode Agent Engine 对照与下一阶段优化计划

日期：2026-06-06

## 目标

本文逐项对照 `~/Downloads/opencode-dev/packages/opencode/src` 与当前
`priority-agent` 的 agent 工程链路，重点回答 4 个问题：

- opencode 在关键工程维度上怎么做；
- 当前项目已经怎么做；
- 还有哪些差距、风险和冗余；
- 下一阶段应借鉴 opencode 做哪些具体优化。

总体判断：当前项目已经具备更强的本地记忆、证据账本、验证 closeout、
checkpoint、usage ledger 和 runtime diet 能力；opencode 的优势不在“规则更多”，
而在“链路更清楚、产品表面更稳定、每个工具结果更容易被用户看懂和回退”。下一
阶段不应继续堆新能力，而应把现有能力压成更稳定、更可见、更像日常工具的路径。

## 1. 工具边界与工具语义

### opencode 怎么做

源码入口：

- `src/tool/registry.ts`
- `src/tool/tool.ts`
- `src/session/tools.ts`
- `src/acp/tool.ts`

opencode 的工具注册非常集中。内置工具由 `ToolRegistry` 统一收集，插件工具也在
注册边界被转换成同一个 `Tool.Def` 形态。`session/tools.ts` 在工具执行前统一注入
`ctx.ask`、session/message/call 元数据、插件 before/after hook 和输出截断。ACP 层
再把工具映射成较粗的 `kind`，例如 shell 是 execute，read 是 read，grep/glob 是
search，write/edit/apply_patch 是 edit。

这个设计的关键不是工具少，而是“工具身份稳定”：权限、UI、事件、usage 和外部
协议都能根据同一套工具语义工作。

### rust-agent 现状

源码入口：

- `src/tools/schema.rs`
- `src/tools/tool_trait.rs`
- `src/tools/mod.rs`
- `src/engine/conversation_loop/tool_metadata.rs`
- `src/engine/conversation_loop/permission_controller.rs`

当前项目已经补上 `ToolOperationKind`、`ToolKind`、`ToolFamily`，也把工具语义传给
permission 和 metadata。这个方向是对的，比只按工具名判断更稳。

### 差距与风险

- 语义已经进入 Rust 类型层，但 TUI/desktop 展示、session store、usage/debug 输出
  还没有全部统一消费这套语义。
- 一些老工具仍可能只靠 `name()` 和 `operation_kind()` 被识别，未来新增工具容易
  漏填 `tool_kind` / `tool_family`。
- ACP 风格的外部工具语义还没有形成独立兼容层，后续接外部客户端时可能继续把
  内部工具细节暴露出去。

### 可借鉴优化

优先级：高。

- 增加工具语义一致性测试：遍历 registry，断言写文件工具都是 `ToolFamily::Edit`，
  shell 永远是 `ToolFamily::Shell`，MCP/plugin 不默认被归入 edit。
- 让 TUI/desktop 的工具卡片优先用 `ToolKind` / `ToolFamily` 渲染，而不是只用工具名。
- 为未来 ACP/外部 API 增加一层 `ToolSemanticView`，只输出稳定语义、标题、风险、
  permission key、diff/diagnostics metadata，不暴露内部复杂字段。

## 2. Bash 与文件修改纪律

### opencode 怎么做

源码入口：

- `src/tool/shell.ts`
- `src/tool/shell/prompt.ts`
- `src/tool/write.ts`
- `src/tool/edit.ts`
- `src/tool/apply_patch.ts`

opencode 的 shell prompt 明确要求：文件操作优先使用专门工具，而不是 shell。实际
执行时，shell 会解析命令 AST，提取命令、路径和 pattern，再按 bash permission
发起询问。它不是完全禁止 shell 写入，但产品上明确把 write/edit/apply_patch 作为
主编辑路径。

write/edit/apply_patch 的共同特征：

- 先读取旧内容并生成 diff；
- 权限请求使用统一的 `permission: "edit"`；
- permission metadata 带上 diff 和文件路径；
- 写入后自动 format；
- 发布文件更新事件；
- 触发 LSP diagnostics；
- 工具输出直接告诉模型 diagnostics 是否需要修复。

### rust-agent 现状

源码入口：

- `src/tools/bash_tool/mod.rs`
- `src/tools/bash_tool/command_classifier/`
- `src/tools/file_tool/`
- `src/engine/action_review.rs`
- `src/engine/conversation_loop/tool_batch_result_processor.rs`
- `src/permissions/mod.rs`

当前项目已经更严格：raw bash workspace mutation 会被 action review/checkpoint 规则
拦截，最近也加入了“bash 文件修改失败后提示模型改用 file_write/file_edit/file_patch”
的修复提示。权限层的 `edit` alias 也覆盖 file_write/file_edit/file_patch/format，
但不会把 bash 误归入 edit。

### 差距与风险

- opencode 的专门编辑工具会把 diff、format、LSP diagnostics 自然串在一起；当前项目
  的文件工具、checkpoint、diagnostic、TUI 展示仍更分散。
- bash 拦截后已经会提示改用文件工具，但如果模型一直弱，仍可能反复尝试 shell
  heredoc/redirect，需要更强的工具路由和失败分类。
- 当前 TUI 对“为什么 bash 被挡住、下一步应该点什么或用什么工具”还不够产品化。

### 可借鉴优化

优先级：高。

- 把 file_write/file_edit/file_patch 的结果 metadata 统一成 `{diff, files,
  diagnostics, checkpoint_id, formatted}`。
- TUI/desktop 的 permission prompt 对 edit family 默认展示 diff；对 bash file
  mutation block 默认展示“请用文件工具”的明确恢复路径。
- route 层对编程任务进一步收窄工具面：编辑阶段优先暴露 file tools；bash 主要用于
  读取状态、运行测试、启动服务。
- 增加真实任务 gate：如果代码修改完全由 bash 写入完成，应记录为 framework risk，
  除非是在明确允许的生成 artifact / fixture 场景。

## 3. 记忆、长期上下文与项目指令

### opencode 怎么做

源码入口：

- `src/session/system.ts`
- `src/session/instruction.ts`
- `src/session/compaction.ts`
- `src/skill/`
- `src/session/prompt.ts`

opencode 没有当前项目这种完整长期记忆系统。它更依赖：

- AGENTS/instruction 类项目指令；
- skill 列表和按需 skill tool；
- session compaction summary；
- references/file attachments；
- config/agent permission 控制。

换句话说，opencode 的“记忆”主要是项目指令、技能、会话摘要和显式上下文，而不是
自动主动沉淀个人长期记忆。

### rust-agent 现状

源码入口：

- `src/memory/manager/mod.rs`
- `src/memory/retrieval.rs`
- `src/memory/provider/`
- `src/engine/conversation_loop/request_preparation_controller.rs`
- `src/engine/retrieval_context.rs`

当前项目的记忆明显更强：有冻结快照、按轮 prefetch、memory files、records.jsonl、
decision log、quality gate、review/proposal、provider registry、scope、flush lifecycle。
这比 opencode 更接近 personal agent 的方向。

### 差距与风险

- 能力强，但产品解释成本高。用户需要知道什么时候记、记什么、存在哪、什么时候用。
- 记忆 prefetch 虽有冻结和 cache stability 设计，但动态注入仍需持续监控 cache miss。
- 记忆 review UX 还不如 opencode 的 permission/diff 那样可见、可操作。

### 可借鉴优化

优先级：中高。

- 保留当前长期记忆优势，不退回 opencode 的轻量模型。
- 借鉴 opencode 的简单性，把记忆用户界面分成 3 个动作：`/memory status`、
  `/memory review`、`/memory files`，让用户能直接看到来源、作用域、是否会进 prompt。
- 在 usage ledger 和 cache diagnostic 中把 memory/skill 造成的 miss 单独列出来。
- 对“自动主动记住”保持 review gate：模型可提议，runtime 可过滤，真正长期写入应可追踪、
  可撤销、可解释。

## 4. 权限、风险与用户确认

### opencode 怎么做

源码入口：

- `src/permission/index.ts`
- `src/session/tools.ts`
- `src/cli/cmd/tui/routes/session/permission.tsx`

opencode 的权限规则很清楚：规则集按 permission/pattern 匹配，默认 ask；pending
permission 通过事件发布到 UI，用户可以 once/always/reject。reject 可带反馈，
反馈会作为工具失败返回给模型。edit/write/apply_patch 被折叠为同一个 edit 权限。

### rust-agent 现状

源码入口：

- `src/permissions/mod.rs`
- `src/engine/action_review.rs`
- `src/engine/action_policy.rs`
- `src/tui/app.rs`
- `src/tui/permission_diff.rs`

当前项目的权限更复杂：有 PermissionMode、RuleSource、once TTL、bash command
classifier、action review、checkpoint requirement、高风险路径、destructive scope。
这比 opencode 更安全，但也更难解释。

### 差距与风险

- opencode 的 permission UX 是核心产品路径；当前项目 permission 的底层强，但 UI
  还没有足够直观地呈现“规则命中、风险原因、diff、持久化范围”。
- deny/allow/ask 优先级和 last-match 规则语义与 opencode 不完全一致，文档和 UI
  必须清楚说明，避免用户以为配置行为一样。

### 可借鉴优化

优先级：高。

- TUI permission 弹窗分区展示：工具语义、匹配规则、风险原因、diff/命令、允许一次、
  本会话允许、项目允许、拒绝并反馈。
- reject feedback 进入 ToolObservation，让模型知道不是工具坏了，而是用户或规则拒绝。
- `/permissions explain <tool>` 输出实际匹配 key，例如 `edit`、`bash:git status`、
  `mcp/server/tool`。

## 5. Checkpoint、diff、rollback 与会话回退

### opencode 怎么做

源码入口：

- `src/snapshot/index.ts`
- `src/session/processor.ts`
- `src/session/summary.ts`
- `src/session/revert.ts`
- `src/cli/cmd/tui/routes/session/dialog-message.tsx`

opencode 每个 assistant step 会先 track snapshot，step finish 后生成 patch，并把 diff
写入 session summary。用户可以 revert 到某个 message/part；revert 同时处理会话消息
和文件 patch，TUI 里 `/undo` 是日常能力。

### rust-agent 现状

源码入口：

- `src/engine/checkpoint.rs`
- `src/engine/action_review.rs`
- `src/engine/task_context/state.rs`
- `src/tui/commands/catalog.rs`
- `src/tui/app.rs`

当前项目已有 checkpoint manager、rollback candidates、`/checkpoints`、`/restore`、
`/rollback`。而且 action review 对 raw bash mutation 的 checkpoint 边界更严格。

### 差距与风险

- 当前 rollback 更像“安全机制”，opencode 的 undo 更像“正常交互按钮”。
- session message 回退与文件 rollback 的耦合不如 opencode 顺滑。
- TUI/desktop 的 diff/revert 面板还可以更日常化：用户应能按 turn 看到文件变化并一键回退。

### 可借鉴优化

优先级：中高。

- 增加 message/turn 级变更摘要：每轮记录 changed files、additions、deletions、
  checkpoint ids、validation proof。
- TUI 增加 `/undo` alias，明确表示回退最近一次 assistant 文件变更；保留 `/rollback`
  给更高级目标。
- desktop 状态页展示最近 turn 的 diff 和可回退点。

## 6. 验证、修复循环与 closeout 证据

### opencode 怎么做

源码入口：

- `src/session/processor.ts`
- `src/tool/write.ts`
- `src/tool/edit.ts`
- `src/tool/apply_patch.ts`
- `src/lsp/`

opencode 的验证更偏工具结果自然反馈：编辑后 LSP diagnostics 直接进工具输出，shell
命令有 timeout/truncation metadata，doom loop threshold 防止重复工具循环。它没有当前
项目这样强的 closeout proof 体系。

### rust-agent 现状

源码入口：

- `src/engine/conversation_loop/closeout_controller.rs`
- `src/engine/verification_proof.rs`
- `src/engine/evidence_ledger.rs`
- `src/engine/code_change_workflow.rs`
- `src/engine/auto_verify.rs`

当前项目在 verified closeout 上更强：代码修改类任务会检查 validation required、
runtime validation label、tool evidence、changed files、proof status，并阻止虚假成功。

### 差距与风险

- closeout 证据强，但模型/用户看到的实时修复线索不总是像 opencode 的 diagnostics
  那样直接。
- 弱模型失败时，系统可能产生较多 runtime hint/context zone，影响 token 和缓存。
- LSP diagnostics 和 required validation 的关系还可以更稳定：什么时候必须修、什么时候
  标记 residual risk，应有明确策略。

### 可借鉴优化

优先级：高。

- 文件工具执行后，如果有 diagnostics，统一进入 `ToolObservation` 和 TUI 诊断展示。
- closeout 里区分 3 类证据：tool evidence、validation command evidence、diagnostic evidence。
- daily real-task baseline 记录“最终是否修对、用了几轮工具、失败 owner、是否为弱模型问题”。

## 7. 上下文压缩、prompt cache 与 token 控制

### opencode 怎么做

源码入口：

- `src/session/compaction.ts`
- `src/session/overflow.ts`
- `src/session/llm/request.ts`
- `src/provider/transform.ts`

opencode 对上下文控制比较直接：

- 根据模型 context/input limit 和 max output reserve 判断 overflow；
- 自动或手动 `/compact`；
- compaction 保留最近 tail turns，并生成 anchored summary；
- maxOutputTokens 统一来自 provider transform 和 runtime flag；
- provider transform 支持 cache control / prompt cache key / reasoning effort。

### rust-agent 现状

源码入口：

- `src/engine/cache_stability.rs`
- `src/engine/context_assembly.rs`
- `src/engine/conversation_loop/runtime_diet.rs`
- `src/engine/conversation_loop/context_budget_controller.rs`
- `src/engine/conversation_loop/request_preparation_controller.rs`
- `src/cost_tracker/usage_ledger.rs`

当前项目已经有静态前缀 fingerprint、tool schema manifest、dynamic tail fingerprint、
miss reason inference、runtime diet、输出上限和 usage ledger。这部分已经明显受
reasonix/opencode 启发，方向正确。

### 差距与风险

- request preparation 里仍有多个 dynamic context zone：task-state、task-contract、
  focused repair、memory prefetch、project map、context ledger hint、自进化 guidance。
  这些有价值，但容易造成 cache miss 或 prompt 变重。
- opencode 的 compaction 是用户能理解的 `/compact`；当前项目的 context budget/runtime
  diet 仍偏内部 trace。

### 可借鉴优化

优先级：高。

- 将 dynamic zone 分成 `stable-prefix eligible`、`last-user dynamic`、`repair-only`
  三类，并在 trace/usage ledger 中记录 zone 分类。
- `/cost` 显示最近一次 miss reason、tool_schema_tokens、dynamic_tail_hash 是否变化。
- 增加 `/compact` 或 `/context compact` 的显式用户操作，即使内部已有压缩，也要让用户
  有一个可理解入口。
- 对 repeated repair turns 自动降低输出上限、减少工具 schema、只保留相关文件和失败证据。

## 8. Provider 慢尾、usage/cost 与真实账本

### opencode 怎么做

源码入口：

- `src/acp/usage.ts`
- `src/session/llm/request.ts`
- `src/session/llm/ai-sdk.ts`
- `src/session/llm/native-runtime.ts`
- `src/provider/provider.ts`
- `src/provider/transform.ts`

opencode 会从 assistant message tokens/cost 构造 usage update，并在 TUI/footer 展示
context 使用量和总成本。provider 层支持 header timeout、request timeout、max output、
cache read/write cost、cache key 等。

### rust-agent 现状

源码入口：

- `src/cost_tracker/usage_ledger.rs`
- `src/cost_tracker/prompt_cache.rs`
- `src/engine/streaming.rs`
- `src/tui/app.rs`
- `src/tui/commands/catalog.rs`

当前项目已经有 `usage.jsonl` 和 SQLite projection，字段包含 session、model、prompt、
completion、cached、cache_miss、cost、stable_prefix hash、tool_schema hash、
miss_reason、output cap、tool round count、compaction decision。这比 opencode 的 ACP
usage 更适合做本地账本和 daily baseline。

### 差距与风险

- ledger 强，但仍需确认所有 provider path 都稳定落账，包括失败、timeout、abort、
  fallback 和 streaming partial。
- TUI/desktop 的 usage 展示应从真实 ledger 读取，而不是只看内存态。
- 慢尾需要从“请求耗时分布、首 token、完成耗时、timeout reason、provider/model”维度
  记录，不能只记录 token。

### 可借鉴优化

优先级：高。

- usage ledger 增加或确认字段：request_id、provider、latency_ms、time_to_first_token_ms、
  finish_reason、error_kind、timeout_kind、retry_count。
- `/cost`、desktop status、daily baseline 全部基于 ledger 汇总。
- 对 provider timeout 做产品策略：短任务默认较小输出 cap；修复循环降 cap；长任务需要
  明确进入 long-running mode。

## 9. TUI/CLI/Desktop 产品化路径

### opencode 怎么做

源码入口：

- `src/cli/cmd/tui/routes/session/index.tsx`
- `src/cli/cmd/tui/routes/session/permission.tsx`
- `src/cli/cmd/tui/feature-plugins/system/session-v2.tsx`
- `src/cli/cmd/tui/feature-plugins/system/diff-viewer.tsx`
- `src/cli/cmd/tui/feature-plugins/sidebar/files.tsx`
- `src/cli/cmd/tui/feature-plugins/system/notifications.ts`

opencode 把核心 runtime 状态直接变成 TUI 产品体验：permission 待处理提示、diff viewer、
diagnostics、usage footer、undo/compact、sidebar changed files、notification。用户不需要
理解内部模块，也能知道 agent 正在做什么。

### rust-agent 现状

源码入口：

- `src/tui/app.rs`
- `src/tui/tool_view.rs`
- `src/tui/runtime_panels.rs`
- `src/tui/commands/catalog.rs`
- `src/engine/runtime_facade.rs`
- `src/engine/runtime_controller.rs`

当前项目 TUI 已经有工具状态、permission、usage snapshot、runtime panels、diff、
checkpoints、restore、rollback、cost 等命令。desktop 也走 `StreamingQueryEngine`，
不是单独逻辑。

### 差距与风险

- 命令很多，但默认路径还不够“少而强”。opencode 的常用入口更直觉：permission、
  diff、undo、compact、usage。
- runtime panels 偏工程调试；日常用户界面需要更直接地展示“当前是否安全、花了多少、
  改了什么、能否回退、验证了吗”。

### 可借鉴优化

优先级：中高。

- TUI 首页/底部固定展示：pending permission、usage/cost、changed files、validation status。
- `/panel` 继续保留给 debug；新增或强化日常命令 `/changes`、`/undo`、`/cost`、
  `/validate`、`/memory`。
- desktop status 与 TUI 使用同一个 runtime facade snapshot，避免两个产品面状态漂移。

## 10. 观测、trace、事件与可复盘性

### opencode 怎么做

源码入口：

- `src/event-v2-bridge`
- `src/session/processor.ts`
- `src/session/summary.ts`
- `src/server/routes/instance/httpapi/groups/session.ts`
- `src/cli/cmd/export.ts`

opencode 的事件流贯穿 session、tool、permission、diff、usage 和 TUI plugin。session 可
导出，HTTP API 可查询 session/messages/diff/revert/permission。

### rust-agent 现状

源码入口：

- `src/engine/trace.rs`
- `src/session_store/`
- `src/engine/evidence_ledger.rs`
- `src/engine/evalset.rs`
- `src/services/api/`

当前项目 trace 和 eval evidence 很强，能分析 failure_owner、proof、permission、
checkpoint、runtime diet、usage ledger。但很多能力服务于测试和调试，还没完全变成
稳定的用户/外部 API。

### 差距与风险

- trace 事件多，如果缺少稳定导出 schema，后续 daily baseline 和桌面诊断容易脆。
- 用户遇到问题时，需要一键导出“可复盘包”，而不是手动找 trace/session/db/log。

### 可借鉴优化

优先级：中。

- 定义 `run_report.json` 稳定 schema：request、tool rounds、changed files、proof、
  usage、timeout、permission、memory、failure_owner。
- TUI/desktop 增加“导出本次运行诊断包”的命令/按钮。
- daily baseline 只依赖稳定 report schema，不直接解析临时日志文本。

## 11. 工程可维护性、测试夹具与主线稳定

### opencode 怎么做

源码入口：

- `src/tool/*`
- `src/session/*`
- `src/cli/cmd/tui/*`
- `src/server/routes/*`

opencode 的模块边界很清楚：tool、permission、session、provider、TUI、server routes。
每条链路不一定比当前项目更强，但更容易定位：工具问题看 tool，权限问题看 permission，
会话问题看 session，界面问题看 TUI plugin。

### rust-agent 现状

源码入口：

- `src/engine/conversation_loop/`
- `src/tools/`
- `src/permissions/`
- `src/tui/`
- `fixtures/`
- `docs/PROJECT_MAP.md`

当前项目已经做过大规模瘦身和模块拆分，主文件长度和边界比早期健康很多。近期也加入
真实任务 fixtures，用来支撑 CLI/TUI 编程链测试。

### 差距与风险

- `engine/conversation_loop` 下控制器很多，能力强但认知成本高。
- eval、fixtures、TUI expect、daily baseline 的关系需要更清楚：哪些用于发现框架问题，
  哪些用于模型能力观察，哪些是发布 gate。
- 文档较多，需要继续以 `PROJECT_STATUS.md` / `PROJECT_MAP.md` 为锚，避免计划文档堆积。

### 可借鉴优化

优先级：中。

- 建立“真实任务稳定性矩阵”：fixture、入口、模型、期望工具链、期望验证、failure_owner。
- 对 `conversation_loop` 控制器做职责索引，不急着继续拆文件，先保证每个控制器有 owner
  和测试。
- 清理过期计划文档，把完成状态沉淀到 `PROJECT_STATUS.md`，把入口沉淀到
  `PROJECT_MAP.md`。

## 下一阶段分期计划

### Phase A：工具与权限产品化硬化

目标：让工具语义、权限、diff、诊断成为同一条稳定链路。

- 为所有内置工具补齐并测试 `ToolKind` / `ToolFamily`。
- 统一 file_write/file_edit/file_patch metadata：diff、files、diagnostics、checkpoint。
- TUI permission prompt 对 edit family 展示 diff，对 bash mutation block 展示替代工具建议。
- `/permissions explain` 输出实际匹配 key 和规则来源。

建议验证：

```bash
cargo test -q permissions
cargo test -q tool_metadata
cargo test -q route_scoped_tools
cargo test -q tool_batch_result_processor
```

### Phase B：TUI/Desktop 日常操作面

目标：把 opencode 的 permission/diff/undo/usage/compact 思路落到当前产品。

- 增强 `/cost`，直接读取 usage ledger，显示最近 miss reason 和 token 分解。
- 增加 `/changes` 或增强 `/diff`，展示最近 turn changed files、additions、deletions。
- 增加 `/undo` alias，面向最近文件变更回退；保留 `/rollback` 给高级恢复。
- desktop status 与 TUI runtime facade 使用一致字段。

建议验证：

```bash
cargo test -q tui
cargo test -q runtime_facade
cargo test -q session_store
```

### Phase C：Provider 慢尾与真实 usage baseline

目标：让 token、cache、慢尾、错误都进入真实账本。

- usage ledger 覆盖成功、失败、timeout、abort、partial streaming。
- 记录 provider/model/request latency、first token latency、finish reason、retry count。
- daily baseline 输出 usage/cost/latency 分布，而不是只看最终任务成败。
- 修复循环按风险和上下文压力自动降低 output cap。

建议验证：

```bash
cargo test -q cost_tracker
cargo test -q prompt_cache
cargo test -q runtime_timeouts
bash scripts/workflow-production-gates.sh
```

### Phase D：记忆 UX 与 cache 卫生

目标：保留当前项目的长期记忆优势，但让使用方式更清楚、更可控。

- `/memory status` 展示冻结快照、retrieval items、active files、records count。
- `/memory review` 聚合 proposed/rejected/accepted，支持接受和拒绝。
- cache diagnostic 明确 memory/skill 是否导致 miss。
- 对 memory prefetch 加真实任务 baseline：命中是否帮助修复，是否增加无效 token。

建议验证：

```bash
cargo test -q memory
cargo test -q prompt_context
cargo test -q cache_stability
```

### Phase E：真实编程任务发布 gate

目标：把 eval、fixtures、TUI expect 合并成能发现框架缺陷的稳定测试方法。

- 建立 `fixtures/` 真实任务矩阵：前后端、小 CLI、数据库 pipeline、修 bug、重构。
- 每个任务记录期望工具链：read/search -> edit/patch -> test -> closeout。
- failure_owner 明确区分 `framework`、`provider_model`、`harness`、`environment`。
- 发布前固定跑一组 daily baseline，并保存 run_report。

建议验证：

```bash
cargo test -q evalset
bash -n scripts/run_live_eval.sh
python3 -m py_compile scripts/live_eval_report_parser.py
```

## 非目标

- 不为了模仿 opencode 而移除当前项目更强的记忆、verification proof、checkpoint
  和 usage ledger。
- 不放松权限、checkpoint、高风险 gate 来让弱模型更容易通过。
- 不把 bash 重新变成主要写文件工具；bash 应主要用于读取状态、运行测试和启动服务。
- 不继续无限增加 prompt 规则；优先做工具契约、runtime check、TUI/desktop 产品化。

## 结论

当前项目和 opencode 的主要差距不是“缺少某个核心算法”，而是工程链路的产品化程度：
opencode 把工具、权限、diff、diagnostics、usage、undo、compact 都做成了用户每天能
看见和操作的稳定表面。当前项目底层更强，但还需要把这些能力压缩成更少、更直观、
更一致的入口。

下一阶段最值得做的是 Phase A 到 Phase C：工具语义和权限可见化、TUI/Desktop 日常
操作面、provider/usage/慢尾账本。这三块做好后，真实编程任务测试会更容易定位问题，
也能更清楚地区分框架缺陷和弱模型本身的问题。
