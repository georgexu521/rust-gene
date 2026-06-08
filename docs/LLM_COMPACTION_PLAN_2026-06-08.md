# LLM-Based Context Compaction Plan — 2026-06-08

将现有规则压缩升级为 OpenCode 式的 LLM 语义压缩。保留 closeout 证据安全。

---

## 1. 现状

### 我们已经有的基础

| 已有 | 位置 | 状态 |
|------|------|------|
| `AutoCompact` 策略 | `context_compressor.rs:796` | ✅ LLM 摘要模板已存在（8-section Hermes 格式） |
| `SUMMARY_TEMPLATE` | `context_compressor.rs:73` | ✅ Goal/Constraints/Progress/Decisions/NextSteps/CriticalContext/RelevantFiles |
| `SUMMARY_PREFIX` | `context_compressor.rs:101` | ✅ 摘要前缀 |
| LLM 调用能力 | `context_compressor.rs::compress_via_llm()` | ✅ 已有 |
| 选择性压缩 | `message_compression.rs` | 🟡 刚加的，基于规则 |

### 和 OpenCode 的核心差距

| | OpenCode | Priority Agent |
|---|---|---|
| 触发时机 | **主动**（token 快满前就压） | **被动**（80% 才动） |
| 谁做压缩 | 专用 compaction agent（LLM） | 规则引擎（extract key lines） |
| 压什么 | **对话历史 + 工具输出** | 只压工具输出 |
| 多轮压缩 | 支持（previous summary → merge） | 不支持 |
| 摘要格式 | 8-section Markdown（与 Hermes 同） | 结构化 [compressed-tool-output] |

实际上我们的 `AutoCompact` 策略已经有 LLM 摘要能力——和 OpenCode 使用同一个 8-section 模板！差距只是触发策略和压缩范围。

---

## 2. 改进方案：三层压缩

```
Layer 1: 规则压缩（现有 message_compression.rs，改造）
  → 压缩单个超大 tool output（>2000 chars 的 build/test output）
  → 保留 exit code + 关键行
  → 不调用 LLM，零成本

Layer 2: LLM 对话压缩（新增，借鉴 OpenCode）
  → 当 token 达到 60% 预算时主动触发
  → 把旧轮对话（不含最后 2 轮）发给 LLM 生成 8-section 摘要
  → 旧轮被替换为摘要消息
  → 成本：每次压缩 1 次 LLM 调用

Layer 3: 增量压缩（新增，借鉴 OpenCode 的 anchored summary）
  → 再次触发时，把之前的摘要 + 新对话发给 LLM
  → LLM 更新摘要（保留仍有效的事实，合并新信息）
  → 单次摘要可以跨多轮复用
```

### Layer 1 vs Layer 2/3 的区别

```
Layer 1（规则）:
  5000 行 build output → [compressed] exit=0, pass: "19 passed", raw=5000
  免费、可靠、但不理解语义

Layer 2/3（LLM）:
  10 轮对话 → "用户正在重构 scoring.rs，已完成系数提取，
  当前卡在 test_score_memory_write 的 use_super 导入缺失"
  有成本、有语义理解、可能有遗漏
```

---

## 3. 实现细节

### Layer 2：LLM 对话压缩

**触发条件**：
```rust
// 在 request_preparation_controller 中，zone 注入之后
let estimated_tokens = estimate_request_tokens(&request_messages);
if estimated_tokens > max_tokens * 0.6 {
    llm_compact_conversation(&mut request_messages, provider, model, trace).await;
}
```

**压缩范围**：
```
messages: [sys] [u1] [a1] [t1] [t2] [u2] [a2] [u3] [a3] [t3] [t4] [u4]
                                                    ↑
                                              preserve_boundary (最后 2 个 user turn)
                                         
压缩部分: [u1][a1][t1][t2] → LLM → summary
保留部分: [u2][a2][u3][a3][t3][t4][u4] ← 原文不动
```

**消息替换**：
```rust
// 替换后:
messages: [sys] [user(compaction)] [assistant(summary)] [u2][a2][u3][a3][t3][t4][u4]
```

- 被压缩的消息被移除
- 插入一个 compaction user 消息（含 `[COMPACTION_BOUNDARY]` 标记）
- 插入一个 summary assistant 消息（LLM 生成的 8-section 摘要）
- 保留的消息（最后 2 轮）原文不动

**LLM prompt**：复用现有 `SUMMARY_TEMPLATE`，和 OpenCode 内容一致：

```markdown
Output exactly the Markdown structure shown below:

## Goal
- [single-sentence task summary]

## Constraints & Preferences
- [user constraints, preferences, or "(none)"]

## Progress
### Done
- [completed work]

### In Progress
- [current work]

### Blocked
- [blockers or "(none)"]

## Key Decisions
- [decision and why]

## Next Steps
- [ordered next actions]

## Critical Context
- [important technical facts, errors, open questions]

## Relevant Files
- [file path: why it matters]

Rules:
- Keep every section, even when empty.
- Use terse bullets, not prose paragraphs.
- Preserve exact file paths, commands, error strings, identifiers.
- Do not mention the summary process or that context was compacted.
```

**closeout 证据保护**：
```rust
// 在压缩时，标记哪些消息是 closeout 需要的证据
fn is_closeout_evidence(msg: &Message) -> bool {
    // Required validation 命令的输出不压缩
    matches!(msg, Message::Tool { content, .. } 
        if content.contains("required command") 
        || content.contains("[exit status:")
        || content.contains("cargo test"))
    // 最后 2 轮的 tool output 不压缩
}
```

### Layer 3：增量压缩

当再次触发压缩时，不是压全部历史，而是：

```
上一次的 summary + 新对话 → LLM → 更新后的 summary
```

prompt 格式（借鉴 OpenCode）：
```
Update the anchored summary below using the conversation history above.
Preserve still-true details, remove stale details, and merge in the new facts.
<previous-summary>
(上次的 summary)
</previous-summary>
```

---

## 4. 改动文件

| 文件 | 改动 |
|------|------|
| `src/engine/context_compressor.rs` | 复用现有 `AutoCompact` 和 `SUMMARY_TEMPLATE`，无改动 |
| `src/engine/llm_compaction.rs` | **新建**：LLM 对话压缩 + 增量压缩 |
| `src/engine/message_compression.rs` | 保留 Layer 1 规则压缩，改名为 `rule_compression.rs` 或保留 |
| `src/engine/conversation_loop/request_preparation_controller.rs` | 接入触发逻辑 |
| `src/engine/mod.rs` | 注册 `llm_compaction` 模块 |

### llm_compaction.rs 核心函数

```rust
// 检查是否需要 LLM 压缩
pub fn should_compact(messages: &[Message], max_tokens: usize) -> bool {
    estimate_tokens(messages) > max_tokens * 6 / 10  // 60% 触发
}

// 执行 LLM 压缩
pub async fn compact_conversation(
    messages: &mut Vec<Message>,
    provider: &dyn LlmProvider,
    model: &str,
    preserve_turns: usize,
    previous_summary: Option<&str>,
    trace: &TraceCollector,
) -> LlmCompactionReport {
    // 1. 找到压缩边界（最后 preserve_turns 个 user turn 之前）
    // 2. 提取被压缩的消息
    // 3. 构建 prompt（含 previous summary 如果有）
    // 4. 调用 LLM 生成 8-section 摘要
    // 5. 替换消息列表
    // 6. 记录 trace
}
```

### 环境变量控制

```
PRIORITY_AGENT_LLM_COMPACTION=1       # 启用 LLM 压缩（默认关闭）
PRIORITY_AGENT_LLM_COMPACTION_THRESHOLD=0.6  # 触发阈值（默认 60%）
PRIORITY_AGENT_LLM_COMPACTION_PRESERVE_TURNS=2  # 保留轮数（默认 2）
```

---

## 5. 安全约束

1. **不压缩 closeout 证据**：required_validation 命令的输出、`[exit status:]` 标记的行不进入压缩范围
2. **保留最后 N 轮原文**：默认 2 轮，确保 LLM 有足够上下文理解当前状态
3. **摘要标记为不可信**：compaction summary 消息标记 `_compacted=true`，closeout controller 不能把它当验证证据
4. **增量压缩不丢关键事实**：每次压缩前把 previous summary 传给 LLM，LLM 自己判断哪些还有效
5. **单 LLM 调用的 cost 预算**：压缩用的 provider 可以降级（比如用 flash 而不是 pro），不影响 agent 质量

---

## 6. 验证方式

| 验证项 | 方法 |
|--------|------|
| 压缩后 closeout 不 false green | `context-closeout-evidence-preserved` YAML 重跑 |
| 压缩后 token 减少 | `ContextTokenBreakdown` trace 对比压缩前后 |
| LLM 摘要质量 | 人工抽查 3-5 次压缩结果 |
| 增量压缩不过度丢失 | 对比"无压缩"vs"压缩"两组的 closeout pass rate |

---

## 7. 推荐执行顺序

1. **Layer 1 保留**：`message_compression.rs` 继续做超大 tool output 的规则压缩（零成本）
2. **Layer 2 先实现**：LLM 对话压缩（主动触发、8-section 摘要、保留 2 轮）
3. **验证**：跑 context-closeout-evidence-preserved 确认不 false green
4. **Layer 3 后实现**：增量压缩（需要 Layer 2 的摘要格式稳定后再做）
5. **阈值调优**：用真实长任务的 token breakdown 数据决定 60% 是否合适
