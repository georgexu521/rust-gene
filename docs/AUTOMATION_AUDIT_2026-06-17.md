# 自动化程度审计报告

日期：2026-06-17  
更新：2026-06-18

## 审计范围

本次审计对照当前代码检查“哪些事情会自动发生、哪些需要配置、哪些必须用户确认”。重点覆盖：

- 记忆读取、主动召回、提案生成、提案应用
- 上下文检索、上下文注入、压缩和缓存诊断
- follow-up queue / wake / drain
- closeout、验证、权限和状态可见性
- TUI slash 命令、doctor/quick/pulse 面板

重点文件包括：

- `src/engine/conversation_loop/memory_sync_controller.rs`
- `src/engine/conversation_loop/closeout_controller/mod.rs`
- `src/engine/conversation_loop/turn_context_bootstrap_controller.rs`
- `src/engine/conversation_loop/turn_retrieval_context_controller.rs`
- `src/engine/conversation_loop/memory_snapshot_controller.rs`
- `src/memory/active.rs`
- `src/memory/manager/mod.rs`
- `src/engine/run_coordinator.rs`
- `src/api/state.rs`
- `src/tui/app/slash_commands.rs`
- `src/tui/app/memory.rs`
- `src/tui/slash_handler/learning/memory_proposals.rs`
- `src/tui/slash_handler/learning.rs`
- `src/tui/slash_handler/runtime.rs`

## 审计结论

原文的核心观察“长期记忆默认不自动写入”是对的，但结论过窄，也有几处代码事实不准确。当前项目并不是自动化不足，而是刻意把高风险持久化动作保留为 review-first，同时在请求准备、检索、压缩、验证、权限、closeout、follow-up queue 等运行时链路里做了大量自动化。

需要修正的点：

- 默认记忆写入确实是 `review_only`，但这不是单纯 UX 缺陷，而是当前项目的安全边界：长期记忆持久化不应默认静默发生。
- `PRIORITY_AGENT_AUTO_MEMORY_WRITE=narrow` 已经存在，只自动持久化显式用户偏好，不是待实现能力。
- `/memory status` 已经存在，不应列为待做。
- 记忆提案并非完全“无通知”：closeout final、`/quick`、`/active-task`、`/memory doctor`、Project Pulse 都能暴露 memory proposal 状态；真实缺口是没有强提醒/主动审阅入口。
- active memory 默认关闭是事实，但它是有明确 gate 的只读原型，会跳过 eval/headless/automation/internal 路径，不能简单建议默认全开。
- 时间压缩和后台修剪不应写成稳定全自动生产路径；当前更像 helper/实验路径。
- 原文漏掉了 run coordinator 的 durable queue、wake/drain、idempotent admission，这是当前自动化程度的重要组成部分。

## 当前自动化能力矩阵

### 1. 默认自动执行

| 系统 | 当前状态 | 代码事实 |
|------|----------|----------|
| 任务上下文与上下文注入 | 自动 | `TurnContextBootstrapController` 构建 retrieval context、retained context、task bundle 和 workflow state |
| pinned memory snapshot | 自动但受 retrieval policy 约束 | `MemorySnapshotController` 在 memory enabled 且 route 允许 memory context 时注入 `<memory-context>` |
| 项目/会话/记忆检索 | 自动但按 route 策略 | `TurnRetrievalContextController` 自动构建 project、session、memory retrieval context |
| 技能触发上下文 | 自动但按 dynamic-context profile | `SkillRuntime::load(...).search(...)` 写入 retained context |
| 选择性消息压缩 | 默认开启 | `PRIORITY_AGENT_SELECTIVE_COMPRESSION` 默认开启，只压缩请求局部旧工具输出 |
| 预检压缩 | 自动 | token 压力达到阈值时 preflight compact 并记录 compact boundary |
| API 反应式压缩 | 自动 | provider 上下文过长后记录 `Retrying`，压缩有效后记录 `Recovered` |
| 缓存诊断 | 自动 | `cache_stability.rs` 计算请求形状、cache usage、miss reason |
| closeout 证据整理 | 自动 | closeout 阶段写 execution report、trace、memory proposal evidence |
| follow-up queue/wake | 自动 | API/TUI 使用 `SessionRunCoordinator`、`session_inputs`、wake/drain 处理排队输入 |
| 权限与验证门控 | 自动筛查，必要时阻塞 | runtime 自动评估风险、权限、validation proof 和 closeout 状态 |

### 2. 默认生成候选，但不默认持久化

| 系统 | 当前状态 | 说明 |
|------|----------|------|
| closeout memory proposal | 默认 review-first | closeout 可生成 `MemoryProposal`，`write_performed=false`，进入 review queue |
| 后台 memory review nudge | 自动触发候选生成 | `MemoryManager::advance_nudge()` 默认每 10 轮触发后台审查，但结果仍是 review queue |
| project progress ledger | closeout 后台写入 | 这是执行进展证据，不等同于把长期记忆静默写入用户/项目记忆 |
| memory repair proposal | 手动触发扫描 | `/memory-proposals repair-drift` 或相关 doctor/repair 流程生成 projection repair proposals |

### 3. 需要配置才开启

| 系统 | 默认 | 开关 | 说明 |
|------|------|------|------|
| legacy 自动记忆写入 | 关 | `PRIORITY_AGENT_AUTO_MEMORY_WRITE=legacy` | 旧式自动写入，高风险，不建议默认开启 |
| narrow 自动记忆写入 | 关 | `PRIORITY_AGENT_AUTO_MEMORY_WRITE=narrow` | 只自动保存显式用户偏好，如“我喜欢 / I prefer” |
| active memory worker | 关 | `PRIORITY_AGENT_ACTIVE_MEMORY=1` | 只读 FTS 召回，有 eval/headless/automation/internal gate |
| LLM compaction | 关 | `PRIORITY_AGENT_LLM_COMPACTION=1` | 允许 LLM 参与摘要压缩 |
| background prune helper | 关 | `PRIORITY_AGENT_BACKGROUND_PRUNE=1` | helper 存在，但当前未见稳定主循环调用 |
| time-based compression 判断 | 关 | `PRIORITY_AGENT_TIME_BASED_COMPRESSION=1` | 判断函数和测试存在，当前不应写成主路径自动触发 |

### 4. 需要用户显式操作

| 功能 | 命令/入口 | 当前能力 |
|------|-----------|----------|
| 查看记忆状态 | `/memory status` | 已实现，含 use/generate/recall/write-policy/active 状态 |
| 详细记忆诊断 | `/memory doctor` | 已实现，包含 proposal queue、quality gates、calibration/eval 摘要 |
| 查看提案 | `/memory-proposals list` | 已实现，支持 source/status/scope/project 过滤 |
| 查看提案详情 | `/memory-proposals show <task-id>` | 已实现 |
| 接受/拒绝提案 | `/memory-proposals accept|reject <task-id>` | 已实现 |
| 批量处理提案 | `/memory-proposals batch-accept|batch-reject|cleanup-stale` | 已实现 |
| 应用提案 | `/memory-proposals apply <task-id>` 或 `apply --accepted` | 已实现 |
| 冲突查看/解决 | `/memory-proposals conflicts` / `resolve-conflict` | 已实现 |
| 手动压缩 | `/compact` | 已实现 |
| 查看压缩状态 | `/compact-status` | 已实现 |
| 查看统一任务状态 | `/quick` / `/active-task` / Project Pulse | 已实现，包含 memory proposal 状态 |

## 关键代码事实

### 1. 记忆写入默认 review-only

当前默认来自配置：

- `src/services/config.rs` 默认 `engine.auto_memory_write = "review_only"`。
- `src/engine/conversation_loop/memory_sync_controller.rs` 将未知/未设置策略映射为 `ReviewOnly`。
- Review-only closeout 会记录 `MemoryBoundaryEvaluated`，但不会调用长期记忆写入。

这点和原文一致，但建议不能简单写成“把默认改成 narrow”。当前项目对长期记忆的安全边界非常明确：自动写入会影响未来上下文，应避免把模型误判或临时任务状态静默固化。

更合理的产品改进是：保持默认 review-only，同时提高候选可见性和一键审阅效率。

### 2. narrow 自动写入已存在

`PRIORITY_AGENT_AUTO_MEMORY_WRITE=narrow` 已经实现。它只接受明确用户偏好标记：

- 中文：`我喜欢`、`我更喜欢`、`我希望`、`我的偏好`
- 英文：`I prefer`、`my preference`

命中后通过 `candidate_from_content(...).explicit(true)` 提交到用户记忆目标。这个能力不是 TODO。后续真正要讨论的是：是否要把 narrow 从 opt-in 改为默认，以及是否要增加更多低风险表达模式。

### 3. active memory 是只读 gated prototype

`src/memory/active.rs` 默认 `enabled=false`。启用后也只有在所有 gate 通过时才运行：

- memory 可用；
- route 允许 memory context；
- user-facing；
- 有 persistent session id；
- 有 timeout budget；
- 不是 eval/headless/automation/internal。

它只做本地 FTS 召回，输出 fenced `<active-memory-context>`，标记为 untrusted retrieval evidence，不调用 LLM、不写记忆、不做决策。原文“开启主动记忆，默认 true”的建议风险偏高；更合理的是先把 status/doctor/pulse 里的可见性做好，再决定默认策略。

### 4. 记忆提案已有多处可见入口，但缺少强提醒

当前已有入口：

- final closeout 可追加 `Memory proposal:` 摘要。
- `/quick` 的 Contracts 区显示 `Memory proposal`。
- `/active-task` 聚合目标、workflow、验证、closeout 和 memory proposal evidence。
- `/memory doctor` 显示 pending memory candidates。
- Project Pulse 显示 `Memory proposal:` 状态。
- `/memory-proposals` 提供 list/show/accept/reject/edit/apply/batch/conflict/cleanup。

所以“没有任何提示”不准确。更准确的问题是：这些入口偏 pull-first，用户需要主动打开面板；还没有一个轻量、可消退的 pending proposal nudge，例如在 closeout 后显示“有 N 条待审，运行 /memory-proposals list --status proposed”。

### 5. run coordinator 是自动化审计应纳入的核心能力

当前已有：

- `SessionRunCoordinator` 保证单 session 最多一个 active run。
- `wake()` / `accept_wake()` 合并 drain 请求，避免并发 drain。
- `session_inputs` 持久化 pending/promoted/running/cancelled 状态。
- API 路径支持 idempotency key、queue/admit/steer/run delivery。
- TUI 在 run 完成后可 drain 下一条 queued input。

这说明项目已经有“用户 follow-up 自动排队/唤醒/继续”的基础，不应只用记忆写入来衡量自动化程度。

## 原文问题修正

### 1. “记忆系统形同虚设”表述过重

**状态：文档已修正，代码未变。**  
默认不自动持久化长期记忆是真的，但系统仍会加载 pinned/static memory、构建 retrieval context、生成 review proposals、暴露 proposal queue，并可通过 narrow/legacy 配置改变写入策略。

建议把问题描述为：记忆候选可见性和 review flow 还不够主动，而不是记忆系统不可用。

### 2. “后台审查每 10 轮自动运行”需要加条件

**状态：文档已修正，代码未变。**  
`advance_nudge()` 默认 10 轮触发，但如果本轮调用了 memory tool 会重置；触发后还需要 provider 存在才会 spawn background review。生成结果仍进入 review queue。

### 3. “时间压缩自动触发”不准确

**状态：文档已修正，代码未变。**  
`needs_time_based_compression()` 和 `PRIORITY_AGENT_TIME_BASED_COMPRESSION` 存在，但当前未看到主对话循环稳定调用。它不应被列为默认全自动能力。

### 4. “后台修剪默认关闭但类似选择性压缩”需要分层

**状态：文档已修正，代码未变。**  
选择性压缩是请求准备阶段默认启用的 request-local 压缩；background prune helper 是另一个默认关闭且未见主路径接入的能力。两者不能简单等同。

### 5. `/memory status` 已实现

**状态：原 TODO 已不成立。**  
TUI slash command 已支持 `/memory status` 和 `/memory status --json`，展示 use/generate/recall/write-policy/active 和解释文本。

### 6. “提案无通知”应改为“缺少强提醒”

**状态：文档已修正，代码未变。**  
现有可见性入口不少，但主要是面板/命令式。建议补一个低噪声 closeout nudge 或 status bar badge，而不是说完全无通知。

### 7. “将默认记忆策略改为 narrow”应降级为产品决策

**状态：文档已修正，代码未变。**  
`narrow` 已实现，是否默认开启应由产品安全边界决定。当前更推荐先增加 review queue 可见性、批量审阅体验和 explicit opt-in，而不是直接改默认。

## 后续建议

### P0：保持安全边界，提升可见性

- [ ] 在 closeout 后增加低噪声 proposal nudge：显示 proposed 数量和下一步命令。
- [ ] 在 TUI 状态栏或 `/quick` 顶部显示 pending proposal badge。
- [ ] 在 `/memory status` 中直接显示 proposed/accepted/applied 数量，并给出最短下一步。
- [ ] 给 `/memory-proposals apply --accepted` 增加更清晰的成功/失败摘要和 rollback 提示。

### P1：整理配置与生产状态

- [ ] 新增“自动化配置矩阵”文档，区分 read-only retrieval、request-local compression、history rewrite、long-term memory write、experimental helper。
- [ ] 把 `PRIORITY_AGENT_ACTIVE_MEMORY`、`PRIORITY_AGENT_AUTO_MEMORY_WRITE`、`PRIORITY_AGENT_SELECTIVE_COMPRESSION` 等开关在 `/doctor` 或 `/config` 中集中展示。
- [ ] 标注 `BACKGROUND_PRUNE`、`TIME_BASED_COMPRESSION`、`CONTEXT_COLLAPSE` 是未接入/实验路径，避免误判生产行为。

### P2：可选的更主动记忆策略

- [ ] 评估是否在个人本地默认启用 `narrow`，但只限显式偏好、非敏感、无冲突内容。
- [ ] 增加 first-run 或 `/memory control write narrow|review_only|legacy` 的明确 opt-in。
- [ ] 扩展 narrow 规则前先补测试，覆盖中文/英文偏好、否定表达、敏感信息、临时任务状态和重复记忆。

### P3：自动化链路测试

- [ ] 增加 closeout proposal nudge 的 TUI/trace 测试。
- [ ] 增加 active memory gate 的 regression 测试，确保 eval/headless/automation/internal 不运行。
- [ ] 增加 run coordinator queue/wake/drain 的 API/TUI 集成测试。
- [ ] 增加 `memory status --json` 包含 proposal queue 的测试。

## 本次文档更新已完成

- [x] 将审计范围从“记忆自动写入”扩展为整体自动化能力。
- [x] 修正 `narrow` 自动写入已实现的事实。
- [x] 修正 `/memory status` 已实现的事实。
- [x] 修正 active memory 默认关闭但 gated/read-only 的说明。
- [x] 修正时间压缩、后台修剪的接入状态。
- [x] 补充 run coordinator queue/wake/drain 自动化能力。
- [x] 补充 memory proposal 的现有可见入口。
- [x] 将“默认改 narrow”调整为需产品决策的 opt-in/可选策略。

## 建议验证

本文是文档更新，未改运行时代码。若后续按本文修改实现，建议按范围运行：

```bash
cargo fmt --check
cargo test -q memory
cargo test -q active_memory
cargo test -q run_coordinator
cargo test -q closeout
cargo test -q tui::slash_handler
cargo check -q
```
