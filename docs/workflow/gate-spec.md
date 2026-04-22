# Gate 方案规格（W3 交付物）

> 目标：定义 Direct Mode 与 Workflow 之间的判定闸门，包括误判检测与回退策略。
> 状态：V1 冻结
> 基准准确率目标：离线判定 >= 75%

---

## 1. 设计原则

1. **宁可误判为 Workflow，不可误判为 Direct**。Workflow 可以 `/skip` 回到 Direct，但 Direct 中跑偏后再升级会丢失上下文。
2. **快速通道硬规则优先于 LLM 判断**。减少简单请求的 token 开销。
3. **所有判定必须可解释**。Gate 输出必须包含判定原因文本。

---

## 2. 判定架构

```
用户请求
    │
    ▼
┌─────────────────┐
│ 快速通道检查     │ ← 硬规则，O(1)
│ (Fast Lane)     │
└────────┬────────┘
         │
    ┌────┴────┐
    ▼         ▼
 Direct   继续判定
            │
            ▼
┌─────────────────┐
│ 关键词启发式扫描 │ ← 正则/关键词，O(n)
│ (Heuristic)     │
└────────┬────────┘
         │
    ┌────┴────┐
    ▼         ▼
 Workflow  继续判定
            │
            ▼
┌─────────────────┐
│ LLM 轻量分类    │ ← 1-2 轮，低成本
│ (Classifier)    │
└────────┬────────┘
         │
    ┌────┴────┐
    ▼         ▼
 Workflow  Direct
```

---

## 3. 快速通道（Fast Lane）

### 3.1 匹配规则

以下请求**直接**走 Direct，不进入后续判定：

| 类别 | 匹配规则（正则/精确） | 示例 |
|------|---------------------|------|
| 帮助类 | `^/(help|clear|status|doctor|quit)\b` | `/help`, `/doctor gap` |
| 只读查询 | `^(git status|ls|cat|echo|pwd)\b` | `git status`, `cat README.md` |
| 问候闲聊 | `^(你好|在吗|谢谢|再见|hi|hello|thanks)\b` | `你好`, `谢谢` |
| 记忆操作 | `^/(memory|save|load)\b` | `/memory show` |
| 系统查询 | `^/(cost|token|model|tools)\b` | `/cost`, `/model` |

### 3.2 实现

```rust
fn fast_lane_check(input: &str) -> Option<GateDecision> {
    static FAST_LANE_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| vec![
        Regex::new(r"^/(help|clear|status|doctor|quit)\b").unwrap(),
        Regex::new(r"^(git status|ls|cat|echo|pwd)\b").unwrap(),
        // ...
    ]);
    
    for pattern in FAST_LANE_PATTERNS.iter() {
        if pattern.is_match(input) {
            return Some(GateDecision::Direct {
                reason: format!("Fast lane: matched {}", pattern.as_str()),
            });
        }
    }
    None
}
```

---

## 4. 关键词启发式扫描

### 4.1 高风险关键词（直接 -> Workflow）

以下关键词出现即判定为 Workflow：

| 类别 | 关键词 |
|------|--------|
| 架构 | "重构", "redesign", "architecture", "拆分", "解耦" |
| 多文件 | "所有文件", "批量", "全局", "cross-module" |
| 新增系统 | "新增模块", "实现系统", "添加引擎", "引入框架" |
| 高风险 | "删除", "迁移", "升级", "替换底层" |

### 4.2 低风险关键词（直接 -> Direct）

以下关键词且无高风险词时，判定为 Direct：

| 类别 | 关键词 |
|------|--------|
| 单点修复 | "修复", "fix", "改正", "typo" |
| 查看 | "查看", "显示", "列出", "grep", "find" |
| 配置 | "改默认值", "调参数", "开关" |

### 4.3 实现

```rust
fn heuristic_scan(input: &str) -> Option<GateDecision> {
    let high_risk = ["重构", "redesign", "拆分", "批量", "删除", "迁移"];
    let low_risk = ["修复", "fix", "typo", "查看", "显示", "列出"];
    
    let has_high = high_risk.iter().any(|w| input.contains(w));
    let has_low = low_risk.iter().any(|w| input.contains(w));
    
    if has_high {
        return Some(GateDecision::Workflow {
            reason: "Heuristic: high-risk keywords detected".into(),
            confidence: 0.8,
        });
    }
    if has_low && !has_high {
        return Some(GateDecision::Direct {
            reason: "Heuristic: low-risk keywords only".into(),
        });
    }
    None
}
```

---

## 5. LLM 轻量分类器

### 5.1 输入

- 用户原始请求（截断至 500 字符）
- 当前会话上下文摘要（最近 3 条消息，可选）

### 5.2 输出 Schema

```json
{
  "decision": "workflow" | "direct",
  "confidence": 0.0-1.0,
  "reason": "一句话解释",
  "dimensions": {
    "semantic_complexity": 1-5,
    "estimated_scope": "small" | "medium" | "large",
    "has_risk_actions": true | false,
    "needs_architecture_decision": true | false
  }
}
```

### 5.3 Prompt 模板

```
你是一个请求分类器。判断以下用户请求是应该"直接回答"还是"需要结构化思考流程"。

用户请求: "{input}"

考虑:
1. 这个请求是否涉及多文件修改或架构决策?
2. 是否包含文件写入、命令执行等风险操作?
3. 是否需要先理解大量现有代码?
4. 用户是否要求"想清楚再做"或"给出计划"?

只输出 JSON，不要解释。
```

### 5.4 判定规则

```
if confidence >= 0.7:
    采用 LLM 判定结果
elif confidence >= 0.5:
    偏向 Workflow（宁可误判）
else:
    默认 Workflow（低置信度 = 复杂可能性高）
```

---

## 6. 误判检测与回退

### 6.1 复杂判为简单（漏判）检测

**检测信号：**
- 用户在 Direct 模式下连续 2 次说"再想一下"/"列个计划"/"先分析"
- 单次 Direct 对话中工具调用次数 > 5 次（说明实际很复杂）
- 用户输入 `/workflow` 或 `/think` 命令

**回退动作：**
```
1. 保存当前 Direct 上下文快照
2. 自动升级至 Workflow，携带快照作为上下文
3. 通知用户："检测到复杂任务，已自动切换至结构化思考模式"
4. 从 THINKING 状态开始，而非 GATE_CHECK
```

### 6.2 简单判为复杂（误判）检测

**检测信号：**
- Gate score < 1.5（判定边缘）
- LLM classifier confidence < 0.6
- 用户输入 `/skip` 或 "不用想太多"

**回退动作：**
```
1. 允许用户一次 /skip 回到 Direct
2. 记录此次误判（用于后续调优阈值）
3. 降级到 Direct 模式，保留已生成的思考成果（可选显示）
```

### 6.3 误判日志格式

```json
{
  "timestamp": "2026-04-22T10:00:00Z",
  "input": "用户请求",
  "decision": "workflow",
  "reason": "LLM: confidence=0.85",
  "actual": "direct",
  "misclassification_type": "false_positive",
  "recovery_action": "user_skip"
}
```

---

## 7. 阈值调优策略

### 7.1 初始阈值（W3-W4）

| 参数 | 值 | 说明 |
|------|-----|------|
| `gate_heuristic_high_threshold` | 1 | 高风险关键词命中即 Workflow |
| `gate_llm_confidence_threshold` | 0.7 | LLM 判定置信度门槛 |
| `gate_llm_low_confidence_bias` | Workflow | 低置信度偏向 Workflow |
| `gate_upgrade_trigger` | 2 次纠偏 | Direct 中用户 2 次要求升级 |
| `gate_skip_allowed` | true | 允许用户 /skip 回退 |

### 7.2 调优方法（W7+）

1. 每周从误判日志中采样
2. 调整关键词列表和 LLM prompt
3. A/B 测试：随机 10% 请求使用新阈值
4. 指标监控：false_positive_rate, false_negative_rate

---

## 8. 与现有系统集成

### 8.1 插入点

```rust
// engine/conversation_loop.rs
async fn run_inner(&mut self,
    input: &str,
    history: &[Message],
) -> Result<Vec<StreamEvent>> {
    // === 新增：Gate 检查 ===
    let gate = Gate::new(self.llm_provider.clone());
    match gate.decide(input, history).await? {
        GateDecision::Direct { reason } => {
            self.emit_stream(StreamEvent::GateDecision {
                decision: "direct",
                reason,
            });
            return self.run_direct(input).await;
        }
        GateDecision::Workflow { reason, confidence } => {
            self.emit_stream(StreamEvent::GateDecision {
                decision: "workflow",
                reason: format!("{} (confidence={:.2})", reason, confidence),
            });
            return WorkflowEngine::new(self.clone())
                .run(input)
                .await;
        }
    }
}
```

### 8.2 降级路径

```
Workflow -> (budget 耗尽 / 错误 / 用户中断)
    -> FALLBACK_DIRECT
    -> 通知用户："已降级到直接对话模式"
    -> 保留已产生的思考成果作为上下文
```

---

## 9. 验收标准

| 检查项 | 标准 |
|--------|------|
| 快速通道覆盖率 | >= 80% 的简单请求命中 Fast Lane |
| 启发式准确率 | 在标注样本上 >= 70% |
| LLM 分类器准确率 | 在标注样本上 >= 75% |
| 端到端误判率 | false_positive + false_negative < 25% |
| 降级成功率 | 从 Workflow 降级到 Direct 不丢失上下文 |
| 升级成功率 | 从 Direct 升级至 Workflow 携带完整上下文 |

---

## 10. 风险

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| LLM 分类器延迟 | Gate 增加 1-2s 延迟 | Fast Lane + Heuristic 覆盖 80% 请求，不调用 LLM |
| 阈值过严 | 大量简单请求进 Workflow | 每周调优，用户可 /skip |
| 阈值过松 | 复杂任务走 Direct 跑偏 | 自动升级检测（2 次纠偏触发） |
| Prompt 注入 | 用户输入欺骗 Gate | 输入截断 + 不暴露 Gate 内部逻辑 |

---

*本文档冻结后，阈值参数可通过环境变量调优，但判定架构不再变更。*
