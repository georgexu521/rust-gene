# Conversation Loop 控制器合并计划

日期：2026-06-08

## 结论

`conversation_loop` 的文件数确实偏多，但原计划里的基线和若干合并方向已经和当前代码不匹配。当前应该先做小而可验证的收敛：合并纯状态、小型薄包装器、同一入口前后的上下文准备逻辑；不要为了文件数把权限、请求、主循环、retrieval、workflow 等边界硬塞进大文件。

更合理的目标是先从当前约 99 个文件降到 87-89 个文件，而不是一步压到 79 个。等第一轮合并后，再根据模块实际责任决定是否继续。

## 当前代码事实

统计命令：

```bash
find src/engine/conversation_loop -maxdepth 2 -type f | wc -l
find src/engine/conversation_loop -maxdepth 2 -type f -print0 | xargs -0 wc -l | tail -n 1
find src/engine/conversation_loop -maxdepth 2 -type f -not -name '*tests.rs' | wc -l
find src/engine/conversation_loop -maxdepth 2 -type f -not -name '*tests.rs' -print0 | xargs -0 wc -l | tail -n 1
```

结果：

| 口径 | 文件数 | 行数 |
|------|--------|------|
| `conversation_loop` 下全部文件，含嵌套目录和测试 | 99 | 44,355 |
| 排除 `*tests.rs` 后 | 93 | 38,194 |
| 顶层文件 | 90 | - |
| 二级子模块文件 | 9 | - |

最大文件：

| 文件 | 行数 | 备注 |
|------|------|------|
| `permission_controller.rs` | 1,420 | 已接近 1,500 行上限，不能再合并更多逻辑进去 |
| `api_request_controller.rs` | 980 | LLM 请求路径边界，应保持独立 |
| `tool_batch_result_processor.rs` | 896 | 工具结果处理热点，后续更适合拆小而不是并入 |
| `session_processor.rs` | 750 | session/streaming 入口核心，不适合塞更多策略 |
| `turn_iteration_controller.rs` | 676 | 主迭代编排，合并时要避免继续变成大杂烩 |

## 原计划主要问题

1. 基线过期：当前不是 94 个文件，而是 99 个文件；最终 `94 - 15 = 79` 的目标不成立。
2. `permission_recovery.rs` 不应该并入 `permission_controller.rs`。后者已有 1,420 行，再加 133 行会超过项目“源文件低于 1,500 行”的约束。
3. `tool_failure_stop_controller.rs` 目前只在 `#[cfg(test)]` 下声明，生产路径没有使用。它不应该被当成生产工具失败管线的一部分合并。
4. `force_summary.rs` 不是普通配置文件，它包含强制总结 prompt 和 LLM 总结逻辑；把它描述为“配置整合”不准确。
5. `runtime_timeouts.rs` 同时被 `api_request_controller.rs` 和 `session_processor.rs` 使用，直接内联到其中一个会让另一个反向依赖变难看。
6. 把 4 个 workflow 文件合成一个 `workflow_helpers.rs` 会丢失语义边界。`workflow_prompt_policy`、`workflow_runtime`、`workflow_trace`、`workflow_change_tracker` 现在至少有不同变更原因。

## 合并原则

- 单次合并只处理一个局部责任，不跨权限、请求、工具执行、retrieval、closeout 大边界。
- 合并后单文件保持在 1,500 行以下，最好低于 800 行。
- 优先消除只有结构体转发、单函数包装、同一调用点前后连续使用的小文件。
- 保留能表达运行时边界的文件，即使文件数量暂时更多。
- 每个阶段单独提交，方便回滚。

## 修订后的阶段计划

### Phase 0：基线和护栏

目标：先建立可比较的事实，不改行为。

操作：

1. 记录当前文件数、行数和最大文件。
2. 跑基线验证：
   ```bash
   cargo check -q
   cargo fmt --check
   cargo test -q route_scoped_tools
   cargo test -q closeout
   cargo test -q prompt_context
   ```
3. 每个后续阶段只 stage 对应文件，避免把无关 dirty tree 混进提交。

### Phase 1：低风险薄文件合并

这些合并基本不改变控制流，风险最低。

#### 1.1 合并 turn 状态文件

目标：创建 `turn_state.rs`，合并：

| 文件 | 当前行数 | 说明 |
|------|----------|------|
| `turn_loop_state_controller.rs` | 52 | `TurnLoopState` 和默认工厂 |
| `turn_runtime_state.rs` | 55 | `TurnRuntimeState`、`FocusedRepairRuntimeState` |
| `turn_runtime_context.rs` | 93 | `TurnRuntimeContext`、`SessionRuntimeState` |

预期：消除 2 个文件，得到约 200 行的状态模块。

注意：更新所有 `use super::turn_loop_state_controller`、`turn_runtime_state`、`turn_runtime_context` 的导入。

#### 1.2 内联迭代预算控制器

目标：把 `iteration_budget_controller.rs` 内联到 `tool_round_controller.rs`。

理由：当前只有 `ToolRoundBudgetOutcome` 和 `record_tool_round()`，唯一生产调用点在 `tool_round_controller.rs`。

预期：消除 1 个文件。

#### 1.3 内联工具暴露计划

目标：把 `tool_exposure_plan.rs` 内联到 `turn_iteration_setup_controller.rs`。

理由：`ToolExposurePlan::build()` 只服务 iteration setup，合并后文件约 237 行，仍然清晰。

预期：消除 1 个文件。

#### 1.4 合并 turn entry 的两个小控制器

目标：把以下文件内联到 `turn_entry_gate_controller.rs`：

| 文件 | 当前行数 | 说明 |
|------|----------|------|
| `session_goal_controller.rs` | 82 | 会话目标更新 |
| `task_context_trace_controller.rs` | 160 | task context trace 记录 |

理由：两个控制器都只服务 turn entry gate，合并后 `turn_entry_gate_controller.rs` 约 585 行，仍低于风险阈值。

预期：消除 2 个文件。

#### 1.5 内联 runtime diet bootstrap

目标：把 `turn_runtime_diet_bootstrap_controller.rs` 内联到 `turn_loop_bootstrap_controller.rs`。

理由：它只在 loop bootstrap 阶段观察 runtime diet、retrieval context 和技能摘要。

预期：消除 1 个文件。

Phase 1 预期合计：消除 7 个文件。

推荐验证：

```bash
cargo fmt --check
cargo check -q
cargo test -q route_scoped_tools
cargo test -q prompt_context
cargo test -q closeout
cargo test -q conversation_loop
```

### Phase 2：中等风险管线合并

这些合并会触及跨文件调用链，必须小步提交。

#### 2.1 收敛 retrieval 构建和注入

推荐做法：

1. 把 `retrieval_context_builder.rs` 内联到 `turn_retrieval_context_controller.rs`。
2. 把 `retrieval_prompt_controller.rs` 内联到 `turn_request_bootstrap_controller.rs`，而不是塞进 retrieval build controller。

理由：retrieval build 和 prompt injection 是两个阶段。前者负责收集 project/session/memory context，后者负责把结果放进 request messages。这样可以减少文件数，同时保留阶段边界。

预期：消除 2 个文件。

#### 2.2 合并工具轮次结果映射

推荐做法：把 `turn_tool_round_outcome_controller.rs` 合并进 `turn_tool_round_step_controller.rs`。

理由：`TurnToolRoundState` 是 step 层的返回值，`TurnToolRoundOutcomeController::from_batch()` 只是把 batch outcome 转成 step outcome。不要把它并入 `tool_round_controller.rs`，因为后者负责执行工具 batch，不应该再承担 step-level closeout 状态。

预期：消除 1 个文件。

#### 2.3 简化工具失败 follow-up

推荐做法：

1. 把 `turn_tool_failure_followup_controller.rs` 的薄包装逻辑并入 `tool_failure_guided_debugging.rs`。
2. 单独评估 `tool_failure_stop_controller.rs`：它现在是 `#[cfg(test)]` 模块且生产路径无引用，若保留测试价值不足，可以删除；否则保留为 test-only advisory fixture。

预期：消除 1-2 个文件。

#### 2.4 谨慎处理 post-change closeout

`turn_post_change_closeout_controller.rs` 可以考虑并入 `turn_iteration_controller.rs`，但这个阶段要单独做。

风险：`turn_iteration_controller.rs` 当前 676 行，合并后约 893 行，行数可接受；但 closeout 是验证/proof 边界，不应和主循环逻辑混到难以审查。

建议：除非它确实只是顺序编排，否则先保留。

Phase 2 预期合计：消除 4-5 个文件。

### Phase 3：暂缓或改方向

这些原计划项不建议按原方案执行。

#### 3.1 不合并权限恢复到权限控制器

原方案：`permission_recovery.rs` 并入 `permission_controller.rs`。

修订：不要做。`permission_controller.rs` 已经 1,420 行，正确方向是后续把它拆成 `permission_controller/` 子模块，例如：

```text
permission_controller/
  mod.rs
  evaluation.rs
  approval.rs
  recovery.rs
  tests.rs
```

这会增加或持平文件数，但会改善可维护性，符合项目文件行数约束。

#### 3.2 不把 runtime timeouts 内联到 session/api

原方案：`runtime_timeouts.rs` 内联到 `session_processor.rs` 或 `api_request_controller.rs`。

修订：保留独立文件，或改名为 `request_timeouts.rs`。它同时服务 LLM request timeout 和 stream idle timeout，放进任一调用方都会制造不自然依赖。

#### 3.3 不创建泛化的 workflow_helpers.rs

原方案：合并 `workflow_trace.rs`、`workflow_runtime.rs`、`workflow_prompt_policy.rs`、`workflow_change_tracker.rs`。

修订：不要创建泛化 helpers。可以单独评估：

| 文件 | 建议 |
|------|------|
| `workflow_prompt_policy.rs` | 保留，prompt policy 是明确边界 |
| `workflow_runtime.rs` | 保留，runtime activation/learning event 是明确边界 |
| `workflow_trace.rs` | 可和具体调用方小步合并，但不急 |
| `workflow_change_tracker.rs` | 可移到更贴近 git/change tracking 的模块，但不应和 prompt/runtime 混合 |

#### 3.4 loop profile 和 force summary 可后置合并

`main_loop_profile.rs` 和 `force_summary.rs` 都服务 loop policy，后续可以合成 `turn_loop_policy.rs`。

不要把 `force_summary.rs` 当成普通配置，它包含 prompt 和 LLM fallback summary 逻辑。若合并，必须保留现有测试：

```bash
cargo test -q force_summary
```

预期：最多消除 1 个文件。

## 修订后预期结果

| 阶段 | 预期消除文件数 | 累计 |
|------|----------------|------|
| Phase 1 | 7 | 7 |
| Phase 2 | 4-5 | 11-12 |
| Phase 3 | 0-1 | 11-13 |

以当前 99 个文件为基线，第一轮合理目标是：

```text
99 - 11 ~= 88 个文件
```

如果排除 `*tests.rs`，则大约从 93 个非测试文件降到 81-83 个。

## 执行顺序

1. Phase 1.1：`turn_state.rs`，只移动状态和上下文类型。
2. Phase 1.2：`iteration_budget_controller.rs` 内联。
3. Phase 1.3：`tool_exposure_plan.rs` 内联。
4. Phase 1.4：entry gate 小控制器合并。
5. Phase 1.5：runtime diet bootstrap 内联。
6. Phase 2.1：retrieval build/injection 收敛。
7. Phase 2.2：tool round outcome 合并。
8. Phase 2.3：tool failure follow-up 简化。
9. 重新统计文件数、最大文件和验证耗时，再决定是否做 Phase 3。

## 每个提交的验证清单

- [ ] `cargo fmt --check`
- [ ] `cargo check -q`
- [ ] 与合并区域相关的窄测试通过
- [ ] 如果涉及 route/tools/closeout，运行：
  ```bash
  cargo test -q route_scoped_tools
  cargo test -q closeout
  ```
- [ ] 如果涉及 prompt/context，运行：
  ```bash
  cargo test -q prompt_context
  cargo test -q conversation_loop
  ```
- [ ] 合并后最大文件没有超过 1,500 行
- [ ] 没有把权限、验证证据、checkpoint、closeout proof 边界弱化

## 不做清单

- 不为了达到 79 个文件而合并大文件。
- 不把 `permission_recovery.rs` 直接塞进 `permission_controller.rs`。
- 不把 `runtime_timeouts.rs` 随便内联到 `session_processor.rs` 或 `api_request_controller.rs`。
- 不创建语义模糊的 `workflow_helpers.rs`。
- 不在同一个提交里同时做多个管线合并。
