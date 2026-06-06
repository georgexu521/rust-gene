# Closeout Hang 问题分析

## 问题现象

Agent 完成 eval 任务后，进程在 `closeout` 事件发出后仍占用 **100% CPU** 持续 **2~5 分钟** 才退出。

### 典型数据

| 任务 | closeout 前耗时 | closeout 后空转 | 总耗时 | 浪费比例 |
|------|----------------|----------------|--------|----------|
| `fullstack-auth-app` | ~90s | **~260s** | ~350s | **74%** |
| `rust-cli-scaffold` | ~250s | 0s | ~250s | 0% (未触发 closeout) |
| `data-pipeline-with-db` | ~300s | 0s | ~313s | 0% |

**关键观察**：只有触发 `closeout` 事件的 Rust 任务会 hang，Python 任务正常。`rust-cli-scaffold` 某次运行未触发 closeout 也未 hang。

---

## 根因分析

### 代码位置

`src/engine/streaming.rs:947` — `StreamingQueryEngine::query_stream` 方法：

```rust
tokio::spawn(async move {
    // 1~5. 执行 LLM 查询循环，发送 StreamEvent 到 tx
    // ...
    
    // 6. 自动 flush 记忆（每次查询结束后）
    if let Some(ref mem_mutex) = engine.memory_manager {
        let session_id = engine.session_id.clone().unwrap();
        let flush_history = history.lock().await.clone();
        let mut mem = mem_mutex.lock().await;
        mem.flush_session_with_reason_async(
            session_id,
            MemoryFlushReason::SessionEnd,
            &flush_history,
        ).await;  // ← 这里阻塞了 260 秒
    }
});
```

### 为什么 main.rs "看不到" 这个阻塞

`main.rs:run_eval_task` 的消费逻辑：

```rust
while let Some(event) = stream.next().await {
    match event {
        StreamEvent::Complete => {
            write_eval_event(...)?;
            break;  // 退出循环
        }
        // ...
    }
}
// 这里会等待 trace_store
let latest_trace = components.streaming_engine.trace_store().latest();
// ...
```

**问题**：`break` 只是退出了 `while` 循环，`run_eval_task` 函数返回后，`#[tokio::main]` 会继续等待所有 spawned tasks 完成。`tokio::spawn` 的 memory flush task 还在 100% CPU 运转，所以进程不退出。

### 为什么 100% CPU

`flush_session_with_reason_async` → `flush_session_async_unchecked` → `extract_session_learnings` 做字符串处理和遍历。如果消息历史很长（eval 任务通常有几十条消息），这个同步CPU操作在异步任务中就会表现为 100% CPU。

更深层可能：`BackgroundMemoryReviewWorker::review_execution_report` 或 `proposal_store.upsert` 触发了繁重的文件 I/O（读写 `~/.priority-agent/memory_proposals.jsonl`）。

---

## 为什么之前没发现

1. **正常 CLI 模式**：用户在 `Complete` 事件后继续交互，下一个用户消息到来前，memory flush 在后台完成，用户感知不到延迟
2. **测试模式**：单元测试不测试 eval-run 完整生命周期
3. **Rust 任务的特殊性**：只有涉及 code-change workflow + closeout + memory_generate_enabled 的 eval 任务才会触发这段逻辑。Python 任务的 closeout 路径不同（没有 cargo 编译验证链）

---

## 修复方案对比

### 方案 A：Memory Flush 加 Timeout（最小侵入性）

在 `streaming.rs` 的 memory flush 处加超时：

```rust
tokio::time::timeout(
    Duration::from_secs(5),
    mem.flush_session_with_reason_async(...)
).await.ok();  // 超时忽略，不阻塞进程退出
```

**优点**：一行代码修复，不影响正常 CLI 交互体验  
**缺点**：memory flush 可能未完成就被中断，导致学习记录丢失。非根本解。

---

### 方案 B：Eval-Run 模式立即退出（对 eval 最实用）

在 `main.rs` 的 EvalRun 分支：

```rust
StartupMode::EvalRun => {
    let components = init_app_or_exit(&working_dir).await;
    match run_eval_task(&args, &components).await {
        Ok(()) => {
            // eval-run 成功，立即退出，不等待 background memory flush
            std::process::exit(0);
        }
        Err(e) => {
            std::process::exit(1);
        }
    }
}
```

**优点**：eval-run 不再被 background task 阻塞，测试时间缩短 70%+  
**缺点**：只修复 eval-run，CLI 模式的 memory flush 仍然可能卡（虽然用户感知不到）

---

### 方案 C：Memory Flush 异步解耦（架构最干净）

把 memory flush 从主查询 spawned task 中拆出来：

```rust
tokio::spawn(async move {
    // 主查询任务：只负责 LLM 循环和发送事件
    // 完成后立即退出
});

// memory flush 放在独立的 background task，不阻塞任何流程
if let Some(ref mem_mutex) = engine.memory_manager {
    if let Some(session_id) = engine.session_id.clone() {
        let mem = mem_mutex.clone();
        let history = history.lock().await.clone();
        tokio::spawn(async move {
            let _ = tokio::time::timeout(
                Duration::from_secs(30),
                async {
                    let mut mem = mem.lock().await;
                    mem.flush_session_with_reason_async(
                        session_id,
                        MemoryFlushReason::SessionEnd,
                        &history,
                    ).await;
                }
            ).await;
        });
    }
}
```

**优点**：主查询任务和 memory flush 完全解耦，互不阻塞  
**缺点**：需要重构 `query_stream` 的任务边界，改动范围中等。需要确认 memory flush 失败不影响用户体验。

---

### 方案 D：优化 Memory Flush 性能（最深层的根本解）

`flush_session_async_unchecked` 中的 `extract_session_learnings` 是同步 CPU 密集型操作：

```rust
pub(super) fn extract_session_learnings(messages: &[Message]) -> Vec<String> {
    // 遍历所有消息，拼接字符串，HashMap 统计工具使用频率
    // 对于 50+ 条消息的 eval session，这里可能耗时数秒
}
```

优化方向：
1. 用 `tokio::task::spawn_blocking` 把同步操作放到阻塞线程池
2. 限制消息历史长度（eval 模式不需要完整历史）
3. 减少重复的文件 I/O（batch write memory_proposals.jsonl）

**优点**：从性能根源解决问题  
**缺点**：改动最大，需要 profiling 确认瓶颈具体在哪里

---

## 建议的优先级

**短期（今天）**：实施方案 B（eval-run 立即退出），让 eval 运行时间恢复正常。

**中期（本周）**：实施方案 A（加 timeout）作为 safety net，防止任何模式下 flush 无限阻塞。

**长期（下周）**：评估方案 C 或 D，彻底重构 memory flush 的生命周期管理。

---

## 验证方法

修复后验证步骤：

```bash
# 1. 运行 tier-4 Rust 任务
./scripts/eval-run.sh tier-4

# 2. 检查 closeout 到退出的时间差
tail -20 /tmp/eval-reports/fullstack-auth-app-*.jsonl | grep -E "closeout|complete"

# 3. 期望结果：closeout 和 complete 之间 < 5 秒
```

---

## 附录：事件序列对比

### 正常（Python db_pipeline）
```
tool_execution_complete → closeout → text_chunk → runtime_diagnostic → complete → trace_summary
(总耗时 313s，closeout 后 13s 内完成)
```

### 异常（Rust auth_app）
```
tool_execution_complete → closeout → [260s 空转，100% CPU] → text_chunk → runtime_diagnostic → complete → trace_summary
(总耗时 350s，closeout 后 260s 才继续)
```

### 正常（Rust cli_tool，未触发 closeout）
```
tool_execution_complete → [迭代继续] → complete → trace_summary
(总耗时 250s，无 closeout 事件)
```
