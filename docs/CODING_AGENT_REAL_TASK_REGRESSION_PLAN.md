# Coding Agent Real Task Regression Plan

> 目的：把“感觉编程能力变好了”变成可重复验证的工程指标。
>
> 当前项目已经有 `evalsets/`、`/eval`、`/benchmark`、workflow baseline
> 和 memory/skill/evolution gate，但这些更多验证路由、trace、结构契约。
> 下一步要补的是“真实编程任务回归”：让 agent 在真实仓库任务中反复执行、
> 记录、评分、对比，最终证明能力提升不是偶然。

## 1. 先说结论

真实任务回归不应该一开始就做成复杂 benchmark 平台。

第一版最重要的是：

1. 选一批真实、可复现、可人工判定的任务。
2. 给每个任务写清楚目标、初始状态、允许工具、验收标准。
3. 让 agent 实际跑一次，保存完整轨迹。
4. 人工评审是否成功，标注失败原因。
5. 后续每次核心改动后重跑同一批任务，对比指标。

也就是说，起点不是“自动化程度”，而是“样本质量”和“判定标准”。

## 2. 为什么不能只靠 cargo test

`cargo test` 只能说明代码没有破坏已有单元测试。

它不能回答这些问题：

- agent 是否先找对主线 blocker？
- 是否问了该问的问题？
- 是否误用记忆或召回无关内容？
- 是否选择了合适工具？
- 是否过早编辑、过度重构或绕开验收？
- 是否能从失败测试中恢复？
- 是否能把经验沉淀为 memory / skill，而不是污染长期上下文？

真实任务回归要测的是 agent 行为，而不只是 Rust 代码行为。

## 3. 推荐从三层评测开始

### Layer A: Contract Eval

已有基础：`evalsets/smoke.yaml`、`feature_reality.yaml`。

目标：

- 路由是否正确
- 工具推荐是否合理
- 权限/资源策略是否符合预期
- trace event 是否完整
- reflection / repair gate 是否触发

这层必须能在 CI 里稳定运行，不依赖真实 LLM。

### Layer B: Replay Eval

目标：

- 用录制的工具结果重放一段任务轨迹
- 验证 agent 在“工具失败、测试失败、权限拒绝、上下文压缩”等情况下的决策
- 不要求真的修改文件，但要验证状态机、ReflectionPass、LearningEvent、SkillProposal 是否正确生成

适合覆盖：

- 工具失败后是否进入 guided debugging
- 测试失败后是否阻断 closeout
- memory conflict 是否降低召回权重
- skill proposal 是否被 fitness gate 拦住

### Layer C: Live Task Eval

目标：

- 在临时工作区复制真实仓库
- 让 agent 真正读代码、改代码、跑测试
- 记录 diff、工具轨迹、最终回答和验收结果

这是最接近 Claude Code / Codex 实战能力的层。

第一版不需要全自动判分，可以人工评审；但必须保存轨迹，方便复盘和后续自动化。

## 4. 第一批任务怎么选

不要从“最难的大项目”开始。

建议第一批 20 个任务，按复杂度分层：

| 类型 | 数量 | 目的 |
| --- | ---: | --- |
| 简单修复 | 5 | 验证直接编辑、最小改动、基础测试 |
| 中等功能 | 6 | 验证读代码、计划、跨文件修改 |
| Bug 修复 | 4 | 验证复现、定位、修复、回归测试 |
| 重构任务 | 3 | 验证范围控制、避免过度重构 |
| 记忆/自进化任务 | 2 | 验证 memory recall、skill proposal、fitness gate |

每个任务都应该来自真实历史，而不是凭空编造。

推荐来源：

- 当前项目过去的 commit
- 之前用户真实提出的问题
- review finding 修复记录
- CLI UX 修复任务
- memory / evolution / permissions 这类高风险改动

## 5. 任务样本格式

建议新增 `evalsets/live_tasks/*.yaml`，第一版可以先不接自动 runner，
但格式要稳定。

```yaml
id: memory-save-quality-gate
title: memory_save should not bypass quality gate
type: bug_fix
complexity: medium
risk: high

repo:
  fixture: priority-agent
  base_ref: c235aff
  setup:
    - cargo test -q memory_quality

prompt: |
  修复 memory_save 绕过记忆质量门控的问题。
  模型调用 memory_save 时不能直接 override，只有用户显式 /save 才能降低阈值。

allowed_tools:
  - grep
  - file_read
  - file_edit
  - bash

forbidden_tools:
  - memory_clear
  - git_push

expected_behavior:
  - memory_save 走普通候选门控
  - explicit override 不再直接 Accepted
  - /save 能展示真实 outcome
  - 增加或更新相关测试

acceptance:
  required_commands:
    - cargo test -q memory_quality
    - cargo test -q -- --test-threads=1
  diff_constraints:
    max_files_changed: 6
    forbidden_paths:
      - target/
      - .git/
  human_review:
    - 是否保持“AI 判断 + 数学门控”的设计边界？
    - 是否没有把用户显式保存和模型工具保存混在一起？

scoring:
  success: 0.40
  minimal_diff: 0.15
  test_pass: 0.20
  correct_tool_use: 0.10
  no_goal_drift: 0.10
  closeout_quality: 0.05
```

## 6. 评测指标

第一版建议只保留 8 个核心指标。

| 指标 | 说明 |
| --- | --- |
| TaskSuccess | 最终是否满足验收标准 |
| TestPassRate | 目标测试和最终测试是否通过 |
| MainlineHitRate | 首个关键动作是否对准 blocker |
| PlanCoverage | 首轮计划是否覆盖关键步骤 |
| ReworkCount | 因错误方向或失败导致的返工次数 |
| ToolEfficiency | 工具调用是否合理，是否无意义重复 |
| DiffDiscipline | 是否只修改必要文件 |
| CloseoutAccuracy | 最终总结是否如实说明变更和测试 |

可以用一个简单总分：

```text
TaskScore =
  0.35 * TaskSuccess
+ 0.20 * TestPassRate
+ 0.15 * MainlineHitRate
+ 0.10 * PlanCoverage
+ 0.10 * DiffDiscipline
+ 0.05 * ToolEfficiency
+ 0.05 * CloseoutAccuracy
- 0.10 * NormalizedRework
```

注意：总分只是排序和趋势观察，不应该替代人工评审。

## 7. 人工评审怎么做

第一阶段人工评审非常重要，因为 agent 行为很多时候不能靠脚本完全判定。

每个任务跑完后，评审人填写：

```yaml
review:
  accepted: true
  task_success: 0.9
  test_pass_rate: 1.0
  mainline_hit: true
  plan_coverage: 0.8
  rework_count: 1
  tool_efficiency: 0.7
  diff_discipline: 0.9
  closeout_accuracy: 0.8
  failure_modes:
    - none
  notes: |
    首轮计划正确，但 grep 重复了一次。最终修改范围合理。
```

评审不是为了批评 agent，而是为了给后续 memory / skill / planner 的改进提供训练信号。

## 8. 和记忆、自进化怎么接上

真实任务回归应该成为 memory 和 self-evolution 的验证环境。

### Memory

每个任务记录：

- 使用了哪些记忆
- 召回记忆是否相关
- 是否有冲突记忆
- 记忆是否帮助减少工具调用或返工
- 是否有错误记忆污染计划

新增指标：

```text
MemoryRecallPrecision =
  relevant_recalled_memories / total_recalled_memories

MemoryUsefulness =
  tasks_helped_by_memory / tasks_with_memory_recall
```

### Skill

每个任务记录：

- 是否触发 skill
- skill 是否影响计划或工具选择
- skill 使用后成功率是否提高
- 是否减少返工和工具调用

skill promotion 不能只看“结构像不像 skill”，必须看真实任务回归：

```text
Promote if:
  NewFitness - OldFitness > threshold
  AND regression_rate == 0
  AND eval_count >= N
  AND live_task_score_not_worse
```

### Evolution

任何 prompt / workflow / skill 更新都应该绑定至少一个 eval 或 live task。

高风险变更必须回答：

- 它修复了哪个失败模式？
- 哪些任务证明它变好了？
- 哪些任务证明它没有变坏？
- 如何 rollback？

## 9. 起步路线

### Step 1: 建 20 个人工样本

从当前项目历史中选 20 个任务，写入 `docs/evals/live-task-catalog.md` 或
`evalsets/live_tasks/*.yaml`。

先只要求字段完整，不要求自动运行。

### Step 2: 手工跑 5 个任务

选择：

- 1 个简单修复
- 1 个中等功能
- 1 个 bug fix
- 1 个重构
- 1 个 memory/evolution 任务

每个任务保存：

- prompt
- agent trace
- tool calls
- diff
- test output
- closeout
- human review

### Step 3: 固化最小 runner

写一个 `scripts/run_live_eval.sh` 或 Rust runner：

1. 复制 fixture 到临时目录
2. checkout base ref
3. 启动 agent
4. 注入 prompt
5. 收集 trace/diff/test
6. 输出 report

第一版可以半自动：runner 准备环境，人手操作 agent，最后脚本收集结果。

当前已落地第一版脚本：

```bash
# 查看样本
scripts/run_live_eval.sh --list

# 为某个任务准备隔离 worktree、prompt 和 RUNBOOK
scripts/run_live_eval.sh --case memory-save-quality-gate --mode prepare

# 使用 MiniMax 调本项目 API，让模型先输出计划响应
scripts/run_live_eval.sh --case memory-save-quality-gate --mode api-plan

# 人工/agent 在 worktree 中完成任务后，收集 diff 和验收命令结果
scripts/run_live_eval.sh \
  --case memory-save-quality-gate \
  --mode collect \
  --workdir target/live-evals/<run-id>/memory-save-quality-gate/worktree \
  --run-tests
```

`api-plan` 和 `full` 模式要求 `MINIMAX_API_KEY`，并会强制通过 MiniMax provider
启动本项目 API。第一版的 `api-plan` 只验证模型计划与 API 调用链路，不声称已经
自动完成代码修改；真正的代码修改仍通过 prepared worktree 里的 interactive CLI
或后续更完整的 runner 执行。

当前基线：

- 2026-04-29 已扩展到 11 个 live task，覆盖 baseline、near-neighbor variant
  和 broader coding-agent workflow 三层。
- 严格计划门禁运行通过：`validation-round3-strict-final`，11/11 pass。
- 汇总报告：`docs/benchmarks/live-validation-round3-strict-final/summary.md`。
- `api-plan` lint 已能拦截 hidden reasoning / pseudo tool call / action-like
  plan closeout，例如 `let me run` 或 `ready to proceed with implementation`。
- `--case all` 已能正确传播单个任务失败，不会在部分失败后仍返回成功。

### Step 4: 接入评分报告

生成 `docs/benchmarks/report-live-task-YYYYMMDD.md`：

- 每个任务 pass/fail
- 总分趋势
- 最常见失败模式
- memory recall precision
- skill fitness delta
- 和上一次报告对比

### Step 5: 用结果驱动开发

只有当 live task report 显示某个失败模式稳定存在时，再改 planner/memory/skill。

这能避免“凭感觉继续堆功能”。

## 10. 第一批建议任务

可以直接从这些任务开始：

1. 修复 `memory_save` 质量门控绕过。
2. 修复 `/save` 成功提示与真实 outcome 不一致。
3. 修复 persistent memory 没进入 workflow planning。
4. 给 `skill-proposals apply` 增加 promotion gate。
5. 给 EvolutionController 增加持久 cooldown。
6. 优化 CLI 启动欢迎区和状态栏。
7. 修复 CLI 用户消息重复显示。
8. 增加 `/resume` 历史会话选择。
9. 增加一个 read-only tool 的参数完整性保护。
10. 增加一个权限策略回归任务：低风险默认放行，危险删除必须确认。
11. 新增一个小工具模块，要求 schema、validate、tests、registry 全链路。
12. 修改一个已有工具的输出格式，要求不破坏 trace 和 CLI 展示。
13. 新增一个 memory doctor 报告字段。
14. 新增一个 skill fitness 统计字段。
15. 修复一个无关高置信记忆影响计划权重的 case。
16. 修复一个 conflict memory 误降级有效记忆的 case。
17. 对一个中型模块做小范围重构，要求 diff 限制。
18. 在失败测试后触发 ReflectionPass 阻断 closeout。
19. 让 agent 在信息不足时提出 1-3 个关键问题，而不是直接改。
20. 让 agent 对一个前端/CLI 体验任务先做短计划，再执行和验收。

这些任务的好处是：我们已经有历史上下文，知道正确答案和常见错误，适合做第一批回归。

## 11. 最小验收标准

第一阶段完成的标志不是自动化多强，而是：

- 至少 20 个 live task 样本。
- 至少 5 个任务有完整人工评审记录。
- 每个任务都有明确 base ref、prompt、acceptance、allowed tools。
- 至少生成 1 份 live task report。
- 能从 report 中得出 3 个可执行改进项。

第二阶段再追求：

- 自动复制 fixture。
- 自动运行 required commands。
- 自动收集 diff 和 trace。
- 自动计算 TaskScore。
- 自动对比前后版本。

## 12. 我的建议

不要先追求“像 SWE-bench 那样完整”。

对我们现在最有价值的是一个小而真实的回归集：

```text
20 个真实任务
5 个先人工跑通
每周固定重跑
每次核心改动都对比报告
失败模式进入开发计划
```

这套机制建立起来之后，Priority Agent 的改进才会从“功能越来越多”
变成“编程表现可证明地变好”。
