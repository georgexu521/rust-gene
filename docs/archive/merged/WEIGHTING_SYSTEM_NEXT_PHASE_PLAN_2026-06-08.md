# Weighting System Next Phase Plan — 2026-06-08

本文档接在 `docs/WEIGHTING_SYSTEM_AUDIT_2026-06-08.md` 之后，目标是把
weighting system 从“代码和观测点已补齐”推进到“有可重复校准证据、可判断系数
是否有效、可安全进入日常 gate”。

当前已经完成的基础：

- `MemoryRecallScored`、`MemoryWriteScored`、`MemoryKeepScored` 已接入真实
  call site。
- `/trace` summary 已有统一 `Scoring:` 行。
- `CandidateAction` 默认降噪，shadow/gated 需要显式 env 开启。
- `task-guidance` 最小实验已实现，默认关闭。
- P0 五个 live task YAML 已落地。
- `scripts/weighting-calibration-gate.sh` 已覆盖当前已实现的窄回归测试。

下一阶段不应该先改系数。先要拿到真实运行证据，确认 scoring trace、runtime
gate、failure owner 和 closeout 证据是否能解释 agent 行为。

---

## 目标

### 主要目标

1. 跑通 P0 五个真实 live eval，得到 trace 和失败归属。
2. 把 P1/P2 十个 case 补成可执行 YAML。
3. 建立 weighting calibration report，能对每次系数或 gate 变更做 AB 对比。
4. 把稳定的 weighting gate 接入日常产品 gate，但不把未稳定 live eval 误标为
   release blocker。

### 非目标

- 不做自动调参。
- 不把软分数变成新的硬拦截，除非它映射到权限、预算、证据、范围、检查点或
  高风险安全门。
- 不用更长的 always-on prompt 修补单个 provider 的错误。
- 不合并 `workflow_contract.rs` 和 legacy `workflow/weights.rs`。现阶段只保留
  “不同路径、不同责任、legacy maintenance-only”的结论。

---

## Phase 0: 固定基线

目的：保证后续 eval 失败能归因到 agent flow、harness、provider 或具体代码变更，
而不是工作树漂移。

### 工作项

1. 记录当前 git 状态和 diff 摘要。
2. 跑当前 focused gate。
3. 跑一次基础编译检查。
4. 确认五个 P0 YAML 能被 harness 发现、能被 YAML parser 读取。

### 命令

```bash
git status --short
git diff --stat
bash scripts/weighting-calibration-gate.sh
cargo check -q
bash scripts/run_live_eval.sh --list | rg 'weighting-p0'
ruby -e 'require "yaml"; ARGV.each { |path| YAML.load_file(path) }' evalsets/live_tasks/weighting-p0-*.yaml
```

### 验收

- focused gate 通过。
- `cargo check -q` 通过，允许现存 warning，但不允许新增 error。
- 五个 `weighting-p0-*` case 全部出现在 `--list` 输出中。
- YAML parse 无异常。

### 输出

- 在后续 calibration report 中记录 baseline commit、dirty diff 摘要、命令结果。

---

## Phase 1: 跑 P0 真实 live eval

目的：验证最关键的 safety/budget/memory/risk 路径在真实 agent-run 下是否可解释。

### P0 case

| Case | 文件 | 验证重点 |
|------|------|----------|
| C1 | `weighting-p0-premature-edit-revise.yaml` | 未读文件直接改动时，ActionReview 能 revise；最终有读取证据 |
| C2 | `weighting-p0-high-risk-bash-ask-user.yaml` | 高风险 bash 不应静默 allow |
| C3 | `weighting-p0-budget-exceeded-deny.yaml` | 预算耗尽后 deny，而不是继续循环 |
| C6 | `weighting-p0-memory-write-low-quality-rejected.yaml` | 低质 memory write 不 accepted，且有 `MemoryWriteScored` |
| C10 | `weighting-p0-high-risk-contract-activation.yaml` | 高风险 code-change 激活 workflow contract |

### 命令

```bash
bash scripts/run_live_eval.sh --case weighting-p0-premature-edit-revise --mode agent-run --run-tests
bash scripts/run_live_eval.sh --case weighting-p0-high-risk-bash-ask-user --mode agent-run --run-tests
bash scripts/run_live_eval.sh --case weighting-p0-budget-exceeded-deny --mode agent-run --run-tests
bash scripts/run_live_eval.sh --case weighting-p0-memory-write-low-quality-rejected --mode agent-run --run-tests
bash scripts/run_live_eval.sh --case weighting-p0-high-risk-contract-activation --mode agent-run --run-tests
```

### 每个 case 必须记录

- provider / model / runtime profile。
- pass、fail、partial 或 not_verified。
- `failure_owner`：`agent_flow`、`harness`、`provider`、`fixture`、`product_gap`
  之一。
- 关键 trace event 是否存在：
  - `action_decision`
  - `action_review`
  - `tool_observation`
  - `memory_write_scored` 或 workflow/risk 相关事件
  - `completion_contract`
- 最终 closeout 是否有真实证据，不能只有口头完成声明。

### 失败处理规则

- 如果 gate 正确拒绝危险动作，provider 仍然反复绕过，这是 provider 行为问题或
  bounded repair 问题，不要弱化 gate。
- 如果 required command 通过但 closeout 缺少证据，归为 closeout/flow 问题。
- 如果 trace 缺少关键 scoring event，归为 observability/product_gap。
- 如果 YAML 的断言无法被现有 harness 表达，归为 harness 问题，先修 harness 或
  调整断言字段，不把 case 标绿。

### 验收

- P0 五个 case 至少得到完整运行记录。
- 所有 fail/partial 都有明确 `failure_owner`。
- 不出现 false green closeout。
- 不为了通过弱 provider 而降低权限、预算、检查点、验证或高风险门。

---

## Phase 2: 修 P0 暴露的问题

目的：只修真实运行暴露出的产品问题，不根据预想继续加复杂权重。

### 修复优先级

1. False green closeout。
2. 安全门误 allow。
3. 预算门不生效或循环无法停止。
4. trace event 缺失，导致无法解释行为。
5. harness 无法表达已有行为断言。
6. provider 特定误判。

### 修复原则

- 硬约束问题优先用 deterministic gate、tool contract、required evidence 修。
- LLM 行为偏差优先通过 structured observation 回流和 bounded repair 修。
- prompt 只做短事实 guidance，不加长规则墙。
- scoring 系数只有在 P0 evidence 证明误排序或误导时才调整。

### 验收

修复后至少重跑：

```bash
bash scripts/weighting-calibration-gate.sh
cargo check -q
bash scripts/run_live_eval.sh --case <fixed-case-id> --mode agent-run --run-tests
```

如果修复影响 action review、memory、workflow 或 closeout 公共路径，再加跑相关
窄测试：

```bash
cargo test -q closeout
cargo test -q memory_tool -- --test-threads=1
cargo test -q route_scoped_tools
cargo test -q prompt_context
```

---

## Phase 3: 补 P1/P2 十个 live task

目的：把 audit 文档里的 15-case 校准集补完整，让 weighting system 有长期回归
样本。

### P1 cases

| Case | 建议文件名 | 重点 |
|------|------------|------|
| C4 | `weighting-p1-verified-closeout-allowed.yaml` | 修改 + 验证通过后 closeout 被接受 |
| C7 | `weighting-p1-memory-write-high-quality-accepted.yaml` | 高质量记忆 accepted，trace 有 score/threshold |
| C8 | `weighting-p1-memory-recall-budget-capped.yaml` | 相关记忆很多时，retrieval context 不过量注入 |
| C13 | `weighting-p1-tool-failure-weight-feedback.yaml` | tool failure 后 workflow feedback/reweight 有 trace |

### P2 cases

| Case | 建议文件名 | 重点 |
|------|------------|------|
| C5 | `weighting-p2-repeated-noop-action-revised.yaml` | 重复无效动作触发低价值 revise |
| C9 | `weighting-p2-conflicting-memory-recall-capped.yaml` | 冲突记忆降权或不 inject |
| C11 | `weighting-p2-ordinary-qa-risk-ordinary.yaml` | 普通问答不被误判为高风险 |
| C12 | `weighting-p2-failed-validation-risk-escalates.yaml` | 验证失败后 risk 升级 |
| C14 | `weighting-p2-task-guidance-includes-risk.yaml` | env 开启时 guidance 包含风险事实 |
| C15 | `weighting-p2-task-guidance-default-off.yaml` | 默认不注入 guidance |

### YAML 编写要求

每个 YAML 至少包含：

- `id`
- `title`
- `eval_intent`
- `risk`
- `runtime_profile`
- `repo.fixture`
- `prompt`
- `allowed_tools`
- `forbidden_tools`
- `expected_behavior`
- `behavior_assertions`
- `runtime_spine_assertions`
- `acceptance.required_commands`
- `acceptance.diff_constraints`
- `acceptance.human_review`
- `scoring`

### 断言要求

- 每个 case 至少有一个行为断言和一个 runtime-spine 断言。
- 不只验证“命令成功”，还要验证 trace 是否能解释 agent 为什么这么做。
- Memory case 必须检查 score、threshold、decision/status。
- Risk/workflow case 必须检查 risk level 或 workflow activation/progress。
- task-guidance case 必须覆盖默认关闭和显式开启两条路径。

### 验收

```bash
bash scripts/run_live_eval.sh --list | rg 'weighting-p[12]'
ruby -e 'require "yaml"; ARGV.each { |path| YAML.load_file(path) }' evalsets/live_tasks/weighting-p*.yaml
```

P1/P2 YAML 全部可被发现、可 parse。真实 agent-run 可以分批跑，不要求同一提交内
全部稳定通过。

---

## Phase 4: Calibration report

目的：把每次 live eval 的结果沉淀成可比较记录，而不是只看一次终端输出。

### 建议新增文档

`docs/WEIGHTING_SYSTEM_CALIBRATION_REPORT_2026-06-08.md`

### Report 结构

```markdown
# Weighting System Calibration Report — 2026-06-08

## Baseline
- commit:
- provider:
- runtime_profile:
- dirty_diff_summary:

## Summary
| Priority | Total | Pass | Partial | Fail | Not Verified |
|----------|-------|------|---------|------|--------------|

## Case Results
| Case | Status | failure_owner | Key evidence | Follow-up |
|------|--------|---------------|--------------|-----------|

## Scoring Observations
- Action:
- Memory:
- Workflow:
- Risk:

## Fix Queue
1.

## AB Notes
- No coefficient changes in this run.
```

### 记录原则

- `partial` 和 `not_verified` 是有效结果，不要强行归为 pass/fail。
- 如果 provider 做错但 runtime 给出诚实失败证据，不算 runtime flow 失败。
- 如果 product gate 通过但 closeout 证据不足，不能标绿。
- 每个建议修复必须绑定到 case id 和 trace 证据。

---

## Phase 5: Gate 集成

目的：让 weighting calibration 成为日常开发保护网，但避免慢 eval 阻塞所有开发。

### 分层 gate

#### Tier 1: 每次相关改动必跑

```bash
bash scripts/weighting-calibration-gate.sh
cargo check -q
```

适用范围：

- `src/engine/action_*`
- `src/engine/conversation_loop/*`
- `src/engine/trace/*`
- `src/memory/*`
- `src/tools/memory_tool/*`
- `evalsets/live_tasks/weighting-*`

#### Tier 2: 改动触及 runtime spine 时跑

```bash
cargo test -q closeout
cargo test -q route_scoped_tools
cargo test -q prompt_context
cargo test -q instructions
```

#### Tier 3: 每日或发布前跑

```bash
bash scripts/run_live_eval.sh --case weighting-p0-premature-edit-revise --mode agent-run --run-tests
bash scripts/run_live_eval.sh --case weighting-p0-high-risk-bash-ask-user --mode agent-run --run-tests
bash scripts/run_live_eval.sh --case weighting-p0-budget-exceeded-deny --mode agent-run --run-tests
bash scripts/run_live_eval.sh --case weighting-p0-memory-write-low-quality-rejected --mode agent-run --run-tests
bash scripts/run_live_eval.sh --case weighting-p0-high-risk-contract-activation --mode agent-run --run-tests
```

P1/P2 全量 live eval 先作为 calibration suite，不直接 release-blocking。等连续多轮
稳定后，再挑选最可靠的 case 进入发布 gate。

### 验收

- 文档明确哪些命令是必跑，哪些是每日/发布前跑。
- product gate 不依赖未稳定 provider 行为。
- release-blocking 只绑定硬安全、预算、证据和 closeout 真实性。

---

## Phase 6: 系数 AB 策略

目的：为后续调整 `ActionDecision`、memory 或 workflow 权重系数建立规则。

### 什么时候允许改系数

只有满足以下至少一个条件才改：

- 多个 case 显示同一类动作长期被错误排序。
- scoring summary 与最终正确行为稳定冲突。
- memory write/recall/keep 的 threshold 导致明显误收或误拒。
- workflow step importance 导致 agent 连续跳过关键验证或修复步骤。

### AB 方法

1. 固定同一组 cases、provider、runtime profile。
2. A 组跑旧系数，B 组跑新系数。
3. 比较：
   - pass/partial/fail/not_verified 分布
   - false green closeout 数量
   - safety deny/revise 命中率
   - memory accepted/proposed/rejected 分布
   - tool count 和 repair count
4. 只有 B 组减少产品级失败，且不增加 false green/safety bypass，才合并。

### 禁止项

- 禁止只因为单个 provider 一次误判就调全局系数。
- 禁止为了减少 fail 而降低验证、权限、预算或检查点门槛。
- 禁止把 `not_verified` 包装成 pass。

---

## 推荐执行顺序

1. Phase 0：固定基线。
2. Phase 1：跑 P0 五个真实 live eval。
3. Phase 2：修 P0 暴露出的 product/harness 问题。
4. Phase 3：补 P1/P2 YAML。
5. Phase 4：写第一版 calibration report。
6. Phase 5：把 Tier 1 gate 作为相关改动的固定检查。
7. Phase 6：只有在 calibration report 有证据后，再考虑系数 AB。

最重要的判断标准：这套系统不是为了让每个 provider 都绿，而是为了让 agent 在
风险、证据、预算、记忆和 closeout 上保持可解释、可验证、可复现。真实失败可以
接受，false green 不可以接受。
