# Routing And Context Analysis - 2026-06-08
Status: Reference

Priority Agent 路由系统、上下文注入、缓存和压缩策略的当前状态分析，以及下一步
改进方向。

本文档的结论基于当前代码，而不是只按设计意图推断。核心判断是：

- 路由有价值，但它不应该替 LLM 做语义决策。
- 路由的职责是确定工具面、上下文深度、资源预算和风险边界。
- LLM 继续负责理解用户意图、工程判断、修复方向和最终表达。
- 任何压缩方案都必须保护 closeout 所需的验证证据，不能只追求 token 下降。

---

## 1. 当前路由架构

### 入口

当前 turn setup 入口是：

```text
src/engine/conversation_loop/turn_setup_controller.rs
  -> IntentRouter::new().route_with_learning(...)
  -> agent_mode.apply_to_route(...)
  -> ResourcePolicy::from_route(...)
```

`route_with_learning()` 先用启发式规则分类，再根据最近 learning events 做后置
调整。它不会让 LLM 重新分类，也不会让 runtime 接管语义规划。

### 路由输出

`IntentRoute` 当前包含：

```rust
IntentRoute {
    intent,
    confidence,
    workflow,
    retrieval,
    reasoning,
    risk,
    recommended_tools,
    dependency_install_intent,
    mcp_auth_intent,
    reason,
}
```

这些字段主要影响：

- route-scoped tools 和 recommended tools。
- retrieval policy。
- resource policy。
- workflow/risk/task-context controller 的后续行为。
- `/trace` 中的 `IntentRouted` 和 `ResourcePolicySelected` 事件。

### Intent / Workflow / Retrieval 枚举

当前 `IntentKind` 是 9 个：

| Intent | 典型含义 |
|--------|----------|
| `DirectAnswer` | 直接回答、局部读取、终端检查、计算 |
| `CodeChange` | 实现、修改、优化、创建代码产物 |
| `Debugging` | 修复报错、失败、bug |
| `Research` | web 搜索、外部调研、最新信息 |
| `Memory` | 记忆读取或保存，不涉及代码改动 |
| `Configuration` | 配置、权限、provider、MCP |
| `Delegation` | 子 agent、并行、委派 |
| `Planning` | 计划、架构、设计，不直接改代码 |
| `Unknown` | 保留枚举，当前正常 route 很少直接返回 |

当前 `WorkflowKind` 是 6 个：`Direct`、`CodeChange`、`BugFix`、`Research`、
`Planning`、`Delegation`。

当前 `RetrievalPolicy` 是 6 个：`None`、`Light`、`Project`、`Memory`、`Web`、
`Full`。

---

## 2. 当前决策链

### 主分类仍是启发式 cascade

`src/engine/intent_router.rs` 先计算一组布尔信号：

- memory signal
- live coding code-change / audit signal
- read-only signal
- code-change signal
- debug signal
- file mutation signal
- local inspection / file read signal
- terminal operation signal
- dependency install signal
- MCP auth / configuration signal
- delegation / research / planning signal

之后通过一串有顺序的 `if` / `return IntentRoute` 选路由。也就是说：

- 它不是所有候选 intent 共同打分后取最高。
- 它仍然存在“谁先匹配谁赢”的顺序依赖。
- 当前代码已经通过若干专门分支处理了一些误分类风险，例如 memory 相关代码改动
  不应被当成普通 memory save/load turn。

### Learning feedback 是后置调整，不是语义分类

`route_with_learning()` 会读取最近 learning events：

- recent recovery plans 会降低 confidence，并把 Light retrieval 升到 Project。
- recent failed turns 会提高 reasoning、risk，并降低 confidence。
- successful/failed tool outcomes 会调整 recommended tools。

这说明文档不能写成“完全不看历史”。更准确的表述是：

> 当前 route 的主分类仍只看最后一条 user message 和启发式信号；learning
> feedback 会调整 confidence、retrieval、reasoning、risk 和 tools，但不会重新
> 解释“这一轮到底是不是上一轮任务的继续”。

### 2026-06-08 已补的实现

本次分析暴露的最确定实现缺口是：上一轮 CodeChange/Debugging 未完成时，下一轮
弱 follow-up 容易 fallback 到 DirectAnswer/Light。

已补最小修复：

- 在 `TurnSetupController` 中增加 unfinished route continuation。
- 只在上一轮 route 是 `CodeChange` 或 `BugFix`，上一轮 trace 非 `Completed`，
  当前消息是弱 follow-up，且没有明显 topic switch 时继承。
- 继承后将 retrieval 提升到 `Project`，reasoning 提升到 `High`，保留较强 risk。
- 明显 topic switch、只读要求、搜索/记忆/新话题等不会继承。

这不是 LLM 辅助分类，也不是 runtime 替 LLM 做语义决策；它只是把未完成任务的
runtime 边界延续到下一轮，避免“还有那个文件也改一下”这种弱指代掉到 Light。

---

## 3. OpenCode 对比

### OpenCode 的简化点

OpenCode 更偏向用户显式选择 agent/mode：

- build agent 可以改代码。
- plan agent 不改代码。
- 权限 ruleset 就是主要路由边界。
- 子 agent 委托模型更简单。

因此 OpenCode 的误分类问题更少，因为一部分分类责任交给了用户。

### Priority Agent 的优势

Priority Agent 自动 route 的价值是：

- 用户不需要显式选择模式。
- 可以按 route 控制工具面。
- 可以按 retrieval policy 控制上下文预算。
- 可以按 reasoning/risk 分配资源和验证要求。
- 可以把 route、resource、context、risk 都写入 trace，便于诊断。

### Priority Agent 的代价

自动 route 带来的问题是：

- 关键词 cascade 有顺序依赖。
- 弱 follow-up 容易丢上下文。
- route 解释需要 trace，否则很难知道为什么给了某个工具面。
- 规则越多，边缘 prompt 越需要校准测试。

---

## 4. 路由的价值

LLM 会自己选择工具，但它只能在 runtime 暴露的工具面里选择。

```text
路由决定:
  - 能看到哪些工具
  - 能拿到多少上下文
  - 有多少预算和重试空间
  - 当前风险等级和验证要求

LLM 决定:
  - 当前目标如何理解
  - 在可用工具中调用哪个
  - 如何解释证据
  - 如何修复失败
  - 如何向用户收口
```

因此 route 不应该变成“runtime 选下一步工具”。它应该是确定性边界：

- 问答 turn 不应暴露大范围写工具。
- code-change turn 应给 Project 级上下文和验证约束。
- high-risk turn 应触发更严格的 workflow/risk/checkpoint。
- memory turn 应暴露 memory 工具，但 memory 相关代码改动不应误进 memory route。

---

## 5. 主要问题

### 问题 1: 弱 follow-up 容易丢 intent

旧问题：

```text
第 1 轮: "帮我重构 scoring.rs" -> CodeChange
第 2 轮: "还有那个文件也改一下" -> 可能 DirectAnswer/Light
```

这类问题不是 LLM 不会理解，而是 runtime 在 LLM 请求前就可能给了较窄工具面和
较少上下文。

当前状态：已补最小 continuation 修复，只对未完成 CodeChange/BugFix 生效。

剩余风险：

- 如果上一轮已经 `Completed`，但用户仍然自然 follow-up，当前实现不会继承。
- 这是有意保守，避免 completed turn 后的“换话题”被误判为继续改代码。
- 后续可以通过 edge-case eval 决定是否扩大到“最近 completed code task + 强
  follow-up reference”。

### 问题 2: Cascade 没有候选竞争评分

当前 router 是有序分支，不是：

```text
collect candidates -> score confidence -> choose highest -> trace alternatives
```

这会导致：

- 有些 prompt 同时命中 research/planning/code-change 时，先匹配的分支胜出。
- route confidence 是固定分支常量，不是候选间比较后的置信度。
- `/trace` 只记录最终 route，不记录被压下去的候选。

建议下一步先补 diagnostics，而不是直接重写：

1. 为 router 增加 optional shadow candidates。
2. trace 记录 top 3 candidates、score、matched signals。
3. 用 edge-case tests 证明哪些 cascade 顺序真的错了。
4. 再考虑 confidence-based routing。

### 问题 3: 首轮弱信号 fallback 偏 Light

`direct()` fallback 当前是：

```text
DirectAnswer / Direct / Light / Low / Low
```

learning feedback 能在失败或 recovery 后把 Light 升成 Project，但这是事后补救。
首轮“看看这个”“还有那个”这种弱指代仍可能缺少项目上下文。

不建议简单把所有低 confidence 都升到 Project，因为会增加普通问答成本。

更稳的策略：

- 弱 follow-up + unfinished code/debug -> Project，已实现。
- 弱 follow-up + recent completed code/debug -> 先只做 trace shadow，不立即升。
- 无 follow-up reference 的普通 direct fallback 继续 Light。

### 问题 4: LLM 辅助分类不是第一优先级

“confidence 低时让 LLM 自报 intent”有吸引力，但风险是：

- 多一次模型调用或更复杂的 prompt schema。
- provider 不稳定时会把 route 本身变成新故障源。
- 容易让 runtime prompt 变长，和当前 runtime diet 方向冲突。

更好的顺序：

1. 先做 deterministic continuation。
2. 再做 route candidate diagnostics。
3. 用 edge-case eval 看误分类分布。
4. 只有确定规则无法覆盖时，再做 LLM-assisted route shadow，不直接 gate。

---

## 6. 上下文注入现状

### LLM 请求结构

动态上下文主要在 `RequestPreparationController` 里注入到最后一条 user message，
而不是不断追加新的 system message。当前可注入内容包括：

- `<task-state>`
- `<task-contract>`
- `<context-pack>`
- model-led candidate action hint
- self-evolution guidance
- focused repair zone
- context ledger hint
- project map zone
- `<task-guidance>`
- memory prefetch / retrieval context

这个设计的目的很明确：动态内容靠近当前用户请求，同时尽量保持 stable prefix
cache-friendly。

### Trace 已有 context observability

当前会记录：

- `ContextZonesMaterialized`
- `CacheStabilitySnapshot`
- `PromptCacheUsageRecorded`
- runtime diet token/tool budget 信息

因此 context 问题不应该只靠肉眼看 prompt。下一步应该把这些 trace 字段放进
route/context edge-case eval 的验收标准。

---

## 7. 缓存分析

### 命中便宜，但不是免费

provider prompt cache 的 cached tokens 通常比 miss tokens 便宜，但不是免费。
而且每轮新增的 assistant/tool output 会成为新的动态尾部，仍然会产生新增成本。

文档里不要写死某个 provider 的价格作为架构事实。价格应放到 provider/cost
配置或单独 benchmark 里。这里保留原则：

- stable prefix 越稳定，cache 越容易命中。
- dynamic tail 每轮增长，会持续增加 prompt 压力。
- 工具 schema 变化会影响 cache shape。
- 动态上下文放进最后 user message，有助于减少 system prefix 变化。

### 当前 cache diagnostics

当前 `cache_stability` 会记录：

- stable prefix fingerprint
- tool schema fingerprint
- tool count / tool schema tokens
- dynamic zone message count
- dynamic zones before last user

这比单纯“MD5 指纹监控”更具体。它能帮助判断 miss 是因为 system prompt、tools、
few shots / memory / skills，还是 dynamic tail 变化。

### 当前判断：应该学 OpenCode，但不能照抄

OpenCode 值得学习的是主动上下文管理意识：不要等到上下文快满才被动 compact，
也不要让旧 tool output 无限制堆在 dynamic tail 里。Priority Agent 现在确实存在
长任务 token 成本问题：即使 stable prefix 命中 provider cache，每轮新增的
assistant/tool 历史仍然会形成新的 cache miss 或 prompt pressure。

但 Priority Agent 不能直接照搬 OpenCode 的整段历史压缩，因为这里的 closeout
verification 依赖真实 runtime evidence。代码修改任务最后需要回答：

- 哪些文件实际变了。
- 哪些验证命令运行过。
- 验证命令的 exit/status 是什么。
- 哪些关键输出证明 acceptance criteria 已满足。
- 如果没有验证，为什么只能 `not_verified` 或 `partial`。

因此下一步应该做的是 **evidence-aware selective compression**，而不是普通
conversation summarization。

---

## 8. 压缩分析

### 不能全量照搬 OpenCode

Priority Agent 的 closeout 依赖 runtime evidence。对于代码修改任务，最终通过
不是靠一句“tests passed”，而是需要工具观察、验证命令、diff 状态和 acceptance
证据链。

如果把早期工具输出粗暴压缩成一句自然语言摘要，可能导致：

- closeout controller 找不到足够证据。
- repair loop 被触发。
- false green 风险增加，或者 not_verified 变多。

### 当前压缩基础

当前 compaction strategy 不止三种，而是：

- `NoOp`
- `Snip`
- `MicroCompact`
- `AutoCompact`
- `ReactiveCompact`
- `SessionMemoryCompact`

并且 request preparation 里还有 message healing，会 shrink oversized tool
results 并 drop dangling tool calls，以避免 provider 400。

因此“新增一个 SelectiveCompress 策略”不是简单插一个 enum 就结束。真正需要先定义
证据边界。

### 建议的选择性压缩 contract

选择性压缩可以做，但必须满足：

1. 保留最近 2 轮 tool output 原文。
2. 保留被 closeout/validation 标记为 required evidence 的 tool output。
3. 对可压缩 tool output 保留：
   - tool name
   - command / path / args 摘要
   - exit code
   - success/failure
   - 关键匹配行或失败行
   - trace/event id 或 call id
4. 压缩结果必须能被 closeout 和 context ledger 识别为 evidence summary，而不是
   普通 assistant prose。
5. 每次压缩都记录 `CompactionAttemptRecord` 和 retained evidence count。

建议不要马上改 50% 主动压缩阈值。先加 observability：

- dynamic tail tokens
- tool result tokens
- high-value evidence tokens
- compressible tool output tokens

拿到数据后再决定阈值。

### 第一版实现建议

第一版压缩不要动 user/assistant 对话文本，也不要压最近 tool results。只处理
“旧的、大的、非 closeout 必需”的 tool output。

推荐规则：

```text
keep raw:
  - 最近 2 轮 tool output
  - validation/closeout required evidence
  - failed tool output 的关键错误上下文
  - 用户明确要求查看的命令输出

compress:
  - 更早轮次的大 stdout/stderr
  - 已经被 context ledger / validation ledger 结构化记录过的输出
  - 重复的 build/test/log 噪声
```

压缩后的格式应该是结构化 evidence summary，而不是自然语言摘要：

```text
[compressed-tool-output]
tool=bash
cmd=cargo test -q
exit=0
status=passed
raw_preserved=false
evidence_safe_for_closeout=true
key_lines:
  - test result: ok
  - 19 passed; 0 failed
call_id=...
source_turn=...
```

如果 `evidence_safe_for_closeout=false`，closeout controller 不能把这条摘要当作
完整验证证据，只能当成历史参考。

### 先做 observability，再做阈值

不建议第一步就改成“50% token 主动压缩”。更稳的顺序是：

1. 先记录 dynamic tail、tool result、required evidence、compressible output 四类
   token。
2. 在 `/trace` 或 runtime diet summary 中显示这些占比。
3. 用真实长任务观察：到底是 tool output、tool schema、memory/context zone，还是
   assistant prose 在吃 token。
4. 证明旧 tool output 是主因后，再启用 selective compression。
5. 只有当 selective compression 稳定不破坏 closeout 后，再考虑主动阈值。

建议新增 trace/runtime-diet 字段：

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

### 必须补的安全测试

Selective compression 合并前必须有测试证明：

- 压缩后 verified closeout 仍然能找到验证证据。
- 压缩后不能把普通摘要误当成真实验证证据。
- 没有验证命令时，不能因为摘要里有 “passed” 字样就 false green。
- 最近 2 轮 tool output 原文仍保留。
- 失败输出的关键错误行不会被裁掉。

这组测试比 token 节省比例更重要。token 可以后续优化，false green 不可以接受。

---

## 9. 推荐下一步

### P0: 已完成的最小修复

1. 修正文档结构和事实错误。
2. 增加 unfinished CodeChange/BugFix route continuation。
3. 增加测试：
   - 弱 follow-up 继承未完成 code-change route。
   - 明显 topic switch 不继承。

### P1: Route diagnostics

新增 router shadow diagnostics，不改变行为：

- `RouteCandidateEvaluated { intent, confidence, matched_signals, reason }`
- `RouteCompetitionSummary { selected, runner_up, delta }`

先在 trace 里看候选竞争，不直接改变 route 选择。

### P2: Edge-case eval

新增 routing/context eval cases：

| Case | 目标 |
|------|------|
| `routing-followup-unfinished-codechange` | 未完成代码任务弱 follow-up 继承 Project |
| `routing-topic-switch-readonly` | 换话题 + 只读不继承代码 route |
| `routing-memory-codechange-not-memory-route` | memory 相关代码修复不误进 Memory |
| `routing-research-vs-codechange` | search 字样在 code eval 中不误进 Research |
| `context-closeout-evidence-preserved` | 压缩/清理后 closeout 证据仍可见 |
| `context-cache-dynamic-tail-tracked` | dynamic tail 增长能在 trace 里解释 |

### P3: Selective compression prototype

只在有 P2 evidence 后做，且分两步：

Phase A 只加观测，不压缩：

1. 记录 dynamic tail / tool result / required evidence / compressible output token。
2. 在 trace summary 或 runtime diet 中展示压缩机会。
3. 跑真实长任务，确认旧 tool output 是主要 token 来源。

Phase B 再加 evidence-aware selective compression：

1. 标记 validation/closeout required evidence。
2. 为 old tool output 生成 structured evidence summary。
3. 保留最近 2 轮原文。
4. 增加 closeout 测试，证明压缩后不 false green、不误 not_verified。
5. 默认只压旧 tool output，不压 user/assistant 对话。
6. 再考虑主动压缩阈值。

### P4: LLM-assisted routing shadow

只有当 P1/P2 证明 deterministic route 无法覆盖某些高价值边缘场景时，再做：

- 低 confidence 时让 LLM 输出 intent guess。
- 只记录 shadow trace。
- 不直接改变工具面。
- 和 deterministic route 做 AB 比较。

---

## 10. 总结

这份分析的核心方向成立：Priority Agent 需要 route，因为 route 决定工具面、
上下文、预算和风险，不是替 LLM 选工具。

需要修正的是执行顺序：

1. 先修事实和 observability。
2. 先补 deterministic continuation。
3. 再做 route candidate diagnostics 和 edge-case eval。
4. 最后才考虑 LLM-assisted route 或更激进的 selective compression。

当前最重要的边界仍然是：runtime 负责确定性筛查、上下文组织、证据保存和安全门；
LLM 负责语义判断和工程决策。任何 routing/context 改动都不能把这个边界写反。
