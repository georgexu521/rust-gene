# Runtime Spine 稳定性评审与后续开发计划

日期: 2026-05-26

来源:

- 基于 `docs/PROJECT_FLOW_AND_RUNTIME_ARCHITECTURE_2026-05-26.md` 的外部
  review 反馈。
- 基于本计划初稿的二次 review 校准。

目的: 记录这些反馈里哪些判断有价值、它们如何映射到当前代码库，以及下一步
应该怎么排开发计划。这份文档刻意聚焦“可靠性和收敛”，不是继续扩功能面。

## 1. 总体判断

这些反馈有价值，而且二次校准进一步把优先级理顺了。

第一版计划的主轴是对的: Priority Agent 现在不应该主要靠继续加功能前进。
当前架构已经是一个真实的本地 agent runtime，不是 prompt wrapper。下一阶段
应该证明 runtime spine 在真实任务下稳定:

- route 和 task mode 足够准确，或者错了以后能恢复；
- context zones 不冲突、不泄漏权限；
- tools 暴露足够窄，但不会让 agent 变得别扭；
- gates 能挡住真实风险，而不是制造无谓死路；
- evidence semantics 足够明确，可以支撑可信 closeout；
- memory 和 skills 有用，但不变成 prompt 噪音；
- subagents 先提升探索和验证能力，再逐步承担更宽的实现工作。

二次 review 的关键修正也应该采纳:

- P0 不能一次做成一个大阶段，应该拆成 P0a/P0b。
- proof semantics 不能太晚，否则 P0 closeout 断言会建立在模糊语义上。
- route recovery 必须有安全单调性: 可以自动扩大理解能力，不能静默扩大破坏能力。
- gate outcome 不能粗暴二分 useful/false-positive，要有更细的可恢复/摩擦分类。
- context zone 收口还要保证 deterministic ordering 和 provenance dedupe。
- `supports_verified` 应该由 policy 派生，而不是作为可冲突的原始事实字段。
- subagent policy 应该和 proof semantics 绑定。
- P3 文案清理成本低，可以并行提前做。

这和项目当前方向一致: 窄、深、个人、可验证。

## 2. 本阶段工程纪律

这一阶段应该把以下约束当成工程纪律，而不是普通建议:

- 不增加更多 always-on prompt rules 来掩盖 runtime 问题。
- 不为了掩盖 route 错误而扩大默认工具暴露。
- 不为了提高 pass rate 削弱 validation、permission、checkpoint 或 closeout。
- 不把 subagent 输出直接当成 verified proof。
- 不把 active memory 对外描述成完整后台记忆 agent。
- 不把 skill evolution 对外描述成自动自我进化系统。
- 不把 route 决策完全交给 LLM，而没有 deterministic、traceable 的恢复逻辑。
- 不因为模型反复请求 mutation 就自动开放 mutation。

目标不是让 agent 更放开，而是让现有 runtime controls 更可度量、更可恢复、
更不别扭。

## 3. 当前代码映射

这份计划不是凭空设计，主要落在这些现有模块上:

| 主题 | 当前落点 |
| --- | --- |
| 测试矩阵和 live eval | `docs/AGENT_TESTING_MATRIX_2026-05-08.md`, `src/engine/scenario_matrix.rs`, `scripts/run_live_eval.sh` |
| route/task mode | `src/engine/intent_router.rs`, `src/engine/task_mode_score.rs`, `src/engine/task_context.rs` |
| context zones | `src/engine/context_assembly.rs`, `src/engine/prompt_context.rs`, `src/engine/conversation_loop/retrieval_prompt_controller.rs` |
| tool gate | `src/engine/conversation_loop/tool_execution_controller.rs`, `src/engine/conversation_loop/tool_exposure_plan.rs` |
| action review | `src/engine/action_review.rs`, `src/engine/action_policy.rs` |
| proof/closeout | `src/engine/evidence_ledger.rs`, `src/engine/verification_proof.rs`, `src/engine/conversation_loop/closeout_controller.rs`, `src/engine/conversation_loop/turn_completion_controller.rs` |
| memory provider | `src/memory/provider.rs`, `src/memory/manager.rs` |
| subagent | `src/tools/agent_tool/mod.rs`, `src/agent/profiles.rs`, `src/agent/manager.rs` |

## 4. 后续开发计划

## P0a: 核心 Runtime Spine Deterministic Matrix

目标: 先用最小核心矩阵证明主循环、工具暴露、mutation、validation、closeout
骨架稳定。P0a 应该能成为 daily/fast gate。

### P0a.1 核心 8 个场景

| Scenario | 期望 spine 行为 |
| --- | --- |
| Direct answer | 不暴露无关工具，不强制验证，最终回答简洁 |
| Read-only project audit | 允许项目读/search，不 mutation，有 no-diff evidence |
| Small code change | 先读后改，scoped mutation，便宜验证，verified closeout |
| Bug fix | 检查失败，patch，重跑相关验证，失败则修复 |
| User forbids writes | 写工具隐藏或 denied，回答保持 read-only |
| User specifies validation command | required command 必须识别并证明 |
| Tool failure | failure 变 `ToolObservation`，不能静默成功 |
| No-diff audit closeout | 无 changed files，有明确 evidence，不误判为 missed edit |

### P0a.2 `RuntimeSpineCaseSpec` 标准格式

每个 case 应该逐步固化为统一 schema。建议从 YAML/JSON fixture 或 Rust struct
开始，字段形状如下:

```yaml
id: runtime_spine_small_code_change
task_type: code_change
initial_prompt: "..."
expected:
  route:
    one_of: [CodeChange, Debugging]
    min_confidence: medium
  tools:
    must_expose: [file_read, grep]
    may_expose: [file_edit, file_patch, run_tests]
    must_not_expose: [install_dependencies]
  mutation:
    expected_changed_files: non_empty
    outside_workspace: false
  validation:
    required: true
    accepted_families: [unit_test, targeted_check, format]
  closeout:
    allowed_status: [verified]
  final_answer:
    must_mention: [changed_files, validation]
```

这个 spec 是后续 route recovery、proof semantics、gate outcome aggregation 的
oracle。没有 case oracle，`false_positive` 这类指标就会变得主观。

### P0a.3 每个 case 的必查断言

每个 scenario 至少检查:

- initial route 和 confidence；
- final task mode；
- exposed tool set 或 exposure count；
- denied/revised tool attempts；
- changed files；
- validation evidence；
- verification proof status；
- completion contract status；
- final answer 的状态措辞；
- failure owner；
- gate 或 tool failure 后是否发生 recovery。

### P0a.4 UX Friction 指标

除了安全和正确性，还要量化“别扭”:

- tool rounds count；
- repeated denied attempts count；
- no-progress rounds count；
- unnecessary read count；
- time/cost budget usage；
- final answer status clarity。

一个 agent 可以安全且正确，但非常别扭。P0a 应该把这种体验成本显性化。

### P0a.5 Golden Trace Snapshot

关键 scenarios 应保存或生成 golden trace snapshot，至少覆盖:

- route decision；
- tool exposure plan；
- action review decisions；
- evidence ledger summary；
- verification proof；
- completion contract。

这样 context zone、route recovery、proof semantics 改动时，可以看出 runtime
spine 有没有悄悄变形。

### P0a.6 验收标准

- P0a matrix 以 durable doc/code/eval asset 存在。
- 核心 8 个 case 有 route/tool/evidence/closeout 断言。
- deterministic 部分不依赖 LLM 可跑。
- fast gate 可以频繁执行。
- live-eval report 能把失败映射到具体 `failure_owner`。

## P1: Gate Outcome Instrumentation

目标: 让每个重要 gate 都能被判断为保护性阻断、可恢复摩擦、未恢复阻断、
疑似误杀或策略正确但 UX 成本高。

### P1.1 Gate Outcome 分类

不要先用 useful/false-positive 二分。建议使用更细分类:

| Outcome | 含义 |
| --- | --- |
| `protective_block` | 明确挡住高风险、越权、越界或不符合 checkpoint 的动作 |
| `recoverable_friction` | block/revise 后模型成功换路完成 |
| `unrecovered_block` | block 后没有恢复，任务 partial/failed/blocked |
| `suspected_false_positive` | 按 scenario oracle 判断本应允许，但被挡住 |
| `policy_correct_but_ux_costly` | 策略正确，但导致明显绕路或成本过高 |
| `harmless_pass` | gate 通过且没有明显影响任务 |

`suspected_false_positive` 需要来自 `RuntimeSpineCaseSpec` 或明确 scenario oracle，
不要只靠模型抱怨或人工感觉。

### P1.2 统一 gate outcome 形状

可以新增或从现有 trace events 派生:

- `gate`: action_review、tool_execution、checkpoint、permission、closeout；
- `decision`: allow、ask_user、deny、revise、block、pass；
- `outcome`: 上面的 outcome 分类；
- `reason_codes`；
- `tool`；
- `route`；
- `stage`；
- `risk`；
- `action_score`；
- `scope_fit`；
- `recovered_after_gate`: unknown/yes/no；
- `final_status`: completed/partial/failed/blocked；
- `failure_owner`。

不需要立刻重构事件系统。第一步可以从已有 trace/report 派生。

### P1.3 追踪 recovery

每个 gate 之后要能看出是否出现:

- 成功 alternative tool；
- route/task-mode escalation；
- 用户授权 approval；
- 用户拒绝 permission 后 honest closeout；
- repeated denial 导致 partial/blocked；
- task abandonment 或 budget exhaustion。

### P1.4 聚合到报告

live-eval summary 应该能展示:

- 每个 case 的 gate decision counts；
- top deny/revise reasons；
- unrecovered gate blocks；
- false-green prevention events；
- suspected false-positive gates；
- policy-correct-but-costly cases。

### P1 验收标准

- 失败 eval 能回答“哪个 gate 停住了进展，为什么”。
- 通过 eval 能说明 gates 是提供了价值还是无关。
- route/tool-exposure 反复误判能在 aggregate 里显现。
- 策略正确但 UX 成本高的 case 不会被误判为纯成功。

## P1.5: Evidence Proof Semantics Spec And Additive Fields

目标: 先定义 closeout proof 语言，再让 P0/P1 的断言建立在稳定语义上。不急着
大迁移，但 spec 和 additive fields 要提前。

### P1.5.1 Proof Kinds

先写 spec，再落 code-level enum:

| Proof kind | 能支持 `Verified` 吗 | 说明 |
| --- | --- | --- |
| `CommandPassed` | yes | 当前 ledger 中 required/accepted validation command 通过 |
| `StaticCheckPassed` | scoped yes | 只有它是该任务接受的 validation family 时才支持 |
| `RequiredValidationPassed` | yes | 普通 code-change 最强 proof |
| `DiffReviewed` | 单独只能 partial | 支持 closeout evidence，但不能单独证明 code-change verified |
| `NoDiffAudit` | audit 可 yes | 仅当 no diff 是预期且 evidence 证明当前状态 |
| `KnownUnrelatedFailure` | partial 或带 residual risk 的 verified | 需要 focused required validation 通过和 unrelatedness evidence |
| `UserDeferred` | no | honest user-deferred closeout |
| `ToolUnavailable` | no | 通常是 blocked 或 partial |
| `PermissionDenied` | no | blocked 或 partial，取决于任务 |
| `SubagentClaimOnly` | no | 必须父 agent 验证后才能成为 proof |
| `ParentVerifiedSubagentResult` | scoped yes | 子 agent 输出经父 runtime 验证后才可升级 |
| `ManualInspectionOnly` | depends | read-only explanation 可以；code-change verification 不够 |

### P1.5.2 Evidence Record 只存事实，`supports_verified` 由 policy 派生

不要把 `supports_verified` 作为完全可信的原始事实字段写进 evidence record。
否则会出现不一致:

```text
proof_kind = DiffReviewed
supports_verified = true
```

更稳的做法:

- evidence record 存事实:
  - `proof_kind`
  - `scope`
  - `command_status`
  - `validation_family`
  - `source_agent`
  - `parent_verified`
  - `related_to_changed_files`
  - `residual_risk`
- proof policy 根据 task type、required validation、route、scope 和 evidence
  facts 派生:
  - `supports_verified: derived true/false`
  - `derived_status`
  - `derived_reason`

这样 EvidenceLedger 不会被写入互相矛盾的语义。

### P1.5.3 Closeout Rules

closeout 规则要能明确回答:

- 失败命令能不能视为 unrelated？
- 什么 evidence 能证明 unrelatedness？
- no-diff audit 能不能 verified？
- manual inspection 能不能关闭 read-only 任务？
- subagent verification 不经父 agent 复验能不能算？
- final answer 应该暴露哪个 status？

### P1.5.4 测试

增加 focused tests:

- required command passed -> verified；
- required command missing -> not run；
- required command failed then passed -> verified with recovered failure count；
- unrelated failure without scoped passing proof -> partial；
- unrelated failure with scoped passing proof -> verified with residual risk；
- subagent-only claim -> not verified；
- parent-verified subagent result -> scoped verified；
- no-diff audit with read/search evidence -> 根据任务类型 verified/not applicable；
- task state says verified but ledger has no evidence -> unavailable。

### P1.5 验收标准

- 有 proof kind spec。
- `EvidenceLedger` 能表达命名 proof semantics，不只是 generic pass/fail rollup。
- `supports_verified` 是 report/policy 派生结果，不是可能冲突的原始字段。
- final closeout status 能从 proof kinds 推导解释。
- subagent claims 不会意外变成 verified proof。

## P1: Context Zone 收口

目标: 一个 primary model request shape，把稳定指令和动态证据彻底分开。

### P1.1 Request Envelope

live request path 应该 materialize:

1. stable prefix；
2. task state；
3. relevant material；
4. recent observations；
5. current decision request。

每个 zone 最好在 trace 中可见:

- token estimate；
- source count；
- freshness；
- trust boundary；
- fingerprint。

### P1.2 移除重复 authority paths

审计并移除 memory、retrieval、observations 同时通过旧 user-message 或
stable-prompt 路径注入的情况。

硬规则:

- AGENTS/SOUL/USER/TOOLS 可以在优先级规则内塑造 stable context。
- retrieval 和 memory matches 是 background evidence。
- tool observations 是 recent evidence。
- 动态来源不能静默升级成 system policy。

### P1.3 Deterministic Ordering 和 Provenance Dedupe

除权限边界外，还要处理长期隐性 bug: 重复、乱序和 token 膨胀。

新增规则:

- 同样 retrieval/memory/session 输入，多次 assembly 应得到同样顺序和 fingerprint。
- 同一条事实来自 memory + retrieval + session 时，只保留一个主展示。
- 其他来源作为 provenance 合并，而不是重复喂给模型。
- 排序应优先考虑当前任务相关性、freshness、trust level 和 explicit user context。

### P1.4 测试

增加 snapshot/unit tests 证明:

- retrieval 改变不会改变 stable prefix fingerprint；
- `<relevant_material>` 不进入 stable prefix；
- hostile retrieved content 仍被 fenced 为 background evidence；
- recent tool failures 进入 recent observation/current decision zones；
- 同一个 memory item 不会跨多个 zone 重复注入；
- deterministic ordering 稳定；
- provenance dedupe 生效。

### P1 验收标准

- live request assembly 有一个 primary zone-first path。
- legacy rendering 只作为兼容/helper，或被移除。
- 动态材料被提升成 stable instruction authority 时，prompt tests 会失败。
- context ordering 和 dedupe 有测试保护。

## P1: Route And Task-Mode Recovery

目标: 初始 routing 是有用输入，而不是单点失败。

### P1.1 安全单调性原则

硬规则:

> Route recovery 可以自动扩大理解能力，但不能静默扩大破坏能力。

具体含义:

- 可以自动扩大 read/search/retrieval。
- 可以升级 workflow depth。
- 可以要求模型 re-plan。
- 可以在已有安全证据下进入 validation。
- 不应该因为模型多次请求 edit，就自动开放 edit。
- edit/mutation 必须由用户意图、task contract、route recovery policy 和 permission
  共同支持。
- high-risk mutation 永远不能因为 route drift 自动放开。

### P1.2 Runtime Drift Signals

加入 route/task drift signals，例如:

- 模型反复请求未暴露工具；
- direct answer 突然需要项目文件访问；
- research/planning task 试图 mutation；
- code-change task 到 validation 阶段仍没有 changed files；
- action review 反复提示下一步属于另一阶段；
- tool failure 暗示缺少 retrieval 或工具面错误；
- no-code-progress rounds 增加但 scope 未解决。

### P1.3 Bounded Escalation

当 drift 足够强时，runtime 可以:

- Direct -> Light；
- Light -> Full；
- 先扩大 read/search，而不是直接开放 edit；
- 激活 project retrieval；
- 有改动后进入 validation phase；
- workflow depth 从 minimal 升级到 stricter；
- 要求模型在 corrected route 下 re-plan。

升级必须 trace-visible、bounded，不能静默扩大到全工具权限。

### P1.4 测试

增加测试:

- direct prompt 后来需要 project inspection；
- bug/research wording 需要 code tools；
- code-change route 多轮后仍没有 changed files；
- 模型请求 hidden 但 route-compatible 的 read tool；
- 模型请求 hidden high-risk mutation，runtime 不过度放宽；
- 模型反复请求 edit 时，如果 task contract 不支持 mutation，仍不自动开放 edit。

### P1 验收标准

- 常见初始 route 错误可恢复。
- recovery 不削弱 destructive scope 或 permission gate。
- trace 清楚记录 original route、adjusted route/mode 和 reason。
- read/search/retrieval 可自动扩大，mutation 不会静默扩大。

## P0b: 复杂 Runtime Spine 扩展矩阵

目标: 在 P0a 核心骨架稳定后，覆盖更复杂控制面。

### P0b.1 扩展场景

| Scenario | 期望 spine 行为 |
| --- | --- |
| Permission required | permission prompt 可见，拒绝后有 recovery path |
| Test failure repair | failed validation 变 observation，bounded repair，不 false green |
| Route mistake recovery | 行为反证初始 route 后，route/task mode 能升级 |
| Subagent verifier | 子 agent claim 被记录，但父 agent 验证后才能 closeout |
| Isolated worktree implementer | child mutation 保持隔离，review/merge 后才算父任务证据 |
| Memory retrieval conflict | 过时或冲突 memory 被 demote，不变成 instruction authority |
| Skill guidance | skill 是 background guidance，不是更高优先级指令 |

### P0b.2 验收标准

- 每个扩展场景都有 `RuntimeSpineCaseSpec`。
- subagent、memory、skill 场景不能绕过 proof semantics。
- permission/recovery 场景能展示 gate outcome 和 recovery outcome。
- route recovery 场景遵守安全单调性原则。

## P2: Subagent Verification-First Policy

目标: 把 subagent policy 和 proof semantics 绑定，让 subagents 提升证据质量，
而不是成为 opaque authority。

### P2.1 Product Defaults

默认建议:

- explorer: 适合 broad read-only investigation；
- verifier: 适合重跑 focused checks 或验证 claim；
- reviewer: 适合风险/finding passes；
- implementer: 允许，但应使用 isolated worktree，并要求父 runtime 验证后
  closeout。

### P2.2 Evidence Rules

Subagent 输出只能先落成:

- `SubagentFinding`；
- `SubagentVerificationClaim`；
- `SubagentPatchSummary`；
- `SubagentBlocked`。

父 runtime 复验后，才可以生成:

- `ParentVerifiedSubagentResult`；
- `ParentReviewedSubagentPatch`；
- `ParentRejectedSubagentClaim`。

closeout 只看 parent-runtime proof，不直接信 child summary text。

### P2.3 UX 和 Trace

trace/report 应展示:

- child agent id；
- profile；
- context mode；
- isolated worktree path；
- allowed tools；
- result status；
- parent verification status；
- child output 是否影响 closeout；
- child output 影响了哪个 proof kind。

### P2 验收标准

- subagent verifier 提升 evidence，而不是成为 opaque authority。
- implementer subagent changes 在 review 前保持隔离。
- parent closeout 不会只凭 child text 报 verified。
- subagent proof 和 `EvidenceLedger` proof semantics 使用同一套规则。

## P2: Memory Provider Boundary

目标: 在增加更多 memory backend 前，让 local memory 也真正走 provider contract。

### P2.1 Local Provider Extraction

把这些行为移动到 local provider boundary 后面:

- frozen snapshot read；
- prefetch；
- search；
- candidate submission；
- quality gate；
- safety gate；
- typed record write；
- markdown projection；
- session-end sync。

`MemoryManager` 应该变成 local + optional external providers 的 orchestrator，
而不是长期持有所有本地细节。

### P2.2 Scope Discipline

一个 authoritative `MemoryScope` 应该贯穿:

- retrieval；
- memory tool writes；
- auto extraction；
- background learning；
- session sync；
- provider hooks；
- subagent/forked contexts。

隔离或移除 `unbound-session` defaults。

### P2.3 测试

增加测试:

- provider fanout；
- provider failure isolation；
- session id propagation；
- project root propagation；
- subagent parent session id；
- local provider write/read/search parity；
- hostile persisted memory 仍被跳过。

### P2 验收标准

- local memory 行为能通过 provider contract 测试。
- 增加 external provider 不需要复制 local manager 逻辑。
- scope propagation failure 有测试可见。

## P3: Public/Product Wording Cleanup

目标: 避免对外过度描述仍处于 gated/prototype 状态的功能。

这项成本低，可以和 P0/P1 并行提前做，不必等所有工程工作结束。

需要在用户可见 docs 中说清楚:

- active memory 是 opt-in、本地、bounded、read-only active retrieval；
- skill evolution 会提出 candidates，但不会自动安装 trusted skills；
- subagents 是 scoped workers，不是无限制自治 agent；
- verified closeout 表示 runtime evidence 存在；
- partial/not verified 是合法且诚实的结果。

## 5. `failure_owner` 枚举规范

`failure_owner` 必须枚举化或至少规范化，否则 report 会出现近义字符串，最后
无法聚合。

建议枚举:

| Owner | 含义 |
| --- | --- |
| `intent_router` | 初始 route 或 recovery 判断错误 |
| `tool_exposure` | 工具面过窄/过宽导致任务失败 |
| `action_review` | action review 误杀或误放 |
| `permission` | 权限流程或恢复路径问题 |
| `model_planning` | 模型计划/执行选择错误，runtime 已正确反馈 |
| `tool_runtime` | 工具实现或执行系统失败 |
| `validation_command` | 验证命令本身不可用、错误或环境不满足 |
| `evidence_ledger` | evidence 记录/归类/rollup 错误 |
| `closeout` | closeout status 或 final wording 错误 |
| `context_assembly` | prompt/context zones 注入、去重、排序或权限错误 |
| `subagent` | 子 agent 生命周期、隔离、结果传递或验证问题 |
| `user_blocked` | 用户明确拒绝或阻止必要动作 |
| `external_environment` | 外部服务、依赖、网络、provider 或环境问题 |
| `harness` | eval harness、fixture、report parser 自身问题 |

## 6. 推荐实施顺序

1. P0a: 核心 runtime-spine deterministic matrix。
2. P1: gate outcome instrumentation，先从已有 trace 派生，不急着重构事件系统。
3. P1.5: proof semantics spec + additive fields。
4. P1: context zone convergence，建立 primary zone-first request envelope。
5. P1: route/task-mode recovery，加安全单调性原则。
6. P0b: 复杂 runtime-spine matrix，覆盖 subagent/memory/skill/permission/recovery。
7. P2: subagent verification-first policy，和 proof semantics 绑定。
8. P2: memory provider boundary cleanup。
9. P3: product wording cleanup，可并行提前做。

只有当 active memory 或 external memory 成为立即产品重点时，memory provider
work 才应该提前。否则 spine stability 优先。

## 7. 验证策略

纯文档/spec 变更:

```bash
git diff --check
```

deterministic runtime-spine 变更:

```bash
cargo test -q scenario_matrix
cargo test -q intent_router
cargo test -q route_scoped_tools
cargo test -q task_mode_score
cargo test -q closeout
cargo test -q evidence_ledger
python3 -m py_compile scripts/live_eval_report_parser.py
```

context 变更:

```bash
cargo test -q prompt_context
cargo test -q retrieval_context
cargo test -q runtime_diet
```

proof semantics 变更:

```bash
cargo test -q evidence_ledger
cargo test -q verification_proof
cargo test -q closeout
```

memory provider 变更:

```bash
cargo test -q memory
cargo test -q memory_provider
```

subagent 变更:

```bash
cargo test -q agent_tool
cargo test -q route_scoped_tools
cargo test -q evidence_ledger
```

广泛 runtime-spine batch 合并前:

```bash
scripts/coding-workflow-gates.sh standard
cargo clippy --all-features -- -D warnings
```

route recovery、proof semantics、subagent behavior 这类改动，在 deterministic
gates 通过后应该跑 live eval。

## 8. 成功标准

下一阶段成功的标志是: 一个真实任务失败时，可以在一屏里解释清楚:

- initial route 和 adjusted route；
- exposed tools 和 denied tools；
- gate decisions、outcomes 和 reasons；
- evidence collected；
- proof kinds；
- validation proof；
- closeout status；
- failure owner；
- UX friction；
- next recovery action。

产品体验目标不是“一个聪明模型和复杂 runtime 互相打架”，而是“一个有可靠执行
脊柱的本地编程伙伴”。目标不是最大自治，而是带诚实证据的可靠本地编程工作。

## 9. 执行记录

### 2026-05-26 第一批落地

已完成:

- 在 `src/engine/scenario_matrix.rs` 增加 `RuntimeSpineCaseSpec` 标准结构。
- 增加 P0a 核心 8 个 runtime-spine deterministic cases:
  - direct answer
  - read-only project audit
  - small code change
  - bug fix
  - user forbids writes
  - user specified validation
  - tool failure
  - no-diff audit closeout
- 增加 P0a case 的 route/tool/mutation/validation/closeout/final-answer oracle。
- 增加 UX friction budget 和 golden trace surface 要求。
- 增加 gate outcome 分类:
  - `protective_block`
  - `recoverable_friction`
  - `unrecovered_block`
  - `suspected_false_positive`
  - `policy_correct_but_ux_costly`
  - `harmless_pass`
- 增加 `src/engine/gate_outcome.rs`，先从现有 trace events 派生 gate outcome
  records 和 summary，不迁移 trace schema。
- 增加 `FailureOwner` 规范枚举，并兼容现有 `llm_reasoning`、`eval_harness`
  字符串映射。
- 在 `src/engine/verification_proof.rs` 增加 `VerificationProofKind` 和
  derived support policy。
- 在 `src/engine/evidence_ledger.rs` 给 validation evidence 增加 additive proof
  fields，并保持 `supports_verified` 由 policy/report 派生，不作为原始事实字段。
- 增加 runtime-spine 行为测试，证明 proof kind 能随 ledger evidence 进入
  `VerificationProof`。
- 增加 `scripts/runtime-spine-fast-gate.sh`，固定 P0a deterministic 快速验证入口。

已验证:

```bash
cargo fmt --check
cargo test -q scenario_matrix
cargo test -q gate_outcome
cargo test -q verification_proof
cargo test -q runtime_spine_behavior
cargo test -q evidence_ledger
cargo test -q closeout
cargo test -q task_mode_score
cargo test -q route_scoped_tools
python3 -m py_compile scripts/live_eval_report_parser.py
scripts/runtime-spine-fast-gate.sh
```

### 2026-05-26 第二批落地

已完成:

- 在 `scripts/live_eval_report_parser.py` 增加 gate outcome 派生逻辑，从现有
  `trace_summary` 中的 runtime events 直接生成报告字段，暂不迁移 trace schema。
- 派生覆盖的 gate:
  - `action_reviewed`
  - `permission_resolved`
  - `final_closeout_prepared`
- 派生输出的 outcome 继续沿用计划里的五类加 harmless pass:
  - `protective_block`
  - `recoverable_friction`
  - `unrecovered_block`
  - `suspected_false_positive`
  - `policy_correct_but_ux_costly`
  - `harmless_pass`
- 新增 report/summary/aggregate 字段:
  - `gate_outcomes`
  - `gate_outcome_records`
  - `gate_outcome_total`
  - `gate_outcome_protective_blocks`
  - `gate_outcome_recoverable_friction`
  - `gate_outcome_unrecovered_blocks`
  - `gate_outcome_suspected_false_positives`
  - `gate_outcome_policy_correct_but_ux_costly`
  - `gate_outcome_harmless_passes`
  - `gate_outcome_failure_owners`
- 在 `scripts/run_live_eval.sh` 的单任务 Quality signals 中输出 gate outcome
  字段。
- 在 run-level summary 的 Runtime Spine Evidence 区块增加 gate outcome 总数、
  分类计数和 per-task Gate Outcome Matrix。
- 在 `scripts/live-eval-aggregate-summary.sh` 中聚合 gate outcome tasks、
  records 和各类 outcome counts。
- 更新 `scripts/live-eval-summary-smoke.sh`，覆盖:
  - 从真实 `agent-events.jsonl` trace 派生 harmless pass；
  - 从 report text fallback 读取 protective/unrecovered gate outcome；
  - run summary 和 aggregate summary 的新增字段。

已验证:

```bash
python3 -m py_compile scripts/live_eval_report_parser.py
bash scripts/live-eval-summary-smoke.sh
scripts/runtime-spine-fast-gate.sh
```

第二批之后的状态:

- P0a deterministic code asset 已完成，并且已有 fast gate。
- P1 gate outcome 的第一阶段完成: 可以从 trace/report 派生，并进入
  live-eval report、run summary 和 aggregate summary。
- `suspected_false_positive` 和 `policy_correct_but_ux_costly` 目前仍需要
  `RuntimeSpineCaseSpec` / scenario oracle 提供判断依据；现阶段不会靠模型抱怨
  自动归类。
- 还没有把 gate outcome 反向接入 runtime 决策，只作为评估和诊断面。

### 2026-05-26 第三批落地

已完成:

- 在 `src/engine/verification_proof.rs` 增加 derived proof support rollup:
  - `VerificationProofStatus::Partial`
  - `VerificationProofSupportReport`
  - `supports_verified`
  - `residual_risk`
  - verified/partial/blocking proof kind 分组
- `EvidenceLedger` 继续只存事实，不在 evidence record 里写
  `supports_verified`。`supports_verified` 只由 proof policy 派生。
- `EvidenceLedger` 现在会根据 required/current validation rollup 自动补充
  policy proof kinds:
  - required command 全部通过 -> `RequiredValidationPassed` + `CommandPassed`
  - generic validation 通过且没有显式 proof kind -> `CommandPassed`
  - 显式 `DiffReviewed`、`SubagentClaimOnly` 等不会被 generic command proof
    静默升级
- `CloseoutEvaluator` 现在会把 derived support 应用到 closeout:
  - support verified -> 允许 verified/passed closeout
  - support partial -> passed 降为 partial
  - support blocked/unavailable/not-run -> passed 降为 not_verified/failed
  - residual risk 会写入 closeout risks
- `FinalCloseoutPrepared` trace 增加 proof support 字段:
  - `verification_proof_kind_summary`
  - `verification_proof_support_status`
  - `verification_proof_support_summary`
  - `verification_proof_supports_verified`
  - `verification_proof_residual_risk`
- live-eval report、run summary 和 aggregate summary 增加 proof support 指标:
  - proof support verified/partial/not-verified task counts
  - residual-risk task count
  - per-task Proof Support Matrix
- smoke fixture 覆盖:
  - command/required validation 支持 verified
  - diff-reviewed-only 只能 partial
  - report text fallback 也能进入 aggregate

已验证:

```bash
cargo test -q verification_proof
cargo test -q evidence_ledger
cargo test -q closeout
cargo test -q runtime_spine_behavior
cargo test -q trace_summary_includes_closeout_tool_record_count
python3 -m py_compile scripts/live_eval_report_parser.py
bash scripts/live-eval-summary-smoke.sh
```

第三批之后的状态:

- P1.5 的 proof support policy 已进入 closeout/report 主链路。
- closeout 现在能区分 proof status 和 proof-kind support:
  - proof status 说明 ledger/validation rollup 是否通过；
  - support status 说明这些 proof kinds 在当前任务上下文里能不能支撑 verified。
- `DiffReviewed` 这类 evidence 不再会因为 generic validation pass 被静默升级为
  verified support。
- subagent 相关 proof kinds 已有 policy 语义；第四批已开始和真实 subagent
  manager 输出绑定。

### 2026-05-26 第四批落地

已完成:

- `src/tools/agent_tool/mod.rs` 的真实 subagent 输出现在会携带结构化 proof
  metadata:
  - `proof_kind` / `verification_proof_kind`
  - `source_agent`
  - `parent_verified`
  - `subagent_output_kind`
  - `scope`
  - `related_to_changed_files`
  - `residual_risk`
- 单 agent、resume、parallel subtasks 和 fork branch 结果都会标注为
  `SubagentClaimOnly`，除非未来有父 runtime 复验链路显式写入
  `ParentVerifiedSubagentResult`。
- `subagent_output_kind` 先按输出性质归类:
  - `SubagentFinding`
  - `SubagentVerificationClaim`
  - `SubagentPatchSummary`
  - `SubagentBlocked`
- `src/engine/evidence_ledger.rs` 现在会读取 agent tool 的 proof metadata，
  把 child 输出记录为 validation fact，但 proof kind 仍是
  `SubagentClaimOnly`。
- agent tool 的 subagent proof 会进入 tool execution relevance:
  - `validation=true`
  - `closeout=true`
  这样 trace/closeout 诊断能看到子 agent claim 参与了证据面。
- `EvidenceLedger` 已覆盖两种关键语义:
  - child-only claim: ledger proof status 可以是 passed，但 derived support
    只能是 partial，不能支持 verified closeout。
  - parent-verified subagent result: 只有 `parent_verified=true` 且 proof kind
    是 `ParentVerifiedSubagentResult` 时，derived support 才能 scoped verified。
- `src/engine/scenario_matrix.rs` 增加 P0b extended deterministic matrix:
  - permission required
  - test failure repair
  - route mistake recovery
  - subagent verifier
  - isolated worktree implementer
  - memory retrieval conflict
  - skill guidance
- P0b cases 现在有 route/tool/mutation/validation/closeout/failure-owner/gate
  outcome/friction/golden-trace oracle，后续可以升级为 live-eval fixtures。

已验证:

```bash
cargo test -q scenario_matrix
cargo test -q agent_tool
cargo test -q evidence_ledger
cargo test -q verification_proof
cargo test -q gate_outcome
cargo test -q memory
bash scripts/runtime-spine-fast-gate.sh
cargo clippy --all-features -- -D warnings
```

第四批之后的状态:

- P2 Subagent Verification-First Policy 已完成第一段代码接线: child output
  不再是 opaque text，而是进入同一套 proof semantics。
- P0b 扩展矩阵已完成 deterministic spec，复杂控制面不再只停留在文档表格。
- 目前仍没有自动“父 runtime 复验生成 ParentVerifiedSubagentResult”的完整
  workflow；本批只把结构化入口和 policy gate 打好。
- 下一步应把 P0b spec 转成 live-eval fixture / golden trace，并把父 runtime
  复验动作变成显式 evidence-producing step。

后续保持:

- P0a live-eval 侧目前完成的是 parser/summary/aggregate 指标接线，还没有把
  P0a 八个 case 全部做成 live-eval sample fixtures。
- Gate outcome 已经进入 report/aggregate；下一步应结合 `RuntimeSpineCaseSpec`
  做 suspected false positive 和 UX-cost oracle。
- Proof support 已经进入 closeout/report；subagent proof binding 已完成第一段，
  下一步应该接 P0b live-eval fixtures 和父 runtime 复验 workflow。
- P0b、route recovery、context zone convergence、subagent parent verification
  workflow 和 memory provider boundary 仍按本计划后续推进。

### 2026-05-26 第五批落地

已完成:

- 增加 `runtime-spine-p0b` live-eval suite，可通过
  `scripts/run_live_eval.sh --list --case runtime-spine-p0b` 列出。
- 把 P0b 7 个复杂控制面场景落成 live-eval YAML fixtures:
  - `runtime-spine-p0b-permission-required`
  - `runtime-spine-p0b-test-failure-repair`
  - `runtime-spine-p0b-route-mistake-recovery`
  - `runtime-spine-p0b-subagent-verifier`
  - `runtime-spine-p0b-isolated-worktree-implementer`
  - `runtime-spine-p0b-memory-retrieval-conflict`
  - `runtime-spine-p0b-skill-guidance`
- `scripts/live_eval_report_parser.py` 的 runtime-spine assertion 语言新增:
  - `verification_proof_kind:<kind>`
  - `verification_proof_support_status:<status>`
  - `verification_proof_supports_verified:<true|false>`
- 新增 parser unit tests，证明:
  - child-only subagent claim 可以被 fixture 断言为
    `subagent_claim_only + partial + supports_verified=false`；
  - 只有 child claim 时，`parent_verified_subagent_result + verified`
    断言会失败。

已验证:

```bash
python3 -m py_compile scripts/live_eval_report_parser.py scripts/test_live_eval_report_parser.py
python3 -m unittest scripts.test_live_eval_report_parser
bash -n scripts/run_live_eval.sh scripts/live-eval-summary-smoke.sh
scripts/run_live_eval.sh --list --case runtime-spine-p0b
ruby -ryaml -e 'ARGV.each { |path| YAML.load_file(path); puts path }' evalsets/live_tasks/runtime-spine-p0b-*.yaml
```

第五批之后的状态:

- P0b 已经从 deterministic Rust spec 推进到 live-eval fixture asset。
- Subagent proof binding 现在能被 live-eval report parser 直接断言。
- 仍未完成真实父 runtime 复验 workflow；下一批应实现显式
  `ParentVerifiedSubagentResult` 生成路径，随后把对应 P0b fixture 从
  child-claim-only oracle 升级为 parent-verified oracle。

### 2026-05-26 第六批落地

已完成:

- 在 `src/engine/evidence_ledger.rs` 增加父 runtime 复验派生路径:
  - child subagent 结果仍先记录为 `SubagentClaimOnly`；
  - 只有当同一 ledger 里还有父 runtime 独立验证通过时，才派生
    `ParentVerifiedSubagentResult`；
  - 独立验证目前限定为 required validation 全部通过，或父进程记录的
    非 subagent validation command 通过。
- `VerificationProofSupportContext.parent_verified` 现在来自同一套 ledger
  事实，而不是 child 输出文字。
- 新增 ledger unit tests:
  - child claim + 父验证命令通过 -> proof kinds 同时包含
    `subagent_claim_only` 和 `parent_verified_subagent_result`，support
    可达 verified；
  - child claim + 非验证命令 -> 仍保持 partial，不能支持 verified。
- 更新 `runtime-spine-p0b-isolated-worktree-implementer` fixture:
  - oracle 从 child-claim-only partial 升级为 parent-verified proof；
  - 同时要求 `SubagentClaimOnly` 和 `ParentVerifiedSubagentResult` 出现在
    proof kinds；
  - support status 期望为 verified 且 `supports_verified=true`。

已验证:

```bash
cargo test -q evidence_ledger
ruby -ryaml -e 'ARGV.each { |path| YAML.load_file(path); puts path }' evalsets/live_tasks/runtime-spine-p0b-isolated-worktree-implementer.yaml
```

第六批之后的状态:

- P2 Subagent Verification-First Policy 的核心证据链已经闭环: child claim
  不能自行 verified，但父 runtime 验证可以显式升级为 parent-verified proof。
- 当前实现先使用“同一 ledger 内存在父验证证据”的保守判据；后续如果需要更硬，
  可以继续加入时间顺序、changed-file scope 和 child artifact id 绑定。
- 下一批建议转向 P1 route/task-mode recovery，或继续补 context zone
  deterministic ordering / provenance dedupe。

### 2026-05-26 第七批落地

已完成:

- 新增 `src/engine/route_recovery.rs`，把 route/task-mode recovery 做成独立
  runtime policy:
  - 记录 `HiddenReadSearchToolRequested` 和 `HiddenMutationToolRequested` 两类
    drift signal；
  - read/search recovery 只允许一次性扩大 `project_list`、`glob`、`grep`、
    `file_read`、`lsp`、`symbol_query`、`ask_user`；
  - mutation drift 只记录 recovery plan，不自动暴露 `file_edit`、`file_write`、
    `file_patch`、`format`、`install_dependencies`、`git`、`worktree` 等破坏性工具。
- `TurnRuntimeState` 增加 `route_recovery` 状态，用于跨 iteration 记住已启用
  的 bounded recovery。
- `TurnLoopBootstrapController` 现在同时保留:
  - route-scoped `base_tools`，用于初始请求；
  - permission-filtered `available_tools`，只在 recovery policy 允许时补入安全工具。
- `TurnIterationSetupController` 在下一轮 exposure plan 中应用 recovery:
  - 初始 route 不变；
  - 只有当 `read_search_expanded=true` 时，从 `available_tools` 补入安全读/搜工具；
  - 不补入 mutation/shell/install 等工具。
- `ToolBatchResultProcessor` 现在会从 unexposed-tool failure 中派生 recovery:
  - unexposed `file_read` / `grep` / `glob` 等安全工具 -> 生成
    `RecoveryPlan`，把 task mode 从 `Direct` 升级到 `Light`，并给下一轮
    暴露 read/search tools；
  - unexposed mutation tool -> 生成 `no_silent_mutation_expansion` recovery
    plan，要求 re-plan 或用户明确授权，不改变 tool exposure。

已验证:

```bash
cargo test -q route_recovery
cargo test -q turn_iteration_setup_controller
cargo test -q tool_batch_result_processor
cargo test -q route_scoped_tools
cargo test -q runtime_spine_behavior_tests
cargo fmt --check
cargo check -q
git diff --check
```

第七批之后的状态:

- P1 Route And Task-Mode Recovery 已完成第一段主链路: route drift 可以恢复到
  更强理解能力，但不会静默扩大写入/破坏能力。
- 目前 recovery 主要覆盖 unexposed-tool drift。后续可以继续加入:
  - code-change 多轮无 diff -> replan/repair owner；
  - action review 反复 revise/deny -> route/task-mode signal；
  - tool failure 暗示缺少 retrieval -> retrieval expansion；
  - route recovery metrics 进入 live-eval aggregate。

### 2026-05-26 第八批落地

已完成:

- route recovery 已进入 live-eval parser、run summary 和 aggregate summary:
  - 新增 `route_recovery_events`、`route_recovery_failure_types`、
    `route_recovery_kinds`、`route_recovery_read_search_expanded`、
    `route_recovery_mutation_blocked`、`route_recovery_safety_monotonic`、
    `route_recovery_unsafe_mutation_expansion`；
  - run summary 增加 Route Recovery 计数和逐任务 matrix；
  - aggregate summary 增加跨 run 计数和 Route Recovery Matrix。
- `runtime_spine_assertions` 支持 route recovery oracle:
  - `route_recovery_plan` 要求 runtime trace 里存在 `source=route_recovery`
    的 recovery plan；
  - `route_recovery_read_search_expanded` 要求出现
    `expand_read_search_only` / `hidden_read_search_tool_requested`；
  - `route_recovery_mutation_blocked` 要求出现
    `no_silent_mutation_expansion` / `hidden_mutation_tool_requested`；
  - `route_recovery_safety_monotonic` 要求 route recovery 没有把 mutation
    tool 放进 allowed alternatives，也没有写入/编辑/安装/git/worktree 类
    expansion kind。
- `score_live_eval_record` 增加 process penalty:
  - 如果 route recovery 出现 unsafe mutation expansion，summary 会用
    `route_recovery_unsafe_mutation_expansion` 暴露，并扣 process 分。
- `runtime-spine-p0b-route-mistake-recovery` fixture 现在显式要求:
  - route recovery plan 出现；
  - read/search expansion 出现；
  - safety monotonic 通过；
  - recovery kind 为 `expand_read_search_only`。
- `scripts/live-eval-summary-smoke.sh` 增加 synthetic route recovery trace，覆盖
  report、summary、aggregate 和 matrix 输出。

已验证:

```bash
python3 -m py_compile scripts/live_eval_report_parser.py scripts/test_live_eval_report_parser.py
bash -n scripts/run_live_eval.sh scripts/live-eval-aggregate-summary.sh scripts/live-eval-summary-smoke.sh
python3 -m unittest scripts.test_live_eval_report_parser
bash scripts/live-eval-summary-smoke.sh
scripts/run_live_eval.sh --list --case runtime-spine-p0b
ruby -ryaml -e 'ARGV.each { |path| YAML.load_file(path); puts path }' evalsets/live_tasks/runtime-spine-p0b-route-mistake-recovery.yaml
```

第八批之后的状态:

- 第七批的 route/task-mode recovery 不再只是 runtime 行为；现在 report 层可以
  看到它是否发生、是什么 kind、是否只扩大 read/search、是否保护了 mutation
  authority。
- P0b route mistake recovery 从“场景描述”升级为“可断言 oracle”，如果未来
  route recovery 没被触发或静默扩大了破坏性工具，live-eval 会直接暴露。
- 下一批建议转向 Context Zone Convergence 的 deterministic ordering /
  provenance dedupe，或补 code-change 多轮 no-diff -> route/task-mode recovery
  signal。

### 2026-05-26 第九批落地

已完成:

- `RetrievalContext` 的动态材料入口增加 deterministic ordering:
  - 排序不再只依赖 score；
  - 显式使用 score、conflict、trust、freshness、source priority、title、
    provenance、id 作为稳定 tie-breaker；
  - 相同 retrieval/memory/session/project 输入即使插入顺序不同，
    `format_for_prompt()` 也保持一致。
- `RetrievalContext::add_item` / `extend` 增加 provenance dedupe:
  - 对 normalized content preview 形成 fact-level dedupe key；
  - 同一事实从 memory、project、session 等多个来源进入时，只保留一个
    primary display item；
  - 其他来源合并到 `provenance` 的 `also=` 列表，而不是重复喂给模型；
  - token estimate 会按去重后的 primary item 重新计算，避免重复材料膨胀预算。
- primary 选择遵循保守 runtime 规则:
  - 更高 score 优先；
  - 非 conflict 优先；
  - trust 更高优先；
  - freshness/source priority 用作 tie-breaker；
  - workspace/project/file/tool/session 证据优先于 memory/web 的旧背景材料。
- 新增 retrieval context unit tests:
  - 同分项目/session items 反序插入仍得到同样顺序和 prompt；
  - memory + project + session 的同一事实只展示一次；
  - merged provenance 同时保留 primary project evidence 和 memory/session
    `also=` provenance；
  - 反序插入的 duplicate facts 也得到同一个 prompt。

已验证:

```bash
cargo test -q retrieval_context
cargo test -q context_assembly
cargo fmt --check
```

第九批之后的状态:

- P1.3 Deterministic Ordering 和 Provenance Dedupe 已在 retrieval/material
  入口落第一段主链路，后续 memory/project/session 混合输入不会因为来源顺序
  不稳定而改变 relevant material prompt。
- 这批还没有重写 request message 的 zone-first primary envelope；它先把
  最大 token 膨胀风险的 retrieval material 入口收稳。
- 下一批建议继续 Context Zone Convergence:
  - 将 `<relevant_material>` / `<recent_observation>` 多段 system message 合并为
    primary zone-first envelope；
  - 在 trace 里增加 dedupe count / provenance count；
  - 增加 hostile retrieved content fencing 的 request-preparation 回归测试。

### 2026-05-26 第十批落地

已完成:

- `RequestPreparationController` 增加 primary zone-first envelope:
  - 每次局部 request assembly 会扫描 dynamic context system messages；
  - 将 `<task-state>` / `<task_state>`、`<task-contract>`、`<context-pack>`、
    `<relevant_material>`、`<recent_observation>` 合并到单个
    `<context_zones ...>` system message；
  - envelope 插入在最后一个 user message 之前，不回写长期 conversation
    history，避免污染后续轮次；
  - 原有 stable system prompt 保持独立，不被 dynamic material 混入。
- dynamic material 现在显式标记为
  `policy="dynamic_background_not_system_policy"`，避免 retrieval/memory/tool
  observation 被误读成 stable policy。
- zone envelope 内部增加 block-level dedupe:
  - 相同 `<relevant_material>` 或 `<recent_observation>` block 多次出现时只保留一份；
  - context ledger 的 untagged guidance、MVA hint 等动态说明会被保存在
    task-state zone，而不是提升成 stable prefix。
- `TraceEvent::ContextZonesMaterialized` 增加 trace-visible metrics:
  - `zone_envelope_messages`
  - `zone_source_messages`
  - `zone_duplicate_blocks_removed`
  - `zone_provenance_markers`
- live-eval report 输出增加对应 context-zone metrics:
  - `context_zone_envelope_messages`
  - `context_zone_source_messages`
  - `context_zone_duplicate_blocks_removed`
  - `context_zone_provenance_markers`
- 新增 request-preparation 回归测试:
  - 多段 dynamic zone system messages 会合并成单个 envelope；
  - duplicate relevant material 会被去重并记录 trace metric；
  - stable prompt 里仅提到 zone tag 示例时不会被 envelope 归并误吞；
  - hostile retrieved content 仍留在 `<relevant_material>` 内，stable prefix
    fingerprint 与 relevant material fingerprint 分离；
  - task-state/task-contract/context-pack 仍在 envelope 内按 zone 保存。

已验证:

```bash
cargo test -q request_preparation_controller
cargo test -q context_assembly
cargo test -q prompt_context
cargo test -q runtime_spine_behavior_tests
python3 -m unittest scripts.test_live_eval_report_parser
python3 -m py_compile scripts/live_eval_report_parser.py && bash -n scripts/run_live_eval.sh
cargo check -q
bash scripts/live-eval-summary-smoke.sh
cargo fmt --check
git diff --check
```

第十批之后的状态:

- Context Zone Convergence 的 primary envelope 已经接入主请求准备路径；
  动态材料现在有更清晰的 role 和 trace 证据。
- P1.3 的 deterministic ordering / provenance dedupe 已覆盖 retrieval item
  入口和 request assembly zone block 两层。
- 下一批建议继续把 context zone metrics 接入 aggregate summary，或者开始做
  code-change 多轮 no-diff -> route/task-mode recovery signal。

### 2026-05-26 第十一批落地

已完成:

- context-zone envelope metrics 进入 run summary 和 aggregate summary:
  - run summary 增加 context-zone envelope/source/dedupe/provenance 总数；
  - Runtime Spine Evidence 表增加对应 rows；
  - 新增 per-task Context Zone Matrix；
  - aggregate summary 增加跨 run context-zone totals 和 Context Zone Matrix；
  - summary fallback parser 保留 context-zone 字段，避免只读 summary 时丢失。
- `scripts/live-eval-summary-smoke.sh` 增加 context-zone synthetic evidence，
  覆盖:
  - context-zone envelope task count；
  - source message count；
  - duplicate block removal count；
  - provenance marker count；
  - run summary 和 aggregate matrix 输出。
- route/task-mode recovery 增加 code-change no-diff drift signal:
  - 新增 `RouteRecoveryDriftSignal::CodeChangeNoDiffAfterRepeatedProgress`；
  - action checkpoint 在 code-change 多轮 no-diff / existing diff repair /
    focused repair stalled 时记录 `source=route_recovery` 的
    `code_change_no_diff_replan` recovery plan；
  - 该 recovery plan 不把 `file_edit` / `file_write` / `file_patch` 等
    mutation tools 放入 allowed alternatives，只要求在已有 task contract 下
    re-plan、targeted lookup 或 honest `not_verified` closeout；
  - live-eval parser 可用既有 `route_recovery_kind` oracle 断言
    `code_change_no_diff_replan`，并继续用 safety monotonic 检查 mutation
    authority 没有静默扩大。
- P3 产品文案收口落到 README:
  - active memory 是 opt-in/local/bounded/read-only retrieval；
  - skill evolution 只产生 reviewed candidates，不自动启用 trusted skills；
  - subagents 是 scoped workers，child claims 不是 verified proof；
  - `verified` closeout 代表 runtime evidence，`partial` / `failed` /
    `not_verified` 是合法诚实状态。
- Memory provider boundary 补一个实际 scope 清理:
  - streaming memory flush 不再 fallback 到 `unbound-session`；
  - 没有 persistent session id 时直接跳过 memory flush，避免无作用域写入。
- provider contract 测试补强:
  - `prefetch_all` 会收集可用 provider 的 records；
  - external provider prefetch failure 被隔离为 outcome，不会丢弃 local
    provider records；
  - provider 收到的 `MemoryScope` 保留 session id 和 project root。

已验证:

```bash
bash scripts/live-eval-summary-smoke.sh
python3 -m unittest scripts.test_live_eval_report_parser
cargo test -q route_recovery
cargo test -q action_checkpoint
cargo test -q streaming
cargo test -q provider
cargo test -q memory_scope
scripts/runtime-spine-fast-gate.sh
cargo check -q
python3 -m py_compile scripts/live_eval_report_parser.py
bash -n scripts/run_live_eval.sh scripts/live-eval-aggregate-summary.sh scripts/live-eval-summary-smoke.sh
```

第十一批之后的状态:

- Context Zone Convergence 的 trace/report/aggregate 观察面闭环完成；
  后续 context 改动可以直接看到 envelope 和 dedupe 是否漂移。
- Route recovery 现在覆盖两类主要 drift:
  - hidden read/search tool -> bounded read/search expansion；
  - code-change 多轮 no-diff -> trace-visible replan signal，不扩大 mutation
    authority。
- P3 wording cleanup 已有用户可见 README 边界说明。
- Memory provider boundary 的 `unbound-session` 普通路径已清理；local provider
  真实抽象仍可继续分阶段推进，但不再有 src 里的 `unbound-session` fallback。
