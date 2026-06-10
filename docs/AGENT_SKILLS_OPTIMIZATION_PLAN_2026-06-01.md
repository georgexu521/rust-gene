# Agent 通用技巧优化计划
Status: Active

> 基于 Reasonix (DeepSeek) 与 Priority Agent 的深度对比，聚焦于**模型无关**的通用 agent 技巧差距。
> 创建日期：2026-06-01

---

## 总览

| # | 技巧 | 难度 | 影响 | 预估行数 |
|---|---|---|---|---|
| 1 | 真正并行工具调度 | 中 | 高 | ~100 |
| 2 | Read-before-Edit 强制执行 | 低 | 高 | ~80 |
| 3 | 用户 Steer 自我修正 | 低 | 中 | ~60 |
| 4 | 强制摘要 Wrap-up | 低 | 中 | ~100 |
| 5 | Sub-agent 进度流 | 中 | 中 | ~150 |
| 6 | Event Sourcing 轻量版 | 中 | 高 | ~200 |

**总计**：~690 行新增/修改代码，6 个独立的 PR-worthy 改动。

---

## 优化 1：真正并行工具调度

### 现状

`execute_tools_parallel` 的名字有误导性。read-only 工具被收集到 `read_only_jobs` 中，但遇到第一个非并发安全的工具时，这批 jobs 会被 `collect_read_only_results` 通过 `buffer_unordered(concurrency)` 并发执行。然而：
- **写工具是顺序的** — 逐个 dispatch
- **batch boundary 导致读工具被分段执行** — 如果中间夹杂写工具，读工具被切成多个小 batch

Reasonix 的做法：收集所有 `parallelSafe` 工具 → `Promise.allSettled` → 一次性全部并发执行。

### 改进方案

**文件**：`src/engine/conversation_loop/tool_execution_controller.rs`

**改动 1**（L1704）：将 `read_only_jobs` 从顺序 job vector 改为两阶段收集器：

```rust
// 阶段 1：扫描所有 tool_calls，分类为 parallel_safe 和 serial
let mut parallel_jobs: Vec<(usize, ReadOnlyJob)> = Vec::new();
let mut serial_jobs: Vec<(usize, &ToolCall)> = Vec::new();

for (i, tc) in tool_calls.iter().enumerate() {
    if tool_call_is_concurrency_safe(registry, &tc.name, &tc.arguments) {
        parallel_jobs.push((i, self.read_only_job(...)));
    } else {
        serial_jobs.push((i, tc));
    }
}

// 阶段 2：并行执行所有 parallel-safe 工具
if !parallel_jobs.is_empty() {
    let parallel_results = self.collect_read_only_results(
        parallel_jobs, concurrency, tx, lifecycle
    ).await;
    results.extend(parallel_results);
}

// 阶段 3：顺序执行 serial 工具
for (i, tc) in serial_jobs {
    let result = self.execute_single_tool(tc, ...).await;
    results.push((i, (tc.clone(), result)));
}
```

**关键变化**：
- 不再需要中间 flush 逻辑（L1751-1757、L1778-1784、L1804-1810），简化 ~30 行
- `collect_read_only_results` 保持不变（已经用 `buffer_unordered` 实现并发）

### 验证

```bash
cargo test engine::conversation_loop::tool_execution_controller
```

---

## 优化 2：Read-before-Edit 强制执行

### 现状

`FileStateTracker`（`src/tools/file_tool/mod.rs:330-438`）**已经有完整的 read tracking 基础设施**：
- `mark_read_coverage()` — 标记文件被读过
- `is_file_read()` — 检查是否读过
- `read_before_edit_status()` — 检查编辑范围是否与已读行重叠
- `FILE_STATE_TRACKER` 全局单例

但 `FileWriteTool` 和 `FileEditTool` 在 execute 时**没有强制检查**这个状态。模型可以"盲写"未读过的文件。

### 改进方案

**文件**：`src/tools/file_tool/mod.rs`

**改动 1**：在 `FileEditTool::execute()`（~L2188）和 `FileWriteTool::execute()`（~L1270）的开头，添加 read-before-edit 检查：

```rust
// FileEditTool::execute 中，gate.evaluate() 之后、实际编辑之前
let session_id = context.session_id.as_deref().unwrap_or("unknown");
let status = FileStateTracker::read_before_edit_status(
    session_id,
    &resolved_path,
    Some(start_line),
    Some(end_line),
);

match status {
    ReadBeforeEditStatus::NotRead => {
        return ToolResult::error(format!(
            "File '{}' has not been read yet. Read the file first before editing. \
             Use file_read to load its contents, then retry the edit.",
            resolved_path.display()
        ));
    }
    ReadBeforeEditStatus::PartialOnly { .. } => {
        // Allow but warn — the edit range partially overlaps read lines
        // (当前已有此行为，保持不变)
    }
    ReadBeforeEditStatus::Allowed => {
        // OK — proceed
    }
}
```

**改动 2**：在 `FileWriteTool::execute()` 中同样检查（写整个文件必须有 read）：

```rust
if !FileStateTracker::is_file_read(session_id, &resolved_path) {
    return ToolResult::error(format!(
        "File '{}' has not been read yet. Read it first with file_read, \
         then retry the write.",
        resolved_path.display()
    ));
}
```

**改动 3**：添加 `PRIORITY_AGENT_READ_BEFORE_EDIT` 环境变量开关（默认 `"1"`），允许 opt-out：

```rust
fn read_before_edit_enabled() -> bool {
    std::env::var("PRIORITY_AGENT_READ_BEFORE_EDIT")
        .unwrap_or_else(|_| "1".to_string())
        .trim()
        != "0"
}
```

### 验证

```bash
cargo test file_tool
```

---

## 优化 3：用户 Steer 自我修正

### 现状

当用户在模型输出中途打断并说"不对，用 snake_case 不是 camelCase"时：
1. 当前 assistant 消息（含错误的工具调用和回复）**留在消息历史中**
2. 用户纠正作为新的 user 消息追加
3. 新的一轮开始，模型看到：错误回复 + 用户纠正 + 新指令

这浪费 token 且可能误导模型。

### 改进方案

**文件**：`src/engine/conversation_loop/mod.rs` 或新文件 `self_correction.rs`

**改动 1**：在 `ConversationLoop` 中添加方法：

```rust
/// 替换最后一条 assistant 消息（用户 steer 纠正时调用）
pub fn replace_last_assistant_message(&mut self, new_content: &str) {
    // 找到最后一条 role=assistant 的消息，替换其 content
    if let Some(last) = self.messages.iter_mut().rev()
        .find(|m| matches!(m, Message::Assistant { .. }))
    {
        if let Message::Assistant { content, .. } = last {
            let replacement = format!(
                "[User corrected the previous response. The correct approach:]\n{}",
                new_content
            );
            *content = replacement;
        }
    }
}
```

**改动 2**：在 `TurnIterationController::run()` 中调用。当检测到用户中断 + 纠正时（`is_drift_interruption_signal` 返回 true，或 `StopCheckReason::UserInterrupted`），调用 `replace_last_assistant_message`。

```rust
// turn_iteration_controller.rs ~L80，在处理 user steer 时
if is_drift_interruption_signal(&user_message) {
    self.replace_last_assistant_message(&user_message);
    continue; // 跳过这一轮，让模型重新处理
}
```

**改动 3**：环境变量 `PRIORITY_AGENT_SELF_CORRECTION` 控制（默认 `"1"`）。

### 验证

```bash
cargo test conversation_loop::self_correction
```

---

## 优化 4：强制摘要 Wrap-up

### 现状

当 token 预算耗尽或达到迭代上限时：
- `context_compressor` 压缩消息历史（snip/LLM summarize）
- 如果压缩后仍超预算或迭代次数用完 → **直接报错或返回不完整结果**

没有"强制收尾"机制——给模型一次机会做最终输出。

### 改进方案

**文件**：新建 `src/engine/conversation_loop/force_summary.rs`

**改动 1**：实现 `ForceSummaryController`：

```rust
pub struct ForceSummaryController;

impl ForceSummaryController {
    /// 检查是否应该触发强制摘要
    pub fn should_force_summary(
        iteration: usize,
        max_iterations: usize,
        token_usage_ratio: f64,
    ) -> bool {
        iteration >= max_iterations.saturating_sub(2)  // 最后 2 轮
            || token_usage_ratio >= 0.80                // 或 token 超 80%
    }

    /// 生成强制摘要指令
    pub fn force_summary_instruction() -> String {
        r#"<wrap-up>
You are approaching the context or iteration limit. Do NOT start any new
multi-step task. Instead:
1. Summarize what has been accomplished so far.
2. List any remaining work that the user should handle.
3. If there are pending changes, commit them or describe them clearly.
4. End your response after this summary — do not call any more tools.
</wrap-up>"#.to_string()
    }
}
```

**改动 2**：在 `TurnIterationLoopController::run()` 中注入：

```rust
// turn_iteration_loop_controller.rs ~L40，在每次迭代开始前
if ForceSummaryController::should_force_summary(
    iteration,
    max_loop_iterations,
    token_usage_ratio,
) {
    let instruction = ForceSummaryController::force_summary_instruction();
    messages.push(Message::system(instruction));
    // 标记这是最后一轮
    break_after_this_turn = true;
}
```

### 验证

```bash
cargo test conversation_loop::force_summary
```

---

## 优化 5：Sub-agent 进度流

### 现状

`AgentManager::wait_for_result`（`src/agent/manager.rs:525`）通过 oneshot channel 等待最终结果。发起方**看不到子 agent 的中间进度**——只有状态转换（Pending → Running → Completed）。

Reasonix 的子 agent 发出 `start` → `progress` → `phase` → `end` 事件流，TUI 可以渲染实时指示器。

### 改进方案

**文件**：`src/agent/manager.rs`、新建 `src/agent/progress.rs`

**改动 1**：定义进度事件类型：

```rust
// src/agent/progress.rs
#[derive(Debug, Clone)]
pub enum AgentProgressEvent {
    Started { agent_id: String, task: String },
    Phase { agent_id: String, phase: String },       // e.g., "exploring", "summarizing"
    ToolCall { agent_id: String, tool: String, args_summary: String },
    ToolResult { agent_id: String, tool: String, result_summary: String },
    TextChunk { agent_id: String, text: String },     // 流式文本（节流）
    Completed { agent_id: String, result: AgentResult },
    Failed { agent_id: String, error: String },
}
```

**改动 2**：在 `AgentManager` 中注册进度 channel：

```rust
// manager.rs — 在 AgentManager struct 中添加
progress_senders: RwLock<HashMap<AgentId, broadcast::Sender<AgentProgressEvent>>>,

// spawn 时创建 channel
pub async fn spawn_with_progress(
    &self,
    config: AgentConfig,
) -> anyhow::Result<(AgentHandle, broadcast::Receiver<AgentProgressEvent>)> {
    let (tx, rx) = broadcast::channel(64);
    self.progress_senders.write().await.insert(config.id.clone(), tx);
    let handle = self.spawn(config).await?;
    Ok((handle, rx))
}
```

**改动 3**：在 Agent 执行循环中发送进度事件。`agent.rs` 的工具执行后、文本生成时发送事件。

```rust
// agent.rs — 在工具执行完成后
if let Some(tx) = progress_sender {
    let _ = tx.send(AgentProgressEvent::ToolCall {
        agent_id: self.config.id.clone(),
        tool: tool_name,
        args_summary: summarize_args(&args),
    });
}
```

### 验证

```bash
cargo test agent::progress
```

---

## 优化 6：Event Sourcing 轻量版

### 现状

Priority Agent 使用可变状态 + SQLite 持久化。Trace（`TurnTrace`）记录关键事件但**不是状态的真实来源**——它是附加的元数据。

Event Sourcing 的完整实现是重大架构变更。但可以在**不改变现有架构**的前提下，增强 Trace 使其**足够详细以支持调试可重放**。

### 改进方案

**文件**：`src/engine/trace.rs`、新增 `src/engine/trace_replay.rs`

**改动 1**：扩展 `TraceEvent` 枚举以覆盖关键状态转换：

```rust
// trace.rs — 在现有 TraceEvent 枚举中添加
pub enum TraceEvent {
    // 现有变体保持不变...
    
    // 新增：关键状态快照
    MessagesSnapshot { messages: Vec<Message>, turn: usize },
    TokenBudgetSnapshot { used: usize, limit: usize, ratio: f64 },
    ToolCallDecision { call_id: String, decision: String, reason: String },
    CompressionEvent { level: String, before_tokens: usize, after_tokens: usize },
    PlanStepCompleted { step_id: String, outcome: String },
    
    // 现有变体...
}
```

**改动 2**：实现 `TraceReplay` 轻量回放：

```rust
// src/engine/trace_replay.rs
pub struct TraceReplay {
    events: Vec<TraceEvent>,
}

impl TraceReplay {
    pub fn from_trace(trace: &TurnTrace) -> Self {
        Self { events: trace.events().to_vec() }
    }

    /// 重建关键决策时间线
    pub fn decision_timeline(&self) -> Vec<DecisionPoint> {
        self.events.iter().filter_map(|e| match e {
            TraceEvent::ToolCallDecision { call_id, decision, reason } => {
                Some(DecisionPoint {
                    time: String::new(), // 可从 event timestamp 获取
                    summary: format!("[{}] {}: {}", decision, call_id, reason),
                })
            }
            TraceEvent::CompressionEvent { level, before_tokens, after_tokens } => {
                Some(DecisionPoint {
                    time: String::new(),
                    summary: format!("Compressed: {} → {} tokens ({})", before_tokens, after_tokens, level),
                })
            }
            _ => None,
        }).collect()
    }

    /// 导出为 JSON 用于外部分析
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(&self.events).unwrap_or_default()
    }
}
```

**改动 3**：在 `TurnRecording` 中记录新增的 TraceEvent。在工具决策点（`execute_tools_parallel`）、压缩点（`context_compressor`）、plan 步骤完成点（`plan_mode`）插入 `trace.record(TraceEvent::...)`。

### 验证

```bash
cargo test engine::trace_replay
```

---

## 实施顺序

按依赖关系和风险排序：

```
Phase A（本周，低风险，每个独立）
  ├── 优化 2：Read-before-Edit（~80 行，已有基础设施）
  ├── 优化 3：用户 Steer 自我修正（~60 行，纯新增功能）
  └── 优化 4：强制摘要 Wrap-up（~100 行，纯新增功能）

Phase B（下周，中风险）
  ├── 优化 1：真正并行工具调度（~100 行，修改现有复杂逻辑）
  └── 优化 5：Sub-agent 进度流（~150 行，新增 channel 基础设施）

Phase C（长期）
  └── 优化 6：Event Sourcing 轻量版（~200 行，增强现有 trace 系统）
```

## 验证检查清单

每个优化完成后：

```bash
cargo fmt --check
cargo check -q
cargo clippy --all-features -- -D warnings
cargo test -q <touched_module>
bash scripts/test-fast-lane.sh
```

## 环境变量汇总

| 变量 | 默认值 | 说明 |
|---|---|---|
| `PRIORITY_AGENT_READ_BEFORE_EDIT` | `"1"` | 启用 read-before-edit 强制检查 |
| `PRIORITY_AGENT_SELF_CORRECTION` | `"1"` | 启用用户 steer 自我修正 |
| `PRIORITY_AGENT_PARALLEL_MAX` | `"3"` | 并行工具最大并发数 |
| `PRIORITY_AGENT_GIT_ROLLBACK` | `"on"` | 启用 auto-git-rollback（已在 repair 模块实现） |
