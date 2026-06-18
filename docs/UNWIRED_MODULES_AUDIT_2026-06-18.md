# 主线接入状态审计

日期：2026-06-18

## 结论

本次对照代码扫描了压缩、缓存、上下文注入、API、路由、记忆、自动化和 TUI 命令相关模块。当前项目确实存在一些“实现了但默认不跑”的模块，但不能简单等同为遗漏：

- 有些已经在本轮接入主请求路径。
- 有些应该继续默认关闭，因为它们会写长期状态、改写历史、增加 LLM 成本，或只是诊断影子路径。
- 有些是产品化候选，后续如果要加强 API/桌面体验，可以继续接。
- 少数是明确 deprecated、placeholder 或保留测试/参考用途，不应该再接主线。

## 本轮已接入

| 能力 | 当前状态 | 代码位置 | 说明 |
|------|----------|----------|------|
| `cache_write_tokens` | 已接入 | provider usage、stream event、session projection、TUI、cost tracker、usage ledger | 请求后真实 usage 现在能区分 cached/read 与 cache write；OpenAI/Kimi 无字段时为 `None`，MiniMax 已做 alias 提取 |
| `background_prune_tool_outputs()` | 已接入，默认开 | `turn_request_bootstrap_controller.rs` | 每轮 request bootstrap 运行，只修剪本轮请求里的旧工具输出，不写 compact boundary |
| time-based compression | 已接入，默认开 | `preflight_compression_controller.rs` | token 压力没到阈值但消息数/会话时长超阈值时，以 `time_based` trigger 复用 preflight 压缩链路 |
| `ContextCollapseService` | 已接入但默认关 | `context_collapse.rs` + request bootstrap | `PRIORITY_AGENT_CONTEXT_COLLAPSE=1` 后才运行；会折叠旧消息并写磁盘 collapse entries，仍是实验路径 |
| real tokenizer profile | 部分接入 | `context_compressor.rs` / `context_budget_controller.rs` | OpenAI GPT-4o/GPT-4.1/reasoning family 使用 `tiktoken-rs` 的 `o200k_base`/`cl100k_base`；MiniMax/Kimi/Claude 仍使用 provider profile fallback 启发式 |
| provider-specific cache write pricing | 已接入 | `cost_tracker.rs` | 成本模型已区分 uncached/cached/cache-write/completion；支持 provider/model/global env override |
| full-agent prompt API visibility | 已产品化当前状态 | `api/state.rs` / `api/routes/mod.rs` | 生产 API startup 已注入 `RuntimeController` runtime；`/api/config` 暴露 `runtime.full_agent_prompt_available` |
| 配置总览 | 已接入 TUI/API | `/config effective` / `/api/config` | TUI 展示模型上下文、token counter、压缩开关、cache write 计价覆盖和 full-agent API 状态入口 |

## 仍应保持显式 opt-in

| 能力 | 默认 | 原因 |
|------|------|------|
| LLM compaction | 关 | 会增加 LLM 调用成本和语义压缩风险；当前启发式压缩和 deterministic snip 是默认安全路径 |
| active memory worker | 关 | 只读 FTS 召回已经有 eval/headless/automation/internal gates；默认开启前需要更多 soak evidence |
| 自动长期记忆写入 `legacy` / `narrow` | 关 | 长期记忆会影响未来上下文，当前 review-first 是正确安全边界 |
| `ContextCollapseService` | 关 | 它会从 live request 移走旧消息，不同于 compact summary；适合作为实验或特殊长会话开关 |
| auto code review | 关 | `PRIORITY_AGENT_AUTO_REVIEW=1` 目前是轻量静态审查，不应该默认阻塞所有修改 |
| LLM route shadow / route diagnostics | 关 | 只应记录诊断，不应默认改变 deterministic routing |
| route debug/full tool exposure | 关 | 调试用全工具面会绕过 route-scoped tool surface 的默认收敛 |

这些不是“漏接”。它们涉及成本、安全、可解释性或产品噪声，默认关闭是合理的。

## 后续产品化候选

| 候选 | 当前问题 | 建议 |
|------|----------|------|
| provider-native tokenizers | MiniMax/Kimi/Claude 没有本地精确 tokenizer | 若 provider 发布官方 tokenizer 或兼容 tokenizer，再补 provider-specific exact counter；继续保留 provider usage 作为请求后真实账本 |
| proposal nudge | memory proposal 已有 review queue，但强提醒不足 | closeout 或 status bar 增加低噪声 pending proposal badge |
| 配置总览独立文档 | TUI/API 已可见，但还没有单独维护手册 | 后续可把 `/config effective` 的字段拆成用户/维护者文档 |

## 明确不建议接回主线

| 模块/能力 | 状态 | 原因 |
|-----------|------|------|
| `ContextManager` | deprecated | 主路径已经是 `PreflightCompressionController + ContextBudgetController + ContextCompressor`，旧 wrapper 不应复活 |
| TUI placeholder commands | 有意隐藏于默认 help | placeholder/usable/production 已有成熟度分层；占位命令不应误当生产能力 |
| patch synthesis bypass | 默认禁用 | 项目原则要求权限、stale-read、diff、rollback checks 留在工具路径，不应让 synthesis 绕开硬边界 |
| broad/default full tool surface | 默认不启用 | route-scoped tools 是当前 runtime 收敛方向，debug/full profile 只适合调试 |

## 对用户问题的直接回答

1. 80% 压缩机制仍需要发送前预判。因为“是否到 80%”本身只能在发送前用 tokenizer/估算判断；provider 的真实 usage 只能在请求完成后返回。
2. `cache_write_tokens` 值得补，已接入请求后真实 usage 链路。
3. `background_prune_tool_outputs()` 值得作为亮点接入，已放进 request bootstrap，默认启用。
4. time-based compression 仍有价值。它不是替代 80% 机制，而是处理“token 还没爆，但会话已经很长”的整理触发；现在复用 preflight 链路。
5. `ContextCollapseService` 已接入流水线，但保持默认关闭是必要的。它会真实移走旧消息，风险级别高于 request-local prune。
6. 其它未接主线模块主要是有意门控，不是功能缺口。本轮已经推进 full-agent API 可见性、OpenAI-family 真实 tokenizer、provider-specific cache write pricing 和配置总览；仍值得后续推进的是 provider-native tokenizer 覆盖、配置手册和 memory proposal nudge。

## 验证建议

本轮改动后建议至少跑：

```bash
cargo fmt --check
cargo check -q
cargo test -q usage_ledger
cargo test -q cost_tracker
cargo test -q context_usage
cargo test -q message_compression
cargo test -q preflight_compression_controller
cargo test -q turn_request_bootstrap_controller
cargo test -q cache_stability
cargo test -q session_store
git diff --check
```
