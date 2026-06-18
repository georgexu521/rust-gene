# 记忆系统审计报告

日期：2026-06-17  
更新：2026-06-18

## 审计范围

本报告对 `src/memory/` 及其当前运行时接入点进行审计，覆盖：

- `src/memory/` 的长期记忆管理、提取、检索、质量、安全、持久化和评估代码；
- `src/engine/conversation_loop/` 中的记忆快照注入、动态召回、closeout 同步和 proposal 生成；
- `src/engine/task_contract/` 中的 `MemoryProposal`、review gate 和 proposal store；
- `src/tools/memory_tool/` 的手动记忆工具路径；
- `src/agent/memory.rs` 的子 agent 角色记忆边界。

## 审计结论

**整体评估：可继续演进，核心边界需要在文档中写清。**

当前记忆系统不是简单的“自动写入长期记忆”模块，而是一套带运行时边界的记忆管线：

- 默认 closeout 策略是 `review_only_default`，会记录边界评估和 review-required proposal，不默认把长期记忆静默写入主记忆；
- 自动写入只有在显式启用 `narrow` 或 `legacy` policy 时发生；
- LLM 可以提出候选记忆，但运行时仍负责质量、安全、去重、scope、stale/conflict 和持久化 gate；
- 对话循环已经接入静态 snapshot、动态 recall、request prefetch、closeout sync、background review nudge 和 memory tool 路径；
- `src/agent/memory.rs` 是子 agent KV/角色记忆，不应和 `src/memory/` 的长期用户/项目记忆合并。

因此，原始结论中“所有核心路径都已正确接入对话循环”方向基本成立，但表述过宽。更准确的说法是：主要运行时读链路和 proposal/write 链路已经接入，但不同入口有不同 policy、gate 和默认写入行为。

## 当前运行时集成链路

### 读取与注入

- `ConversationLoop` 持有可选 `MemoryManager`，并通过 `memory_use_enabled`、`memory_generate_enabled` 和 `memory_recall_mode` 分离读取、召回和生成行为。
- 静态记忆通过 `memory_snapshot_controller` 注入 `<memory-context>`，作为稳定前缀。
- 动态记忆通过 `turn_retrieval_context_controller` 构建 retrieval context，可走 LLM rerank、active memory worker 和 recall policy。
- 请求准备阶段在没有动态 memory item 时可通过 `request_preparation_controller` 做 prefetch。

### 写入与 proposal

- 默认 closeout 同步由 `memory_sync_controller` 执行，但默认 policy 是 review-only：记录边界事件，不直接写长期记忆。
- 成功、失败、partial、not verified 的任务结果会通过 closeout 形成 `MemoryProposal`，写入 review store，并保持 `write_policy=review_required`。
- 背景 review 和 memory nudge 可以生成或整理 proposal，但仍走 proposal/review 边界。
- 用户显式使用 memory tool 时，可以经由 `submit_candidate_with_provider_notifications` 进入质量、安全、去重和 provider notification 路径。
- legacy workflow gate 仍使用同步 `save_workflow_decision` 保存 workflow 决策；这属于旧路径，应避免扩大它的默认自动写入范围。

### 存储与安全

- 主长期记忆由 `MemoryManager` 管理，底层包括 markdown memory files、JSONL records、operation journal 和 search index。
- 写入候选经过 `assess_memory_candidate`、safety scan、dedup、score、target path 选择和 provider notification。
- provider 层维护本地 JSONL 记录、operation journal、search index 和 lifecycle hooks。

### 两套记忆系统

项目包含两套独立记忆系统：

1. **主记忆系统** (`src/memory/`)
   - 长期用户/项目记忆；
   - 跨会话保留；
   - 支持检索、提取、质量评估、安全扫描、proposal review、provider lifecycle。

2. **Agent 记忆系统** (`src/agent/memory.rs`)
   - 子 agent 的 KV 存储；
   - 用于 agent 工具和角色记忆；
   - 按角色/快照隔离，路径与长期项目记忆不同。

这是合理的架构分离，不是重复实现问题。

## 已完成工作

### 1. 清理 `save_workflow_decision_async` 死代码

**原位置：** `src/memory/manager/mod.rs:506-518`

**状态：已完成。**

异步版本没有调用者，已删除。当前保留同步 `save_workflow_decision`，实际调用点仍在 `legacy_workflow_gate_controller`。后续如果要恢复 provider notification，应显式改造 legacy workflow 写入路径，而不是保留未使用 wrapper。

### 2. 统一 `MAX_LEARNINGS_PER_SESSION_EXTRACT`

**原位置：**

- `src/memory/manager/helpers.rs`
- `src/memory/extraction.rs`

**状态：已完成。**

`extraction.rs` 已删除本地重复常量，改为从 manager helper 导入统一定义。

### 3. 收紧 `extract_learnings_from_turn` 可见性

**原位置：** `src/memory/extraction.rs`

**状态：已完成。**

函数已从 `pub(super)` 改为私有 `fn`。原 manager tests 中对该私有启发式的直接依赖已移除，测试迁移到 `extraction.rs` 内部，保持测试精度，同时减少跨模块 API 暴露。

## 未完成工作

### 4. 文件锁实现重复

**位置：**

- `src/memory/files.rs` 的 `MemoryFileLock`
- `src/memory/provider.rs` 的 `LocalMemoryFileLock`

**状态：未完成，建议延后。**

两个实现确实接近，后续可以抽出公共锁工具。但该项应保持低优先级，因为它触及 markdown memory files、JSONL records 和 operation journal 的并发写入路径。合并时必须保持现有 lock path 生成规则和非 Unix fallback 行为不变。

建议验收：

- `cargo fmt --check`
- `cargo test -q memory`
- `cargo check -q`

### 5. `contains_any` 辅助函数重复

**位置：**

- `src/memory/quality.rs`
- `src/memory/scoring.rs`
- `src/memory/files.rs` (`file_contains_any`)

**状态：未完成，不建议作为独立任务推进。**

重复事实成立，但这是低成本局部 helper，全 repo 也有多个按模块本地定义的 `contains_any`。把它提升到 `types.rs` 或 `mod.rs` 会让核心类型模块承担非类型职责，收益不高。

建议处理方式：只有在做统一 text utility 或 memory quality/scoring 重构时顺手整理，不作为当前 cleanup blocker。

### 6. 拆分 `src/memory/eval.rs`

**位置：** `src/memory/eval.rs`

**状态：未完成，建议 opportunistic 处理。**

文件约 1417 行，接近 1500 行 guardrail。已有 `src/memory/eval/review_workflow.rs` 子模块，后续可以继续按 recall、quality、conflict、proposal review 等类别拆分。

建议验收：

- `cargo fmt --check`
- `cargo test -q memory`
- 必要时补跑 `cargo check -q`

## 需要修正的原始表述

### “无 TODO/FIXME”

如果限定在 `src/memory/` 当前代码，未发现明显 TODO/FIXME；但不能扩展为整个代码库结论。仓库中的 scripts、docs、tests 和其他源码路径仍有 TODO/FIXME 样例、测试夹具或历史计划项。

### “无 `#[allow(dead_code)]`”

如果限定在 `src/memory/` 当前代码，基本成立；但不能扩展为整个代码库结论。其他模块仍存在局部 `#[allow(dead_code)]`，其中部分是诊断、UI 或工具兼容代码。

### “完整集成”

应改为：记忆读链路、召回链路、proposal 链路和手动工具写入链路已接入主运行时；但默认 closeout 不直接写长期记忆，自动写入受 policy 和 gate 限制。

## 后续计划

1. 保持当前默认 `review_required` 边界，不为追求自动化而放宽 memory write gate。
2. 下一次触碰 provider/file persistence 时，再合并文件锁实现，并保留现有锁文件路径语义。
3. 下一次扩展 memory eval 时，把 `src/memory/eval.rs` 拆成更小的子模块。
4. 不单独推进 `contains_any` 抽象；只有当 text utility 有更广泛需求时再统一。
5. 如果更新项目状态文档，应明确记忆系统当前是“review-first memory persistence”，不是“silent auto-memory”。

## 本次验证

2026-06-18 已验证：

```bash
cargo fmt --check
cargo test -q memory
cargo check -q
```

## 审计方法

- 静态代码分析；
- 依赖关系追踪；
- 对话循环运行时入口核对；
- 记忆写入 gate 和 proposal 边界核对；
- 目标测试验证。
