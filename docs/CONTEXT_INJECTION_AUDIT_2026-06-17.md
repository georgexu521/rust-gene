# 上下文注入系统审计报告

日期：2026-06-17  
更新：2026-06-18

## 审计范围

本报告对当前代码中的上下文注入、上下文预算和压缩链路进行审计，覆盖：

- 稳定系统提示：`src/instructions/mod.rs`、`src/engine/prompt_context.rs`、`src/engine/prompt_builder.rs`；
- typed zone 模型：`src/engine/context_assembly.rs`；
- turn bootstrap 注入：`src/engine/conversation_loop/turn_request_bootstrap_controller.rs`；
- per-request 动态注入：`src/engine/conversation_loop/request_preparation_controller.rs`；
- 记忆和检索上下文：`memory_snapshot_controller`、`turn_retrieval_context_controller`、`src/engine/retrieval_context.rs`；
- 预算和压缩：`preflight_compression_controller`、`context_budget_controller`、`context_compressor`、`message_compression`、`message_healing`；
- cache 稳定性观测：`src/engine/cache_stability.rs`。

`opencode` 对比只作为设计参考，不作为本次结论的主要证据；本报告优先以当前 repo 代码为准。

## 审计结论

**整体评估：功能链路完整，但文档需要更精确地区分稳定前缀、动态尾部、压缩和 cache 观测。**

当前上下文系统已经不是单一“prompt 拼接器”，而是多阶段运行时管线：

1. 稳定前缀由 base prompt、AGENTS.md 和根上下文层组成；
2. 动态材料主要被前置到最后一条 user message，以减少 system prefix 变化；
3. 运行时用 5 个 typed zones 记录和追踪上下文形态；
4. turn bootstrap 阶段先处理 memory snapshot、preflight compression、stream start 和 retrieval prompt；
5. request preparation 阶段继续注入 task state、task contract、context ledger、project map、memory prefetch、task guidance 等动态块；
6. request 发送前会记录 context zone、token breakdown、cache stability、budget，然后做旧工具输出选择性压缩和 message healing；
7. provider 报 context limit 时，API 层还有 reactive compaction retry。

原始文档方向基本正确，但存在几类问题：

- 把“动态上下文多次 prepend”直接判断为“可能前置两次”过强；当前代码有 typed zone 记录、current decision 清理、system-zone envelope 归一化和部分去重，但确实仍缺一个统一动态块收集器。
- 对压缩链路描述不全：除了 preflight 和 selective compression，还有 streaming pre-query compression、API reactive compression、message healing 和 runtime diet/circuit breaker。
- 对 cache stability、context budget trace、tool schema token、dynamic tail fingerprint 的覆盖不足。
- 对未使用函数和死代码的判断需要收窄：部分原始条目已不存在或已改为实验/诊断保留，当前只剩少量维护项。
- 标签命名确实不一致，但当前代码已经兼容 `task-state` 和 `task_state`，问题更准确地说是“规范和共享检测函数未统一”，不是已知运行时 bug。

## 当前上下文管线

### 1. 稳定系统提示

- `PromptContextAssembler` 通过 `instructions::compose_system_prompt` 组合 base prompt、AGENTS.md 和根上下文层。
- `instructions` 会对 root context 做 prompt-injection safety scan、XML escape、按层预算截断。
- `context_assembly` 建模 5 个 zones：`stable_prefix`、`task_state`、`relevant_material`、`recent_observation`、`current_decision_request`。
- 当前 legacy render 仍只把 `stable_prefix + task_state` 放入系统提示；大量动态材料在运行时转入 user message tail。

### 2. Turn bootstrap 注入

`TurnRequestBootstrapController` 当前顺序是：

1. `MemorySnapshotController::inject`：注入稳定 memory snapshot；
2. `PreflightCompressionController::run`：在请求前根据 budget 做最多 3 pass compaction；
3. 发送 `StreamEvent::Start`；
4. `RetrievalPromptController::inject`：把 retrieval context 放进 `<relevant_material>` 后前置到最后一条 user message。

这个顺序比原始文档中的流程更准确：preflight compression 发生在 retrieval prompt 注入前，request preparation 阶段的动态块注入发生在后续 per-request prepare 中。

### 3. Request preparation 注入

`RequestPreparationController::prepare` 在 `inject_dynamic_context=true` 时会依次处理：

- `<task-state>`：来自 `AgentTaskState`；
- `<task-contract>` 和 `<context-pack>`：来自 task contract；
- MVA/candidate action hint：作为 `<recent_observation>`；
- self-evolution guidance；
- focused repair hint；
- context ledger hint：把已读文件、bash 只读、编辑、diff、验证、确认、tool observation 分入 `relevant_material` / `recent_observation`；
- project map；
- task guidance；
- memory prefetch：当 turn retrieval context 里没有 memory item 时补充 LLM rerank / dialectic memory retrieval；
- context zone envelope normalization；
- context zone/token/cache/budget trace；
- selective tool-output compression；
- message healing；
- `ChatRequest` 组装。

### 4. Zone 归一化和 cache 观测

- `normalize_context_zone_envelope` 当前只消费 system message 中的动态 zone，并把它们合并为一个 `<context_zones>` block 再前置到 user message。
- 多数新动态块本来就是通过 `prepend_to_last_user_message` 写入 user message，因此不会被 system-zone envelope 再消费。
- `record_context_zones` 会从 system/user message 提取 zone 内容，记录 token、fingerprint、budget overflow、source message count、dedupe count 和 provenance marker。
- `cache_stability` 会区分 stable prefix、dynamic tail、tool schema manifest 和 dynamic context message，用于解释 cache miss。

当前设计目标是让稳定系统前缀尽量不变，把 volatile context 放进 dynamic tail。这个方向合理，但动态块收集和标签检测仍有重复实现。

### 5. 压缩和恢复链路

当前至少有四条相关路径：

1. **Preflight compaction**：`PreflightCompressionController` 在请求前根据 context budget 触发 `ContextCompressor`。
2. **Streaming pre-query compression**：streaming engine 在 query 前也会检查历史并压缩。
3. **API reactive compression**：provider 返回 context size error 时，`api_request_controller` 会 reactive compact 并 retry。
4. **Selective tool-output compression**：`message_compression` 在 active request 中压缩旧 tool output，保留最近 2 轮和 validation evidence。

另有 `message_healing` 负责发送前收缩 oversized tool result 和清理 dangling tool calls。它不属于语义压缩，但对上下文可发送性非常关键。

## 当前完成状态

**核心文档、代码注释和本轮剩余架构整理已经对齐；后续只剩更高精度 tokenizer / 外部参考验证这类增强项。**

- 已完成：`ContextCollapseService` 已标为实验性未接入；压缩职责边界已写入 `context_compressor.rs` 模块文档；repair output cap 已移入 turn/session state；`ContextAssemblyPlan` 已明确为观测模型；request preparation 动态块已集中到 `DynamicContextBlockBuilder`；dynamic context tag/prefix 已抽为共享 registry；token estimator 已增加 JSON/tool-schema 和 model-context profile 入口。
- 已确认不成立：`PromptContextAssembler::build_for_single_user_message` 和 `ContextAssemblyPlan::render_zoned_context` 当前代码中不存在，不应继续列为待删除项。
- 仍未完成：未接入真实 tokenizer；未重新验证 opencode 当前源码。

## 发现的问题和状态

### 1. `ContextCollapseService` 已明确为实验性未接入

**位置：** `src/engine/context_collapse.rs`

**状态：已完成当前决策。**

`ContextCollapseService` 是基于磁盘的折叠服务，由 `PRIORITY_AGENT_CONTEXT_COLLAPSE` 配置门控，但当前主对话循环没有实例化或调用它。`context_collapse.rs` 中的 compaction metadata、strategy、attempt record 等类型仍被 `ContextCompressor` 使用，不能删除整个文件。

当前代码已经把 `ContextCollapseService` 标为“实验性，未接入主运行时”，并在 `ContextCompressor` 模块文档中说明它是磁盘折叠的实验性替代路径。后续不需要为了本审计再做立即改动；如果以后要清理，只能在确认不会影响 compaction metadata/types 后再拆。

验收建议：

- `cargo test -q context_compressor`
- `cargo check -q`

### 2. 未使用或只测试使用的函数

**状态：已清理到合理状态。**

| 函数 | 当前判断 | 建议 |
|------|----------|------|
| `PromptContextAssembler::assembly_plan_for_single_user_message` | 仍有生产调用，`query_engine` 使用它构建单轮 prompt plan | 保留 |
| `PromptContextAssembler::build_for_single_user_message` | 当前代码中不存在 | 不再列为待办 |
| `ContextAssemblyPlan::render_zoned_context` | 当前代码中不存在 | 不再列为待办 |
| `MemorySnapshotController::has_dynamic_memory_recall` | 已限制为 `#[cfg(test)]`，只服务诊断测试 | 保留即可 |

这些不是功能故障；当前没有需要立即处理的生产死代码项。

### 3. 压缩职责边界已补入代码注释

**状态：已完成代码侧说明；仍可补产品级文档。**

原始文档称“双重压缩系统”基本成立，但范围不完整。实际不是两套，而是多阶段压缩/修复链路：

- full-message compaction：`ContextCompressor`；
- request-local old tool output compression：`message_compression`；
- provider error reactive compaction：`api_request_controller`；
- streaming pre-query compression；
- send-before-heal：`message_healing`。

`src/engine/context_compressor.rs` 现已在模块文档中固化这些职责边界，说明哪些路径改变历史、哪些只影响本次 request，以及 `ContextCollapseService` 与 `ContextCompressor` 的关系。后续如果要继续完善，可以把这段代码注释同步到产品/维护者文档。

已覆盖：

- 哪些路径改变历史消息；
- 哪些路径只改变本次 request；
- 哪些路径保留 closeout evidence；
- 哪些路径会记录 compact boundary / runtime diet / trace。

### 4. 动态块注入分散，缺统一收集器

**状态：已完成。**

`prepend_to_last_user_message` 被多个注入器直接调用，动态块顺序依赖调用顺序。当前并非简单“必然重复前置”：

- system message 中的 dynamic zones 会被 `normalize_context_zone_envelope` 消费和 dedupe；
- user message 中的 zones 会被 `record_context_zones` 读取；
- `current_decision_request_content` 会 strip zone tags，避免把动态块当成用户原始请求。

当前已新增 `src/engine/dynamic_context.rs`，并在 request preparation 阶段使用 `DynamicContextBlockBuilder` 收集动态块，再统一 prepend 到最后一条 user message。各注入器不再直接修改 user message，避免 `task-contract` 这类重复 prepend 问题复发。

当前 builder 已集中处理：

- 动态块收集；
- 规范化 dedupe；
- 显式渲染顺序；
- 一次性 user-tail prepend。

后续如果需要更精细预算裁剪，可以在 `DynamicContextBlockBuilder` 内继续加 per-block token cap，而不需要再改各注入器。

### 5. 标签命名和检测函数不统一

**状态：已完成。**

当前标签混用连字符和下划线：

- `task-state` / `task_state`
- `task-contract`
- `context-pack`
- `relevant_material`
- `recent_observation`
- `self-evolution-guidance`
- `context_zones`
- `retrieval-context`

代码已兼容部分变体，尤其是 `task-state` 和 `task_state`。当前 tag/prefix 列表已沉到 `src/engine/dynamic_context.rs`，`cache_stability` 和 `request_preparation_controller` 都使用同一份检测函数：

- `dynamic_context::is_dynamic_context_system_message`
- `dynamic_context::user_message_contains_dynamic_context`

这降低了新增 tag 时 cache 观测和 request preparation 漏同步的风险。

### 6. 修复轮输出 cap 已改为会话/turn 状态

**位置：** `request_preparation_controller::output_cap_for_turn`

**状态：已完成。**

原始问题是进程全局 `CONSECUTIVE_REPAIRS` 会让并发会话或不同会话共享 repair count。当前代码已经不再使用进程全局 `AtomicU32`；`RequestPreparationContext` 接收 `consecutive_repairs: &mut u32`，实际状态保存在 `TurnState`，注释也标明是 session-scoped consecutive repair count。

后续无需继续把此项作为上下文注入风险追踪。

### 7. Token 估算对 CJK 和混合文本偏粗

**位置：** `context_compressor::estimate_tokens`

**状态：已完成当前启发式；真实 tokenizer 仍是增强项。**

当前估算已经从纯 `text.len().div_ceil(4)` 改为按字符类别估算：ASCII word、whitespace、ASCII punctuation、CJK、其他 Unicode 分开计数。工具调用 JSON 和 provider tool schema 会走 `TokenEstimateProfile::JsonToolSchema`，`ModelContextProfile` 可映射到 `TokenEstimateProfile`。

剩余问题是它仍然不是真实 tokenizer：不同模型 tokenizer 的精确差异仍只能粗略估计。

建议：长期如果需要更高精度，再接入 provider/model 对应 tokenizer 或模型上下文 profile 中的 tokenizer hint。

### 8. `ContextAssemblyPlan` 是观测模型，不是完整渲染路径

**状态：已完成代码侧说明。**

5 zone 模型用于命名、预算、fingerprint 和 trace，但当前发送给模型的真实渲染仍混合了：

- stable system prompt；
- user-tail dynamic context；
- retrieval context；
- tool messages；
- healed/compressed request messages。

因此不能把 `context_assembly.rs` 当成唯一渲染器。当前代码注释已经明确它是 runtime 的 typed reporting spine / 观测脊线。

## 当前覆盖较好的点

- AGENTS.md/root context 有 per-layer budget、prompt-visible selection、safety scan 和 XML escape。
- Retrieval context 带 source、score、trust、conflict、provenance、reason 和 memory trace。
- Memory snapshot 和 dynamic recall 分离，stable memory 不直接替代 dynamic relevant material。
- Context zones materialized trace 覆盖 token、fingerprint、budget overflow、source/dedupe/provenance。
- Cache stability 已经跟踪 stable prefix fingerprint、tool schema manifest、dynamic tail 和 dynamic zone count。
- Selective compression 会保护近期 tool output 和 validation evidence。
- API reactive compression 有 compaction decision、circuit-open 和 recovery plan 记录。

## 仍不完善的覆盖

- 压缩职责边界已有代码侧说明，但还可以同步到面向维护者的 docs。
- token estimator 已有 profile-aware 启发式，但还未接入真实 tokenizer。

## 后续行动

- [x] 明确 `ContextCollapseService` 命运：已标为实验性未接入，保留 compaction metadata 类型供 `ContextCompressor` 使用。
- [x] 增加压缩职责边界说明：preflight、streaming pre-query、API reactive、selective tool-output、message healing 已写入 `context_compressor.rs` 模块文档。
- [x] 将 repair output cap 计数移入 session/turn state。
- [x] 将 `MemorySnapshotController::has_dynamic_memory_recall` 限制为 `#[cfg(test)]` 诊断 helper。
- [x] 引入统一动态上下文 block builder，集中处理收集、排序、dedupe 和 user-tail 渲染。
- [x] 统一 dynamic context tag/prefix registry，并让 request preparation 与 cache stability 共享。
- [x] 改进 token estimator，使其纳入 JSON/tool schema 密集文本和 model context profile。
- [ ] 如需更高精度，接入真实 tokenizer 或 provider/model tokenizer hint。
- [ ] 保持 opencode 参考为“可借鉴模式”，不要用它弱化本项目已有的 evidence、memory、permission、closeout 边界。

## 本次文档更新状态

**已完成：**

- 修正请求准备流程顺序；
- 补全 bootstrap、request preparation、cache stability、context budget、message healing 和 reactive compression；
- 调整“重复前置”“双重压缩”“标签不一致”等原始表述，改为当前代码能支撑的风险描述；
- 更新已完成/不成立项：`ContextCollapseService` 实验标记、压缩职责边界、repair output cap、`ContextAssemblyPlan` 观测模型、测试专用 memory recall helper、token estimator 当前实现。
- 完成剩余代码项：统一 dynamic context registry、request preparation 动态块 builder、JSON/tool-schema 与 model-context token estimate profile。

**未完成：**

- 未重新验证 opencode 当前源码；
- 未接入真实 tokenizer；
- 未运行全量测试；本次已运行上下文注入相关窄测试和 `cargo check -q`。

**本次验证：**

- `cargo check -q`
- `cargo test -q task_guidance_controller`
- `cargo test -q prompt_context`
- `cargo test -q request_preparation_controller`
- `cargo test -q cache_stability`
- `cargo test -q context_compressor`
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `git diff --check`

## 建议验证命令

后续若改上下文注入或压缩代码，建议按影响面选择：

```bash
cargo fmt --check
cargo test -q prompt_context
cargo test -q request_preparation_controller
cargo test -q context_compressor
cargo test -q cache_stability
cargo test -q conversation_loop
cargo check -q
```

## 审计方法

- 静态代码分析；
- 当前运行时入口核对；
- trace/cache/budget/压缩链路核对；
- 与原始审计结论逐项对照。
