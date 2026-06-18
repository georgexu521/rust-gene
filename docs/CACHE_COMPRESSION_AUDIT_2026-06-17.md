# 缓存命中与动态压缩系统审计报告

日期：2026-06-17  
更新：2026-06-18

## 审计范围

本次审计对照当前代码检查缓存命中诊断、上下文压缩、预算观察、压缩持久化以及相关实验路径。重点文件包括：

- `src/engine/cache_stability.rs`
- `src/engine/context_compressor.rs`
- `src/engine/context_compressor/compressor.rs`
- `src/engine/context_collapse.rs`
- `src/engine/message_compression.rs`
- `src/engine/conversation_loop/preflight_compression_controller.rs`
- `src/engine/conversation_loop/api_request_controller.rs`
- `src/engine/conversation_loop/context_budget_controller.rs`
- `src/engine/conversation_loop/request_preparation_controller.rs`
- `src/engine/streaming.rs`
- `src/session_store/compact_store.rs`
- `src/session_store/message_ops.rs`

本文保留与 opencode 的对比作为设计参考，但本次更新没有重新拉取或验证 opencode 当前源码；相关结论只作为历史对照，不作为当前代码事实。

## 审计结论

整体结论：缓存诊断和压缩主链路已经比较完整，可观测性强，且比简单“超限后裁剪”方案更适合长期本地编程会话。但原文有几处过时或过强的判断，需要修正：

- `CompactionDecision::Retrying` / `Recovered` 已经在 API 反应式压缩重试路径使用，不能再列为未使用死枚举。
- `ContextCollapseService` 本体未接入生产对话循环，但同文件中的压缩元数据、决策、运行时记录类型被生产路径使用，不能简单建议删除整个文件。
- `last_result_idx` 确实是死变量，但工具调用/工具结果边界对齐并非完全无效；当前问题是局部冗余代码和注释噪音。
- `Message::content()` 不是重复实现；它是 `context_compressor.rs` 中的私有 helper。可以优化为公共借用 helper，但不是 Rust 维护冲突。
- token 估算是粗略启发式，但因为 `str::len()` 返回字节数，不能简单说“CJK 严重低估”。更准确的问题是模型无关、混合内容误差不可控。
- 后台修剪、时间触发压缩、`ContextCollapseService` 属于未接入或弱接入路径，需要在文档里和生产主链路分开。

## 当前架构事实

### 1. 缓存稳定性系统

| 组件 | 文件 | 当前作用 |
|------|------|----------|
| `ToolSchemaCacheManifest` | `cache_stability.rs` | 规范化工具 manifest，记录工具名、schema 指纹和估算 token |
| `PromptCacheUsage` | `cache_stability.rs` | 从 provider usage 中提取 prompt/cache hit/cache miss token |
| `CacheDiagnosticShape` | `cache_stability.rs` | 请求级形状快照，包含稳定前缀、系统、工具、few-shot、动态尾部指纹 |
| `DynamicZoneTier` | `cache_stability.rs` | 将动态系统消息分为 `StablePrefix`、`RepairOnly`、`LastUserDynamic` |
| `CacheMissReason` | `cache_stability.rs` | 根据前后请求形状和 provider cache usage 推断 cache miss 类型 |

缓存诊断当前覆盖的核心问题是：稳定前缀是否变化、工具 schema 是否变化、few-shot/记忆/技能是否移动、动态区域是否进入可缓存前缀。这个方向是正确的，尤其适合调试“为什么 prompt cache 忽然失效”。

需要注意的是，`dynamic_tail_fingerprint` 本身覆盖所有消息，因此它对普通对话变化天然敏感。它适合作为诊断证据，不应被误读为缓存应该稳定的前缀指纹。

### 2. 压缩触发路径

| 路径 | 代码位置 | 当前状态 | 说明 |
|------|----------|----------|------|
| 预检压缩 | `preflight_compression_controller.rs` | 已接入 | 请求发送前基于 token 压力最多尝试多轮压缩，并写入 compact boundary |
| streaming 预查询压缩 | `streaming.rs` | 已接入 | query 前可执行自动压缩，压缩后会清理 stale file-read context |
| 手动压缩 | `streaming.rs` | 已接入 | slash/手动 compact 路径使用 `SessionMemoryCompact` 策略 |
| API 反应式压缩 | `api_request_controller.rs` | 已接入 | provider 返回上下文过长后记录 `Retrying`，压缩成功后记录 `Recovered` 或 `Failed` |
| 选择性消息压缩 | `message_compression.rs` + request preparation | 已接入 | 请求局部压缩旧工具输出，默认开启，不一定改写历史 |
| 后台工具输出修剪 | `message_compression.rs` | 辅助函数，未见生产调用 | 受 `PRIORITY_AGENT_BACKGROUND_PRUNE` 门控，但当前更像保留能力 |
| 时间触发压缩判断 | `context_compressor.rs` / `compressor.rs` | 判断函数和测试存在，未见主循环调用 | 不应写成稳定生产触发路径 |
| `ContextCollapseService` | `context_collapse.rs` | 实验服务未接入 | 默认关闭，本体无生产调用；共享类型仍被生产路径使用 |

### 3. 压缩管道

`ContextCompressor` 的生产主线大致是：

1. 读取预算、消息、工具 schema token 和压缩历史。
2. 根据 token 使用率、失败次数、LLM 摘要可用性选择压缩级别。
3. 保护系统前缀、最近尾部、工具调用/工具结果配对和 runtime continuity facts。
4. 对中段进行启发式摘要，必要时可走 LLM compaction。
5. 注入 compact boundary、运行时连续性事实、会话记忆 compact 和 preserved markers。
6. 清理孤立 tool pairs，并记录 `CompactionRuntimeRecord`。

这个方向符合当前项目原则：压缩可以帮助 runtime 管理上下文，但不能让 runtime 假装拥有语义判断。需要继续保留真实验证、失败记录和 closeout proof。

### 4. 持久化与可观测性

当前生产路径已经有比较完整的压缩证据链：

- `CompactionRuntimeRecord` 记录压缩策略、前后 token、消息数、原因、决策和 boundary id。
- `compact_store.rs` 将压缩边界写入 SQLite。
- `message_ops.rs::replace_session_messages()` 支持压缩后替换会话消息。
- `event_store.rs` / streaming 路径会写入压缩事件，便于 UI/API 查询。
- `cache_stability.rs` 能对 provider cache usage 和请求形状做离线/运行时诊断。

后续文档或 UI 应继续区分两类事实：一类是“历史真的被改写/压缩”，另一类是“本次请求局部压缩了旧工具输出”。这两类对用户可见性和调试含义不同。

### 5. 环境变量与配置

当前压缩相关开关较分散：

| 配置 | 默认 | 作用 |
|------|------|------|
| `PRIORITY_AGENT_SELECTIVE_COMPRESSION` | 开 | 请求准备阶段选择性压缩旧工具输出 |
| `PRIORITY_AGENT_BACKGROUND_PRUNE` | 关 | 后台工具输出修剪 helper 的开关 |
| `PRIORITY_AGENT_LLM_COMPACTION` | 关 | 是否允许 LLM 参与摘要压缩 |
| `PRIORITY_AGENT_CONTEXT_COLLAPSE` | 关 | 实验 `ContextCollapseService` 开关 |
| `PRIORITY_AGENT_TIME_BASED_COMPRESSION` | 关 | 时间/消息数触发判断的开关 |
| `PRIORITY_AGENT_SESSION_DURATION_THRESHOLD` | 1 小时 | 时间触发阈值 |
| `PRIORITY_AGENT_MESSAGE_COUNT_THRESHOLD` | 100 | 消息数量触发阈值 |
| `PRIORITY_AGENT_IDLE_THRESHOLD` | 30 分钟 | idle 触发阈值 |

这些开关不是同一层级：有的是请求局部压缩，有的是历史压缩，有的是未接入实验路径。建议后续整理为一张用户/开发者可读的配置表，明确默认值、生产状态和影响面。

## 发现的问题与状态

### 1. `SessionMemoryCompact.user_preferences` 仍未由生产代码填充

**状态：未完成。**  
`SessionMemoryCompact::analyze()` 仍将 `user_preferences` 设置为 `Vec::new()`，注释写着“由外部注入”，但当前未看到生产路径注入该字段。测试中存在手工构造偏好的用例，说明字段设计意图存在。

建议二选一：

- 如果要保留该字段，应从 memory manager、用户画像或稳定偏好来源注入，并在 compact metadata 中保留来源。
- 如果近期不打算接入，应删除字段或改名为明确的 future field，避免审计误认为已具备用户偏好压缩能力。

### 2. `last_result_idx` 是死变量，但边界对齐不是完全无效

**状态：未完成。**  
`compressor.rs` 中 `last_result_idx` 被计算后通过 `let _ = last_result_idx;` 丢弃。这段代码确实应该清理。

不过原文“工具组边界对齐代码实际上没有效果”过强。当前代码仍会在 tail 起点落到 `Tool` 消息时向前找对应 assistant，也会通过后续 sanitize 处理孤立工具消息。问题应描述为：局部变量冗余、意图不清、缺少针对复杂 tool-call group 的边界测试。

### 3. `CompactionDecision::Retrying/Recovered` 已接入

**状态：原问题已不成立。**  
`api_request_controller.rs` 在 provider 报上下文过长时记录 `Retrying`，压缩重试有效缩小时记录 `Recovered`，失败或无收益时记录 `Failed`。这两个枚举不应删除。

后续可以补充测试，覆盖“上下文过长 -> 反应式压缩 -> retrying/recovered 记录”的行为，但不能再把它列为死代码。

### 4. `ContextCollapseService` 本体未接入生产路径

**状态：未完成，需要明确归属。**  
`ContextCollapseService` 受 `PRIORITY_AGENT_CONTEXT_COLLAPSE` 门控，默认关闭，当前未看到主对话循环调用它。它更像早期 compact boundary / collapse 实验服务。

但 `context_collapse.rs` 中的 `CompactMetadata`、`ContextCompactionStrategy`、`CompactionDecision`、`CompactionAttemptRecord`、`ContextTokenPressure`、`CompactionRuntimeRecord` 已被当前压缩主链路使用。后续如果清理，应该只清理或标注未接入的 service/config/persistence 片段，不能删除共享类型。

### 5. `has_active_skills` 不是源自真实技能状态

**状态：未完成。**  
`ContextCompressor` 初始化时将 `has_active_skills` 设为 `true`，`mark_skills_active()` 也只能设为 `true`。这意味着 preserved skills marker 基本是默认追加，而不是由当前技能上下文驱动。

如果项目策略是“压缩摘要永远保留技能提醒”，应把字段改成明确常量或策略名。如果策略是“只有活跃技能时保留”，就需要接入真实技能上下文，并增加 inactive/clear 路径。

### 6. `Message::content()` 不是重复实现，但可以优化

**状态：原问题表述不准确，低优先级。**  
`Message` 在 `src/services/api/mod.rs` 中没有公共 `content()` 方法，`context_compressor.rs` 里的 `content()` 是私有 helper，不是重复 API。

可以考虑后续优化为：

- 在 `Message` 定义处增加 `content_str(&self) -> &str`，减少 clone。
- 或在 compressor 内部改名为更局部的 helper，避免误解为通用 API。

这不是功能 bug。

### 7. `ContextManager` 是平行旧路径

**状态：未完成。**  
`src/engine/context_manager.rs` 包装了 `ContextCompressor` 并维护自己的阈值与 `manage()` 流程，但当前主对话路径使用的是 preflight、streaming、API reactive 和 request-local selective compression。

建议确认它是否仍服务 API/测试/旧入口。如果无生产调用，应标记 deprecated 或删除，避免未来修复压缩行为时改错路径。

### 8. token 估算仍是模型无关启发式

**状态：未完成。**  
`estimate_tokens(text)` 使用 `text.len().div_ceil(4)`。因为 `len()` 是 UTF-8 字节长度，对中文不一定“严重低估”，但它仍然无法准确反映不同 provider/model、代码、JSON、emoji、工具 schema 的 tokenization。

建议后续引入 provider/model token profile，至少区分普通文本、CJK、代码块和 JSON/tool payload。压缩触发和 cache 诊断都依赖该估算，误差会直接影响“是否该压缩”的判断。

### 9. 压缩配置矩阵需要整理

**状态：未完成。**  
原文只列了三个环境变量，但当前至少还包括 context collapse 和 time-based compression 相关变量。更大的问题不是变量数量，而是它们跨越不同层级：

- 请求局部压缩：不改写历史，只减少本次请求体。
- 会话历史压缩：会写 compact boundary，并可能替换 session messages。
- 实验服务：默认关闭，未接入主循环。
- 判断 helper：测试存在，但生产触发路径不完整。

建议新增一份压缩配置说明，明确每个开关的默认值、生产状态、是否持久化、是否影响 prompt cache。

### 10. 经济守卫可能跳过有价值的压缩

**状态：未完成，需要测试和产品决策。**  
当前 compressor 会在短对话且连续 no-gain 后跳过 heavy work。这能避免反复做无收益压缩，但也可能在“短而巨大的工具输出”场景下跳过本该有效的轻量裁剪。

建议增加针对短会话大工具输出、连续 no-gain 后再次增长、provider context error 后恢复的测试。更稳妥的策略可能是从 heavy/LLM 压缩降级到 deterministic trim，而不是完全跳过。

### 11. 后台修剪和时间触发压缩不应写成已接入主路径

**状态：文档已修正，代码未变。**  
`background_prune_tool_outputs()` 和 `needs_time_based_compression()` 都有实现/测试痕迹，但当前未看到主对话循环稳定调用它们。后续要么接入主路径并补测试，要么明确标注为保留 helper。

## 与 opencode 对比的保留意见

原文中“优先级代理优势”和“opencode 优势”可以作为设计启发，但需要降低确定性：

- opencode 当前实现未在本次更新中重新验证。
- “溢出后重放最后用户消息”在本项目中不能简单照搬；当前 API reactive path 已经有压缩重试记录，需要先确认是否存在真实丢失用户消息的问题。
- “压缩代理模型选择”可作为成本优化，但不能绕过 runtime 的 validation/proof 边界。
- “受保护工具输出”有价值，尤其是 skills、权限、验证、checkpoint、failure_owner 等证据不应被普通工具输出裁剪策略误删。

优先级代理应继续保留的方向：

- cache miss 可解释性
- dynamic zone 分类
- compact boundary 持久化
- 多级压缩与失败/无收益断路器
- runtime continuity facts
- 压缩后 closeout 仍依赖真实 proof

## 本次文档更新已完成

- [x] 修正 `Retrying` / `Recovered` 未使用的过时结论。
- [x] 修正 `ContextCollapseService` 的清理建议，区分未接入服务和已使用共享类型。
- [x] 修正 `last_result_idx` 问题描述，避免误判整个工具边界对齐无效。
- [x] 修正 `Message::content()` “重复实现”的误判。
- [x] 修正 token 估算对 CJK 的表述，改为模型无关启发式风险。
- [x] 补充 preflight、streaming、manual compact、API reactive、selective compression 的触发路径区分。
- [x] 补充 compact boundary、message replacement、runtime record 的持久化事实。
- [x] 补充 context collapse、time-based compression、background prune 的未接入/弱接入状态。
- [x] 补充压缩配置矩阵和后续整理建议。

## 后续行动

- [ ] 决定 `SessionMemoryCompact.user_preferences` 是接入真实记忆偏好还是删除。
- [ ] 删除 `last_result_idx` 死变量，并补充 tool-call group 边界测试。
- [ ] 为 API reactive compression 增加 `Retrying` / `Recovered` 记录测试。
- [ ] 标注或清理未接入的 `ContextCollapseService` 本体，保留生产路径使用的共享类型。
- [ ] 将 `has_active_skills` 改为真实技能状态或明确的 always-preserve 策略。
- [ ] 判断 `ContextManager` 是否仍有入口；无入口则标记 deprecated 或删除。
- [ ] 改进 token 估算，至少引入 provider/model profile 或更细分的启发式。
- [ ] 整理压缩相关环境变量文档，区分请求局部压缩、历史压缩和实验路径。
- [ ] 为短会话巨大工具输出和 no-gain 后增长场景补测试。
- [ ] 评估 protected tool outputs，避免验证、权限、checkpoint、skills、failure evidence 被压缩丢失。

## 建议验证

文档更新本身不需要运行全量测试。若后续按本文修代码，建议按改动范围选择：

```bash
cargo fmt --check
cargo test -q cache_stability
cargo test -q context_compressor
cargo test -q request_preparation_controller
cargo test -q conversation_loop
cargo check -q
```

如果触及 session store 或 streaming compact boundary：

```bash
cargo test -q session_store
cargo test -q streaming
```
