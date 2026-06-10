# Hermes Memory Borrow Plan — 2026-06-08
Status: Completed

All 5 features implemented and verified in codebase:
1. Memory Nudge — `advance_nudge()` in `src/memory/manager/mod.rs`
2. Background Review Fork — `src/memory/background_review.rs`
3. Streaming Context Scrubber — `src/engine/streaming/context_scrubber.rs`
4. Dialogic Prefetch — `prefetch_retrieval_context_dialectic()` in `src/memory/retrieval.rs`
5. Contradiction Detection — `src/memory/contradiction.rs`

从 Hermes 借鉴 5 项记忆增强功能，每一节对照 Hermes 源码与 Priority Agent 现有代码，
给出精确的实现路径和改动范围。

---

## 1. 记忆提醒 Nudge

### Hermes 怎么做

**触发逻辑** (`agent/conversation_loop.py:460-470`):

```python
_should_review_memory = False
if (agent._memory_nudge_interval > 0
        and "memory" in agent.valid_tool_names
        and agent._memory_store):
    agent._turns_since_memory += 1
    if agent._turns_since_memory >= agent._memory_nudge_interval:
        _should_review_memory = True
        agent._turns_since_memory = 0
```

**计数器重置** (`agent/tool_executor.py:91-92`):

```python
if function_name == "memory":
    agent._turns_since_memory = 0
```

**配置** (`agent/agent_init.py:1061,1069`):

```python
agent._memory_nudge_interval = 10  # default
agent._memory_nudge_interval = int(mem_config.get("nudge_interval", 10))
```

**问题**：LLM 在专注于编程任务时，经常忘记主动调用 memory 工具保存信息。Nudge 在累计 N 轮没有调用 memory 后触发一次后台审查，提醒框架替 LLM 做这件事。

### Priority Agent 现状

我们有 `sync_turn_llm_background` 每轮都做 LLM 提取（`src/memory/extraction.rs:70-173`），但成本高（每轮额外 LLM 调用），且 `forked_mode` 才有后处理效果。

我们没有按轮次的 nudge 计数器——要么每轮都 LLM 提取，要么不做。

### 实现路径

**新文件**：不需要

**修改的文件**：

| 文件 | 改动 |
|------|------|
| `src/memory/manager/mod.rs` | 在 `MemoryManager` 加 `turns_since_memory_write: usize` 字段，`memory_nudge_interval: usize` 配置 |
| `src/memory/manager/helpers.rs` | 加 `MEMORY_NUDGE_DEFAULT_INTERVAL: usize = 10` 常量 |
| `src/engine/conversation_loop/session_processor.rs` | 每轮调用 `memory_manager.check_nudge_and_review()` |

**具体步骤**：

1. **MemoryManager 中添加字段和函数** (`manager/mod.rs`):

```rust
// 在 MemoryManager 结构体中添加:
pub(super) turns_since_memory_write: usize,
memory_nudge_interval: usize,

// 在 with_base_dir 中初始化:
turns_since_memory_write: 0,
memory_nudge_interval: std::env::var("PRIORITY_AGENT_MEMORY_NUDGE_INTERVAL")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(10),
```

2. **添加 nudge 检查函数** (`manager/mod.rs`):

```rust
impl MemoryManager {
    /// 每轮递增计数器，达到 nudge_interval 时返回 Some(source) 触发后台审查
    pub fn advance_nudge(&mut self, memory_tool_called_this_turn: bool) -> Option<String> {
        if memory_tool_called_this_turn {
            self.turns_since_memory_write = 0;
            return None;
        }
        self.turns_since_memory_write += 1;
        if self.turns_since_memory_write >= self.memory_nudge_interval {
            self.turns_since_memory_write = 0;
            Some("nudge".to_string())
        } else {
            None
        }
    }

    /// 当 LLM 调用 memory_* 工具时，重置 nudge 计数器
    pub fn record_memory_tool_call(&mut self) {
        self.turns_since_memory_write = 0;
    }
}
```

3. **在 conversation loop 中接入** (`session_processor.rs`):

每轮完成后（在 `sync_turn` 之后），调用：
```rust
if let Some(ref memory_manager) = self.memory_manager {
    let mut mem = memory_manager.lock().await;
    let had_memory_tool = trace.events.iter().any(|e| e.tool_name_matches(&["memory_save", "memory_load"]));
    if let Some(source) = mem.advance_nudge(had_memory_tool) {
        // 触发后台 LLM 提取（复用现有 sync_turn_llm_background）
        // 或触发 background_review（下节实现）
    }
}
```

4. **在工具执行时重置计数器** (`tool_execution_controller.rs`):

当 `memory_save` 工具被调用时，调用 `memory_manager.record_memory_tool_call()`。

**风险**：极低。只是加计数器，不改变现有逻辑。

**ROI**：很高。零成本（只在 nudge 触发时才启动 LLM），大幅减少漏记。

---

## 2. 后台审查 Fork

### Hermes 怎么做

**核心思想**：不是"让 LLM 从对话中提取记忆"，而是"fork 一个受限的 agent，用对话历史当输入，让它用 memory/skill 工具自己决定是否存记忆"。

**实现** (`agent/background_review.py:327-555`):

1. Fork 继承父 agent 的 provider/model/credentials/system_prompt
2. 工具面限为 `memory` + `skills` 两种
3. 输入是当前对话历史的 snapshot
4. Prompt 问："对话中发现了哪些值得记住的信息？"
5. 结果通过 `_safe_print` 回调通知用户

**关键约束**：
- `skip_memory=True` 防止递归 fork
- `_memory_nudge_interval = 0` 防止内层触发 nudge
- 继承父 agent 的 `_cached_system_prompt` 复用 prefix cache

**Prompt** (`background_review.py:34-43`):

```
Review the conversation between a user and an AI coding assistant.
1. Has the user revealed personal details worth remembering (name, role, preferences, coding style)?
2. Has the user expressed expectations about agent behavior that should carry forward?
3. Have you identified project conventions, environment facts, or tool quirks that will be useful later?

If yes, save them with the appropriate memory tool.
Do NOT save task progress, arbitrary code excerpts, or temporary state.
If no: return "none" without calling any memory tool.
```

### Priority Agent 现状

我们有 `sync_turn_llm_background` (`src/memory/extraction.rs:70-173`)，但它是"纯提取"模式——LLE 直接吐 JSON，不会跑完整的 agent loop（无法用 memory_save 工具、无法做质量门控）。

我们需要的是：fork 一个受限的 agent loop，让它用 memory 工具自己决定写什么。

### 实现路径

**新文件**：`src/memory/background_review.rs`

**修改的文件**：

| 文件 | 改动 |
|------|------|
| `src/memory/background_review.rs` | 新建，实现 `spawn_background_review` |
| `src/memory/manager/mod.rs` | 加 `background_review_active: bool` 防重入 |
| `src/engine/conversation_loop/session_processor.rs` | nudge 触发时调用 background_review |

**具体步骤**：

1. **创建 `background_review.rs`**:

```rust
//! 后台审查：fork 受限 agent loop 审查对话，自动保存记忆。

use crate::engine::query_engine::QueryEngine;
use crate::memory::MemoryManager;
use crate::services::api::{LlmProvider, Message};
use std::sync::{Arc, Mutex};

const BACKGROUND_REVIEW_PROMPT: &str = r#"Review the conversation between a user and an AI coding assistant.

1. Has the user revealed personal details worth remembering (name, role, preferences, coding style)?
2. Has the user expressed expectations about agent behavior that should carry forward?
3. Have you identified project conventions, environment facts, or tool quirks that will be useful later?

If yes, call memory_save with the appropriate content.
Do NOT save task progress, arbitrary code excerpts, or temporary state.
If there is nothing worth remembering, return exactly "NONE" without calling any tool."#;

pub async fn run_background_review(
    messages: &[Message],
    memory_manager: Arc<Mutex<MemoryManager>>,
    provider: Arc<dyn LlmProvider>,
    model: String,
) {
    // 防重入检查
    {
        let mut mem = memory_manager.lock().unwrap();
        if mem.background_review_active {
            return;
        }
        mem.background_review_active = true;
    }

    // 收集最近 N 轮对话作为输入
    let context = build_review_context(messages, 20);

    // 用单轮 LLM 调用（不需要完整 agent loop，保持简单）
    let request = ChatRequest::new(&model)
        .with_messages(vec![
            Message::system(BACKGROUND_REVIEW_PROMPT),
            Message::user(&context),
        ]);

    let result = match tokio::time::timeout(Duration::from_secs(30), provider.chat(request)).await {
        Ok(Ok(response)) => response.content,
        _ => {
            // 失败：重置 active flag
            let mut mem = memory_manager.lock().unwrap();
            mem.background_review_active = false;
            return;
        }
    };

    let text = result.trim();
    if text.eq_ignore_ascii_case("NONE") || text.is_empty() {
        let mut mem = memory_manager.lock().unwrap();
        mem.background_review_active = false;
        return;
    }

    // 解析响应中的 memory 操作（LLM 可能用 memory_save 工具，也可能直接描述）
    // 如果有明确的记忆内容，走现有的 submit_candidate_with_provider_notifications 流程
    let candidates = parse_review_response(&text);
    if !candidates.is_empty() {
        let mut mem = memory_manager.lock().unwrap();
        for candidate in candidates {
            mem.submit_candidate_with_provider_notifications(
                candidate,
                MemoryWriteTarget::Auto,
            ).await;
        }
    }

    let mut mem = memory_manager.lock().unwrap();
    mem.background_review_active = false;
}

fn build_review_context(messages: &[Message], max_turns: usize) -> String {
    let mut context = String::new();
    let mut turn_count = 0;
    for msg in messages.iter().rev() {
        match msg {
            Message::User { content } => {
                if turn_count >= max_turns { break; }
                context = format!("User: {}\n{}", content, context);
            }
            Message::Assistant { content, tool_calls, .. } => {
                if let Some(ref tc) = tool_calls {
                    if !tc.is_empty() {
                        context = format!(
                            "Assistant (used tools: {}): {}\n{}",
                            tc.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", "),
                            content.as_deref().unwrap_or(""),
                            context,
                        );
                        continue;
                    }
                }
                context = format!("Assistant: {}\n{}", content.as_deref().unwrap_or(""), context);
                turn_count += 1;
            }
            _ => {}
        }
    }
    context
}
```

2. **在 `MemoryManager` 中加防重入标志** (`manager/mod.rs`):

```rust
pub(super) background_review_active: bool,  // 加在结构体字段
// 初始化: background_review_active: false,
```

3. **在 session_processor.rs 中接入**:

```rust
// nudge 触发时
if nudge_triggered {
    let mem_clone = Arc::clone(&memory_manager);
    let provider_clone = // ...获取当前 provider
    let model_clone = // ...获取当前 model
    tokio::spawn(async move {
        background_review::run_background_review(
            &messages_snapshot,
            mem_clone,
            provider_clone,
            model_clone,
        ).await;
    });
}
```

**风险**：中等。fork 调用了新的 LLM request，有 cost。用 tokio::spawn 异步执行，不阻塞主循环。

**ROI**：很高。当前 background LLM 提取只做 JSON 解析，不跑质量门控。用 fork agent 后，记忆写入经过完整的 submit 管道。

---

## 3. 流式输出清理器

### Hermes 怎么做

`StreamingContextScrubber` (`agent/memory_manager.py:62-225`) 是一个跨 chunk 的状态机：

- 输入：每个流式 delta text chunk
- 输出：清理后的 visible text（移除了 `<memory-context>...</memory-context>` 区间）
- 状态：`_in_span`, `_buf`（挂起的部分 tag），`_at_block_boundary`
- 在 each `feed()` 中：检测 `<memory-context>` 开标签（只在新行开头），进入 span 状态，丢弃 span 内容直到遇到 `</memory-context>`
- 挂起检测：如果 chunk 末尾可能是"<memory-context>"的子串（如 `<mem`），挂起并在下一 chunk 继续判断

**为什么需要**：我们通过 `prepend_to_last_user_message` 把 `<relevant-memory>...</relevant-memory>` 注入到用户消息中。如果模型在流式输出中复述了这些内容，用户会看到记忆上下文泄漏到 UI。

### Priority Agent 现状

我们在 `src/engine/conversation_loop/request_preparation_controller.rs:619` 中注入：

```rust
prepend_to_last_user_message(request_messages, retrieval_block);
```

但 `retrieval_block` 的格式是：

```html
<relevant-memory>
<relevant-memory-instructions>...</relevant-memory-instructions>
[Relevant Memory]
- ...
</relevant-memory>
```

如果 LLM 在流式输出中复述了 `<relevant-memory>...</relevant-memory>` 这段内容，我们没有清理机制。

### 实现路径

**新文件**：`src/engine/streaming/context_scrubber.rs`

**修改的文件**：

| 文件 | 改动 |
|------|------|
| `src/engine/streaming/context_scrubber.rs` | 新建，实现 `ContextScrubber` 状态机 |
| `src/engine/streaming.rs` | 在 `emit_text_progressively` 中接入 scrubber |

**具体步骤**：

1. **创建 `context_scrubber.rs`**:

```rust
/// 流式输出清理器：跨 chunk 移除 <relevant-memory> 和 <memory-context> 区间。
///
/// LLM 有时会在输出中复述注入的记忆上下文块。这个状态机确保
/// 这些块中的内容永远不会泄漏到用户 UI。
pub struct ContextScrubber {
    in_span: bool,
    buf: String,
}

impl ContextScrubber {
    const TAGS: &'static [(&'static str, &'static str)] = &[
        ("<relevant-memory>", "</relevant-memory>"),
        ("<memory-context>", "</memory-context>"),
        ("<relevant-memory-instructions>", "</relevant-memory-instructions>"),
        ("<memory-context-instructions>", "</memory-context-instructions>"),
        ("<memory-instructions>", "</memory-instructions>"),
    ];

    pub fn new() -> Self { Self { in_span: false, buf: String::new() } }

    pub fn reset(&mut self) {
        self.in_span = false;
        self.buf.clear();
    }

    /// 喂入一个 chunk，返回可见部分
    pub fn feed(&mut self, text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }
        let mut input = self.buf.clone() + text;
        self.buf.clear();
        let mut out = String::new();

        while !input.is_empty() {
            if self.in_span {
                if let Some(idx) = Self::find_any_close_tag(&input) {
                    input = input[idx..].to_string();
                    self.in_span = false;
                } else {
                    // 还在 span 内，挂起可能的部分 close tag
                    self.buf = Self::hold_partial_suffix(&input);
                    return out;
                }
            } else {
                if let Some((open_tag, idx)) = Self::find_boundary_open_tag(&input) {
                    out.push_str(&input[..idx]);
                    input = input[idx + open_tag.len()..].to_string();
                    self.in_span = true;
                } else {
                    let held = Self::hold_partial_suffix(&input);
                    if held > 0 {
                        out.push_str(&input[..input.len() - held]);
                        self.buf = input[input.len() - held..].to_string();
                    } else {
                        out.push_str(&input);
                    }
                    return out;
                }
            }
        }
        out
    }

    pub fn flush(&mut self) -> String {
        if self.in_span {
            self.in_span = false;
            self.buf.clear();
            return String::new();
        }
        let tail = std::mem::take(&mut self.buf);
        tail
    }

    fn find_boundary_open_tag(input: &str) -> Option<(&'static str, usize)> {
        for (open, _) in Self::TAGS {
            if let Some(idx) = input.find(open) {
                // 必须在新行的开头（或整个文本的开头）
                let prefix = &input[..idx];
                if prefix.is_empty() || prefix.ends_with('\n') {
                    return Some((open, idx));
                }
            }
        }
        None
    }

    fn find_any_close_tag(input: &str) -> Option<usize> {
        let lower = input.to_lowercase();
        for (_, close) in Self::TAGS {
            if let Some(idx) = lower.find(close) {
                return Some(idx + close.len());
            }
        }
        None
    }

    fn hold_partial_suffix(input: &str) -> usize {
        let lower = input.to_lowercase();
        for (open, _) in Self::TAGS {
            for i in (1..open.len()).rev() {
                if lower.ends_with(&open[..i]) {
                    return i;
                }
            }
        }
        0
    }
}
```

2. **在 `streaming.rs` 的 `emit_text_progressively` 中接入**:

```rust
pub async fn emit_text_progressively(
    tx: &mpsc::Sender<StreamEvent>,
    text: String,
) {
    let mut scrubber = ContextScrubber::new();
    let visible = scrubber.feed(&text);
    if !visible.is_empty() {
        let _ = tx.send(StreamEvent::TextChunk(visible)).await;
    }
    let tail = scrubber.flush();
    if !tail.is_empty() {
        let _ = tx.send(StreamEvent::TextChunk(tail)).await;
    }
}
```

**更精确的接入点**：查看 `turn_assistant_response_controller.rs` 中如何发送流式文本——在 `StreamEvent::TextChunk` 发送前调用 scrubber。

**风险**：低。只是添加过滤，不改变逻辑。需要确保在 TUI/CLI 模式下 scrubber 也被正确初始化。

**ROI**：中。对生产环境重要，但在开发阶段很少遇到泄漏。属于防御性功能。

---

## 4. Dialogic 多轮推理 Prefetch

### Hermes 怎么做

Honcho provider 的 dialectic 推理 (`plugins/memory/honcho/__init__.py:949-989`):

```python
def _run_dialectic_depth(self, query: str) -> str:
    results = []
    for pass_idx in range(self._dialectic_depth):  # 1-3 passes
        if pass_idx == 0:
            # Cold: "Who is this person? What are their preferences, goals, and working style?"
            prompt = "First ask: what do I know about this user?"
        elif pass_idx == 1:
            # Self-audit: "What gaps remain?"
            if signal_sufficient(results[-1]):
                break  # bail early
            prompt = "What gaps remain in your understanding?"
        else:
            # Reconciliation: "Do these assessments cohere?"
            prompt = "Reconcile any contradictions and synthesize"
        
        reasoning_level = _resolve_pass_level(pass_idx, query)
        result = dialectic_query(session_key, prompt, level=reasoning_level)
        results.append(result)
    return results[-1]
```

核心思路：不是一次 prefetch 就完事，而是 1-3 个连续的 LLM 调用逐步深化理解：
- Pass 0: 初始评估
- Pass 1: 自审"还有什么遗漏？"（如果 Pass 0 已经足够则跳过）
- Pass 2: 调和矛盾

信号充足判断：response > 300 字符 或 > 100 字符且有结构化内容，则跳过后续 pass。

### Priority Agent 现状

我们的 `prefetch_with_llm_rerank` (`src/memory/retrieval.rs:104-124`) 用单次 LLM 调用来 rerank 候选记忆，没有多轮推理。

### 实现路径

**新文件**：不需要

**修改的文件**：

| 文件 | 改动 |
|------|------|
| `src/memory/retrieval.rs` | 在 `prefetch_retrieval_context_with_llm_rerank` 中加 dialectic depth 参数 |

**具体步骤**：

1. **在 `Manager` 中加配置** (`manager/mod.rs`):

```rust
dialectic_depth: usize,  // 1-3, default 1
// 从 env 读取 PRIORITY_AGENT_MEMORY_DIALECTIC_DEPTH
```

2. **修改 `prefetch_retrieval_context_with_llm_rerank`** (`retrieval.rs`):

```rust
pub async fn prefetch_retrieval_context_dialectic(
    &mut self,
    user_message: &str,
    provider: &dyn LlmProvider,
    model: &str,
    policy: RetrievalPolicy,
    depth: usize,  // 1-3
) -> Option<RetrievalContext> {
    let mut accumulated_context = String::new();
    let ranks = self.dialectic_rank_memories(user_message, provider, model, depth).await;
    
    // 多轮推理的结果合并成 enhanced retrieval context
    let matches = self.merge_dialectic_results(ranks);
    // ... 同样的 from_memory_matches_with_budget 流程
}
```

3. **Dialectic prompt 构建** (`retrieval.rs` 私有函数):

```rust
const DIALECTIC_PASS_0_PROMPT: &str = r#"Given the user's request and available memory context, identify which memories are most relevant. Focus on concrete facts that directly inform the current task."#;

const DIALECTIC_PASS_1_PROMPT: &str = r#"Review your initial selection. What gaps remain? Are there memories outside your first picks that might also be relevant? Consider cross-references between memories."#;

const DIALECTIC_PASS_2_PROMPT: &str = r#"Do these memory selections cohere? Reconcile any apparent contradictions and produce a final, concise set of the most actionable memories."#;

fn build_dialectic_prompt(pass: usize, prior_results: &str) -> &'static str {
    match pass {
        0 => DIALECTIC_PASS_0_PROMPT,
        1 => DIALECTIC_PASS_1_PROMPT,
        _ => DIALECTIC_PASS_2_PROMPT,
    }
}

fn dialectic_signal_sufficient(result: &str) -> bool {
    let len = result.chars().count();
    if len > 300 { return true; }
    if len > 100 && (result.contains("##") || result.contains("•") || result.contains("- ")) { return true; }
    false
}
```

**风险**：中等。每轮 prefetch 可能发起 1-3 次额外 LLM 调用，有 cost。建议 depth 默认为 1（相当于当前行为），只在需要 deep reasoning 时用环境变量开启更多 pass。

**ROI**：中。对 Memory/Project 等需要深度理解上下文的 policy 有帮助，但对简单的 Light policy 没必要。建议只在 policy 为 Memory 且 depth > 1 时启用。

---

## 5. 矛盾主动检测

### Hermes 怎么做

Holographic provider 的 `contradict()` 函数 (`plugins/memory/holographic/retrieval.py:338-442`):

**算法**:
1. 加载所有有 HRR 向量的 facts
2. 为每个 fact 构建 entity set（人、项目、概念等实体）
3. O(n²) 比较所有 fact pair（上限 500 个）
4. 对每对计算：
   - `entity_overlap = Jaccard(A_entities, B_entities)` — 实体重叠越高，越可能是同一主题
   - `content_similarity = cosine(hrr_vector_A, hrr_vector_B)` — 内容越不相似，越矛盾
   - `contradiction_score = entity_overlap × (1.0 - normalized_content_similarity)`
5. 返回排序后的矛盾 fact pair 列表

**关键代码**:

```python
contradiction_score = entity_overlap * (1.0 - (content_sim + 1.0) / 2.0)
```

- 高 entity_overlap (≥ 0.3) + 低 content_similarity = 潜在矛盾
- 阈值 0.3 过滤噪声

### Priority Agent 现状

我们有 `memory_conflicts` (`src/memory/manager/mod.rs`) 可以检测 key-value 冲突：

```rust
pub fn memory_conflicts(&self, limit: usize) -> Vec<String> {
    let mut keys: HashMap<String, HashSet<String>> = HashMap::new();
    // ... 解析记忆中的 key: value 对，返回有冲突的 key
}
```

但这是基于 `key: value` 格式的简单检测，不做语义比较。我们的 typed records (`records.jsonl`) 有更丰富的结构化数据，但没做主动矛盾检测。

### 实现路径

**新文件**：`src/memory/contradiction.rs`

**修改的文件**：

| 文件 | 改动 |
|------|------|
| `src/memory/contradiction.rs` | 新建，实现 `detect_contradictions` |
| `src/memory/manager/mod.rs` | 暴露 `contradictions()` 方法 |
| `src/memory/persistence.rs` | 在 `maintain_memory` 中加入矛盾检测 |

**具体步骤**：

1. **创建 `contradiction.rs`**:

```rust
use crate::memory::types::{MemoryRecord, MemoryKind};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct ContradictionPair {
    pub record_a: String,  // record id
    pub record_b: String,
    pub keywords_a: Vec<String>,
    pub keywords_b: Vec<String>,
    pub shared_keywords: Vec<String>,
    pub keyword_overlap: f32,
    pub content_similarity: f32,
    pub contradiction_score: f32,
}

pub fn detect_contradictions(
    records: &[MemoryRecord],
    threshold: f32,    // default 0.3
    limit: usize,      // default 10
) -> Vec<ContradictionPair> {
    let candidates: Vec<&MemoryRecord> = records
        .iter()
        .filter(|r| r.status == MemoryStatus::Accepted)
        .filter(|r| matches!(r.kind, MemoryKind::ProjectFact | MemoryKind::Decision | MemoryKind::WorkflowConvention))
        .take(500)  // 防止 O(n²)
        .collect();

    let keywords_per_record: Vec<(Vec<String>, &MemoryRecord)> = candidates
        .iter()
        .map(|r| (extract_keywords(&r.content), *r))
        .collect();

    let mut pairs = Vec::new();
    for i in 0..keywords_per_record.len() {
        for j in (i + 1)..keywords_per_record.len() {
            let (kw_a, rec_a) = &keywords_per_record[i];
            let (kw_b, rec_b) = &keywords_per_record[j];

            let set_a: HashSet<&String> = kw_a.iter().collect();
            let set_b: HashSet<&String> = kw_b.iter().collect();
            let shared: Vec<String> = set_a.intersection(&set_b).map(|s| s.to_string()).collect();

            let keyword_overlap = if set_a.is_empty() && set_b.is_empty() { 0.0 }
            else { shared.len() as f32 / (set_a.union(&set_b).count() as f32) };

            if keyword_overlap < 0.3 {
                continue;  // 同一主题的可能性不高
            }

            // 内容相似度：基于词频的简单余弦相似度
            let content_sim = word_overlap_similarity(&rec_a.content, &rec_b.content);
            
            // 矛盾分数：关键词重叠高 + 内容不同 = 潜在矛盾
            let contradiction_score = keyword_overlap * (1.0 - content_sim);

            if contradiction_score >= threshold {
                pairs.push(ContradictionPair {
                    record_a: rec_a.id.clone(),
                    record_b: rec_b.id.clone(),
                    keywords_a: kw_a.clone(),
                    keywords_b: kw_b.clone(),
                    shared_keywords: shared,
                    keyword_overlap,
                    content_similarity: content_sim,
                    contradiction_score,
                });
            }
        }
    }

    pairs.sort_by(|a, b| b.contradiction_score.partial_cmp(&a.contradiction_score).unwrap_or(std::cmp::Ordering::Equal));
    pairs.truncate(limit);
    pairs
}
```

2. **在 MemoryManager 中暴露** (`manager/mod.rs`):

```rust
pub fn contradictions(&self, threshold: f32, limit: usize) -> Vec<ContradictionPair> {
    let records = self.memory_records();
    contradiction::detect_contradictions(&records, threshold, limit)
}
```

3. **在 maintenance 中接入** (`persistence.rs`):

```rust
// maintain_memory 中加入:
let ctx = self.contradictions(0.3, 10);
if !ctx.is_empty() {
    debug!("Detected {} potential memory contradictions", ctx.len());
    for pair in &ctx {
        debug!(
            "Contradiction: {} vs {} (shared: {:?}, score: {:.2})",
            pair.record_a, pair.record_b, pair.shared_keywords, pair.contradiction_score
        );
    }
    // 将来可以在这里通过 provider_registry 通知外部 provider
}
```

**注意**：我们没有 HRR 向量（这是 Holographic 的特点）。我们用 keyword overlap + word-overlap similarity 做近似，效果会弱一些但足以检测明显的矛盾。

**风险**：低。只是添加检测，不修改数据。O(n²) 但有 500 上限。

**ROI**：中。当记忆库增长后，矛盾检测能主动发现需要清理的记忆。但在记忆量较小（<100 条）时不太触发。

---

## 实施优先级和分阶段

| 阶段 | 功能 | 理由 |
|------|------|------|
| **Phase 1** | Nudge + Background Review Fork | ROI 最高，直接提高记忆覆盖率 |
| **Phase 2** | Streaming Scrubber | 简单，防御性 |
| **Phase 3** | Contradiction Detection | 独立模块，不影响主流程 |
| **Phase 4** | Dialogic Multi-Pass | 成本最高，需验证效果 |

### Phase 1 预计改动量

- `src/memory/manager/mod.rs`: +30 行
- `src/memory/background_review.rs`: 新建 ~120 行
- `src/engine/conversation_loop/session_processor.rs`: +15 行
- `src/engine/conversation_loop/tool_execution_controller.rs`: +5 行
- `src/memory/manager/helpers.rs`: +1 行

总计约 **170 行新增代码**，不涉及协议变更。

### 验证方式

- Nudge: 手动测试——连续 10 轮不调 memory 工具，确认后台审查触发
- Background Review: 检查 `background_review_active` flag 防重入
- Scrubber: 单元测试跨越 chunk 边界的 `<relevant-memory>` 标签
- Contradiction: 单元测试用构造的 conflict pair 验证检测逻辑
- Dialectic: 对比单 pass vs 多 pass 的 prefetch 结果质量
