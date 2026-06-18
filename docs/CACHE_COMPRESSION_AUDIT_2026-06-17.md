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
- `ContextCollapseService` 已有 gated bootstrap bridge；同文件中的压缩元数据、决策、运行时记录类型也被生产路径使用，不能简单建议删除整个文件。
- `last_result_idx` 死变量已清理，复杂 tool-call group 边界已有回归测试。
- `Message::content()` 不是重复实现；它是 `context_compressor.rs` 中的私有 helper。可以优化为公共借用 helper，但不是 Rust 维护冲突。
- token 估算已从纯 `len()/4` 升级为 profile-aware 计数：OpenAI GPT-4o/GPT-4.1/reasoning family 走 `tiktoken-rs` 的真实 `o200k_base`/`cl100k_base`，MiniMax/Kimi/Claude 仍走 provider profile fallback 启发式；provider 返回后的真实 usage 会记录 prompt、completion、cached/read、cache write、reasoning 等字段。
- 后台修剪已经接入 request bootstrap 并默认开启；时间触发压缩已经接入 preflight；`ContextCollapseService` 已有 gated bootstrap bridge，但默认关闭，仍应视为实验路径。

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
| 后台工具输出修剪 | `message_compression.rs` + request bootstrap | 已接入，默认开启 | 每轮发送请求前修剪旧工具输出，保护验证/权限/checkpoint/failure evidence |
| 时间触发压缩判断 | `context_compressor.rs` / `compressor.rs` + preflight | 已接入 | token 压力未到阈值但消息数/会话时长超过阈值时，以 `time_based` trigger 进入 preflight 压缩 |
| `ContextCollapseService` | `context_collapse.rs` + request bootstrap | 已接入但默认关闭 | 受 `PRIORITY_AGENT_CONTEXT_COLLAPSE=1` 门控；会真实移走旧消息并写 collapse 文件，仍是实验路径 |

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
| `PRIORITY_AGENT_BACKGROUND_PRUNE` | 开 | request bootstrap 后台工具输出修剪；设为 `0/false/no/off` 可关闭 |
| `PRIORITY_AGENT_LLM_COMPACTION` | 关 | 是否允许 LLM 参与摘要压缩 |
| `PRIORITY_AGENT_CONTEXT_COLLAPSE` | 关 | 实验 `ContextCollapseService` 开关 |
| `PRIORITY_AGENT_CONTEXT_COLLAPSE_WINDOW` | 20 | collapse 启用后保留的最近消息窗口 |
| `PRIORITY_AGENT_CONTEXT_COLLAPSE_THRESHOLD` | `window + 20` | collapse 启用后的触发消息数 |
| `PRIORITY_AGENT_CONTEXT_COLLAPSE_DIR` | app data dir | collapse 文件持久化目录 |
| `PRIORITY_AGENT_TIME_BASED_COMPRESSION` | 开 | 时间/消息数触发判断的开关；设为 `false` 可关闭 |
| `PRIORITY_AGENT_SESSION_DURATION_THRESHOLD` | 1 小时 | 时间触发阈值 |
| `PRIORITY_AGENT_MESSAGE_COUNT_THRESHOLD` | 100 | 消息数量触发阈值 |
| `PRIORITY_AGENT_IDLE_THRESHOLD` | 5 分钟 | idle 触发阈值；当前 `needs_time_based_compression()` 只使用时长和消息数 |

这些开关不是同一层级：有的是请求局部压缩，有的是历史压缩，有的是 provider 成本/质量权衡，有的是显式 opt-in 实验路径。建议后续整理为一张用户/开发者可读的配置表，明确默认值、生产状态和影响面。

## 发现的问题与状态

### 1. `SessionMemoryCompact.user_preferences` 已从注入上下文提取

**状态：已完成当前低风险接入。**
`SessionMemoryCompact::analyze()` 不再固定返回空 `user_preferences`。当前会从已经进入消息上下文的 memory/profile 行中提取显式偏好，例如 `User preference:`、`Memory preference:`、`用户偏好：` 等，并在压缩摘要中写入 `## User Preferences`。

这个实现刻意保守：它不把普通聊天里的“我喜欢/我希望”直接提升成长期偏好，只接受稳定记忆/画像上下文里的显式标签，避免压缩阶段制造伪记忆。

剩余增强项：如果未来希望做到完整来源追踪，可以让 memory retrieval 直接传入带 metadata 的偏好记录，而不是只从已注入文本中提取。

### 2. `last_result_idx` 死变量已清理

**状态：已完成。**
当前 `compressor.rs` 中已不存在 `last_result_idx` 死变量，也不再有 `let _ = last_result_idx;` 这类无效占位。

原文“工具组边界对齐代码实际上没有效果”过强。当前代码仍会在 tail 起点落到 `Tool` 消息时向前找对应 assistant，也会通过后续 sanitize 处理孤立工具消息。本次已增加复杂 multi-tool-call group 回归测试，确保 tail 包含对应 assistant 和完整 tool results。

### 3. `CompactionDecision::Retrying/Recovered` 已接入

**状态：原问题已不成立。**
`api_request_controller.rs` 在 provider 报上下文过长时记录 `Retrying`，压缩重试有效缩小时记录 `Recovered`，失败或无收益时记录 `Failed`。这两个枚举不应删除。

本次已补充 `Retrying` / `Recovered` attempt record 测试，覆盖 reactive compaction 的记录策略、boundary id 和 recovered 后计数器归零行为。它不是端到端 provider mock 测试，但已经锁住当前决策记录层。

### 4. `ContextCollapseService` 已有 gated 主线桥接

**状态：已接入，但默认关闭。**
`ContextCollapseService` 受 `PRIORITY_AGENT_CONTEXT_COLLAPSE` 门控，默认关闭。当前 request bootstrap 会调用 `apply_session_context_collapse_if_needed()`，只有显式启用时才会为 session 复用一个 collapse service，按 window/threshold 折叠旧消息并写入 collapse 文件。

这个接入是故意 gated：它不同于 `ContextCompressor` 的语义摘要压缩，会直接把较早消息移出 live request，并通过磁盘文件保留 collapse entries。默认生产路径仍应优先使用 preflight/streaming/API reactive compaction。`context_collapse.rs` 中的 `CompactMetadata`、`ContextCompactionStrategy`、`CompactionDecision`、`CompactionAttemptRecord`、`ContextTokenPressure`、`CompactionRuntimeRecord` 继续是生产压缩链路共享类型，不能作为“实验服务”整体删除。

### 5. skills preserved marker 已改为明确策略

**状态：已完成当前策略。**
原来的 `has_active_skills` / `mark_skills_active()` 语义已经改为 `preserve_skills_marker` / `preserve_skills_marker()` / `mark_skills_preserved()`，代码注释也明确：压缩摘要中永远保留技能提醒 marker。

这不是“真实技能状态驱动”，而是显式 always-preserve 策略。考虑到 skills 在压缩中被模型改写或丢失的风险，这个策略目前合理；如果以后要按真实技能状态细分，再接入 SkillRuntime。

### 6. `Message::content()` 不是重复实现，但可以优化

**状态：原问题表述不准确，低优先级。**
`Message` 在 `src/services/api/mod.rs` 中没有公共 `content()` 方法，`context_compressor.rs` 里的 `content()` 是私有 helper，不是重复 API。

可以考虑后续优化为：

- 在 `Message` 定义处增加 `content_str(&self) -> &str`，减少 clone。
- 或在 compressor 内部改名为更局部的 helper，避免误解为通用 API。

这不是功能 bug。

### 7. `ContextManager` 是平行旧路径

**状态：已完成当前决策。**
`src/engine/context_manager.rs` 包装了 `ContextCompressor` 并维护自己的阈值与 `manage()` 流程，但当前主对话路径使用的是 preflight、streaming、API reactive 和 request-local selective compression。

当前文件顶部已经标注“旧路径，已废弃”，`ContextManager` 也带有 `#[deprecated]`，并说明主对话循环使用 `PreflightCompressionController + ContextBudgetController + ContextCompressor`。当前只保留测试/参考用途，避免未来修复压缩行为时改错路径。

### 8. token 估算已接入 tiktoken profile，真实 usage 字段已补齐 cache write

**状态：OpenAI-family 预请求计数已是真实 tokenizer；非 OpenAI provider 仍是 profile fallback。**
`estimate_tokens(text)` 已不再使用纯 `text.len().div_ceil(4)`。当前 `ContextCompressor::from_model_context_profile()` 会根据 provider/model profile 选择计数器：OpenAI GPT-4o/GPT-4.1/reasoning family 使用 `tiktoken-rs` singleton 的 `o200k_base` 或 `cl100k_base`；MiniMax/Kimi 走 CJK-heavy fallback；Anthropic-like 走 general-text fallback。工具调用 JSON 和 provider tool schema 会走 `TokenEstimateProfile::JsonToolSchema`，并提供 `estimate_tokens_for_model_context()` 入口。

请求发送前仍然需要本地 token 计数/估算，因为 runtime 必须先预判本轮是否可能撑爆上下文窗口，provider 的真实 token usage 只能在请求完成或 streaming usage 返回后得到。主流 agent 通常也是“发送前本地 tokenizer/估算 + 返回后以 provider usage 记账”的组合，而不是只靠返回后的真实 token。

本次已把 `cache_write_tokens` 接到 provider usage、stream event、session projection、TUI metadata、cost tracker、usage JSONL 和 SQLite projection。当前 OpenAI/Kimi 适配层没有可用 cache write 字段时记录为 `None`；MiniMax 会从 `prompt_tokens_details.cache_write_tokens` 及兼容 alias 中提取。

成本统计也已区分 uncached prompt、cached/read prompt、cache-write/create tokens 和 completion tokens。默认 cache write 按 prompt lane 计价，并支持三层覆盖：

- provider：`PRIORITY_AGENT_COST_<PROVIDER>_CACHE_WRITE_PER_1K`
- model：`PRIORITY_AGENT_COST_MODEL_<MODEL>_CACHE_WRITE_PER_1K`
- global：`PRIORITY_AGENT_COST_CACHE_WRITE_PER_1K`

同类覆盖也支持 prompt、completion 和 cached prompt multiplier。

剩余边界：

- MiniMax/Kimi/Claude 等 provider 尚未接入官方真实 tokenizer，只能使用 profile fallback。
- 不同 provider 的字段名不统一，后续应继续按 provider certification matrix 补字段映射。

### 9. 压缩配置矩阵已补充，仍可拆成独立维护文档

**状态：文档内已完成；独立维护文档可后续补。**
原文只列了三个环境变量，但当前至少还包括 context collapse 和 time-based compression 相关变量。更大的问题不是变量数量，而是它们跨越不同层级：

- 请求局部压缩：不改写历史，只减少本次请求体。
- 会话历史压缩：会写 compact boundary，并可能替换 session messages。
- 显式 opt-in 实验服务：默认关闭，但已有主线 bridge。
- 时间/消息数触发：已接入 preflight，但仍使用同一个 compact boundary/压缩证据链。

本文已经补充压缩配置矩阵，`context_compressor.rs` 模块文档也写明核心开关和影响面。后续如果要面向用户或维护者，可以再拆成独立配置文档。

### 10. 经济守卫已保留 deterministic trim fallback

**状态：已完成当前修复。**
当前 compressor 会在短对话且连续 no-gain 后跳过 heavy work。这能避免反复做无收益压缩，但也可能在“短而巨大的工具输出”场景下跳过本该有效的轻量裁剪。

当前策略已经调整为：preflight circuit open 后仍然允许一次 deterministic `snip_tool_results` fallback；如果它能减少 token，则记录 `ContextCompactionStrategy::Snip + CompactionDecision::Compacted` 并关闭 no-gain circuit；如果没有收益，才保留 `CircuitOpen`。这样不会重新启用 heavy/LLM 压缩，但能处理短会话巨大旧工具输出。

本次已补充：

- short conversation huge old tool output 的 request-local selective compression 测试。
- circuit open 后 deterministic tool snip 的 preflight 测试。
- reactive `Retrying` / `Recovered` attempt record 测试。

### 11. protected tool outputs 已扩展为 runtime evidence 保护

**状态：已完成当前保护。**
选择性工具输出压缩和历史工具输出 snip 现在都保护 validation、permission、checkpoint、failure_owner、preserved skills 等 runtime evidence。普通旧工具输出仍可被压缩为 `evidence_safe_for_closeout=false` 摘要，但这些 closeout/recovery 关键证据会保留原文。

### 12. 后台修剪和时间触发压缩已接入主路径

**状态：已完成。**
`background_prune_tool_outputs()` 现在在 request bootstrap 中运行，默认开启，显式设置 `PRIORITY_AGENT_BACKGROUND_PRUNE=0/false/no/off` 可关闭。它只改写本轮请求里的旧工具输出，不写 compact boundary；如果发生修剪，会在 trace 中记录 fallback 事件，便于诊断。

`needs_time_based_compression()` 现在接入 preflight。若 token 压力还没达到 80% compact threshold，但会话时长或消息数超过阈值，会以 `time_based` trigger 走同一套 `ContextCompressor`、compact boundary、session event 和 trace 记录。这样不会额外创造第二套压缩系统。

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
- [x] 修正 `ContextCollapseService` 的清理建议，区分默认关闭的 gated 服务和已使用共享类型。
- [x] 修正 `last_result_idx` 问题描述，避免误判整个工具边界对齐无效。
- [x] 修正 `Message::content()` “重复实现”的误判。
- [x] 修正 token 估算对 CJK 的表述，改为模型无关启发式风险。
- [x] 补充 preflight、streaming、manual compact、API reactive、selective compression 的触发路径区分。
- [x] 补充 compact boundary、message replacement、runtime record 的持久化事实。
- [x] 补充 context collapse、time-based compression、background prune 的当前接入状态。
- [x] 补充压缩配置矩阵和后续整理建议。
- [x] 复核并更新当前完成状态：`last_result_idx`、`ContextCollapseService` 标注、skills marker 策略、`ContextManager` deprecation、token estimator profile 已完成。
- [x] 接入 `SessionMemoryCompact.user_preferences` 的显式 memory/profile 行提取。
- [x] 补充复杂 multi-tool-call group 边界测试。
- [x] 补充 API reactive `Retrying` / `Recovered` 压缩决策记录测试。
- [x] 为 preflight circuit-open 场景增加 deterministic tool snip fallback。
- [x] 扩展 protected tool outputs，避免验证、权限、checkpoint、skills、failure evidence 被普通压缩丢失。

## 本次复核验证

- `cargo fmt --check`
- `cargo check -q`
- `cargo test -q usage_ledger`
- `cargo test -q context_usage`
- `cargo test -q cost_tracker`
- `cargo test -q message_compression`
- `cargo test -q preflight_compression_controller`
- `cargo test -q turn_request_bootstrap_controller`
- `cargo test -q context_collapse`
- `cargo test -q cache_stability`
- `cargo test -q session_store`
- `cargo test -q context_compressor`
- `git diff --check`

## 后续行动

- [x] 接入 `SessionMemoryCompact.user_preferences` 的显式记忆/用户画像偏好提取。
- [x] 删除 `last_result_idx` 死变量。
- [x] 补充复杂 tool-call group 边界测试。
- [x] 为 API reactive compression 增加 `Retrying` / `Recovered` 记录测试。
- [x] 为 `ContextCollapseService` 增加 gated bootstrap bridge，并保留生产路径使用的共享类型。
- [x] 将 `has_active_skills` 改为明确的 always-preserve 策略。
- [x] 判断 `ContextManager` 是否仍有入口；当前已标记 deprecated。
- [x] 改进 token 估算，引入 JSON/tool schema 和 model context profile 启发式。
- [x] 补齐真实 usage 链路中的 `cache_write_tokens` 字段。
- [x] 将 background prune 接入 request bootstrap，默认开启且可显式关闭。
- [x] 将 time-based compression 接入 preflight，使用 `time_based` trigger。
- [x] 整理压缩相关环境变量文档，区分请求局部压缩、历史压缩和实验路径。
- [x] 为短会话巨大工具输出和 no-gain 后增长场景补测试并保留 deterministic snip fallback。
- [x] 评估并实现 protected tool outputs，避免验证、权限、checkpoint、skills、failure evidence 被压缩丢失。
- [x] 为 OpenAI-family provider/model profile 接入 `tiktoken-rs` 真实 tokenizer。
- [x] 为 cache write 增加 provider/model/global 单独计价 lane。
- [x] 在 `/config effective` 和 `/api/config` 暴露 token counter、压缩开关和 API runtime 状态。
- [ ] 如果要面向用户暴露完整配置说明，拆出独立压缩/运行时配置文档。
- [ ] 如 provider 发布官方 tokenizer，继续补 MiniMax/Kimi/Claude 的真实 tokenizer。

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
