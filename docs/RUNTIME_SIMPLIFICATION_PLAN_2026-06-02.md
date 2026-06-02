# 运行时精简优化计划

日期：2026-06-02
状态：讨论稿，待评审

本文档详细规划了 Priority Agent 运行时的 4 项精简优化。
每项包含现状分析、具体改动、影响评估和验收标准，可直接作为执行清单。

---

## 1. 配置系统精简（18 key → ~10 key）

### 1.1 现状

配置定义在 `src/services/config.rs`，当前有 **18 个可配置 key**：

```
AppConfig
├── api (5): base_url, model, temperature, max_tokens, api_key
├── ui (3): theme, show_token_usage, compact_mode
├── storage (3): data_dir, persistence_enabled, auto_save_interval_secs
├── features (7): tui_enabled, agent_enabled, mcp_enabled, skills_enabled,
│                 web_search, llm_memory_extraction, plugin_trust_mode
├── engine (2): max_iterations, mcp_servers
└── memory.external_provider (13): enabled, provider_type, name, records_path,
    prompt_block, prefetch, search, queue_prefetch, sync_turn, session_end,
    pre_compress, write_mirror, tools
```

加上 `ConfigLoader`/`ConfigHook` 回调系统、三级 scope path、`CONFIG_KEY_SPECS` schema、`config_schema_json()`、`redacted_config_export()` 等辅助设施，**总代码约 800 行**。

对照 Reasonix：配置约 6 个 key（model、theme、edit mode、max iterations、endpoint、cache）。

### 1.2 运行时消费分析

以下 key **从未被渲染/决策代码读取**（仅在 settings 面板出现）：

| Key | 定义位置 | 运行时消费 |
|-----|---------|-----------|
| `ui.compact_mode` | `config.rs:UiConfig` | ❌ 无渲染代码消费 |
| `features.tui_enabled` | `config.rs:FeatureFlags` | ❌ 无分支检查此 flag |
| `features.agent_enabled` | `config.rs:FeatureFlags` | ❌ 无运行时检查 |
| `storage.auto_save_interval_secs` | `config.rs:StorageConfig` | ❌ 无自动保存逻辑 |

以下 key **仅在 memory provider 初始化时消费一次**，且大多数值始终为默认值：

| Key | 默认值 | 实际使用 |
|-----|--------|---------|
| `memory.external_provider.name` | `"external-memory"` | 仅传给 `MemoryProviderCapabilities` |
| `memory.external_provider.prompt_block` | `true` | 同上 |
| `memory.external_provider.prefetch` | `true` | 同上 |
| `memory.external_provider.search` | `true` | 同上 |
| `memory.external_provider.queue_prefetch` | `false` | 同上 |
| `memory.external_provider.sync_turn` | `false` | 同上 |
| `memory.external_provider.session_end` | `false` | 同上 |
| `memory.external_provider.pre_compress` | `false` | 同上 |
| `memory.external_provider.write_mirror` | `false` — 且 validation 禁止设为 true |
| `memory.external_provider.tools` | `false` — 且 validation 禁止设为 true |

10 个 external provider 子 key 中，仅 `enabled`、`provider_type`、`records_path` 有实际语义。

### 1.3 建议改动

**第一阶段（安全，纯删除）**：
- 删除 `ui.compact_mode`、`features.tui_enabled`、`features.agent_enabled`、`storage.auto_save_interval_secs`
- 从 `UiConfig`、`FeatureFlags`、`StorageConfig` 移除对应字段
- 从 `AppConfig::load()` 的 `set_default` 移除对应行
- 从 `CONFIG_KEY_SPECS` 移除对应条目
- 从 `get_config_value()`/`set_config_value()` 移除对应分支
- 从 `format_config_summary()` 移除对应参数
- 从 `validate_config()` 移除对应检查

影响文件：`src/services/config.rs`（约 -80 行）
协议兼容：向后兼容（TOML 中多余的 key 会被忽略，多余的 key 在 `get_config_value` 返回 `None`）

**第二阶段（需讨论）**：
- 将 `memory.external_provider` 的 10 个 sub-key 合并为 3 个（enabled、provider_type、records_path）
- 其余 7 个 flag（prompt_block 等）改为硬编码默认值
- 从 `ExternalMemoryProviderConfig` 移除对应字段
- 更新 `src/memory/provider_ops.rs` 的 `MemoryProviderCapabilities` 构造（从 config 字段 → 常量）

影响文件：`src/services/config.rs`（约 -60 行）、`src/memory/provider_ops.rs`（约 -15 行）

**第三阶段（可选）**：
- 删除 `ConfigLoader`/`ConfigHook`（仅测试使用，生产路径不触发）
- 合并三级 scope path 为二级（user/legacy）
- 删除 `config_schema_json()`、`redacted_config_export()`

### 1.4 风险

- **第一阶段低风险**：删除的是运行时零消费的字段
- **第二阶段中风险**：如果外部有 TOML 配置文件写了这些字段，反序列化会静默忽略（serde 默认行为），不会报错
- **第三阶段需确认**：`ConfigLoader` 的 `load_from_env()` 是否有非测试调用方

### 1.5 验收

- `cargo check` clean
- `cargo test --lib services::config::tests` 全部通过
- 配置文件 `config.toml` 中写旧字段不报错（向后兼容验证）
- 最终 config key 数从 18 降到 ≤10

---

## 2. 工具注册精简（80+ tools → ~45）

### 2.1 现状

工具注册在 `src/tools/mod.rs:1643-1788` 的 `with_profile()` 函数。
当前共有 **82 个注册工具**（73 个 Core + 9 个 Full-only）。

工具分为两个 profile：
- `Core`（默认）：73 个工具
- `Full`（`PRIORITY_AGENT_TOOL_PROFILE=full`）：额外 9 个工具

### 2.2 分类分析

**A. 核心编码工具（必须保留）— 约 15 个**

| 工具 | 用途 |
|------|------|
| `file_read` / `file_write` / `file_edit` / `file_patch` | 文件操作 |
| `glob` / `grep` | 搜索 |
| `bash` / `bash_output` / `bash_cancel` | Shell 执行 |
| `diff` / `git` / `git_status` / `git_diff` | 版本控制 |
| `symbol_query` / `project_list` | 代码理解 |

**B. Bash 包装器（可删除，bash 可替代）— 3 个**

| 工具 | 等价 bash 命令 |
|------|---------------|
| `run_tests` | `bash` + `cargo test` |
| `start_dev_server` | `bash` + `npm dev` |
| `install_dependencies` | `bash` + `pip install` |

这些工具本质上是预设的 bash 命令，没有专门的参数校验或后处理。
删除后模型可以直接用 `bash` 执行等价命令。

**C. Task 管理工具（可删除，过度工程化）— 6 个**

| 工具 | 说明 |
|------|------|
| `task_create` / `task_get` / `task_list` / `task_update` / `task_stop` / `task_output` | 6 个文件均在 `src/tools/task_tool/mod.rs` |

这些工具模仿 Claude Code 的 TodoWrite 系统，但 Claude Code 只有 1 个 `todo_write`，
Reasonix 完全不提供 task 管理工具（交给模型的 reasoning 自行规划）。

当前项目已有 `todo_write` 工具（#26），task_* 工具与其功能重叠。
**建议**：保留 `todo_write`，删除全部 6 个 task_* 工具。

**D. 会话/UI 工具（可删除，slash command 替代）— 4 个**

| 工具 | slash command 替代 |
|------|-------------------|
| `cost` | `/cost` |
| `brief` | 无 — 用途不明 |
| `clear` | `/clear` |
| `config` | `/config` |

这些工具不应该是模型可调用的工具（模型不需要关心 cost/clear/config）。
应该只保留为 slash command，从 tool registry 中移除。

**E. 效用工具（可保留但需评估）— 约 10 个**

| 工具 | 评估 |
|------|------|
| `calculate` | 简单计算，有时有用 |
| `datetime` | 日期时间查询 |
| `json_query` | JSON 数据查询，偶尔有用 |
| `encode` | 编解码 |
| `notebook` | Jupyter notebook 操作 |
| `repl` | REPL 交互 |
| `powershell` | Windows PowerShell |
| `send_message` | 发送消息 |
| `share` | 分享 |
| `tool_search` | 工具搜索 |
| `sleep` | 等待 |

这些工具各有用处，但使用频率低。建议保留。

**F. 计划/推理工具 — 3 个**

| 工具 | 评估 |
|------|------|
| `enter_plan_mode` / `exit_plan_mode` | Plan 模式管理 |
| `socratic_analyze` | 苏格拉底式分析 |
| `plan` | Plan 审批工具 |

保留。

**G. MCP/集成工具 — 7 个**

保留：`mcp`、`mcp_tool`、`mcp_auth`、`list_mcp_resources`、`read_mcp_resource`、`lsp`、`worktree`

**H. Agent/协作工具 — 7 个**

保留：`agent`、`swarm`、`workbench`、`refactor`、`cron`、`skill_manage`、`skills_list`、`skill_view`

**I. 其他 — 约 10 个**

| 工具 | 保留？ |
|------|--------|
| `web_fetch` / `web_search` | ✅ 保留 |
| `memory_save` / `memory_load` / `memory_clear` | ✅ 保留 |
| `todo_write` | ✅ 保留 |
| `context` / `context_visualization` | ✅ 保留 |
| `copy` / `resume` / `rewind` | ✅ 保留 |
| `format` / `github` | ✅ 保留 |
| `ask_user` | ✅ 保留 |

### 2.3 建议改动

**删除 13 个工具**：
```
run_tests, start_dev_server, install_dependencies,     (3 bash wrappers)
task_create, task_get, task_list, task_update, task_stop, task_output,  (6 task tools)
cost, brief, clear, config                              (4 session/UI tools)
```

**删除对应的源文件**：
```
src/tools/run_tests_tool.rs
src/tools/start_dev_server_tool.rs
src/tools/install_dependencies_tool.rs
src/tools/task_tool/mod.rs (整个目录)
src/tools/cost_tool/mod.rs
src/tools/brief_tool/mod.rs
src/tools/clear_tool/mod.rs
src/tools/config_tool/mod.rs
```

**修改文件**：
- `src/tools/mod.rs`：从 `with_profile()` 移除 13 个注册调用
- `src/tools/mod.rs`：移除对应的 `mod` 声明和 `pub use`

### 2.4 风险

- **模型行为变化**：模型可能依赖被删的工具。需要跑 live eval 验证
- **import 清理**：被删模块可能有其他模块的 import，需确认
- **测试**：`src/tools/task_tool/mod.rs` 有测试，删除测试需确认无其他测试依赖

### 2.5 验证

```
cargo check -q
cargo test -q --lib tools::
scripts/run_live_eval.sh --case <推荐用例> --mode agent-run
```

---

## 3. 启动组件 Lazy 初始化

### 3.1 现状

`src/bootstrap.rs:init_components()` 在每次启动时创建 11 个组件。
其中 3 个可以安全延迟到首次使用时初始化，减少启动 I/O：

| 组件 | 启动行为 | 可 lazy？ |
|------|---------|-----------|
| **SessionStore** | 打开 SQLite DB，写入 session 行 | ✅ 首次 query 时 |
| **MemoryManager** | 创建目录，读配置，初始化 external provider | ✅ 首次 memory 快照时 |
| **AgentManager** | 仅 heap alloc，无 I/O | ✅ 首次 agent 分发时 |

### 3.2 建议改动

在 `StreamingQueryEngine` 中添加 3 个 `OnceLock` 字段：

```rust
// 新增字段（src/engine/streaming.rs）
session_store: OnceLock<Option<Arc<SessionStore>>>,
memory_manager: OnceLock<Arc<MemoryManager>>,
agent_manager: OnceLock<Arc<AgentManager>>,
```

提供 accessor 方法：

```rust
impl StreamingQueryEngine {
    pub fn session_store(&self) -> Option<&Arc<SessionStore>> {
        self.session_store.get_or_init(|| {
            // 原 bootstrap.rs:218-227 的初始化逻辑移到这里
        }).as_ref()
    }

    pub fn memory_manager(&self) -> &Arc<MemoryManager> {
        self.memory_manager.get_or_init(|| {
            // 原 bootstrap.rs:286-292 的初始化逻辑移到这里
        })
    }

    pub fn agent_manager(&self) -> Option<&Arc<AgentManager>> {
        self.agent_manager.get_or_init(|| {
            // 原 bootstrap.rs:300-301 的初始化逻辑移到这里
        })
    }
}
```

### 3.3 改动文件

- `src/bootstrap.rs`：移除 3 个组件的创建，改为传递初始化参数到 engine
- `src/engine/streaming.rs`：添加 3 个 `OnceLock` 字段和 accessor
- `src/engine/query_engine.rs`：更新 `agent_manager()` / `memory_manager()` 调用为 engine 方法
- `src/tui/app.rs`：更新 `flush_memory_for_current_history` 调用路径

### 3.4 影响

- 启动时间减少约 100-300ms（取决于 SQLite 和文件系统速度）
- CLI `--cli` 首次输入前不创建 session/memory/agent
- 向后兼容：accessor 透明返回相同类型，调用方无感知

### 3.5 验收

```
cargo check -q
cargo test -q --lib engine::streaming
cargo test -q --lib tui::
```

---

## 4. 权限模式评估

### 4.1 现状

`src/permissions/mod.rs:58-72` 定义了 5 种 `PermissionMode`：

```rust
pub enum PermissionMode {
    Default,       // 根据规则决定（只问规则说 Ask 的）
    AutoLowRisk,   // 自动允许低风险操作
    AutoAll,       // [默认] 开发者自动模式
    ReadOnly,      // 只读模式
    Once,          // 一次性授权模式
}
```

### 4.2 消费分析

经审计，**所有 5 种变体都在 `requires_confirmation()` 决策逻辑中有独立分支**（`src/permissions/mod.rs:394-457`）。不是 dead code。

但它们的实际使用分布极不均匀：

| 变体 | 默认值？ | 实际使用场景 |
|------|---------|-------------|
| `AutoAll` | ✅ 是 | 开发者日常使用的默认模式 |
| `Default` | ❌ | 按规则决定，用户手动 `/permissions default` 切换 |
| `ReadOnly` | ❌ | plan sub-agent 使用（`conversation_loop/mod.rs:1091`） |
| `Once` | ❌ | `/permissions once` 手动切换 |
| `AutoLowRisk` | ❌ | 无特定场景，可能是早期设计遗留 |

### 4.3 建议

**不建议删除任何变体**（它们都在决策逻辑中），但可以做以下简化：

1. **合并 `Default` 和 `AutoLowRisk`**：`Default` 已经是"按规则决定"，`AutoLowRisk` 只在 score 上略有不同（0.75 vs 0.7）。可以将 `AutoLowRisk` 的语义合并到 `Default` 中。

2. **保留 4 种模式**：`Default`、`AutoAll`、`ReadOnly`、`Once`

3. **简化 confidence scoring**：将 5-分支 match 简化为 3 级（AutoAll=0.9, Once=0.8, ReadOnly/Default=0.7）

改动范围极小，约 10 行。

### 4.4 验收

```
cargo test -q --lib permissions::
```

---

## 执行顺序建议

| 顺序 | 优化项 | 风险 | 预计改动行数 |
|------|--------|------|-------------|
| 1 | 配置精简 Phase 1（删 4 个死字段） | 低 | -80 行 |
| 2 | 工具精简（删 13 个工具） | 高 | -2000 行 |
| 3 | 启动 lazy init（3 个 OnceLock） | 中 | +30/-50 行 |
| 4 | 配置精简 Phase 2（external provider 合并） | 中 | -60 行 |
| 5 | 权限合并（AutoLowRisk → Default） | 低 | -15 行 |

建议先执行 1、3、5（低风险），验证后再执行 2、4（需 live eval 验证）。
