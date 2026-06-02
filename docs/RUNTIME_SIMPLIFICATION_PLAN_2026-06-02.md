# 运行时精简优化计划

日期：2026-06-02
状态：评审后修订版

本文档规划 Priority Agent 运行时的精简路线。目标不是简单追求更少代码，
而是让模型默认看到更少、更清晰的能力，同时保留验证、权限、checkpoint、
证据记录和诚实 closeout 这些硬约束。

参考对象：`/Users/georgexu/Downloads/DeepSeek-Reasonix-main`。
Reasonix 的可取点不是没有护栏，而是把模型可见工具压成少数明确能力：
文件、shell/jobs、todo/plan、memory/skills/web/MCP；安全和恢复主要挂在
shell allowlist、edit gate、pause gate、sandbox、hook 和 tool dispatcher 上。
Priority Agent 可以学习这个方向，但不能把已有的 runtime proof 边界删掉。

---

## 0. 精简原则

### 0.1 保留硬边界

以下能力不能为了减少工具数或让弱模型更容易通过 eval 而削弱：

- 权限和 action review：高风险、网络、安装、原始 bash 写入、外部路径访问仍需确定性拦截或确认。
- checkpoint 和文件变更证据：文件工具、format、diff、git evidence 的绑定关系必须保留。
- 验证闭环：验证失败必须形成 `ToolObservation`，进入下一轮上下文，阻止虚假 verified closeout。
- route-scoped tools：默认暴露面可以收缩，但特定 route 需要的工具应按需暴露。
- memory gates：记忆可以精简配置，但持久化、review、scope、敏感信息过滤不能弱化。

### 0.2 优先减少“模型可见复杂度”

Reasonix 的启发是：工具实现可以有内部复杂度，但模型看到的入口要少。
因此本项目的优先级应是：

1. 缩小默认 tool exposure，而不是先删除工具实现。
2. 合并重复的模型入口，而不是删除 runtime evidence 分类。
3. 删除真正没有协议价值、没有运行时消费、没有 UI/API 语义的配置。
4. lazy 初始化冷路径组件，但保留 `Option`/失败语义和现有调用方行为。

---

## 1. 配置系统精简

### 1.1 当前状态

配置定义在 `src/services/config.rs`，包含：

```text
AppConfig
├── api: base_url, model, temperature, max_tokens, api_key
├── ui: theme, show_token_usage, compact_mode
├── storage: data_dir, persistence_enabled, auto_save_interval_secs
├── features: tui_enabled, agent_enabled, mcp_enabled, skills_enabled,
│            web_search, llm_memory_extraction, plugin_trust_mode
├── engine: max_iterations, mcp_servers
└── memory.external_provider: enabled, provider_type, name, records_path,
   prompt_block, prefetch, search, queue_prefetch, sync_turn, session_end,
   pre_compress, write_mirror, tools
```

辅助设施包括 `ConfigLoader`/`ConfigHook`、scope paths、`CONFIG_KEY_SPECS`、
`config_schema_json()`、`redacted_config_export()`、summary/get/set/validate。

### 1.2 修正后的消费分析

原计划把部分字段标为“只在 settings 出现”，这个判断需要修正：

| Key | 当前消费 | 结论 |
|-----|----------|------|
| `ui.compact_mode` | settings、API config、`/focus` 持久化 | 可删，但要同步改 `/focus` 和 API DTO |
| `features.tui_enabled` | settings、API config | 可删，但属于外部配置协议收缩 |
| `features.agent_enabled` | settings、API config | 可删，但属于外部配置协议收缩 |
| `storage.auto_save_interval_secs` | config validation、settings | 可删，当前没有实际 autosave loop |

`memory.external_provider` 的大量 capability flag 只在 provider 初始化时使用。
其中 `write_mirror`/`tools` 还被 validation 禁止打开。这里可以收缩，但要把
capability 常量化，不要移除 read-only external memory provider 的安全语义。

### 1.3 Phase 1：删除 4 个低价值配置字段

改动范围：

- `src/services/config.rs`
  - 删除 `ui.compact_mode`
  - 删除 `features.tui_enabled`
  - 删除 `features.agent_enabled`
  - 删除 `storage.auto_save_interval_secs`
  - 同步更新 defaults、`CONFIG_KEY_SPECS`、summary、get/set、validation、tests
- `src/tui/components/settings.rs`
  - 删除对应 settings 项
- `src/tui/slash_handler/config.rs`
  - `/focus` 不再写 `config.ui.compact_mode`
  - focus mode 改为纯会话状态，或新增更明确的 `focus` 会话状态持久化方案
- `src/api/state.rs`、`src/api/routes.rs`
  - 删除 config response/update DTO 中对应字段

风险：中低。
不是“纯 dead code 删除”，因为会改变 `/focus` 的持久化行为和 API shape。
如果保留 API 兼容性，可先让 update request 忽略旧字段，再在后续版本删除 DTO 字段。

验收：

```bash
cargo fmt --check
cargo check -q
cargo test -q services::config
cargo test -q tui::slash_handler::config
cargo test -q api
```

### 1.4 Phase 2：收缩 external memory provider 配置

目标字段：

```text
memory.external_provider.enabled
memory.external_provider.provider_type
memory.external_provider.records_path
```

改动：

- `ExternalMemoryProviderConfig` 只保留上述 3 个字段。
- `MemoryProviderCapabilities` 中以下值改为常量：
  - `name = "external-memory"`
  - `prompt_block = true`
  - `prefetch = true`
  - `search = true`
  - `queue_prefetch = false`
  - `sync_turn = false`
  - `session_end = false`
  - `pre_compress = false`
  - `write_mirror = false`
  - `tools = false`
- 旧 TOML 字段允许被 serde 忽略；测试覆盖旧字段不报错。

风险：中。
这是配置协议收缩，但安全方向更清晰：外部 provider 只能是只读背景上下文。

验收：

```bash
cargo fmt --check
cargo check -q
cargo test -q memory::provider_ops
cargo test -q services::config
```

### 1.5 暂缓项

暂缓删除：

- `config_schema_json()`
- `redacted_config_export()`
- `config_scope_paths()`
- `ConfigLoader`/`ConfigHook`

原因：`redacted_config_export()` 被 `/config export` 和 `config` tool 使用；
`config_scope_paths()` 被 `/config paths` 使用。第三阶段应先决定是否保留
`config` tool 或仅保留 `/config` slash command，再处理这些辅助函数。

---

## 2. 工具面精简

### 2.1 当前问题

`src/tools/mod.rs::with_profile()` 默认注册大量工具。真正的问题不是实现文件多，
而是模型默认暴露面过宽，导致：

- 工具选择噪声大；
- route-specific 工具混入普通编码任务；
- session/UI 命令和模型 action 混在一起；
- 重复任务工具和 `todo_write` 并存；
- 一些工具名已经被 action policy、evidence、route tests、scenario matrix 绑定，
  直接删除会破坏 runtime contract。

### 2.2 Reasonix 对照

Reasonix 工具面更简单，但不是完全无结构：

- shell 合并为 `run_command`、`run_background`、`job_output`、`wait_for_job`、
  `stop_job`、`list_jobs`。
- 文件工具少数原语覆盖读、搜索、写、编辑、移动、删除。
- `todo_write` 是单一 in-session task tracker，不维护一组 task CRUD 工具。
- plan mode 在 registry dispatch 层拒绝非 readonly 工具。
- shell mutating/network/install 通过 allowlist 和 pause gate 确认，而不是靠模型自觉。

对 Priority Agent 的启发：

- 可以减少模型可见入口，但不要把 `run_tests`、`start_dev_server`、
  `install_dependencies` 的确定性安全语义退化成裸 `bash`。
- 更适合先做 exposure profile，而不是删实现。

### 2.3 工具分层

#### A. 默认编码核心

默认编码 route 应优先暴露：

```text
file_read, file_write, file_edit, file_patch
glob, grep
bash, bash_output, bash_cancel
diff, git_status, git_diff
todo_write
ask_user
```

按 route 追加：

```text
run_tests              # BugFix/CodeChange 验证 route
start_dev_server       # Frontend/App route 或显式启动服务意图
install_dependencies   # 只有 dependency install intent
git                    # 只有明确 git mutation intent
format                 # 只有 formatting intent 或 post-edit verify
symbol_query, lsp      # 代码理解 route
web_search, web_fetch  # research/current-info route
mcp*, skills*, memory* # 显式集成/记忆/技能 route
agent, swarm, workbench, refactor, cron # 显式协作/批处理 route
```

#### B. 不应默认暴露给模型的 session/UI 工具

候选：

```text
cost, clear, config, brief
```

建议：

- 第一阶段：从默认 exposure 中移除，保留注册和 slash command。
- 第二阶段：确认没有 route 需要后，从 tool registry 删除。
- `/config`、`/clear`、`/cost` 继续作为用户命令存在。
- `brief` 先查清调用价值；如果无 slash/API 替代，可直接删除。

#### C. task_* 工具

候选：

```text
task_create, task_get, task_list, task_update, task_stop, task_output
```

建议删除，但分两步：

1. 先从 default/route exposure 中移除，只保留 `todo_write`。
2. 跑 route tests 和 live eval 后，再删除 `src/tools/task_tool/`、
   `task_manager` 注入、permissions/action weights/reliability samples 里的 task 分支。

删除时必须同步：

- `src/tools/mod.rs`
- `src/tools/reliability.rs`
- `src/permissions/mod.rs`
- `src/engine/workflow/weights.rs`
- `src/engine/conversation_loop/tool_orchestrator.rs`
- `src/engine/conversation_loop/route_scoped_tools_tests.rs`
- `src/tools/examples.rs`

#### D. 暂不删除的 facade tools

以下工具不应在第一轮删除：

```text
run_tests
start_dev_server
install_dependencies
```

原因：

- `run_tests` 是 safe validation facade，拒绝 mutation/install/network/任意 shell，
  并产出结构化 `validation_result`。
- `start_dev_server` 是 background task facade，带 dev-server 分类和 metadata。
- `install_dependencies` 是 package manager contract，参数化生成 argv，
  明确 `requires_confirmation` 和 network/install 风险。

可做的精简：

- 默认不暴露 `start_dev_server` 和 `install_dependencies`。
- `run_tests` 只在需要验证的 route 暴露。
- 如果未来要向 Reasonix 靠拢，可把三者折叠为 `bash` 的 runtime intent wrapper，
  但 evidence/result/action policy 必须保留同等结构化分类。

### 2.4 建议执行顺序

#### Phase 1：收缩默认 exposure，不删实现

目标：

- 默认编码工具面降到约 20 个。
- route-specific 工具仍能按需暴露。
- 不改 tool implementation。

验证：

```bash
cargo fmt --check
cargo check -q
cargo test -q route_scoped_tools
cargo test -q tool_exposure
cargo test -q tool_orchestrator
cargo test -q action_review
```

#### Phase 2：删除 task_* 工具

目标：

- `todo_write` 成为唯一 in-session task tracker。
- 删除 task CRUD 工具和相关注入。

验证：

```bash
cargo fmt --check
cargo check -q
cargo test -q tools
cargo test -q permissions
cargo test -q route_scoped_tools
cargo test -q tool_orchestrator
```

#### Phase 3：删除 session/UI tools

目标：

- `cost`、`clear`、`config` 只保留 slash command。
- `brief` 若无明确用户价值则删除。

注意：

- 删除 `config` tool 前必须确认 `/config export` 仍覆盖 schema/export 需求。
- 删除 `clear` tool 前确认模型没有合法场景需要主动清理上下文。

验证：

```bash
cargo fmt --check
cargo check -q
cargo test -q tools
cargo test -q permissions
cargo test -q closeout
```

#### Phase 4：重新评估 facade tools

只有在 Phase 1-3 稳定后，再评估是否把 `run_tests`、`start_dev_server`、
`install_dependencies` 合并进更通用的 shell/jobs facade。

必要条件：

- `bash` 能产出等价 `validation_result`、`dev_server`、`install` evidence。
- action policy 仍能识别 install/network/local machine mutation。
- route/scenario matrix 更新且 live eval 没有 false green closeout。

---

## 3. 启动组件 Lazy 初始化

### 3.1 当前状态

`src/bootstrap.rs::init_components()` 每次启动会创建：

- `SessionStore`：打开 SQLite DB，并创建 session 行。
- `MemoryManager`：创建目录、读配置、初始化 external provider、freeze snapshot。
- `AgentManager`：构造 agent manager 并绑定 query engine。

这些可以减少启动 I/O，但不能破坏当前 `Option<Arc<...>>` 语义。

### 3.2 修正后的设计

不要直接把字段改成非 optional 的 `OnceLock<Arc<...>>`。
应保留三态：

```text
disabled / open failed / initialized
```

建议形状：

```rust
session_store: OnceLock<Option<Arc<SessionStore>>>
memory_manager: OnceLock<Option<Arc<Mutex<MemoryManager>>>>
agent_manager: OnceLock<Option<Arc<AgentManager>>>
```

或使用显式 lazy factory：

```rust
LazyComponent<T> {
    enabled: bool,
    init: OnceLock<Option<Arc<T>>>,
}
```

`memory_manager` 必须保留 `Arc<tokio::sync::Mutex<MemoryManager>>`，
因为现有调用方依赖 lock 后做 snapshot/prefetch/sync。

### 3.3 分阶段执行

#### Phase 1：lazy AgentManager

风险最低。`AgentManager` 基本无 I/O，只是减少启动对象图和默认 wiring。

改动：

- `StreamingQueryEngine::agent_manager()` 首次调用时创建。
- 保留返回 `Option<Arc<AgentManager>>`。
- `QueryEngine` 相关调用保持行为一致。

验证：

```bash
cargo fmt --check
cargo check -q
cargo test -q agent
cargo test -q route_scoped_tools
```

#### Phase 2：lazy SessionStore

改动：

- 首次需要 session binding、ledger、history persistence 时打开。
- 创建 session 行延迟到首次 query，而不是进程启动。
- `session_binding()` 仍返回 `Option<(Arc<SessionStore>, String)>`。
- 打开失败仍是 `None`，不能 panic。

验证：

```bash
cargo fmt --check
cargo check -q
cargo test -q session
cargo test -q context_ledger
cargo test -q conversation_loop
```

#### Phase 3：lazy MemoryManager

风险最高，最后做。

改动：

- 首次 memory snapshot/prefetch/sync/flush 时初始化。
- 保留 `llm_memory_extraction` feature 开关。
- 初始化后仍执行 `freeze_snapshot()`，但时间点变为首次需要 memory 时。
- `engine.memory_manager().is_some()` 这种能力判断要谨慎替换，避免查询动作反而触发初始化。

建议新增两个方法：

```rust
memory_manager_if_initialized()
memory_manager_or_init()
```

验证：

```bash
cargo fmt --check
cargo check -q
cargo test -q memory_snapshot
cargo test -q memory_sync
cargo test -q prompt_context
cargo test -q closeout
```

---

## 4. 权限模式评估

### 4.1 当前状态

`PermissionMode` 包含：

```rust
Default
AutoLowRisk
AutoAll
ReadOnly
Once
```

原计划认为 `Default` 和 `AutoLowRisk` 只差 confidence score。这个判断不准确。

### 4.2 语义差异

- `Default`：根据规则决定，只有规则结果为 `Ask` 时询问。
- `AutoLowRisk`：规则优先；未命中规则时，`RiskLevel >= Medium` 询问。
- `AutoAll`：开发者默认自动模式，但保留 deny/ask 规则和高风险兜底。
- `ReadOnly`：计划/只读探索路径使用。
- `Once`：一次性授权模式。

`AutoLowRisk` 还暴露在 TUI `/permissions mode`、mode parser、action review tests、
permissions tests 中。因此合并它不是 10 行清理，而是权限 UX 语义变更。

### 4.3 建议

短期保留所有 5 种模式。

可做的精简：

- 文案层面把 `AutoLowRisk` 标为“保守自动模式”。
- `/permissions mode` 输出里说明 `default` 和 `auto_low_risk` 的差异。
- confidence scoring 可简化，但不要改变 `requires_confirmation()` 行为。

如果未来要合并：

- 优先删除 `Default`，让 `auto_low_risk` 成为“规则 + 风险”的普通模式；
  或者保留 `Default`，但把它语义改为当前 `AutoLowRisk`。
- 需要迁移 `/permissions` 文案、parser、tests、action_review 测试。
- 必须跑 permissions/action_review/action_policy 全套。

验收：

```bash
cargo fmt --check
cargo check -q
cargo test -q permissions
cargo test -q action_review
cargo test -q action_policy
```

---

## 5. 推荐执行顺序

| 顺序 | 工作 | 风险 | 目标 |
|------|------|------|------|
| 1 | 工具默认 exposure 收缩 | 中 | 先减少模型可见复杂度，不删实现 |
| 2 | 配置 Phase 1 | 中低 | 删除 4 个低价值字段，同步 UI/API/slash |
| 3 | Lazy AgentManager | 低 | 验证 lazy 形状和调用方习惯 |
| 4 | 删除 task_* | 中 | 用 `todo_write` 取代 task CRUD |
| 5 | Lazy SessionStore | 中 | 减少启动 SQLite/session 写入 |
| 6 | 配置 Phase 2 | 中 | external memory provider 配置常量化 |
| 7 | 删除 session/UI tools | 中 | slash command 保留，tool registry 收缩 |
| 8 | Lazy MemoryManager | 中高 | 最后处理 memory snapshot/prefetch/sync |
| 9 | 权限模式合并评估 | 中 | 单独作为权限 UX 变更 |
| 10 | facade tools 合并评估 | 高 | 只有 evidence/action policy 等价后才做 |

---

## 6. 全局验收门槛

每个阶段至少运行对应窄测试；触及共享 runtime contract 时加宽：

```bash
cargo fmt --check
cargo check -q
cargo test -q route_scoped_tools
cargo test -q action_review
cargo test -q action_policy
cargo test -q permissions
cargo test -q closeout
```

触及工具 evidence、验证、live eval 或 closeout 时追加：

```bash
cargo test -q tool_execution_controller
cargo test -q tool_result_controller
cargo test -q prompt_context
bash scripts/workflow-production-gates.sh
bash -n scripts/run_live_eval.sh
python3 -m py_compile scripts/live_eval_report_parser.py
```

执行后再选择 2-3 个代表性 live eval：

- 简单代码修改 + 必须验证；
- 前端/服务启动场景；
- dependency install intent 场景；
- 权限拒绝或 validation failure 后的 repair 场景。

通过标准不是“工具数最少”，而是：

- 默认模型可见工具明显减少；
- route 需要的工具仍能出现；
- 验证证据和 closeout proof 不退化；
- 高风险/网络/install/raw bash 写入仍被正确分类；
- 弱模型失败时能 honest `failed`/`partial`/`not_verified`，不能 false green。
