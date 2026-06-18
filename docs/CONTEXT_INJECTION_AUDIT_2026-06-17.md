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
- 对未使用函数和死代码的判断基本成立，但应标为“未完成维护项”，避免把保留诊断/实验代码直接当成立即删除项。
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

## 发现的问题和状态

### 1. `ContextCollapseService` 未接入主运行时

**位置：** `src/engine/context_collapse.rs`

**状态：未完成。**

`ContextCollapseService` 是基于磁盘的折叠服务，由 `PRIORITY_AGENT_CONTEXT_COLLAPSE` 配置门控，但当前主对话循环没有实例化或调用它。`context_collapse.rs` 中的 compaction metadata、strategy、attempt record 等类型仍被 `ContextCompressor` 使用，不能删除整个文件。

建议：不要直接删除全部服务代码。更稳的处理是二选一：

- 明确标记 `ContextCollapseService` 为实验性/未接入，并只保留测试覆盖；
- 或删除 service/config/entry persistence 实现，保留被 `ContextCompressor` 使用的公共类型。

验收建议：

- `cargo test -q context_compressor`
- `cargo check -q`

### 2. 未使用或只测试使用的函数

**状态：未完成。**

| 函数 | 当前判断 | 建议 |
|------|----------|------|
| `PromptContextAssembler::build_for_single_user_message` | 未发现生产调用；`assembly_plan_for_single_user_message` 仍被该 wrapper 使用 | 可删除 wrapper，保留 plan 方法和 report 方法 |
| `ContextAssemblyPlan::render_zoned_context` | 未发现调用 | 可删除，或只在测试需要时改为 `#[cfg(test)]` |
| `MemorySnapshotController::has_dynamic_memory_recall` | 仅测试使用，生产未调用；当前有 `#[allow(dead_code)]` | 可改为 `#[cfg(test)]`，或删除诊断 helper 与对应测试 |

这些是维护性清理，不是功能故障。

### 3. 压缩职责边界文档不足

**状态：未完成。**

原始文档称“双重压缩系统”基本成立，但范围不完整。实际不是两套，而是多阶段压缩/修复链路：

- full-message compaction：`ContextCompressor`；
- request-local old tool output compression：`message_compression`；
- provider error reactive compaction：`api_request_controller`；
- streaming pre-query compression；
- send-before-heal：`message_healing`。

建议：新增一份短文档或在本审计后续章节中固化职责边界，明确：

- 哪些路径改变历史消息；
- 哪些路径只改变本次 request；
- 哪些路径保留 closeout evidence；
- 哪些路径会记录 compact boundary / runtime diet / trace。

### 4. 动态块注入分散，缺统一收集器

**状态：未完成。**

`prepend_to_last_user_message` 被多个注入器直接调用，动态块顺序依赖调用顺序。当前并非简单“必然重复前置”：

- system message 中的 dynamic zones 会被 `normalize_context_zone_envelope` 消费和 dedupe；
- user message 中的 zones 会被 `record_context_zones` 读取；
- `current_decision_request_content` 会 strip zone tags，避免把动态块当成用户原始请求。

真实问题是：user-prepended 动态块没有统一 builder 管理，跨模块 dedupe、排序、预算和 provenance 规则分散。

建议：后续引入 `DynamicContextBlock` / `DynamicContextEnvelopeBuilder`，让各注入器返回结构化块，由 request preparation 统一排序、dedupe、预算裁剪和渲染。

### 5. 标签命名和检测函数不统一

**状态：未完成。**

当前标签混用连字符和下划线：

- `task-state` / `task_state`
- `task-contract`
- `context-pack`
- `relevant_material`
- `recent_observation`
- `self-evolution-guidance`
- `context_zones`
- `retrieval-context`

代码已兼容部分变体，尤其是 `task-state` 和 `task_state`。但检测函数仍重复：

- `cache_stability::user_message_contains_dynamic_context` 覆盖 `<context_zones`、`<retrieval-context`、`MVA profile:`；
- `request_preparation_controller::user_message_contains_dynamic_context` 覆盖范围更窄。

建议：把 dynamic context tag/prefix 列表沉到一个共享函数或常量，至少让 request preparation 和 cache stability 使用同一份检测表。

### 6. 修复轮输出 cap 使用进程全局计数

**位置：** `request_preparation_controller::output_cap_for_turn`

**状态：未完成。**

`CONSECUTIVE_REPAIRS` 是进程全局 `AtomicU32`。这会让并发会话或不同会话之间共享 repair count。当前逻辑在非 repair turn 会 reset，但仍不是 session-scoped。

建议：把计数移动到 `ConversationLoop` / `TurnRuntimeState` / session-scoped runtime state。不要用 `OnceLock<HashMap<SessionId, AtomicU32>>` 作为首选，因为它会引入生命周期和清理问题；优先放进已经存在的会话状态。

### 7. Token 估算对 CJK 和混合文本偏粗

**位置：** `context_compressor::estimate_tokens`

**状态：未完成。**

当前估算是 `text.len().div_ceil(4)`。它对英文近似可用，但对中文、混合 UTF-8、多代码块、JSON/tool schema 的估计偏粗。由于 `len()` 是字节数，它不一定总是低估 CJK，但它不能表达不同模型 tokenizer 的真实差异。

建议：短期用更保守的启发式，比如按字符类别分别估算 ASCII、CJK、数字/标点和 JSON 密集文本；长期按 provider/model profile 使用 tokenizer 或模型上下文 profile。

### 8. `ContextAssemblyPlan` 是观测模型，不是完整渲染路径

**状态：需文档明确。**

5 zone 模型用于命名、预算、fingerprint 和 trace，但当前发送给模型的真实渲染仍混合了：

- stable system prompt；
- user-tail dynamic context；
- retrieval context；
- tool messages；
- healed/compressed request messages。

因此不能把 `context_assembly.rs` 当成唯一渲染器。它更像 runtime 的 typed reporting spine。

## 当前覆盖较好的点

- AGENTS.md/root context 有 per-layer budget、prompt-visible selection、safety scan 和 XML escape。
- Retrieval context 带 source、score、trust、conflict、provenance、reason 和 memory trace。
- Memory snapshot 和 dynamic recall 分离，stable memory 不直接替代 dynamic relevant material。
- Context zones materialized trace 覆盖 token、fingerprint、budget overflow、source/dedupe/provenance。
- Cache stability 已经跟踪 stable prefix fingerprint、tool schema manifest、dynamic tail 和 dynamic zone count。
- Selective compression 会保护近期 tool output 和 validation evidence。
- API reactive compression 有 compaction decision、circuit-open 和 recovery plan 记录。

## 仍不完善的覆盖

- 缺统一动态上下文 block builder，导致注入顺序、dedupe、预算和 provenance 分散。
- 缺统一 dynamic tag registry，request preparation 和 cache stability 各有检测列表。
- 压缩路径太多，但还缺一处面向维护者的职责边界说明。
- `ContextCollapseService` 与 `ContextCompressor` 的关系未在代码注释或文档中说清。
- token estimator 缺 provider/model-aware 策略。
- repair output cap 不是 session-scoped。

## 后续行动

- [ ] 明确 `ContextCollapseService` 命运：标实验性或删除未接入 service，保留 compaction metadata 类型。
- [ ] 删除或 `#[cfg(test)]` 化未使用函数：`build_for_single_user_message`、`render_zoned_context`、`has_dynamic_memory_recall`。
- [ ] 增加压缩职责边界说明：preflight、streaming pre-query、API reactive、selective tool-output、message healing。
- [ ] 引入统一动态上下文 block builder，集中处理排序、dedupe、预算和 provenance。
- [ ] 统一 dynamic context tag/prefix registry，并让 request preparation 与 cache stability 共享。
- [ ] 将 repair output cap 计数移入 session/turn state。
- [ ] 改进 token estimator，使其至少区分 ASCII、CJK、JSON/tool schema 和模型 profile。
- [ ] 保持 opencode 参考为“可借鉴模式”，不要用它弱化本项目已有的 evidence、memory、permission、closeout 边界。

## 本次文档更新状态

**已完成：**

- 修正请求准备流程顺序；
- 补全 bootstrap、request preparation、cache stability、context budget、message healing 和 reactive compression；
- 调整“重复前置”“双重压缩”“标签不一致”等原始表述，改为当前代码能支撑的风险描述；
- 标注所有后续代码项为未完成。

**未完成：**

- 未改上下文注入代码；
- 未重新验证 opencode 当前源码；
- 未运行测试，因为本次只更新审计文档。

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
