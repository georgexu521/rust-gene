# Conversation Loop 控制器合并完成记录

日期：2026-06-08
状态更新：2026-06-09

## 结论

`conversation_loop` 控制器收敛已经完成一轮可验证合并。实现没有按旧目标硬压到
79 个总文件，而是按边界小步合并：状态、薄包装器、retrieval 构建/注入、工具轮次
结果映射、失败 follow-up、request timeout 命名、loop policy 等已经收敛到当前承载
模块。

当前结果比第一轮目标更进一步：

| 口径 | 合并前 | 当前 | 变化 |
|------|--------|------|------|
| `conversation_loop` 下全部文件，含嵌套目录和测试 | 99 | 85 | -14 |
| 排除 `*tests.rs` 后 | 93 | 79 | -14 |
| 全部文件总行数 | 44,355 | 44,141 | -214 |
| 非测试文件总行数 | 38,194 | 37,972 | -222 |

## 当前最大文件

| 文件 | 当前行数 | 判断 |
|------|----------|------|
| `tests.rs` | 3,011 | 测试集合，暂不纳入源文件 1,500 行约束 |
| `closeout_controller.rs` | 1,499 | 已压回上限内，需要后续优先拆测试或 proof/report 子模块 |
| `patch_recovery.rs` | 1,465 | test-only 修复覆盖，接近上限 |
| `tool_result_controller.rs` | 1,425 | 工具结果热点，后续更适合拆小而不是继续合并 |
| `permission_controller.rs` | 1,420 | 不应再合并权限恢复逻辑 |
| `request_preparation_controller.rs` | 1,325 | 请求准备热点，保留独立 |
| `tool_metadata.rs` | 1,322 | 元数据/渲染辅助热点，保留独立 |

## 已完成合并

### Phase 1：低风险薄文件合并

完成。

| 原文件/职责 | 当前承载模块 |
|-------------|--------------|
| `turn_loop_state_controller.rs` | `turn_state.rs` |
| `turn_runtime_state.rs` | `turn_state.rs` |
| `turn_runtime_context.rs` | `turn_state.rs` |
| `iteration_budget_controller.rs` | `tool_round_controller.rs` |
| `tool_exposure_plan.rs` | `turn_iteration_setup_controller.rs` |
| `session_goal_controller.rs` | `turn_entry_gate_controller.rs` |
| `task_context_trace_controller.rs` | `turn_entry_gate_controller.rs` |
| `turn_runtime_diet_bootstrap_controller.rs` | `turn_loop_bootstrap_controller.rs` |

### Phase 2：中等风险管线合并

完成。

| 原文件/职责 | 当前承载模块 |
|-------------|--------------|
| `retrieval_context_builder.rs` | `turn_retrieval_context_controller.rs` |
| `retrieval_prompt_controller.rs` | `turn_request_bootstrap_controller.rs` |
| `turn_tool_round_outcome_controller.rs` | `turn_tool_round_step_controller.rs` |
| `turn_tool_failure_followup_controller.rs` | `tool_failure_guided_debugging.rs` |
| `tool_failure_stop_controller.rs` | 删除；原模块为 test-only advisory fixture，生产路径无引用 |
| `turn_post_change_closeout_controller.rs` | `turn_iteration_controller.rs` |

### Phase 3：后置小合并

完成。

| 原文件/职责 | 当前承载模块 |
|-------------|--------------|
| `runtime_timeouts.rs` | 重命名为 `request_timeouts.rs`，保留独立共享边界 |
| `main_loop_profile.rs` | `turn_loop_policy.rs` |
| `force_summary.rs` | `turn_loop_policy.rs` |

## 保留边界

这些边界没有继续合并，原因仍然成立：

- `permission_recovery.rs` 没有塞进 `permission_controller.rs`；`permission_controller.rs`
  已接近行数上限，后续方向应该是拆成权限子模块。
- `request_timeouts.rs` 没有内联到 `session_processor.rs` 或
  `api_request_controller.rs`；它同时服务 API request timeout 和 stream idle timeout。
- `workflow_prompt_policy.rs`、`workflow_runtime.rs`、`workflow_trace.rs`、
  `workflow_change_tracker.rs` 没有合成泛化 `workflow_helpers.rs`；这些文件代表不同变更
  原因。
- `api_request_controller.rs`、`tool_batch_result_processor.rs`、`request_preparation_controller.rs`
  等热点文件没有继续吞并周边逻辑。

## 验证状态

本轮修复后已通过：

```bash
cargo test -q conversation_loop -- --test-threads=1
cargo test -q request_timeouts
```

仍建议提交前完整运行：

```bash
cargo fmt --check
cargo check -q
cargo test -q route_scoped_tools
cargo test -q closeout
cargo test -q prompt_context
cargo test -q conversation_loop
```

如果改到 workflow 或 live-eval 脚本，再追加：

```bash
bash scripts/workflow-production-gates.sh
bash -n scripts/run_live_eval.sh
python3 -m py_compile scripts/live_eval_report_parser.py
```

## 后续建议

1. 优先把 `closeout_controller.rs` 拆出测试或 proof/report 子模块，让它稳定低于
   1,500 行，而不是只贴着上限。
2. 后续如果继续降文件数，不要从权限、closeout proof、checkpoint、workflow runtime
   这些硬边界里拿文件数。
3. 对 `tool_result_controller.rs`、`request_preparation_controller.rs`、
   `permission_controller.rs` 的下一步应该是拆分责任，而不是合并。
4. 保持 `docs/CONTROLLER_INDEX.md` 和 `docs/PROJECT_MAP.md` 与实际模块名同步；这两个
   文档会进入运行时项目上下文，过期路径会直接误导后续 agent。

## 不做清单

- 不为了达到某个文件数而合并大文件。
- 不把 `permission_recovery.rs` 直接塞进 `permission_controller.rs`。
- 不把 `request_timeouts.rs` 随便内联到 `session_processor.rs` 或
  `api_request_controller.rs`。
- 不创建语义模糊的 `workflow_helpers.rs`。
- 不弱化权限、验证证据、checkpoint、closeout proof 边界来换取更少文件。
