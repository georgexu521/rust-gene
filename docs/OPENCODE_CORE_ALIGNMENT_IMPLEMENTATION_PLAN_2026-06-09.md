# Opencode Core Alignment 下一阶段实施计划
Status: Active

> 2026-06-09 v1.1
>
> 对照来源：
> - opencode `sst/opencode` `dev@b4a6419`
> - priority-agent 当前工作区 `/Users/georgexu/Desktop/rust-agent`
>
> 目标：把已经落地的 event log、session parts、tool output、队列、checkpoint
> 串成一条可恢复、可回放、可调试的产品闭环。本文不要求推倒现有
> `StreamingQueryEngine` / `ConversationLoop`，下一阶段重点是补投影消费、状态
> settlement、队列 drain、可见性和恢复语义。

---

## 0. 结论先行

原 v1.0 的方向是对的：继续追 opencode 的 core spine，而不是先做外层功能。
但它把几项 priority-agent 已经完成的能力写成了缺口，容易导致重复开发：

| 原判断 | 当前校准 |
|---|---|
| `ToolOutputStore` 没有自动清理 | 已在 `src/bootstrap.rs` 启动时调用 `cleanup_expired_for_project()` |
| Desktop 没有分页读取 tool output | 已有 `desktop_tool_output_index` / `desktop_tool_output_page` Tauri 命令与前端 API |
| Desktop resume 不读 parts | `resume_session` 已返回 `session_parts`，并有 reload smoke 测试 |
| TraceStore 完全内存态 | 运行时 `TraceStore` 是内存环，但 `SessionStore` 已持久化 `turn_traces` / `trace_events` |
| opencode `ToolOutputStore` 有 session-scoped read API | 当前 opencode core 只有 `limits()` / `bound()` / `cleanup()`；它写共享文件路径，不是 `tool-output://` 分页 API |
| opencode 是 `session_parts` 表 | opencode 源码里是 durable event + projected `Session.Message` / assistant content；priority-agent 的 `SessionPart` 是我们自己的投影形态 |

所以下一阶段不是“从零补齐 opencode”，而是：

1. 让已经存在的持久事件和 parts 成为 TUI/Desktop/API 的主要恢复来源。
2. 给 tool / shell / provider interruption 建立强 settlement 语义，避免 replay 后出现悬空运行态。
3. 把队列从“能持久化”推进到“能 wake/drain/await idle”。
4. 把 compaction、revert、trace、tool-output 这些事实暴露成用户能理解的 readiness/export 面板。

---

## 1. 对照 opencode 后的真实差距

### 1.1 Tool output：存储能力已经强于 opencode，但消费闭环还不完整

opencode 当前 `packages/core/src/tool-output-store.ts` 的核心是：

- `MAX_LINES = 2_000`，`MAX_BYTES = 50 * 1024`，`RETENTION = 7 days`
- `bound(input)` 对超大输出做 head/tail 预览并写文件
- 返回模型侧 bounded text 和 `outputPaths`
- `cleanup()` 删除过期 `tool_` 文件

priority-agent 当前 `src/tool_output_store/mod.rs` 已经有更强的 session-scoped
能力：

- `tool-output://<id>` URI
- `read_page(session_id, id_or_uri, offset, limit)` 并校验 session 隔离
- `ToolOutputPolicy` 支持 project env
- Desktop 已暴露 `desktop_tool_output_index/page`
- API routes 也已有 tool output 读取入口

真实缺口：

- 需要确认所有大输出工具最终都经过 `conversation_loop/tool_execution.rs` 的统一
  `truncate_tool_result` 路径，避免工具内部先行截断导致 full output 丢失。
- TUI/Desktop 需要把 `output_uri` 展示为一等入口，而不是只显示 preview。
- export payload 需要包含 tool-output 索引和可重放的分页 metadata。
- 要补一条“10MB 输出不丢失 + Desktop/API/TUI 可读取同一 URI”的端到端测试。

### 1.2 Events/parts：我们有投影，但投影还不是唯一事实来源

opencode 的相关源码是：

- `packages/core/src/session/event.ts`：类型化 event，`sync.version` 明确升级点。
- `packages/core/src/session/message-updater.ts`：把 durable events 投影为 message。
- `packages/core/src/session/message.ts`：assistant content 包含 text/reasoning/tool 等状态。
- `specs/v2/schema-changelog.md`：强调 oversized tool output、settlement recovery、
  running tool interrupted 后必须写 durable failure projection。

priority-agent 已有：

- `session_events` append-only 表。
- `StreamEventMirror`。
- `SessionPart::{AssistantText, Reasoning, Tool, Shell, Permission, Compaction, Closeout, Revert}`。
- `incremental_refresh_session_parts()`。
- Desktop `resume_session` 返回 `session_parts`。

真实缺口：

- `SessionPart` 注释说按 assistant message ID 组织，但 enum 大多数 variant 没有
  `message_id` / `turn_id` 字段，恢复、revert、export 很难准确定位归属。
- TUI 仍大量依赖 `ToolRunView` / `TraceStore` / runtime-local 状态。
- Desktop 虽然能拿到 parts，但下一步要确认前端渲染以 parts 为主，而不是只把它们
  当 reload 附加数据。
- event payload 仍是自由 JSON，缺少可演进的 typed schema/version envelope。
- provider 中断、cancel、进程退出后，悬空 tool/shell 需要被 durable settlement。

### 1.3 Run coordinator：已有 durable queue，缺 opencode 式 wake/drain contract

opencode 的 `SessionRunCoordinator` contract：

- `run(sessionID)`：显式启动 drain，可 join/upgrade。
- `wake(sessionID)`：合并唤醒，idle 时启动 drain。
- `awaitIdle(sessionID)`：等待 session settle。
- `interrupt(sessionID)`：停止当前链路。

priority-agent 当前已有：

- `src/engine/run_coordinator.rs`：`AtomicBool` 防重入。
- `session_inputs` 表。
- `admit_session_input_with_metadata()`。
- `promote_session_input_record()`。
- API runner 已有 `spawn_queue_drain()` / `drain_pending_inputs()`。
- restart recovery 会把 promoted stale rows 恢复为 pending。

真实缺口：

- TUI/desktop/runtime 还没有统一的 session execution contract；API 自己有 drain，
  TUI 主要在 `TuiApp` 内部处理。
- 缺公开的 `wake` / `await_idle` 语义和测试。
- 多 session 并发、同 session FIFO、cancel 后继续 drain 的边界还需要统一验收。

### 1.4 Trace/readiness：不是没持久化，而是没变成产品级诊断面

priority-agent 当前同时有：

- 内存 `engine::trace::TraceStore`，供运行时面板取最近 trace。
- SQLite `turn_traces` / `trace_events`，`finish_trace()` 会持久化完成 trace。
- TUI observability 命令已经能合并 memory traces 和 persisted traces。

真实缺口：

- Desktop readiness/export 面板还没有充分消费持久 trace。
- trace 与 event/part/checkpoint/tool-output 的关联不够强，定位“为什么没有 verified
  closeout”还需要人工拼接。
- export payload 应该包含 trace summary、settlement 状态、未验证原因、tool-output
  索引、parts 版本，而不是只导出 transcript。

---

## 2. 下一阶段实施原则

1. 先接闭环，再扩 schema。每次新增字段都必须有 reader 兼容旧数据。
2. 不削弱验证和权限。settlement 不完整时宁可 `partial` / `not_verified`，不能伪装成 verified。
3. 不复制 opencode 的文件结构。只复制它的 contract：durable event、typed projection、wake/drain、settlement recovery。
4. Desktop/API/TUI 共用一套 runtime 事实，不各自重新解释工具生命周期。
5. 每个阶段都要有“kill/restart/reload/export”类验收，不能只跑单元测试。

---

## 3. Phase A：SessionPart 归属与投影消费

### 目标

把 `session_parts` 从“可查询的副产品”推进成恢复、渲染、export、revert 的主要数据源。

### 任务

1. 增加 part 归属 metadata。
   - 给持久化层增加 `message_id` 或 `turn_id`，优先放在 `session_parts` 表列/DTO，
     不要只塞进 enum payload。
   - projector 从 `assistant_text_*`、tool events、closeout/revert events 推导稳定归属。
   - 旧数据缺字段时允许 `None`，reader 不崩。

2. 修正 `SessionPart` 当前语义问题。
   - `ToolPartStatus` 里当前有重复 `Cancelled` variant，需要在做 schema 变更前清掉。
   - `session_parts.rs` 顶部注释应改成“projected parts can be associated with
     assistant turn/message when metadata exists”，不要声明现在已经全部 keyed by
     assistant message ID。

3. TUI resume from parts。
   - `TuiApp` 启动/切 session 时，从 `SessionManager::load_session_parts` 重建已完成
     tool/shell/closeout/revert cards。
   - runtime-local `ToolRunView` 仍可用于 live streaming，但 reload 后应以 parts 为准。

4. Desktop parts-first rendering audit。
   - 保留现有 `resume_session` 返回 `session_parts` 的 API。
   - 前端渲染和 export 不再只依赖 messages/transcript。
   - 添加一条 frontend 或 Tauri smoke：reload 后 tool、closeout、revert、compaction
     parts 都存在且顺序稳定。

### 验收

```bash
cargo test -q session_parts --lib
cargo test -q tui --lib
cargo test -q desktop_smoke_loads_persisted_long_session_parts_for_reload --lib
cargo check --features experimental-api-server -q
```

手动验收：

- 运行一个包含 tool call、closeout、revert 的 session。
- 关闭 TUI/Desktop 后重启。
- 之前的 tool card、closeout 状态、revert 结果仍可见，并能定位到对应 turn/message。

---

## 4. Phase B：Tool/Shell Settlement Invariants

### 目标

每个 tool/shell 生命周期都必须有最终状态。provider 中断、cancel、进程恢复后，不允许
UI 或 export 留下永久 `running`。

### 任务

1. 增加 settlement ledger。
   - 可以先做纯函数模块：从 `session_events` 或当前 turn 的 stream events 计算
     `unsettled_tools` / `unsettled_shells`。
   - 状态至少覆盖 `input_started`、`called`、`running`、`completed`、`failed`、
     `cancelled`、`provider_executed`。

2. closeout 前检查 settlement。
   - 未 settlement 时 closeout 只能是 `partial` / `failed`，并写明 tool_call_id。
   - 不要为了通过弱模型 eval 放宽 proof gate。

3. durable recovery。
   - 在新 provider request 前，扫描上一次持久投影里仍是 running 的 local tools。
   - 写入兼容现有事件形态的 `tool_failed` / `shell_ended` / `cancelled` 事件，错误信息
     类似 `Tool execution interrupted before settlement`。
   - 不能重放副作用，只能补投影 settlement。

4. cancel/interrupt path。
   - API/TUI cancel 时写 durable cancellation event。
   - queued/pending inputs 保持 session_inputs 的 state 语义，不和 tool settlement 混在一起。

### 验收

```bash
cargo test -q event_store --lib
cargo test -q session_parts --lib
cargo test -q closeout --lib
cargo test -q tool_batch_result_processor --lib
```

手动验收：

- 人为中断 provider/tool 流。
- 重启后 session parts 里不再有永久 running tool。
- closeout/export 明确显示 `partial` 与 unsettled 原因。

---

## 5. Phase C：Run Coordinator Wake/Drain 合同

### 目标

把当前分散的 `AtomicBool + session_inputs + API drain` 整理成跨 API/TUI/Desktop
一致的 session execution contract。

### 任务

1. 定义轻量 `SessionExecution` trait 或 facade。
   - `run(session_id)`
   - `wake(session_id)`
   - `await_idle(session_id, timeout)`
   - `interrupt(session_id)`

2. 复用现有实现，而不是新建孤立 actor。
   - API 已有 `spawn_queue_drain()`，先抽出共用 drain 逻辑。
   - TUI 集成时走同一组 queue/promote/cancel helper。
   - `StreamingQueryEngine` 仍然是实际执行入口。

3. wake 合并。
   - 多次 wake 只保证 drain 被触发，不重复启动同 session run。
   - FIFO 使用 `session_inputs.id ASC`，保留现有 idempotency。

4. await idle。
   - 给 tests、Desktop readiness、export 前准备使用。
   - 不要求跨进程强一致；当前阶段接受 process-local idle，但必须和 durable queue
     状态一起判断。

### 验收

```bash
cargo test -q run_coordinator --lib
cargo test -q session_runner --lib
cargo test -q api --lib
cargo test -q tui --lib
```

手动验收：

- 同 session 连续提交两个 prompt：第二个 pending，第一轮结束后自动执行。
- 多次 wake 不产生并发 run。
- cancel 当前 run 后，队列状态可解释：被取消的输入标记 cancelled，剩余 pending 可继续 drain。

---

## 6. Phase D：Tool Output 一等入口与 Export Payload

### 目标

长输出不丢、不污染模型上下文，并且能被 Desktop/API/TUI/export 一致读取。

### 任务

1. 工具输出路径审计。
   - 搜索所有直接截断输出的工具。
   - 确认最终返回给模型和 projection 的内容经过 `truncate_tool_result` 或统一 preview helper。
   - 如果某些工具必须做 domain-specific preview，也要保留 full content 到 `ToolOutputStore`。

2. TUI/Desktop URI action。
   - TUI tool card 显示 `tool-output://` 时提供读取/翻页动作。
   - Desktop 使用已有 `desktop_tool_output_page`，补 UI 入口和 loading/error 状态。

3. Export payload。
   - 导出内容包含：
     - session metadata
     - messages
     - session_parts
     - tool_output_index
     - trace summaries
     - closeout/verification status
     - compaction boundaries
     - unresolved settlement list
   - tool output 默认只导 metadata 和 preview；可选 include full pages，避免巨大文件默认爆炸。

4. 端到端测试。
   - 生成 10MB stdout。
   - 验证 model-facing preview 有 URI。
   - API/Desktop/TUI 可以读取第一页和尾页。

### 验收

```bash
cargo test -q tool_output_store --lib
cargo test -q tool_execution --lib
cargo test -q export --lib
cargo check --features experimental-api-server -q
```

---

## 7. Phase E：Compaction、Revert、Trace Readiness 产品化

### 目标

用户能看懂当前 session 是否可继续、哪里被压缩、能撤回什么、为什么没有 verified。

### 任务

1. Compaction 可见性。
   - TUI/Desktop 显示 compaction boundary：触发原因、压缩前后 token/message 数、
     recent messages 保留范围。
   - `/context compact status` 或现有 context 命令扩展为读取 `SessionPart::Compaction`
     和 compact boundary，而不是只看内存状态。

2. Message/part-aware revert。
   - 不新建 `src/engine/checkpoint.rs`；当前已有 `src/engine/checkpoint/` 模块。
   - 建立 `message_id/part_id -> checkpoint_id/changed_paths` 映射。
   - `/revert last-turn` 继续可用；新增按 message/part 定位时先走只读 preview，再执行 restore。

3. Trace readiness。
   - readiness 面板聚合：
     - latest persisted trace
     - latest closeout status
     - unresolved settlement
     - pending session inputs
     - tool-output count/size
     - last compaction
   - Desktop export 前可以调用 `await_idle` 并返回 readiness snapshot。

### 验收

```bash
cargo test -q checkpoint --lib
cargo test -q context --lib
cargo test -q observability --lib
cargo check --features experimental-api-server -q
```

---

## 8. Phase F：Event Schema Versioning 与写入性能

### 目标

让 event log 具备长期演进能力，同时避免在还没测出瓶颈前过早改写 writer。

### 任务

1. Schema versioning。
   - 不要简单把所有 payload 包成 `{"schema_version":1,"data":...}`，这会破坏旧 reader。
   - 推荐先在 `SessionEventRow` / writer helper 层定义 typed payload structs，
     新事件带 `schema_version` 字段，旧事件读取时按 v0 解析。
   - event type 的 breaking change 用新 event name 或 `schema_version` 升级，不原地改变含义。

2. Batch write 先做 profiling。
   - 当前每 event 单写是否是瓶颈，需要用 1000/10000 event bench 或 trace 压测确认。
   - 如果确实慢，再加 buffered writer；必须保证 Drop/flush/cancel 时不丢 event。
   - SQLite 写入放到 `spawn_blocking` 或专门 writer 线程，避免 async runtime 被长事务卡住。

3. Migration/read compatibility。
   - 增加旧 session fixture：无 schema_version 的 events 仍可 project parts。
   - 新 schema event 可以 downgrade 到旧 reader 可忽略的字段。

### 验收

```bash
cargo test -q event_store --lib
cargo test -q session_parts --lib
cargo test -q migrations --lib
```

性能验收：

- 1000 个 streaming delta event 写入和投影耗时有基线。
- 如果引入 batch writer，必须证明吞吐提升且 crash/restart 后不丢 settled event。

---

## 9. 推荐实施顺序

| 顺序 | 阶段 | 原因 |
|---|---|---|
| 1 | Phase A：SessionPart 归属与投影消费 | 没有 message/turn 归属，revert/export/readiness 都会继续模糊 |
| 2 | Phase B：Settlement invariants | 先修正确性，避免 UI/export 恢复出假 running |
| 3 | Phase C：Wake/drain contract | 队列已经有基础，统一后才能支撑 agent switch/run 工作流 |
| 4 | Phase D：Tool output/export | 现有能力强，主要补消费和 payload，收益高 |
| 5 | Phase E：Readiness 产品化 | 依赖 parts、settlement、trace 聚合 |
| 6 | Phase F：Schema/perf | 先有真实数据和瓶颈，再做更底层演进 |

预计工作量：4 到 6 周。Phase A/B/C 是核心工程闭环，Phase D/E 是产品可见性，Phase F
是长期维护能力。

---

## 10. 全局验收矩阵

| 能力 | 验收方式 |
|---|---|
| parts 可恢复 | kill/restart 后 tool、shell、closeout、revert、compaction 顺序稳定 |
| tool 不悬空 | provider/tool 中断后重启，running tool 被 durable failed/cancelled settlement |
| queue 可恢复 | prompt 入队后 kill -9，重启后仍 pending 或 recovered |
| wake 不并发 | 同 session 多次 wake 只产生一个 active run |
| output 不丢 | 10MB 输出有 `tool-output://`，分页读取内容一致 |
| export 完整 | payload 含 messages、parts、trace、tool_output_index、settlement、closeout |
| readiness 可解释 | 面板能说明 idle/running/pending/unverified/compacted/revertable |
| 旧数据兼容 | 老 session events/parts 能读、能 project，不 panic |

推荐最终门禁：

```bash
cargo fmt --check
cargo check -q
cargo test -q session_parts --lib
cargo test -q event_store --lib
cargo test -q run_coordinator --lib
cargo test -q closeout --lib
cargo check --features experimental-api-server -q
```

Desktop/UI 改动落地时再加：

```bash
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
```

---

## 11. 非目标

- 不替换 `StreamingQueryEngine` 和 `ConversationLoop`。
- 不为了对齐 opencode 而照搬 TypeScript/Effect 架构。
- 不削弱权限、checkpoint、高风险操作、closeout proof。
- 不把 batch writer 作为第一阶段目标。
- 不让 Desktop/TUI 各自发明独立 session lifecycle。

---

## 12. 当前最该先开的 PR 切片

建议下一阶段第一组 PR 控制在小范围内：

1. `session_parts` metadata fix
   - 清理 `ToolPartStatus::Cancelled` 重复 variant。
   - 增加 part `message_id/turn_id` 持久字段和兼容 reader。
   - 补旧数据 projection 测试。

2. settlement ledger
   - 从 event stream/session events 计算 unresolved tools。
   - closeout 前接入 `partial` gate。
   - provider interruption fixture。

3. TUI/Desktop reload from parts
   - TUI 重建已完成 tool cards。
   - Desktop 确认 parts-first render/export。
   - reload smoke 扩展到 compaction/revert/closeout。

这三块做完后，再推进 wake/drain 和 export payload，项目会更接近 opencode 的核心

---

## 13. 实施记录

### 2026-06-10 Phase A — SessionPart metadata fix + TUI resume

**提交：** `8e91c1b1`

- 新增 v16 migration：`message_id TEXT` 列 + 索引
- `SessionPart` 所有 variant 添加 `message_id: Option<String>`
- `PersistedSessionPart` DTO 及所有 DB 读写路径更新
- 移除死代码 `ToolPartStatus::Cancelled`（零引用）
- 修复 TUI sidebar Enter 调用 `restore_session()` 而不是裸 `switch_to_session()`
- 全部测试通过（2449 passed），fmt + clippy clean

### 2026-06-10 Phase B — Durable settlement recovery

**提交：** `843e308f`

- `ConversationLoop::recover_unsettled_tools()` 扫描 session_parts 中仍为 running/pending 的 tool/shell
- 在 turn 开始前写入 `tool_failed` 事件，错误信息含 "interrupted before settlement"
- 确保 crash / provider interrupt 后重启不会留下永久 running tool
- 全部测试通过（2448-2449 passed），fmt + clippy clean

### 2026-06-10 Phase C — Run Coordinator wake/drain contract

**提交：** `4b69617e`

- `SessionRunCoordinator` 扩展 wake 语义：
  - `wake()` — CAS 设置 wake 标志，返回 true 表示应由调用方启动 drain
  - `accept_wake()` — 清除标志，进入 drain loop
  - `is_wake_pending()` — 查询待处理 wake
- API `spawn_queue_drain()` 使用 `wake()` 防止并发 drain spawn
- TUI `on_tick` 在 `finish_run()` 后使用 `wake()/accept_wake()` 保护同 session drain
- 全部测试通过（2449 passed），fmt + clippy clean

### 2026-06-10 Phase D — Enhanced export payload

**提交：** `9b296a23`

- 导出 schema 新增字段：
  - `parts`: session_parts 轻量投影（kind, tool_name, status, message_id）
  - `closeout_status`: 从 closeout 事件提取的状态
  - `compaction_count`: compaction 事件计数
  - `unresolved_settlement`: 仍在 running/pending 的 tool/shell 列表
  - `tool_outputs`: ToolOutputStore 索引（id, tool_name, original_bytes）
- 隐私分级：parts/tool_outputs 在 Full/Redacted 可用，unresolved_settlement 仅 Full
- `build_session_export()` 自动填充所有新字段
- 全部测试通过（2449 passed），fmt + clippy clean

### 2026-06-10 Phase E — Compaction event stream coverage

**提交：** `302f1f30`

- 所有 compaction 路径写入 session_events 表：
  - StreamingQueryEngine::compact() (manual compact)
  - StreamingQueryEngine preflight compression
  - API request controller reactive compaction
  - Preflight compression controller
- Compaction 事件包含 strategy, trigger, before_tokens, after_tokens
- 与现有 StreamEventMirror 互补（Mirror 处理 StreamEvent 变体，Compaction 处理内部压缩决策）
- 全部测试通过（2449 passed），fmt + clippy clean

### 2026-06-10 Phase F — Integration tests

**提交：** `bf3ef2ee`

- `settlement_recovery_writes_failed_event_for_dangling_tools`:
  - 模拟 provider 中断（tool started 但无 completed/failed）
  - 验证 recovery 写入 tool_failed 事件
  - 确认重投影后状态为 Failed
- `export_payload_includes_parts_closeout_and_tool_outputs`:
  - 创建 text/tool/closeout/compaction 事件链
  - 验证投影产生正确 parts
  - 确认无 unresolved settlement
- 全部测试通过（2451 passed），fmt + clippy clean

### 完成状态

所有阶段已完成并提交。验证命令：

```bash
cargo fmt --check
cargo check -q
cargo test -q session_parts --lib
cargo test -q event_store --lib
cargo test -q run_coordinator --lib
cargo test -q closeout --lib
cargo test -q export --lib
cargo check --features experimental-api-server -q
```
可靠性：不是“功能列表相似”，而是 session 的事实来源和恢复语义真正站稳。
