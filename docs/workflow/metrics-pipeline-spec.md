# 指标观测体系规格（W7 交付物）

> 目标：为 WorkflowEngine 建立完整的观测、埋点、报告体系，每周可自动产出趋势报告。
> 状态：V1 Frozen
> 依赖：workflow-v1-design.md（指标字典）、weights.rs、questioning-spec.md、planner-executor-spec.md

---

## 1. 观测架构

```
WorkflowEngine 执行过程
        │
        ├── 状态转换事件 ──→ MetricsCollector
        ├── 工具调用事件 ──→      │
        ├── 失败/异常事件 ──→     │
        └── 用户交互事件 ──→     │
                              ▼
                    ┌─────────────────┐
                    │  MetricsStore   │ ← 内存 ring buffer + 可选磁盘
                    │  (线程安全)      │
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
        ┌──────────┐  ┌──────────┐  ┌──────────┐
        │ 实时面板  │  │ 周报生成  │  │ 回放文件  │
        │ (/doctor) │  │ (脚本)   │  │ (JSON)   │
        └──────────┘  └──────────┘  └──────────┘
```

---

## 2. 指标埋点位置

### 2.1 Workflow 生命周期埋点

| 埋点位置 | 事件类型 | 采集字段 |
|---------|---------|---------|
| `GATE_CHECK` 完成 | `gate_decision` | input_hash, decision, confidence, reason, latency_ms |
| `THINKING` 开始 | `thinking_start` | task_hash, mainline_goal, budget_config |
| `THINKING` 每轮 Q&A | `thinking_round` | round_index, question_type, token_cost, mainline_relevance |
| `THINKING` 结束 | `thinking_complete` | total_rounds, total_tokens, convergence_reason, duration_ms |
| `PLANNING` 完成 | `plan_generated` | step_count, recursion_depth, dependency_count, duration_ms |
| `WEIGHTING` 完成 | `weights_computed` | top3_scores, score_variance, duration_ms |
| `EXECUTING` 每步 | `step_executed` | step_index, tool, success, duration_ms, verification_result |
| `VERIFYING` 完成 | `step_verified` | step_index, checks_run, check_results, duration_ms |
| `REWEIGHT` 完成 | `weights_recalculated` | changed_steps, top_score_delta |
| 任意状态 → `FALLBACK_DIRECT` | `fallback_triggered` | from_state, reason, context_snapshot |

### 2.2 四硬指标埋点

```rust
/// 北极星指标事件
#[derive(Debug, Clone, Serialize)]
pub struct NorthStarEvent {
    pub session_id: String,
    pub task_hash: String,
    pub timestamp: DateTime<Utc>,
    pub mainline_hit: Option<bool>,        // 是否命中主线
    pub drift_interrupted: Option<bool>,   // 是否被打断
    pub first_plan_coverage: Option<f64>,  // 首轮覆盖率
    pub rework_count: usize,               // 返工次数
}
```

### 2.3 Gate 专项埋点

```rust
#[derive(Debug, Clone, Serialize)]
pub struct GateEvent {
    pub input_hash: String,
    pub fast_lane_matched: bool,
    pub heuristic_matched: bool,
    pub llm_classified: bool,
    pub final_decision: String,       // "direct" | "workflow"
    pub confidence: f64,
    pub latency_ms: u64,
    pub misclassified: Option<bool>,  // 事后标注
}
```

---

## 3. MetricsStore 实现

### 3.1 数据结构

```rust
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// 线程安全的指标存储
pub struct MetricsStore {
    events: Arc<Mutex<VecDeque<MetricEvent>>>,
    max_size: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum MetricEvent {
    Gate(GateEvent),
    Thinking(ThinkingEvent),
    Planning(PlanningEvent),
    Execution(ExecutionEvent),
    NorthStar(NorthStarEvent),
    Fallback(FallbackEvent),
}

impl MetricsStore {
    pub fn new(max_size: usize) -> Self {
        Self {
            events: Arc::new(Mutex::new(VecDeque::with_capacity(max_size))),
            max_size,
        }
    }

    pub fn record(&self, event: MetricEvent) {
        let mut events = self.events.lock().unwrap();
        if events.len() >= self.max_size {
            events.pop_front();
        }
        events.push_back(event);
    }

    pub fn query(&self, filter: MetricFilter) -> Vec<MetricEvent> {
        let events = self.events.lock().unwrap();
        events.iter()
            .filter(|e| filter.matches(e))
            .cloned()
            .collect()
    }

    pub fn stats(&self) -> MetricsStats {
        let events = self.events.lock().unwrap();
        MetricsStats {
            total_events: events.len(),
            by_type: count_by_type(&events),
            time_range: get_time_range(&events),
        }
    }
}
```

### 3.2 持久化（可选）

```rust
/// 导出到 JSON Lines 文件
pub fn export_to_jsonl(&self, path: &Path) -> Result<(), String> {
    let events = self.events.lock().unwrap();
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    
    let mut writer = std::io::BufWriter::new(file);
    for event in events.iter() {
        let line = serde_json::to_string(event)?;
        writeln!(writer, "{}", line)?;
    }
    Ok(())
}
```

---

## 4. 实时面板（/doctor 集成）

### 4.1 /doctor workflow 子命令

```
/doctor workflow

=== Workflow 指标 ===
本周任务数: 12
主线命中率: 64% (目标 >70%)
漂移打断率: 18% (目标 <15%) ⚠️
首轮计划覆盖率: 75% (目标 >80%)
平均返工: 1.2 次/任务

=== Gate 统计 ===
总判定: 45
Fast Lane: 28 (62%)
Heuristic: 10 (22%)
LLM Classify: 7 (16%)
误判: 6 (13%)

=== 思考效率 ===
平均问题数: 3.8 (预算 5)
平均 token 消耗: 2,100 (预算 3,750)
收敛原因:
  - executable_plan_formed: 8
  - uncertainties_low: 3
  - budget_exhausted: 1

=== 执行质量 ===
步骤成功率: 87%
失败重构: 2 次
人工确认: 0 次
```

### 4.2 实现接口

```rust
/// 集成到现有的 CostTracker
impl CostTracker {
    pub fn workflow_stats(&self) -> WorkflowStats {
        // 从 MetricsStore 查询并聚合
        let store = self.metrics_store.as_ref()?;
        
        WorkflowStats {
            total_tasks: count_tasks(store),
            mainline_hit_rate: compute_mainline_hit_rate(store),
            drift_interruption_rate: compute_drift_rate(store),
            first_plan_coverage: compute_coverage(store),
            avg_rework: compute_avg_rework(store),
        }
    }
}
```

---

## 5. 周报生成

### 5.1 周报脚本

```bash
#!/bin/bash
# scripts/workflow-weekly-report.sh

# 读取本周指标事件
# 计算四项硬指标趋势
# 生成 Markdown 报告
# 可选：发送到 Slack/保存到 docs/workflow/reports/
```

### 5.2 周报模板

```markdown
# WorkflowEngine 周报 — 第 N 周

## 四硬指标趋势

| 指标 | 本周 | 上周 | 变化 | 目标 |
|------|------|------|------|------|
| 主线命中率 | 64% | 58% | +6pp | >70% |
| 漂移打断率 | 18% | 22% | -4pp | <15% |
| 首轮计划覆盖率 | 75% | 70% | +5pp | >80% |
| 平均返工 | 1.2 | 1.5 | -0.3 | <0.8 |

## 本周关键发现

### 做得好的
- [具体发现和数据支撑]

### 需要改进的
- [具体发现和数据支撑]

### 行动项
- [ ] ...

## 原始数据

- 总任务数: N
- 总步骤数: N
- 总 token 消耗: N
- 平均思考轮数: N
```

---

## 6. 回放格式

### 6.1 回放文件结构

```json
{
  "version": "1.0",
  "session_id": "uuid",
  "task": "用户原始请求",
  "mainline_goal": "主线目标",
  "events": [
    { "type": "gate_decision", "timestamp": "...", "data": {} },
    { "type": "thinking_round", "timestamp": "...", "data": {} },
    { "type": "plan_generated", "timestamp": "...", "data": {} },
    { "type": "step_executed", "timestamp": "...", "data": {} }
  ],
  "final_state": "DONE",
  "metrics": {
    "mainline_hit": true,
    "drift_interrupted": false,
    "rework_count": 1,
    "total_tokens": 3200
  }
}
```

### 6.2 回放用途

1. **调试**：复现问题执行路径
2. **标注**：人工标注 mainline_hit / drift_interrupted
3. **训练**：作为 LLM 微调数据（什么问题链效果好）
4. **审计**：检查 AI 决策过程

---

## 7. 与现有系统集成

### 7.1 CostTracker 扩展

```rust
// src/cost_tracker/mod.rs
pub struct CostTracker {
    // 现有字段...
    
    /// 新增：Workflow 指标存储
    pub workflow_metrics: Option<MetricsStore>,
}

impl CostTracker {
    pub fn with_workflow_metrics(mut self) -> Self {
        self.workflow_metrics = Some(MetricsStore::new(10_000));
        self
    }
    
    pub fn record_workflow_event(&self, event: MetricEvent) {
        if let Some(store) = &self.workflow_metrics {
            store.record(event);
        }
    }
}
```

### 7.2 与 /audit 命令集成

```
/audit workflow
  → 导出 Workflow 回放 JSON
  
/audit summary
  → 现有：工具调用统计
  → 新增：Workflow 四硬指标概览
```

---

## 8. 性能要求

| 指标 | 要求 |
|------|------|
| 埋点延迟 | < 1ms（异步写入） |
| 查询延迟 | < 10ms（内存操作） |
| Ring buffer 大小 | 默认 10,000 条 |
| 持久化频率 | 每 100 条或 5 分钟 |
| 内存占用 | < 50MB |

---

## 9. 隐私与安全

| 数据 | 处理方式 |
|------|---------|
| 用户原始请求 | 存储 hash，不存原文 |
| 代码内容 | 不存储，只存文件名 |
| API key | 永不存储 |
| 思考过程 | 可选持久化（默认不存） |

---

## 10. 验收标准

- [ ] MetricsStore 线程安全，支持并发写入
- [ ] 所有 Workflow 状态转换都有埋点
- [ ] /doctor workflow 显示四硬指标
- [ ] 周报脚本可运行并产出 Markdown
- [ ] 回放文件格式可解析、可复现
- [ ] Ring buffer 大小可配置
- [ ] 单元测试覆盖 MetricsStore 核心操作

---

*本文档冻结后，指标字典不再变更。埋点字段可在 W8+ 按需增删。*
