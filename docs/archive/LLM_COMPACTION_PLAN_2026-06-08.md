# LLM Context Compaction Plan - 2026-06-08
Status: Active — Phase 0-2 partial, Phase 3-4 not started

目标：学习 OpenCode 的主动上下文管理，但按 Priority Agent 的 runtime evidence
边界实现，避免为了省 token 破坏 closeout verification。

结论先放前面：

- 应该做上下文压缩，长任务的 dynamic tail 和 tool output 确实会持续增加成本。
- 不应该直接照搬 OpenCode 的整段对话压缩，因为 Priority Agent 的 closeout 依赖
  真实工具证据、验证命令、diff 状态和 context ledger。
- 当前代码已经有 `ContextCompressor`、LLM 摘要能力、preflight compression、
  reactive compression 和 message healing；下一步不是新起一套 `llm_compaction.rs`，
  而是把现有压缩链路做成 **evidence-aware compaction**。
- 第一版不要直接把 60% 阈值作为默认主动压缩。先加观测和证据保护，再打开压缩。

---

## 1. 当前代码事实

### 已有基础

| 能力 | 位置 | 当前状态 |
|------|------|----------|
| `ContextCompressor` | `src/engine/context_compressor.rs` + `context_compressor/compressor.rs` | 已有压缩主模块 |
| 策略枚举 | `src/engine/context_collapse.rs` | `NoOp`、`Snip`、`MicroCompact`、`AutoCompact`、`ReactiveCompact`、`SessionMemoryCompact` |
| Hermes-style summary template | `src/engine/context_compressor.rs` | 已有 `SUMMARY_TEMPLATE` 和 `SUMMARY_PREFIX` |
| LLM 摘要能力 | `ContextCompressor::llm_summarize_middle()` | 已有，但 prompt 不是严格复用 `SUMMARY_TEMPLATE` |
| LLM provider 接入 | `with_llm_provider(...)` | 已有，可用主 provider/model |
| preflight compression | `src/engine/conversation_loop/preflight_compression_controller.rs` | 已接入请求前压缩 |
| reactive compression | `src/engine/conversation_loop/api_request_controller.rs` | provider 报 context size error 时压缩并重试 |
| streaming history preflight | `src/engine/streaming.rs` | streaming history 也有 preflight compression |
| message healing | `src/engine/message_healing.rs` | 每次请求前 shrink oversized tool results、drop dangling tool calls |
| tool result metadata | `src/engine/conversation_loop/tool_result_controller.rs` | 已记录 `compaction_eligible`、`protected_recent_tail`、`ledger_fact_eligible` 等元数据 |
| runtime diet | `src/engine/conversation_loop/runtime_diet.rs` | 已记录 prompt/tool schema/tool result/retrieval/memory 等 token 信号 |

### 原文需要修正的点

- 当前没有 `src/engine/message_compression.rs`。
- 当前没有 `src/engine/llm_compaction.rs`。
- 当前没有名为 `compress_via_llm()` 的函数；实际函数是
  `ContextCompressor::llm_summarize_middle()`。
- `AutoCompact` 已有 LLM 摘要路径，但不是“和 OpenCode 一样只差触发策略”。
  当前还缺少 evidence boundary、strict summary contract、closeout trust boundary 和
  增量 LLM summary 的生产验证。
- LLM 增量压缩不是完全没有基础：`StructuredSummary` 有 heuristic accumulated
  summary / merge；但 LLM anchored summary 还没有作为生产 contract 建立。
- 不能把 request preparation 作为唯一接入点。当前生产路径已经有
  `PreflightCompressionController`，这是更自然的接入点。

---

## 2. 和 OpenCode 的核心差距

| 维度 | OpenCode 倾向 | Priority Agent 当前状态 | 下一步 |
|------|---------------|-------------------------|--------|
| 触发时机 | 主动 compact | preflight 80% + reactive context error | 先观测，再调阈值 |
| 压缩对象 | 对话历史 + tool output | compressor 可压历史，message healing 可截断 tool output | 先压旧 tool output，再扩大 |
| 摘要者 | LLM compaction agent | `ContextCompressor` 可调用 LLM provider | 复用现有 compressor，不新建并行系统 |
| 多轮摘要 | anchored summary | heuristic accumulated summary 有基础 | 补 LLM anchored contract |
| 验证证据 | 不以 closeout evidence 为核心 | closeout 强依赖 runtime evidence | 必须 evidence-aware |

OpenCode 值得学的是主动管理上下文，不是把所有旧消息都压成自然语言摘要。

Priority Agent 的特殊约束是：压缩后的历史不能伪装成验证证据。对于代码任务，
最终 closeout 需要的是：

- required validation command 是否运行过。
- exit/status 是什么。
- 关键输出是否支持 acceptance。
- changed files / diff 状态是否匹配任务。
- 若证据不足，必须保留 `not_verified` 或 `partial`，不能 false green。

---

## 3. 推荐架构：三层，但复用现有 compressor

### Layer 1: Evidence-aware tool-output compaction

目标：先压最贵、最容易膨胀、语义风险相对可控的旧 tool output。

当前基础：

- `message_healing.rs` 已能按字符数截断 oversized tool results。
- `tool_result_controller.rs` 已记录 `compaction_eligible` 和
  `protected_recent_tail`。
- `ContextCompressor::snip_tool_results()` / `prune_old_tool_results()` 已能裁剪旧工具结果。

下一步不是新增 `message_compression.rs`，而是增强现有路径：

```text
keep raw:
  - 最近 2 轮 tool output
  - required validation / closeout evidence
  - failed tool output 的关键错误上下文
  - 用户明确要求查看的输出

compress:
  - 更早轮次的大 stdout/stderr
  - 重复 build/test/log 噪声
  - 已经被 ledger 结构化记录过的输出
```

压缩结果必须是结构化摘要：

```text
[compressed-tool-output]
tool=bash
cmd=cargo test -q
exit=0
status=passed
raw_preserved=false
evidence_safe_for_closeout=false
key_lines:
  - test result: ok
  - 19 passed; 0 failed
call_id=...
source_turn=...
```

`evidence_safe_for_closeout=false` 时，closeout controller 不能把它当完整验证证据。

### Layer 2: Existing LLM conversation compaction hardening

目标：把已有 `ContextCompressor::llm_summarize_middle()` 做成可安全生产使用的
LLM 语义压缩，而不是新建一套平行模块。

当前 LLM prompt 只是要求“Summarize this conversation into 8 sections”，并截取
前 8000 chars。下一步应该改成严格 contract：

```text
Output exactly these sections:

## Goal
## Constraints
## Progress
## Key Decisions
## Relevant Files
## Next Steps
## Critical Context
## Tools & Patterns

Rules:
- Preserve exact file paths, command strings, error strings, identifiers.
- Mark validation evidence as historical unless raw evidence is preserved.
- Do not claim tests passed unless raw validation evidence remains available.
- Do not omit unresolved blockers.
- Do not include secrets.
```

LLM compaction summary 应该是 continuation context，不是 closeout proof。

### Layer 3: Anchored incremental summary

目标：多次 compaction 时，不重复总结全部历史，而是：

```text
previous summary + newly compacted range -> updated summary
```

当前 `StructuredSummary` 已有 heuristic accumulated summary / merge 基础，但 LLM
anchored summary 还需要补：

- previous summary marker。
- summary version / boundary id。
- stale facts removal rule。
- retained evidence counts。
- no-gain / failure circuit breaker。

---

## 4. 接入点

### 不建议新增 `llm_compaction.rs`

当前 `ContextCompressor` 已经包含：

- provider/model。
- LLM summary attempts/failures。
- compact records。
- compact boundary metadata。
- circuit breaker。
- strategy labels。

新增 `llm_compaction.rs` 容易制造第二套状态机。更稳的做法是：

1. 扩展 `ContextCompressor` 的 evidence-aware selection。
2. 扩展 `CompactionRuntimeRecord` / `CompactionAttemptRecord` 的 retained evidence 字段。
3. 让 `PreflightCompressionController` 继续作为请求前主入口。
4. 保留 `ApiRequestController` 的 reactive compression 路径。

### 主要文件

| 文件 | 建议改动 |
|------|----------|
| `src/engine/context_compressor.rs` | 增加 evidence-aware retained item / token 字段，必要时扩展 strategy |
| `src/engine/context_compressor/compressor.rs` | 增强 tool-output selection、LLM summary prompt、anchored summary |
| `src/engine/context_collapse.rs` | 扩展 runtime record 字段，记录 evidence preservation |
| `src/engine/conversation_loop/preflight_compression_controller.rs` | 继续作为主动压缩入口，补 trace / runtime diet 观测 |
| `src/engine/conversation_loop/tool_result_controller.rs` | 将 `compaction_eligible` / `protected_recent_tail` 用于压缩选择 |
| `src/engine/conversation_loop/runtime_diet.rs` | 增加 dynamic tail / compressible / required evidence token 观测 |
| `src/engine/conversation_loop/closeout_controller.rs` | 明确 compact summary 不可作为 raw validation evidence |
| `src/engine/message_healing.rs` | 保留 provider 400 防护；不要把它当完整 compaction 系统 |

---

## 5. 触发策略

### 当前触发

当前 `TokenBudget::needs_compression()` 使用约 80% 可用历史预算作为阈值。
`CompressionWarning` 在 60% 进入 approaching，但 warning 不是主动压缩。

### 推荐顺序

不要第一步就把主动压缩阈值改为 60%。先做：

1. 记录 token breakdown。
2. 看真实长任务里 dynamic tail 的组成。
3. 证明旧 tool output 是主要来源。
4. 再把 Layer 1 tool-output compaction 提前。
5. 最后再讨论是否把 LLM compaction 提前到 60% 或 70%。

建议新增或扩展 runtime diet 字段：

```text
dynamic_tail_tokens
tool_result_tokens
compressible_tool_result_tokens
required_evidence_tokens
raw_tool_outputs_preserved
tool_outputs_compressed
compression_chars_saved
evidence_summaries_emitted
```

---

## 6. 环境变量

建议按两级开关，不要一个总开关直接启用所有压缩：

```text
PRIORITY_AGENT_TOOL_OUTPUT_COMPACTION=1
PRIORITY_AGENT_LLM_COMPACTION=0
PRIORITY_AGENT_LLM_COMPACTION_THRESHOLD=0.8
PRIORITY_AGENT_LLM_COMPACTION_PRESERVE_TURNS=2
PRIORITY_AGENT_LLM_COMPACTION_MAX_INPUT_CHARS=8000
```

默认建议：

- tool-output compaction 可以先在 eval/replay 中开。
- LLM compaction 默认关，等证据保护和 closeout tests 过后再开。
- threshold 初始沿用 80%，不要直接改成 60%。

---

## 7. 安全约束

1. **compact summary 不是验证证据**
   LLM summary 只能作为 continuation context，不能作为 closeout proof。

2. **required validation raw evidence 必须保留或有 ledger-backed proof**
   如果 raw output 被压缩，必须有结构化 ledger evidence 支撑，否则 closeout 只能
   `not_verified` / `partial`。

3. **最后 N 轮原文保留**
   默认至少保留最后 2 个 user turn 对应的 assistant/tool 上下文。

4. **失败输出要保留关键错误行**
   `error[E...]`、panic、failed command、missing file、permission denial 等不能被
   摘要吞掉。

5. **LLM 摘要失败必须 fallback**
   当前 compressor 已有 LLM failure counters 和 circuit breaker；继续保留。

6. **不能增加 prompt 常驻规则墙**
   summary prompt 是压缩调用专用，不进入普通 agent always-on prompt。

---

## 8. 验证方式

### 必须新增/更新的测试

| 测试 | 验证 |
|------|------|
| `tool_output_compaction_preserves_recent_tail` | 最近 2 轮 tool output 原文保留 |
| `required_validation_output_not_trusted_when_compacted` | compact summary 不能让 closeout false green |
| `llm_compaction_summary_is_context_not_evidence` | LLM summary 不被当作 raw validation proof |
| `failed_tool_output_keeps_key_error_lines` | 失败输出关键错误行保留 |
| `preflight_records_compressible_token_breakdown` | runtime diet / trace 有压缩机会观测 |
| `anchored_summary_updates_previous_summary` | 增量 summary 合并新事实并保留仍有效事实 |

### Live eval / YAML

建议新增或复用：

- `context-closeout-evidence-preserved`
- `context-cache-dynamic-tail-tracked`
- `context-llm-compaction-no-false-green`
- `context-tool-output-compaction-saves-tokens`

### 验证命令

```bash
cargo test -q context_compressor -- --test-threads=1
cargo test -q preflight_compression_controller -- --test-threads=1
cargo test -q closeout -- --test-threads=1
cargo test -q request_preparation_controller -- --test-threads=1
```

---

## 9. 推荐执行顺序

### Phase 0: 修正观测

1. 增加 token breakdown：dynamic tail / tool output / required evidence /
   compressible output。
2. 在 trace/runtime diet 中显示压缩机会。
3. 不改变压缩行为。

### Phase 1: Evidence-aware Layer 1

1. 用 `tool_result_controller` 的元数据驱动旧 tool output selection。
2. 保留 raw recent tail 和 required evidence。
3. 输出 `[compressed-tool-output]` 结构化摘要。
4. 跑 closeout tests，证明不 false green。

### Phase 2: Harden existing LLM compaction

1. 强化 `llm_summarize_middle()` 的 prompt contract。
2. 标记 LLM summary 为 context-only。
3. 继续用 preflight controller 触发，不新建并行状态机。
4. 默认 env off，只在 eval/replay 开。

### Phase 3: Anchored incremental summary

1. 将 previous summary 作为输入。
2. 输出 updated summary。
3. 记录 boundary id、summary version、retained evidence。
4. 对比 no-compaction / compaction 两组 closeout pass rate。

### Phase 4: 阈值调优

1. 用真实长任务数据决定 80% 是否太晚。
2. 如果 Layer 1 稳定，可以考虑 tool-output compaction 在 60%-70% 主动触发。
3. LLM compaction 继续保守，除非 token savings 和 closeout safety 都有证据。

---

## 10. 总结

这份计划的方向是对的：Priority Agent 应该学习 OpenCode 的主动上下文压缩，用
LLM 做语义摘要，降低长任务 token 成本。

需要修正的是实现路线：

- 不新建一套 `llm_compaction.rs` 状态机。
- 不把不存在的 `message_compression.rs` 当基础。
- 不直接把阈值改成 60%。
- 不把 LLM summary 当 closeout evidence。
- 先做 evidence-aware tool-output compaction 和 token observability，再启用 LLM
  conversation compaction。

最终目标不是“压得越多越好”，而是在不破坏 runtime proof 的前提下，让长任务的
dynamic tail 可控、可解释、可回放。
