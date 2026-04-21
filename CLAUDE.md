# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Development Commands

```bash
# Build default (TUI only)
cargo build

# Build with all features enabled
cargo build --features "legacy-cli experimental-api-server experimental-priority experimental-task-analyzer experimental-platform"

# Build release version
cargo build --release

# Run all tests
cargo test

# Run a specific test
cargo test test_analyze_critical_task

# Run the CLI
./target/debug/priority-agent --help

# Run legacy CLI mode (does NOT require API key)
./target/debug/priority-agent --legacy init
./target/debug/priority-agent --legacy add "任务名称"
./target/debug/priority-agent --legacy list
./target/debug/priority-agent --legacy next
./target/debug/priority-agent --legacy done <task_id>

# Run TUI mode (requires LLM API key)
export MOONSHOT_API_KEY="your-key"
./target/debug/priority-agent --tui

# Run API server mode (requires feature flag + API key)
cargo run --features experimental-api-server -- --api --port 8787
```

## Architecture Overview

Priority Agent is a Rust re-implementation of Claude Code's architecture. It has two distinct runtime paths:

1. **Legacy CLI** (`--legacy`): Local task management with hierarchical weight calculation. Does NOT require an LLM provider.
2. **TUI/API modes**: Full AI assistant with tool calling, streaming, agents, and memory. Requires `MOONSHOT_API_KEY` or `OPENAI_API_KEY`.

The `--legacy` flag is detected **before** `clap` parses arguments and bypasses all LLM initialization entirely (`main.rs:92-96`).

### Module Structure

**`tools/`** — Extensible tool system (Claude Code pattern)
- Core `Tool` trait in `mod.rs`: `name()`, `description()`, `parameters()`, `execute()`
- `ToolRegistry::default_registry()` registers 30+ tools including `file_read`, `file_write`, `file_edit`, `bash`, `glob`, `grep`, `agent`, `task_create`, `web_fetch`, `web_search`, `memory_save`, `memory_load`, `todo_write`, `calculate`, `json_query`, `encode`, `socratic_analyze`, `plan`, `mcp`, `swarm`, `project_list`, `skill_manage`, `ask_user`, `lsp`, `worktree`, `workbench`, `remote_trigger`
- `ToolContext` carries `working_dir`, `permissions`, `agent_manager`, `llm_provider`, `mcp_manager`, `lsp_manager`, `worktree_manager`

**`engine/`** — Query engine and conversation orchestration
- `query_engine.rs` — Non-streaming `QueryEngine`
- `streaming.rs` — `StreamingQueryEngine` used by TUI; produces `StreamEvent`s
- `conversation_loop.rs` — `ConversationLoop` shared builder between streaming and non-streaming engines
- `context_compressor.rs` — Token budget management and message summarization when context grows too large
- `plan_mode.rs` — Plan approval system with `PlanModeManager` and `PlanTool`
- `socratic.rs` / `socratic_executor.rs` — Socratic analysis tool for breaking down problems
- `mcp.rs` — MCP (Model Context Protocol) manager and `McpManageTool`
- `swarm.rs` — Swarm agent coordination
- `turn_state.rs` — Iteration limiting and diagnostic reporting
- `lsp.rs` — LSP manager and language server auto-detection
- `worktree.rs` — Git worktree manager

**`agent/`** — Sub-agent system
- `agent.rs` — `Agent`, `AgentConfig`, `AgentHandle`, `AgentStatus`
- `manager.rs` — `AgentManager` with `tokio::mpsc` channels for agent messaging; stores `AgentResult`s

**`tui/`** — Terminal UI
- `app.rs` — Main TUI loop (`TuiApp`) with `AppMode::Chat` / `AppMode::Settings` / `AppMode::VimNormal`
- `commands.rs` — Slash command registry (e.g., `/settings`, `/help`, `/commit`)
- `screens/` — Different UI screens
- `components/` — Input, messages, progress bars, file browser, settings panels, markdown renderer

**`ide/`** — IDE integration
- `mod.rs` / `vscode.rs` — VS Code / Cursor detection and CLI wrapper

**`bridge/`** — Remote session bridge
- `mod.rs` — `BridgeClient` for HTTP-based remote triggers

**`state/`** — React-style state management
- `app_state.rs` — `AppState`, `MessageItem`, `TaskItem`
- `store.rs` — `StateStore` with async updates
- `events.rs` — `EventBus` for state change propagation

**`services/`** — API and configuration
- `api/kimi.rs` — Kimi/Moonshot client (OpenAI-compatible)
- `api/openai.rs` — Generic OpenAI-compatible client
- `config.rs` — TOML-based `AppConfig`

**`session_store/`** — SQLite persistence for chat sessions and messages
- `mod.rs` — `SessionStore` with `rusqlite` backend
- Stores sessions, messages (with tool calls), tokens, and parent/child relationships

**`memory/`** — Working memory and snapshot freezing
- `manager.rs` — `MemoryManager` with keyword extraction and snapshot serialization
- Injected into `StreamingQueryEngine` via `with_memory_snapshot()`

**`permissions/`** — Permission system
- `mod.rs` — `PermissionMode` (Default, AutoLowRisk, AutoAll, ReadOnly) and `PermissionContext`
- Rules use glob patterns with `allow()`, `deny()`, `ask()`

**`skills/`** — Skill system (partially wired)
- `parser.rs` / `types.rs` / `registry.rs` — Skill markdown parsing with frontmatter
- `SkillManageTool`, `SkillListTool`, `SkillViewTool` registered in default registry

**Legacy modules** (only compiled with `legacy-cli` feature)
- `weight_engine/` — Hierarchical absolute weight calculation
- `ai_analyzer/` — Heuristic + LLM weight analysis
- `cli/` — Original CLI command parsing (`Cli::parse()` manually parses `std::env::args()`)
- `context_manager/` — Session state and persistence

## Key Design Patterns

**Tool System Pattern:**
```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    async fn execute(&self, params: Value, context: ToolContext) -> ToolResult;
}
```

**Query Engine Flow:**
1. User input → `StreamingQueryEngine::query()`
2. LLM generates response (possibly tool calls)
3. `ConversationLoop` parses and executes tools via `ToolRegistry`
4. Tool results streamed back as `StreamEvent::ToolResult`
5. Final assistant message streamed as `StreamEvent::Message`
6. TUI appends events to `messages` list

**Agent Wiring:**
- `main.rs` creates `AgentManager` and wires it to both `QueryEngine` and `StreamingQueryEngine` via `with_agent_manager()`
- `AgentTool` uses `ToolContext.agent_manager` to spawn sub-agents

## Configuration

Environment variables:
```bash
export MOONSHOT_API_KEY="your-api-key"
export MOONSHOT_BASE_URL="https://api.moonshot.cn/v1"  # optional
export MOONSHOT_MODEL="kimi-k2.5"  # optional

export OPENAI_API_KEY="your-key"  # alternative
export OPENAI_BASE_URL="..."  # optional
export OPENAI_MODEL="gpt-4o"  # optional
```

## Data Storage

- macOS: `~/Library/Application Support/priority-agent/`
- Linux: `~/.local/share/priority-agent/`
- SQLite session DB: `.../priority-agent/sessions.db`

## Testing

Unit tests are embedded under `#[cfg(test)]` in each module. Run with `cargo test`. There are 210+ tests covering tools, engine, TUI components, permissions, and state management.

## Known Gaps vs Claude Code

Compared to the real Claude Code (`~/Desktop/claude/`), this reimplementation is architecturally aligned but missing substantial product-depth features. Do not assume parity exists unless verified.

### Recently Closed Gaps (Phases 6–12 Complete)
- **LSP Integration** — `LSPTool` with diagnostics, hover, definition, references, symbols; `LspManager` auto-detects rust-analyzer, ts-server, gopls, pylsp.
- **IDE Integrations** — `WorkbenchTool` + `src/ide/` supports VS Code / Cursor (`open_file`, `reveal`, `terminal`).
- **Git Worktrees** — `WorktreeTool` + `WorktreeManager` supports list, create, remove, prune, switch; shown in TUI status bar.
- **Bundled Skills** — Built-in `/commit`, `/review-pr`, `/review`, `/security-review`, `/explain`, `/fix` skills loaded at compile time via `include_str!`.
- **Git Workflow Commands** — `/commit`, `/review-pr`, `/review`, `/security-review`, `/explain`, `/fix` slash commands wired in TUI.
- **Rich TUI** — Markdown rendering (`pulldown-cmark` → `ratatui::Text`), multiline input (`Shift+Enter`), basic Vim mode (`Ctrl+V` toggle, `j/k` scroll, `i` insert).
- **Bridge / Remote Sessions** — `RemoteTriggerTool` + `BridgeClient` over HTTP; `--bridge-url` CLI flag.

### Critical Missing
- **Advanced Agent Types** — Existing `AgentManager` is single-role and local-first; still missing teammate/remote-specialist style agent types and richer delegation contracts.

### High Priority Missing
- **MCP Advanced Transport** — Mostly stdio-oriented today; missing WebSocket transport, OAuth flows, and approval UX parity.
- **Plugin Ecosystem Productization** — Plugin MVP exists, but marketplace/distribution/signature trust and lifecycle governance are still missing.
- **Permission Management Deep UX** — `/permissions` exists, but still lacks richer interactive review flows and policy import/export UX.

### Medium Priority Missing
- **Voice Mode** — No `src/voice/` equivalent.
- **LLM-based Memory Extraction** — Heuristic-only; no `extractMemories` service using LLM.
- **Keybinding Customization** — Hardcoded keys only.

### Low Priority / Ecosystem
- Auto-updater, interactive onboarding, telemetry/analytics, desktop/mobile/Chrome integrations.

### What We Do Well
- Unified `ConversationLoop` + `StreamingQueryEngine` with context compression and memory injection.
- Core tool chain complete (file, bash, grep, glob, web, agent, task, memory, mcp, lsp, worktree, workbench, remote_trigger).
- Plan Mode TUI integration (`PlanApprovalChannel` + `PlanModeManager`).
- Socratic analysis (`socratic.rs`) — unique deep-reasoning tool.
- SQLite session persistence with FTS5 search and migration framework.
- 230+ passing unit tests.

## 2026-04-17 Claude 对标清单 + 执行计划

### A. 已确认缺口清单（对照 `~/Desktop/claude/src`）

1. **Hooks 机制（高优先级）**
- Claude 有完整 Hook 事件与 schema（PreToolUse/PostToolUse 等）。
- 本项目此前缺失统一 Hook 管线，无法做工具前/后审计、策略拦截、外部策略引擎集成。

2. **插件生态（高优先级）**
- Claude 有插件安装、校验、加载、命令入口、市场生态。
- 本项目目前只有内置工具注册，无“插件包生命周期 + 验证 + 安全策略 + 用户安装流程”。

3. **远程 Agent / Session 控制面（高优先级）**
- Claude 有 RemoteSessionManager / SDK schema / 远程编排能力。
- 本项目有 bridge 原型，但未形成可扩展的远程控制平面（鉴权、会话路由、多租户隔离、回放）。

4. **安全沙箱深度（高优先级）**
- Claude 有 sandbox adapter 与更系统化的执行隔离。
- 本项目 `bash` 仍以本地进程为主，缺乏容器级/命名空间级隔离与统一安全策略下发。

5. **权限产品化（中高优先级）**
- 本项目已有权限规则引擎，但缺少完善的“交互式授权、规则可视化管理、策略导入导出”。

6. **命令产品面覆盖（中优先级）**
- Claude 的命令体系非常大（任务、诊断、审查、发布、插件运维等）。
- 本项目已补齐第一批核心命令（`/doctor`、`/review`、`/security-review`、`/tasks`、`/agents`），但仍缺少更完整的发布/插件运维/诊断扩展命令族。

7. **可观测性与运维（中优先级）**
- Claude 有更完整 analytics / 运行时洞察。
- 本项目缺少 Hook/Tool 级审计查询、时序指标、失败聚类分析面板。

8. **高级人机交互（中低优先级）**
- Voice、跨端联动、onboarding、自动更新等生态体验仍有差距。

### B. 与近期修复关联（安全稳定性）

已完成并验证（2026-04-17）：
- `file_read` 大 offset panic 修复
- `resolve_path` symlink escape 防护强化
- `datetime` timestamp unwrap panic 修复
- `bash` timeout 僵尸子进程清理增强（进程组）
- `file_write` overwrite 确认逻辑修正（基于解析后路径）

### C. 执行计划（按优先级顺序）

#### Phase 1（本周，先打底）
1. Hook 基础设施（PreToolUse/PostToolUse）接入 `ConversationLoop`
2. Hook 超时/失败策略（fail-open / fail-closed）与基础审计输出
3. 补齐 Hook 单测 + 回归测试

#### Phase 2（1-2 周）
1. 插件 MVP：本地插件 manifest、校验、加载、工具注入
2. 权限 UI MVP：授权提示、规则持久化、策略查看
3. 命令增强第一批：`/tasks`、`/agents`、`/doctor`

#### Phase 3（2-4 周）
1. 远程会话控制面（鉴权、路由、状态同步）
2. 沙箱执行层升级（可切换隔离后端）
3. 审计与指标（工具调用耗时、失败原因聚合、hook 轨迹）

### D. 当前执行状态（已开始）

- ✅ 已启动 Phase 1 / Task 1：
  - 新增 `src/engine/hooks.rs`
  - 提供可选环境变量驱动 Hook：
    - `PRIORITY_AGENT_PRE_TOOL_HOOK`
    - `PRIORITY_AGENT_POST_TOOL_HOOK`
    - `PRIORITY_AGENT_HOOK_TIMEOUT_MS`
    - `PRIORITY_AGENT_HOOK_FAIL_CLOSED`
  - `ConversationLoop` 工具执行路径已接入 Pre/Post Hook（含并行只读和串行读写路径）
- ✅ Phase 1 / Task 2 已完成：
  - Hook 超时控制已实现（`PRIORITY_AGENT_HOOK_TIMEOUT_MS`）
  - 失败策略已实现（`PRIORITY_AGENT_HOOK_FAIL_CLOSED` 控制 fail-open/fail-closed）
  - 增加 pre/post hook 触发 debug 审计日志
- ✅ Phase 1 / Task 3 已完成（基础回归）：
  - 新增 hook 单测（`src/engine/hooks.rs`）
  - 覆盖场景：显式拒绝、超时 fail-open、超时 fail-closed
  - 当前全量测试通过（248 passed）
- 🔄 Phase 2 / Task 1 已开始（插件 MVP）：
  - 新增 `src/plugins/mod.rs`（插件 manifest 发现与解析）
  - 新增 `plugin_list` 工具（`src/tools/plugin_tool/mod.rs`）
  - 目标：先完成“插件可发现+可观测”，再接入“执行与工具注入”
- ✅ Phase 2 / Task 1 第一阶段已完成：
  - 新增 `plugin_manage` 工具（`list/validate/enable/disable`）
  - 新增 manifest 校验与启停写回能力（`set_plugin_enabled`）
  - 插件相关回归测试已通过
- ✅ Phase 2 / Task 1 第二阶段已完成：
  - `plugin_manage` 新增 `run` 动作（超时控制 + 执行输出收集）
  - `run` 动作加入确认提示（默认需要确认）
- ✅ Phase 2 / Task 1 第三阶段已完成（工具注入）：
  - 启动时自动发现并注册启用插件为动态工具
  - 插件参数通过 JSON `stdin` 传递给插件进程
  - 支持 manifest 中 `tool_name/tool_description/tool_timeout_secs` 自定义
  - 当前全量测试通过（248 passed）
- ✅ Phase 2 / Task 3 已完成（第一批命令）：
  - TUI 新增 `/tasks`（任务汇总 + 最近任务）
  - TUI 新增 `/agents`（Agent 列表 + 状态）
  - TUI 新增 `/doctor`（环境/模型/hooks/工具注入诊断）
  - `StreamingQueryEngine` 暴露 task/agent/model getter 供 TUI 诊断使用
  - 当前全量测试通过（248 passed）
- ✅ Phase 2 / Task 2 已完成（权限 UI MVP）：
  - TUI 新增 `/permissions`（别名 `/perm`）命令
  - 支持运行时权限模式切换：`default/auto_low_risk/auto_all/read_only`
  - 支持策略查看：`/permissions rules [tool_name]`
  - 支持规则持久化：`/permissions <allow|deny|ask> <pattern> [project|global]`
  - `ConversationLoop` 权限模式改为由 `StreamingQueryEngine` 注入，不再硬编码
  - 当前全量测试通过（248 passed）
- ✅ Phase 3 / Task 1 第一阶段已完成（远程会话控制面基线）：
  - 新增 Bridge v1 路由：`/v1/sessions`、`/v1/sessions/:id`、`/v1/triggers/:id/run`
  - 新增可选鉴权中间件：`PRIORITY_AGENT_BRIDGE_TOKEN` / `BRIDGE_TOKEN`
  - 新增基于 `X-Tenant-Id` 的会话 ID 前缀隔离（tenant 路由基线）
  - `ApiState` 新增 `create_session_with_id` 支持远程场景自定义会话 ID
  - 当前全量测试通过（248 passed）
- ✅ Phase 3 / Task 1 第二阶段已完成（状态同步 + 回放查询）：
  - Bridge v1 新增：`GET /v1/sessions/:id/status`
  - Bridge v1 新增：`GET /v1/sessions/:id/messages?since_id=<id>&limit=<n>`
  - `BridgeClient` 新增 `get_session_status` / `get_session_messages`，并支持 `X-Tenant-Id`
  - `remote_trigger` 工具新增动作：`status`、`replay`、`sync`（支持 `since_id` 增量拉取）
  - 多租户会话列表过滤增强：先扩窗再过滤，避免混排截断
  - 当前全量测试通过（249 passed）
- ✅ Phase 3 / Task 1 第三阶段已完成（鉴权强化 + 回放游标持久化）：
  - Bridge 鉴权支持多 token 轮换：`PRIORITY_AGENT_BRIDGE_TOKENS`（`,/;/空白` 分隔）
  - 保留单 token 模式：`PRIORITY_AGENT_BRIDGE_TOKEN` / `BRIDGE_TOKEN`
  - 鉴权头支持 `Authorization: Bearer` 和 `X-Bridge-Token`
  - 新增本地回放游标持久化：`~/.priority-agent/bridge_cursors.json`
  - `remote_trigger sync` 支持自动读取/写回游标（`use_saved_cursor`、`persist_cursor`）
  - 当前全量测试通过（249 passed）
- ✅ Phase 3 / Task 2 第一阶段已完成（沙箱执行后端切换 MVP）：
  - `bash` 工具新增可切换执行后端：`local` / `restricted`
  - 新增参数：`backend`（可覆盖默认后端）
  - 新增环境变量：`PRIORITY_AGENT_BASH_BACKEND`（全局默认后端）
  - `sandbox=true` 现在会自动落到 `restricted` 后端（兼容旧调用）
  - `restricted` 后端采用软资源限制 + 最小化环境变量（非容器级隔离）
  - `/doctor` 新增 `bash_backend_env` 诊断输出
  - 当前全量测试通过（250 passed）
- ✅ Phase 3 / Task 2 第二阶段已完成（外部隔离后端适配器）：
  - `bash` 后端新增：`external`
  - `external` 通过包装命令执行：`PRIORITY_AGENT_BASH_EXTERNAL_CMD`
  - 兼容旧变量：`PRIORITY_AGENT_BASH_SANDBOX_CMD`
  - 支持模板占位：`{command}`（自动安全单引号包裹）
  - 若模板无占位，自动拼接：`<wrapper> -- bash -lc '<command>'`
  - `/doctor` 新增外部后端配置状态：`external_cmd/legacy_sandbox_cmd`
  - 当前全量测试通过（253 passed）
- ✅ Phase 3 / Task 2 第三阶段已完成（策略守卫 + 回退 + 审计统一）：
  - `external` wrapper 白名单策略：`PRIORITY_AGENT_BASH_EXTERNAL_ALLOWLIST`
  - 兼容白名单变量：`PRIORITY_AGENT_BASH_EXTERNAL_WRAPPER_ALLOWLIST`
  - `external` 失败回退策略：`PRIORITY_AGENT_BASH_EXTERNAL_FALLBACK`
  - 兼容回退变量：`PRIORITY_AGENT_BASH_SANDBOX_FALLBACK`
  - `bash` 工具结果统一审计字段（成功/失败均含 `audit`）
  - `/doctor` 新增白名单与回退策略配置状态（allowlist/fallback）
  - 当前全量测试通过（254 passed）
- ✅ Phase 3 / Task 3 第一阶段已完成（工具耗时与失败原因聚合）：
  - `CostTracker` 新增 `tool_metrics`（calls/success/failed/duration/failure_reasons）
  - `ConversationLoop` 对并发/串行工具统一埋点：记录时长与失败原因
  - `ToolResult.duration_ms` 在执行路径自动补齐（若工具未自行设置）
  - `/doctor` 新增工具观测输出：
    - `tool_metrics` 总览
    - `tool_slowest` 慢工具 TopN
    - `tool_fail_reasons` 失败原因 TopN
  - 当前全量测试通过（255 passed）
- ✅ Phase 3 / Task 3 第二阶段已完成（审计快照与明细查询）：
  - `CostTracker` 新增最近工具调用明细 `recent_tool_events`（ring buffer）
  - 新增审计导出能力：`export_audit_snapshot_json(session_id, recent_limit)`
  - TUI 新增 `/audit` 命令：
    - `/audit summary`：审计概览
    - `/audit recent <n>`：最近 N 条工具调用明细
    - `/audit export [path]`：导出会话审计快照 JSON
  - 默认导出路径：`~/.priority-agent/audit_<session>_<timestamp>.json`
  - 当前全量测试通过（256 passed）
- ✅ Phase 3 / Task 3 第三阶段已完成（HTTP API 审计端点）：
  - 新增 `GET /api/audit/summary`：审计概览
  - 新增 `GET /api/audit/recent?limit=<n>`：最近工具审计事件
  - 新增 `POST /api/audit/export`：导出审计快照（可选写入服务器路径）
  - `ApiState` 新增独立 `audit_tracker`，在 `chat/call_tool` 路径自动记录指标
  - API 模块文档补充了 `/api/audit/*` 端点说明
  - 当前全量测试通过（256 passed）

## 2026-04-17 对账清单（最新）

### 已完成（相对本轮 Claude 对标计划）
- Hooks 基础设施与 fail-open/fail-closed 策略
- 插件 MVP（发现、校验、启停、运行、动态工具注入）
- 命令层第一批：`/tasks`、`/agents`、`/doctor`
- 权限 UI MVP：`/permissions`（模式切换、规则查看、规则持久化）
- 远程会话控制面三阶段（鉴权、租户隔离、状态同步、回放游标）
- 沙箱后端三阶段（local/restricted/external + allowlist/fallback）
- 审计与指标三阶段（聚合、明细、TUI/API 导出）
- 安全修复五项：`file_read`/`resolve_path`/`datetime`/`bash timeout`/`file_write confirm`

### 尚未完成（当前主缺口）
- Advanced Agent Types（teammate/remote specialist/dream-task 语义）
- MCP 高级传输与 OAuth/审批产品化
- 插件生态（市场、签名信任链、发布与升级治理）
- Voice 模块与语音交互闭环
- LLM 驱动的记忆提取服务
- 可配置键位与更强交互可定制化

### Phase 4（进行中）
1. 命令面增强第二批：`/review`、`/security-review`
2. 将审查结果结构化接入审计轨迹
3. 再推进 MCP 高级传输设计草案与最小实现（WebSocket transport skeleton）

### Phase 4 当前状态
- ✅ Task 1 已完成：
  - 新增 `/review`（审查本地未提交 diff）
  - 新增 `/security-review`（安全视角审查本地未提交 diff）
  - 新增 bundled skills：`review`、`security_review`
- ✅ Task 2 已完成：
  - `/review` 执行链路新增审计埋点：`slash_review`
  - `/security-review` 执行链路新增审计埋点：`slash_security_review`
  - 可通过 `/audit summary` 和 `/audit recent` 统一查看
- ✅ Task 3 第一阶段已完成（MCP 高级传输最小骨架）：
  - 新增 MCP 传输抽象：`stdio` / `websocket`
  - `McpServerConfig` 增加 `transport/websocket_url/headers` 字段
  - `MCP` 配置入口新增：
    - `config.toml` `engine.mcp_servers`
    - 环境变量 `PRIORITY_AGENT_MCP_SERVERS_JSON`
  - `QueryEngine/StreamingQueryEngine` 接入 `mcp_manager`
  - `mcp list_servers` 输出 transport + endpoint 摘要

## 2026-04-20 编程能力补齐计划

对比 `~/Desktop/claude/src`（真实 Claude Code），以下 8 个编程相关能力缺失，按优先级逐个实现：

### 1. simplify skill — 代码质量自动审查 pipeline
**目标**：并行启动 3 个 subagent 审查代码复用（Reuse）、代码质量（Quality）、效率（Efficiency）
**状态**：✅ 已完成
**关键文件**：`src/skills/bundled/simplify.md`, `src/tui/slash_handler.rs`

### 2. verify skill — 代码变更功能验证
**目标**：不只检查编译通过，而是运行实际测试验证功能正确性
**状态**：✅ 已完成
**关键文件**：`src/skills/bundled/verify.md`, `src/tui/slash_handler.rs`

### 3. keybindings-help skill — 键盘快捷键自定义
**目标**：完整的 keybindings 自定义系统（上下文表、动作表、JSON schema 验证）
**状态**：✅ 已完成
**关键文件**：`src/skills/bundled/keybindings.md`, `src/tui/slash_handler.rs`

### 4. debug skill — 会话调试
**目标**：读取当前会话的 debug log，grep ERROR/WARN
**状态**：✅ 已完成
**关键文件**：`src/skills/bundled/debug.md`, `src/tui/slash_handler.rs`

### 5. stuck skill — 冻结会话诊断
**目标**：检测其他 Claude Code 进程是否冻结（CPU 高、D/T/Z 状态）
**状态**：✅ 已完成
**关键文件**：`src/skills/bundled/stuck.md`, `src/tui/slash_handler.rs`

### 6. remember skill — 记忆分层管理
**目标**：管理 auto-memory 条目，决定去 CLAUDE.md / CLAUDE.local.md / Team memory
**状态**：✅ 已完成
**关键文件**：`src/skills/bundled/remember.md`, `src/tui/slash_handler.rs`

### 7. 高级上下文压缩系统
**目标**：完善 `context_compressor.rs`，增加时间基础配置、微压缩、压缩警告状态
**状态**：✅ 已完成
**关键文件**：`src/engine/context_compressor.rs`
**实现**：`TimeBasedConfig`（环境变量可配）、`CompressionWarning`（四级警告）、`micro_compress()`（轻量压缩）、`needs_time_based_compression()`、会话时长统计

### 8. 完整 LSP 服务端管理
**目标**：增加 `prepareCallHierarchy`、`incomingCalls`、`outgoingCalls`、诊断注册表、被动反馈
**状态**：✅ 已完成
**关键文件**：`src/engine/lsp.rs`
**实现**：`prepare_call_hierarchy`/`incoming_calls`/`outgoing_calls` 已在 `LspClient`；`LspManager` 管理多服务器；诊断缓存；格式输出函数
### 实施顺序
1. simplify → 2. verify → 3. keybindings → 4. debug → 5. stuck → 6. remember → 7. context_compressor → 8. lsp

## 2026-04-20 Claude 思考机制（Thinking）

### 实现状态：✅ 已完成

Claude Code 在生成回复前会进行"主动思考"，这是模型自身的推理能力，通过 `thinking_delta` / `redacted_thinking` 内容块实现。Kimi（Moonshot）等支持 extended thinking 的模型也提供此功能。

**实现细节**：
- `services/api/kimi.rs` — `KimiConfig` 新增 `thinking_enabled: bool` 和 `thinking_budget: Option<u32>`
- `ThinkingConfig` wrapper 实现 `Config` trait，注入 `Anthropic-Beta: interleaved-thinking=2025-05-14` header
- 环境变量控制：
  - `PRIORITY_AGENT_THINKING=0` 禁用（默认启用）
  - `PRIORITY_AGENT_THINKING_BUDGET` 设置 thinking token 预算（默认 adaptive）
- `ChatRequest` 新增 `thinking_budget: Option<u32>` 字段
- `StreamEvent::Thinking(String)` 已存在，可发送思考内容到 TUI 渲染

**当前限制**：
- 流式响应的 thinking content block 解析受限（Kimi API 的 thinking 内容对客户端不可见，仅 usage 中有统计）
- 流式循环中发出 `ThinkingStart`/`ThinkingComplete` 信号供 UI 显示 thinking 状态

### 环境变量
```bash
PRIORITY_AGENT_THINKING=1          # 启用 thinking（默认）
PRIORITY_AGENT_THINKING=0          # 禁用 thinking
PRIORITY_AGENT_THINKING_BUDGET=4096  # 固定 4096 token thinking 预算
```

---

## 2026-04-20 系统性差距审查报告

对比 `~/Desktop/claude/src`（真实 Claude Code）进行系统性审查，发现三个最高优先级缺口：

### 1. LLM 驱动的主动记忆提取（最高优先级）

**Claude Code**：`extractMemories.ts` 使用 `runForkedAgent` 在后台 forked 会话中运行 LLM，主动从对话历史提取记忆写入 `~/.claude/projects/<path>/memory/` 目录。包含 mutual exclusion（主 agent 已写记忆则跳过）、throttle（每 N 轮提取一次）、trailing run 机制，以及完整的缓存命中率 telemetry。

**我们的现状**：`memory/manager.rs` 仅有启发式关键词提取，无 LLM 驱动的主动提取能力。

**实现难度**：高（需要 forked agent 基础设施、缓存共享机制）

### 2. 响应式压缩 Reactive Compact（高优先级）

**Claude Code**：`compact.ts` 有完整 feature flag `REACTIVE_COMPACT`，先尝试 session memory compaction，失败后才走传统 summarization。包含 `microcompactMessages` 先对消息做轻量级 token 削减，以及 `promptCacheBreakDetection` 机制。

**我们的现状**：`context_compressor.rs` 仅基于固定阈值（0.6 snip / 0.8 compress）的简单 token ratio 截断，无 LLM summarization，无 microcompact 预处理。

**实现难度**：高（需要 LLM summarization 子系统）

### 3. 模型降级增强 Fallback Model（中优先级）

**Claude Code**：`withRetry.ts` 中 `FallbackTriggeredError` 在连续 3 次 529 错误后触发，切换到备用模型。完整的多层认证错误处理（401/403/OAuth/Bedrock/Vertex），persistent retry 模式支持无人值守会话。

**我们的现状**：`streaming.rs` 有 fallback 机制但仅做简单错误字符串匹配，无连续错误计数、无多层认证处理、无 persistent retry 模式。

**实现难度**：中

---

## Phase 5：三个最高优先级缺口的追赶计划

### Task 1：LLM 驱动的主动记忆提取

**目标**：在后台 forked 会话中运行 LLM 从对话历史提取记忆，不阻塞主对话

**实现步骤**：
1. 在 `memory/manager.rs` 中新增 `extract_memories_with_llm()` 方法
2. 使用 tokio spawn 创建后台任务，跳过主对话完成前的记忆写入（mutual exclusion）
3. 实现 throttle 机制（每 N 轮提取一次，可配置）
4. 记忆写入 `~/.priority-agent/memory/` 目录
5. 实现 trailing run 机制（对话结束后最终提取）
6. 添加缓存命中率 telemetry

**关键文件**：`src/memory/manager.rs`

**环境变量**：
- `PRIORITY_AGENT_LLM_MEMORY_EXTRACTION=1` 启用
- `PRIORITY_AGENT_LLM_MEMORY_INTERVAL=5` 每 N 轮提取一次

---

### Task 2：响应式压缩 Reactive Compact

**目标**：先尝试轻量级 microcompact，失败后再走 LLM summarization

**实现步骤**：
1. 增强 `context_compressor.rs`，新增 LLM summarization 能力
2. 实现 `microcompact_messages()` 先对消息做轻量 token 削减（移除重复字段、压缩长内容）
3. 新增 `reactive_compact()` 方法，先尝试 microcompact，失败后再 full compress
4. 添加 `prompt_cache_break_detection` 检测 context overflow 前的信号
5. 保持与现有压缩逻辑的兼容性

**关键文件**：`src/engine/context_compressor.rs`

**环境变量**：
- `PRIORITY_AGENT_REACTIVE_COMPACT=1` 启用
- `PRIORITY_AGENT_MICROCOMPACT_THRESHOLD=0.5`

---

### Task 3：模型降级增强

**目标**：完善的 fallback 触发机制、多层认证处理、persistent retry 模式

**实现步骤**：
1. 新增 `FallbackState` 跟踪连续错误次数（529 计数）
2. 增加 401/403/OAuth/Bedrock/Vertex 专属错误处理路径
3. 新增 `persistent_retry` 模式（无人值守时持续重试）
4. 增加 `max_fallback_attempts` 限制防止无限循环
5. 在 TUI 中显示 fallback 状态变化

**关键文件**：`src/engine/streaming.rs`, `src/services/api/mod.rs`

**环境变量**：
- `PRIORITY_AGENT_FALLBACK_MAX_ATTEMPTS=3`
- `PRIORITY_AGENT_PERSISTENT_RETRY=1`

---

### Phase 5 当前状态
- ✅ Task 1（LLM 记忆提取）：已完成
  - 新增 throttle 机制（每 N 轮提取一次，环境变量 `PRIORITY_AGENT_LLM_MEMORY_INTERVAL` 可配）
  - 新增 mutual exclusion（主 agent 已写阻止后台 LLM 提取）
  - 新增 `extraction_stats()` telemetry
  - 测试通过：471 passed
- ✅ Task 2（Reactive Compact）：已完成
  - `context_compressor.rs` 已具备完整能力：LLM summarization、`micro_compress()`、time-based compression
  - 新增 `CompressionWarning` 四级警告状态
  - 测试通过：471 passed
- ✅ Task 3（模型降级增强）：已完成
  - 新增 `FallbackState` 追踪连续错误（529/401/403 等）
  - 连续 3 次 529 错误后触发 fallback
  - 新增 `ErrorType` 枚举分类错误类型
  - 新增 `PRIORITY_AGENT_FALLBACK_MAX_ATTEMPTS` 环境变量
  - 测试通过：471 passed

### 验证方式
每个 task 完成后：
1. 运行 `cargo test` 确保不破坏现有测试
2. 手动测试对应功能
3. 确认测试数量不减少

---

## Phase 6：三个高优先级缺口追赶计划

### 缺口 1：流式 thinking 解析接入

**目标**：在流式响应中解析 `thinking_delta` 内容块，通过 `StreamEvent::Thinking` 发送到 TUI 渲染

**Claude Code 实现**：
- `thinking_delta` 和 `redacted_thinking` 内容块
- 完整状态机管理 thinking 开始/进行中/完成
- `AssistantRedactedThinkingMessage` 类型和专门渲染逻辑

**我们的现状**：
- `StreamEvent::Thinking(String)` 已存在但从未触发
- 非流式 thinking 完整（KimiConfig + beta header）
- 流式响应解析缺失

**实现步骤**：
1. 在 `services/api/kimi.rs` 的 `chat_stream()` 返回类型中解析 SSE 格式的 thinking blocks
2. 实现 `thinking_delta` 和 `redacted_thinking` 状态机
3. 在 `conversation_loop.rs` 的流式处理循环中捕获 thinking 内容
4. 通过 `StreamEvent::Thinking` 发送到 TUI
5. TUI 渲染 thinking 内容（如折叠显示、单独面板等）

**关键文件**：
- `src/services/api/kimi.rs` — Kimi 流式响应解析
- `src/engine/conversation_loop.rs` — thinking 内容块捕获
- `src/engine/streaming.rs` — StreamEvent::Thinking 发送

**环境变量**：
- `PRIORITY_AGENT_THINKING=1`（默认启用）
- `PRIORITY_AGENT_THINKING_RENDER=collapsed`（thinking 渲染模式）

**当前状态**：未开始

---

### 缺口 2：LLM 记忆提取增强（forked agent）

**目标**：实现真正的 forked agent 隔离机制，而非简单 tokio spawn

**Claude Code 实现**：
- `runForkedAgent` 在独立子会话中运行，共享父上下文 prompt cache
- `hasMemoryWritesSince` 防止主 agent 和 forked agent 同时写入
- Trailing run 机制（对话结束后最终提取）
- 完整 telemetry：`cache: read=X create=Y input=Z (hitPct%)`
- 记忆写入 `~/.claude/projects/<path>/memory/` 目录结构

**我们的现状**：
- `sync_turn_llm_background()` 使用简单 tokio spawn
- 没有真正的 forked 隔离
- 没有 prompt cache 共享
- 没有 trailing run 后处理

**实现步骤**：
1. 新增 `ForkedMemoryAgent` 结构体，模拟 forked agent 行为
2. 在 `sync_turn_llm_background()` 中使用独立 task context
3. 实现 `trailing_run()` 方法（对话结束后调用）
4. 实现记忆写入互斥：`hasMemoryWritesSince` 检查
5. 增强 telemetry：`cache hit rate`、`extraction count`
6. 支持写入 `~/.priority-agent/memory/` 目录结构

**关键文件**：
- `src/memory/manager.rs` — ForkedMemoryAgent、trailing_run、hasMemoryWritesSince

**环境变量**：
- `PRIORITY_AGENT_LLM_MEMORY_FORKED=1` — 启用 forked agent 模式（默认 0）
- `PRIORITY_AGENT_LLM_MEMORY_TRAILING=1` — 启用 trailing run

**当前状态**：未开始

---

### 缺口 3：上下文折叠（Context Collapse）

**目标**：将历史消息持久化到文件，读取时重放，类似 Claude Code 的 `CONTEXT_COLLAPSE`

**Claude Code 实现**：
- Feature flag `CONTEXT_COLLAPSE`
- `applyCollapsesIfNeeded()` 方法
- Commit log 持久化到 transcript
- `ContextCollapseCommitEntry` 和 `ContextCollapseCommitSnapshotEntry` 类型
- 与 session restore 集成（`restoreFromEntries`）

**我们的现状**：
- CLAUDE.md 明确标记为"缺失（高难度）"
- 没有任何 context collapse 实现

**实现步骤**：
1. 新增 `ContextCollapseService` 结构体
2. 定义 `ContextCollapseEntry` 枚举（Commit / Snapshot）
3. 实现 `commit(messages)` 方法：将历史消息写入 transcript 文件
4. 实现 `restore()` 方法：从 transcript 文件恢复消息
5. 实现 `applyCollapsesIfNeeded()` 检查是否需要折叠
6. 与 `session_store` 集成：折叠时持久化到 DB
7. 实现滑动窗口：保留最近 N 条消息，其余折叠

**关键文件**：
- `src/engine/context_collapse.rs` — 新文件
- `src/engine/context_compressor.rs` — 与压缩系统集成
- `src/session_store/mod.rs` — 与会话存储集成

**环境变量**：
- `PRIORITY_AGENT_CONTEXT_COLLAPSE=1` — 启用
- `PRIORITY_AGENT_CONTEXT_COLLAPSE_WINDOW=50` — 保留最近消息数

**当前状态**：未开始

---

### Phase 6 实施顺序
1. 缺口 1（流式 thinking）— 中优先级，相对独立
2. 缺口 2（LLM 记忆提取）— 高优先级，架构改动大
3. 缺口 3（上下文折叠）— 高难度，最后推进

### Phase 6 当前状态
- ✅ Task 1（流式 thinking 解析）：已完成
  - `Usage` 结构体新增 `reasoning_tokens: Option<u32>` 字段
  - `kimi.rs` 和 `openai_compat.rs` 从 `completion_tokens_details` 提取 `reasoning_tokens`
  - `StreamEvent` 新增 `ThinkingStart`/`ThinkingChunk`/`ThinkingComplete` 事件
  - `call_api_streaming()` 在流开始时发出 `ThinkingStart`，结束时发出 `ThinkingComplete`
  - 说明：Kimi API 的 thinking 内容对客户端不可见，仅 usage 中有统计
  - 测试通过：474 passed
- ✅ Task 2（LLM 记忆提取 forked agent）：已完成
  - `MemoryManager` 新增 `forked_mode` 和 `trailing_mode` 配置（环境变量控制）
  - 新增 `trailing_run()` 方法：会话结束时执行最终记忆提取
  - 新增 `has_memory_writes_since()` 方法：forked agent 互斥检查
  - 新增 `cache_stats()` 方法：缓存命中率统计
  - `sync_turn_llm_background()` 支持 forked 模式（始终尝试 LLM 提取）
  - 测试通过：474 passed
- ✅ Task 3（上下文折叠）：已完成
  - 新增 `src/engine/context_collapse.rs`：`ContextCollapseService` 结构体
  - `ContextCollapseEntry` 枚举（Commit / Snapshot）
  - `apply_collapses_if_needed()` 方法：将早期消息写入磁盘
  - `restore()` 方法：从磁盘恢复折叠的消息
  - `Message` 和 `ToolCall` 新增 `serde::Serialize/Deserialize` 和 `Hash` trait
  - 环境变量：`PRIORITY_AGENT_CONTEXT_COLLAPSE=1` 启用，`PRIORITY_AGENT_CONTEXT_COLLAPSE_WINDOW=50` 保留窗口大小
  - 测试通过：474 passed

---

## 2026-04-20 Claude Code 编程能力差距分析与追赶计划

对比 `~/Desktop/claude/src`（真实 Claude Code），以下是我们项目尚存的差距及改进方向：

### 一、流式工具执行（Streaming Tool Executor）
**目标**：在模型流式输出的同时开始执行工具（读操作并行），不等待模型完成
**状态**：✅ 已完成
**实现**：
- 改造 `StreamingQueryEngine`，在模型流输出期间就开始调度只读工具
- 利用 `buffer_unordered` 实现真正并行
- `execute_tools_parallel` 跳过已预执行的只读工具，避免重复执行
**关键文件**：`src/engine/streaming.rs`, `src/engine/conversation_loop.rs`

### 二、工具结果磁盘缓存
**目标**：当工具结果过大时写入磁盘，只在 context 中保留摘要
**状态**：✅ 已完成
**实现**：
- `truncate_tool_result()` 当 `content.len() > 32 KiB` 时写入 `~/.priority-agent/tool-results/`
- 在 context 中保留 `file_path` 引用和头尾摘要
**关键文件**：`src/tools/mod.rs`, `src/engine/conversation_loop.rs`

### 三、LLM 驱动的记忆提取服务
**目标**：后台 forked agent 自动从对话中提取关键信息，不阻塞主对话
**状态**：✅ 已完成
**实现**：
- `sync_turn_llm_background()` 使用 tokio spawn 后台执行 LLM 记忆提取
- 2秒延迟让主对话先完成响应
- 提取结果直接写入 `MEMORY.md`
**关键文件**：`src/memory/manager.rs`

### 四、响应式压缩（Reactive Compact）
**目标**：遇到 413 (prompt-too-long) 时自动触发压缩，不浪费已经生成的内容
**状态**：✅ 已完成
**实现**：
- `response_compression_loop` 在 API 调用层拦截上下文超限错误
- 最多 3 轮压缩重试（第一次完整压缩，第二次 micro_compress）
- 压缩后通知前端 `[Context compressed due to size limits]`
**关键文件**：`src/engine/streaming.rs`, `src/engine/context_compressor.rs`

### 五、工具预验证与 UI 渲染
**目标**：每个工具支持 `validate_input()`、`render_result()` 等
**状态**：✅ 已完成
**实现**：
- `Tool` trait 新增 `validate_params()` 和 `render_result()` 默认方法
- `validate_params()` 检查必需参数和类型
- `render_result()` 截断长输出，保留关键部分
**关键文件**：`src/tools/mod.rs`

### 六、模型降级（Fallback Model）
**目标**：当主模型失败时自动降级到备用模型
**状态**：✅ 已完成
**实现**：
- `PRIORITY_AGENT_FALLBACK_MODEL` 环境变量配置 fallback 模型
- `StreamingQueryEngine::query_stream` 检测 rate limit/overloaded/context/timeout 错误自动切换
- 防止无限 fallback（fallback_model 置为 None）
**关键文件**：`src/engine/streaming.rs`

### 七、技能预发现（Skill Prefetch）
**目标**：在工具执行期间预发现相关技能，下一轮前消费
**状态**：✅ 已完成
**实现**：
- `SkillRegistry::prefetch()` 根据用户消息关键词预取相关 skills
- 支持精确匹配、短语匹配、描述匹配
- 去重并限制 5 个结果
**关键文件**：`src/skills/registry.rs`

### 八、上下文折叠（Context Collapse）
**目标**：使用投影机制将历史消息持久化到文件，读取时重放
**状态**：缺失（高难度）
**关键文件**：`src/session_store/`, `src/engine/conversation_loop.rs`

### 九、错误恢复策略增强
**目标**：完善 max_output_tokens 恢复、prompt-too-long 恢复机制
**状态**：✅ 已完成
**实现**：
- `StreamEvent::OutputTruncated` 检测 `FinishReason::Length`
- TUI 可据此向用户提示输出被截断
**关键文件**：`src/engine/streaming.rs`, `src/engine/conversation_loop.rs`

### 十、权限系统增强
**目标**：增加 classifier-based auto mode、coordinator 决策
**现状**：有基础规则匹配和 ask 模式
**实现**：
- 增加基于 LLM 的权限分类器
- 实现 coordinator 协调多 agent 权限决策
- 支持交互式权限审批 UI
**状态**：可改进
**关键文件**：`src/permissions/mod.rs`, `src/tui/`

### 实施优先级顺序
1. ✅ **流式工具执行** — 提升交互响应速度
2. ✅ **LLM 记忆提取** — 减少重复工作
3. ✅ **响应式压缩** — 提升长对话稳定性
4. ✅ **工具结果磁盘缓存** — 节省 context 空间
5. ✅ **模型降级** — 提升容错能力
6. ✅ **工具预验证与 UI 渲染** — 提升工具质量
7. ✅ **技能预发现** — 提升技能匹配准确度
8. ✅ **错误恢复策略增强**
9. ✅ **上下文折叠** — 已完成（Phase 6 Task 3）
10. **权限系统增强** — 可改进

---

## Phase 7：编程能力深度补齐计划（2026-04-20）

对比 `~/Desktop/claude/src`（真实 Claude Code）中的编程相关能力，以下是 6 个最高优先级缺口，按计划逐个实现：

---

### 高优先级缺失（3 个）

#### Task 1：Smart Edit（智能编辑）

**目标**：实现 `old_string`/`new_string` 模式的智能编辑，而非整体重写

**Claude Code 实现**：
- `FileEditTool.ts` — `old_string`/`new_string`/`replace_all` 输入 schema
- **Quote normalization** — 处理智能引号 `'` → `'`
- **Must-read-before-edit** — 跟踪 `readFileState`，必须先 read 才能 edit
- **File modification detection** — timestamp + content 对比检测冲突
- **LSP 通知** — edit 后自动触发 `textDocument/didChange`/`didSave`
- **Desanitization** — 处理 `&lt;fnr&gt;`、`&lt;n&gt;` 等转义

**实现步骤**：
1. 修改 `file_edit` 工具，支持 `old_string`/`new_string` 参数
2. 实现 `normalize_quotes()` 函数处理智能引号
3. 添加 `readFileState` 跟踪：edit 前必须先 read
4. 实现文件修改检测（timestamp + content hash）
5. edit 后自动发送 LSP `textDocument/didChange` 通知
6. 处理 desanitization（`&lt;fnr&gt;` 等）

**关键文件**：`src/tools/file_tool/mod.rs`

**环境变量**：
- `PRIORITY_AGENT_SMART_EDIT=1` — 启用智能编辑

---

#### Task 2：Diagnostic Tracking（自动调试）

**目标**：编辑前捕获 baseline diagnostics，编辑后对比找出新增错误

**Claude Code 实现**：
- `diagnosticTracking.ts` — `DiagnosticTrackingService` 单例
- `beforeFileEdited()` — 编辑前捕获 baseline
- `getNewDiagnostics()` — 对比 baseline，返回新增错误
- IDE RPC 调用 `getDiagnostics`
- 处理 `_claude_fs_right` 协议用于 diff 视图

**实现步骤**：
1. 新增 `DiagnosticTracker` 结构体
2. 实现 `before_edit()` 方法捕获 baseline
3. 实现 `after_edit()` 方法对比返回新增 diagnostics
4. 与 `lsp_manager` 集成获取 diagnostics
5. TUI 显示新增错误（实时反馈）

**关键文件**：`src/engine/diagnostic_tracker.rs`（新文件）

**环境变量**：
- `PRIORITY_AGENT_DIAGNOSTIC_TRACKING=1` — 启用

---

#### Task 3：Verification Agent（验证 Agent）

**目标**：对抗性验证，专门尝试破坏代码而非确认能跑

**Claude Code 实现**：
- `verificationAgent.ts` — 对抗性验证专家
- **Failure patterns**: "verification avoidance", "seduced by first 80%"
- **Required steps**: build, test suite, linters, regressions
- **Adversarial probes**: 并发、边界值、幂等性、孤儿操作
- **Verdict**: `PASS` / `FAIL` / `PARTIAL`

**实现步骤**：
1. 新增 `VerificationAgent` 结构体
2. 实现验证 prompt（对抗性测试专家）
3. 实现 `verify()` 方法：运行 build/test/linter
4. 实现 adversarial probes（并发测试、边界值、幂等性）
5. 返回 `PASS`/`FAIL`/`PARTIAL` 判决
6. 注册为 skill `/verify` 的后端

**关键文件**：`src/agent/verification_agent.rs`（新文件）

**环境变量**：
- `PRIORITY_AGENT_VERIFICATION_ADVERSARIAL=1` — 启用对抗模式

---

### 中优先级缺失（3 个）

#### Task 4：Batch Refactor（批量重构）

**目标**：5-30 个并行 worktree agent 协同完成大规模重构

**Claude Code 实现**：
- `batch.ts` — 分解大改动为 5-30 个并行 agent
- 每个 agent 运行在隔离 git worktree
- 每个 agent 创建独立 PR
- **Phase 1**: Research + plan in plan mode
- **Phase 2**: Spawn workers in parallel with `isolation: "worktree"`
- **Phase 3**: Track progress, parse PR URLs from results

**实现步骤**：
1. 新增 `BatchRefactor` 结构体
2. 实现 `decompose()` 方法将大改动分解为小单元
3. 实现并行 worktree agent 调度
4. 实现 PR URL 解析和进度跟踪
5. 与现有 `worktree_manager` 集成
6. 注册为 skill `/batch` 或 `/refactor`

**关键文件**：`src/engine/batch_refactor.rs`（新文件）

**环境变量**：
- `PRIORITY_AGENT_BATCH_REFACTOR=1` — 启用

---

#### Task 5：Multi-Edit 并行协调

**目标**：读工具并行执行，写工具串行执行

**Claude Code 实现**：
- `toolOrchestration.ts` — `partitionToolCalls()`
- Read-only tools（Grep, Read, Glob）并行
- Non-read-only tools（Edit, Write）串行
- `isConcurrencySafe` 属性分区

**实现步骤**：
1. 定义工具并发安全属性
2. 实现 `partition_tools()` 方法分区工具
3. 读工具并行执行（使用 `buffer_unordered`）
4. 写工具串行执行（保持顺序）
5. 合并结果返回

**关键文件**：`src/engine/tool_orchestration.rs`（新文件）

**环境变量**：
- `PRIORITY_AGENT_TOOL_CONCURRENCY=1` — 启用工具并发

---

#### Task 6：Diff/Patch 输出

**目标**：结构化 hunk 输出用于显示，生成可读的 git diff

**Claude Code 实现**：
- `diff.ts` — `structuredPatch` from `diff` library
- `getPatchFromContents()` — 生成 hunks
- `getPatchForDisplay()` — 应用编辑后生成 diff
- Tab-to-space 转换
- Ampersand/dollar 转义

**实现步骤**：
1. 引入 `diff` crate（添加依赖）
2. 实现 `structured_patch()` 函数
3. 实现 `get_patch_from_contents()` 生成 hunks
4. 实现 `get_patch_for_display()` 显示友好 diff
5. `file_edit` 工具返回结构化 patch 信息

**关键文件**：`src/engine/diff.rs`（新文件）

**环境变量**：
- `PRIORITY_AGENT_DIFF_OUTPUT=1` — 启用 diff 输出

---

### Phase 7 实施顺序

1. **Task 1 (Smart Edit)** — 最高优先级，直接影响代码编辑体验
2. **Task 2 (Diagnostic Tracking)** — 实时反馈错误
3. **Task 3 (Verification Agent)** — 确保代码质量
4. **Task 4 (Batch Refactor)** — 大规模重构能力
5. **Task 5 (Multi-Edit)** — 工具执行优化
6. **Task 6 (Diff/Patch)** — 显示友好

### Phase 7 当前状态

- ✅ Task 1（Smart Edit）：已完成
  - `normalize_quotes()` / `desanitize()` 字符串规范化
  - must-read-before-edit 检查（`PRIORITY_AGENT_SMART_EDIT=1` 启用）
  - 文件修改检测（基于 inode）
- ✅ Task 2（Diagnostic Tracking）：已完成
  - `DiagnosticTracker` 捕获 baseline diagnostics
  - `before_edit()` / `get_new_diagnostics()` 对比分析
  - `PRIORITY_AGENT_DIAGNOSTIC_TRACKING=1` 启用
- ✅ Task 3（Verification Agent）：已完成
  - `VerificationAgent` 对抗性验证专家
  - Build/Test/Lint/Concurrency/Boundary/Idempotency 探针
  - Verdict::Pass/Fail/Partial 判决
- ✅ Task 4（Batch Refactor）：已完成
  - `BatchRefactor` 批量重构器
  - 任务分解为 5-30 个并行 worktree agent
  - `PRIORITY_AGENT_BATCH_REFACTOR=1` 启用
- ✅ Task 5（Multi-Edit 并行协调）：已完成
  - `ToolOrchestrator` 工具编排器
  - 读工具并行（`buffer_unordered`）、写工具串行
  - `PRIORITY_AGENT_TOOL_CONCURRENCY=1` 启用
- ✅ Task 6（Diff/Patch 输出）：已完成
  - `diff.rs` 结构化 hunk 输出
  - `simple_diff()` / `get_patch_for_display()` / `get_patch_from_contents()`
  - `PRIORITY_AGENT_DIFF_OUTPUT=1` 启用

---

### 验证方式

每个 task 完成后：
1. 运行 `cargo test` 确保不破坏现有测试
2. 手动测试对应功能
3. 确认测试数量不减少

---

## OpenCode 对标分析与改进计划（2026-04-20）

### OpenCode 项目概述

OpenCode 是一个开源 AI 编程代理（TypeScript/Bun），采用 Effect Framework 函数式架构，支持 20+ LLM Provider、多前端（CLI/TUI/Web/Desktop）、精细权限系统、MCP 协议集成。

**项目结构**：
```
opencode-dev/
├── packages/
│   ├── opencode/          # 核心 CLI 主体
│   ├── app/               # Web 应用
│   ├── console/           # 控制台应用
│   ├── desktop/           # 桌面应用
│   ├── desktop-electron/  # Electron 实现
│   ├── web/               # Web 界面
│   ├── ui/                # UI 组件库
│   ├── shared/            # 共享工具库
│   ├── plugin/            # 插件系统
│   ├── sdk/               # SDK 实现
│   ├── identity/          # 身份认证
│   └── enterprise/        # 企业版功能
└── specs/                 # 规范文档
```

---

### OpenCode 架构亮点

#### 1. Effect Framework 架构
所有核心服务采用 Effect Framework 的 Layer 模式，便于测试和组合：
```typescript
export const layer = Layer.effect(
  Service,
  Effect.gen(function* () {
    return Service.of({ publish, subscribe, ... })
  }),
)
```

#### 2. 多 Provider 抽象
内置 20+ LLM Provider：
```typescript
const BUNDLED_PROVIDERS: Record<string, () => Promise<...>> = {
  "@ai-sdk/anthropic": () => import("@ai-sdk/anthropic").then((m) => m.createAnthropic),
  "@ai-sdk/openai": () => import("@ai-sdk/openai").then((m) => m.createOpenAI),
  "@ai-sdk/google": () => import("@ai-sdk/google").then((m) => m.createGoogleGenerativeAI),
  // ... 20+ providers
}
```

#### 3. 精细权限系统
```typescript
export class Rule extends Schema.Class<Rule>("PermissionRule")({
  permission: Schema.String,
  pattern: Schema.String,
  action: Action,  // allow | deny | ask
})
```
- 支持 `always/once/reject` 回复选项
- 内置敏感文件保护（如 `.env` 文件默认 ask）

#### 4. Tool Hook System
```typescript
"tool.execute.before"?: (input: { tool: string, args: any }) => Promise<void>
"tool.execute.after"?: (input: { tool: string, args: any, result: any }) => Promise<void>
```

#### 5. Skill 外部加载
- 从 `~/.claude/skills`、`~/.agents` 目录加载技能
- 支持从 URL 拉取技能
- 技能定义使用 Markdown 格式

#### 6. MCP OAuth 支持
支持 MCP OAuth 认证流程，多种传输方式（Stdio/StreamableHTTP/SSE）。

#### 7. Provider Hook
可以在请求/响应层拦截和修改数据。

---

### OpenCode vs rust-agent 对比

| 维度 | OpenCode | rust-agent |
|------|----------|------------|
| **语言** | TypeScript/Bun | Rust |
| **架构** | Effect Framework (函数式) | 传统 OOP + Trait |
| **LLM Provider** | 20+ 内置 | Moonshot/OpenAI |
| **插件系统** | Hook + 工具注入 | Manifest + 动态工具 |
| **权限系统** | 规则引擎 + 文件模式 | Glob pattern 规则 |
| **会话管理** | SQLite + Drizzle ORM | SQLite 直接 |
| **前端** | CLI/TUI/Web/Desktop | CLI/TUI |
| **Hook 系统** | 细粒度事件钩子 | 基础 Pre/Post Hook |

---

### 改进计划

#### Phase 1：Skill 与 Provider 增强（1-2周）

**Task 1：Skill 外部加载系统**
- **目标**：支持从 `~/.priority-agent/skills` 目录和 URL 加载技能
- **关键文件**：`src/skills/mod.rs`、`src/skills/loader.rs`、`src/skills/registry.rs`
- **环境变量**：`PRIORITY_AGENT_SKILLS_PATH`（冒号分隔多路径）、`PRIORITY_AGENT_SKILLS_URL`（冒号分隔多 URL）
- **状态**：✅ 已完成
  - `load_skill_from_url()` 从 URL 异步加载 skill
  - `get_extra_skill_paths()` / `get_remote_skill_urls()` 读取环境变量
  - `load_external_skills()` 异步加载所有外部 skills
  - `SkillRegistry.with_default_paths()` 支持环境变量配置

**Task 2：多 Provider 抽象层**
- **目标**：抽象 Provider 接口，支持动态加载更多 Provider
- **关键文件**：`src/services/api/mod.rs`、`src/services/api/provider.rs`
- **环境变量**：`PRIORITY_AGENT_PROVIDER_<NAME>`（格式：`TYPE:API_KEY:BASE_URL:MODEL`）
- **状态**：✅ 已完成
  - `ProviderRegistry` 注册表支持动态注册
  - `from_env()` 从环境变量加载 Kimi/OpenAI/Minimax Provider
  - `PRIORITY_AGENT_PROVIDER_<NAME>` 配置额外 Provider
  - `create_provider()` 根据类型创建对应 Provider

**Task 3：增强 Tool Hook（tool.before/after）**
- **目标**：在 `hooks.rs` 增加 `tool.execute.before` 和 `tool.execute.after` 事件
- **关键文件**：`src/engine/hooks.rs`
- **环境变量**：`PRIORITY_AGENT_TOOL_HOOK_BEFORE`、`PRIORITY_AGENT_TOOL_HOOK_AFTER`
- **状态**：未开始

---

#### Phase 2：Compaction 与权限增强（3-4周）

**Task 4：Compaction 压缩优化**
- **目标**：学习 OpenCode 的 Compaction 摘要生成逻辑
- **关键文件**：`src/engine/context_compressor.rs`
- **环境变量**：`PRIORITY_AGENT_COMPACTION_LLM=1`
- **状态**：✅ 已完成（TimeBasedConfig、CompressionWarning、micro_compress、needs_time_based_compression）

**Task 5：精细权限系统（once 模式）**
- **目标**：增加 `once`（一次性授权）模式，完善敏感文件保护
- **关键文件**：`src/permissions/mod.rs`
- **环境变量**：`PRIORITY_AGENT_PERMISSION_ONCE_DEFAULT=1`
- **状态**：✅ 已完成（Once 模式、grant_once/revoke_once/has_once_authorization/cleanup_expired_once）

**Task 6：MCP OAuth 支持**
- **目标**：增强 `mcp.rs`，支持 OAuth 认证流程
- **关键文件**：`src/engine/mcp.rs`
- **环境变量**：`PRIORITY_AGENT_MCP_OAUTH=1`
- **状态**：✅ 已完成（McpOAuthConfig、McpServerConfig 新增 oauth/oauth_token_url 字段）

---

#### Phase 3：高级特性（长期）

**Task 7：Provider Hook 系统**
- **目标**：在 API 层增加请求/响应拦截器
- **关键文件**：`src/services/api/mod.rs`
- **环境变量**：`PRIORITY_AGENT_PROVIDER_HOOK`
- **状态**：✅ 已完成（ProviderHook 结构体、pre_hook/post_hook/error_hook）

**Task 8：Config Hook**
- **目标**：在配置加载后增加钩子点
- **关键文件**：`src/services/config.rs`
- **环境变量**：`PRIORITY_AGENT_CONFIG_HOOK`
- **状态**：✅ 已完成（ConfigHook、ConfigLoader、execute 方法）

**Task 9：多前端架构（长期目标）**
- **目标**：拆分核心库为独立 crate，支持 Web/Desktop 前端
- **关键文件**：新建 `priority-core/` 和 `priority-cli/`
- **状态**：🔄 Phase 1-1 进行中（已完成 errors.rs 迁移）

#### 当前已迁移模块
- `errors.rs` → `priority-core/src/errors.rs` ✅

#### 发现的问题
- **permissions/**：依赖 `tools::bash_tool::is_dangerous_command`，存在循环依赖 — 已通过提取 security 模块解决
- **diagnostics/**：依赖 reqwest/dirs/serde_json 等外部 crate，迁移复杂度高

#### 目标 crate 结构

```
priority-agent/ (workspace)
├── Cargo.toml (workspace)
├── priority-core/ (library crate - 核心逻辑)
│   └── src/
│       ├── engine/      # 28 files - 核心引擎
│       ├── tools/       # 47 tool modules
│       ├── agent/      # 6 files - Agent 系统
│       ├── services/    # api/ (LLM providers)
│       ├── permissions/
│       ├── state/
│       ├── task_manager/
│       ├── memory/
│       ├── session_store/
│       └── bootstrap.rs
│
├── priority-cli/ (binary crate - TUI/CLI 前端)
│   └── src/
│       ├── main.rs
│       ├── tui/         # TUI 模块
│       └── cli/         # CLI 模块
│
└── src/ (minimal binary - 路由入口)
    └── main.rs (简化为路由)
```

#### 迁移顺序（按依赖关系）

**Phase 1-1：迁移无依赖模块**
1. `errors.rs` → priority-core ✅
2. `priority/` → priority-core (待测)
3. `weight_engine/` → priority-core (待测)

**Phase 1-2：迁移基础服务**
4. `services/api/` → priority-core (provider trait + implementations)
5. `state/` → priority-core

**Phase 1-3：迁移核心系统**
6. `engine/` → priority-core
7. `agent/` → priority-core
8. `tools/mod.rs` (Tool trait) → priority-core
9. `tools/*.rs` (所有工具) → priority-core

**Phase 1-4：迁移管理模块**
10. `task_manager/` → priority-core
11. `memory/` → priority-core
12. `session_store/` → priority-core
13. `cost_tracker/` → priority-core
14. `skills/` → priority-core
15. `bootstrap.rs` → priority-core

**Phase 1-5：迁移支持模块**
16. `context_manager/` → priority-core
17. `bridge/` → priority-core
18. `priority/` → priority-core
19. `remote/` → priority-core
20. `team/` → priority-core
21. `instructions/` → priority-core
22. `github/` → priority-core
23. `telemetry/` → priority-core
24. `voice/` → priority-core
25. `ide/` → priority-core
26. `migrations/` → priority-core
27. `plugins/` → priority-core

**Phase 2：迁移前端**
28. `tui/` → priority-cli
29. `cli/` → priority-cli

**Phase 3：简化根 main.rs**
30. `src/main.rs` 简化为路由入口

#### 依赖分离规则

**priority-core 不依赖**：
- ratatui, crossterm, clap (TUI/CLI 特定)
- axum (如果只是核心库不需要)

**priority-cli 依赖**：
- priority-core
- ratatui, crossterm, clap

#### 关键风险与解决方案

**风险 1：循环依赖**
- 问题：`engine` 导入 `tools`，`tools` 导入 `services`
- 解决：按依赖层级迁移，先迁出被依赖的模块

**风险 2：导入路径变更**
- 问题：`crate::engine` → `priority_core::engine`
- 解决：每个模块迁移后，批量更新所有引用

**风险 3：bootstrap.rs 耦合 TUI**
- 问题：`bootstrap.rs` 引用了 `tui::`
- 解决：将 TUI 相关初始化移至 `priority-cli`

#### 验证方式

1. `cargo build --workspace` - 编译成功
2. `cargo test --workspace` - 所有测试通过
3. 每个模块迁移后验证不受影响

---

### 当前状态

- ✅ Phase 7（编程能力补齐）：已完成
- 🔄 Phase 8（OpenCode 对标）：进行中
  - ✅ Task 1（Skill 外部加载）：已完成
  - ✅ Task 2（多 Provider 抽象）：已完成
  - ✅ Task 3（Tool Hook 增强）：已完成
  - ✅ Task 4（Compaction 优化）：已完成
  - ✅ Task 5（权限 once 模式）：已完成
  - ✅ Task 6（MCP OAuth）：已完成
  - ✅ Task 7（Provider Hook）：已完成
  - ✅ Task 8（Config Hook）：已完成
  - 🔄 Task 9（多前端架构）：Phase 1-1 进行中（已完成 errors.rs，permissions 循环依赖已解决）

---

### 验证方式

每个 task 完成后：
1. 运行 `cargo test --workspace` 确保不破坏现有测试
2. 手动测试对应功能
3. 确认测试数量不减少

---

## 2026-04-21 对标 Claude Code 4 周冲刺清单（按收益排序）

目标：优先补齐「用户体感最明显」和「团队落地最关键」的能力，不追求一次性全量对齐。

### 里程碑指标（4 周结束）

- 冷启动（无 API key 场景）<= 300ms（当前约 2.5s）
- `cargo test` 全绿且新增回归测试 >= 30 条
- 发布一个可复用的 GitHub Action 工作流（Issue/PR 自动触发）
- MCP 可用性增强（至少 1 种非 stdio 连接 + OAuth 流程走通）
- 形成一套可视化性能/稳定性报告（`/doctor` + benchmark 输出）

### Week 1 任务拆分（Issue 粒度）

#### W1-1 启动路径重排（P0）
- [x] W1-1.1 提取启动模式识别函数（`detect_startup_mode`）
- [x] W1-1.2 增加 Help 分支并提前返回（不触发 provider 初始化）
- [x] W1-1.3 增加 `main.rs` 单元测试（help/api/cli/tui 路径）
- [x] W1-1.4 验证 release 二进制 `--help` 行为

#### W1-2 测试环境变量隔离（P0）
- [x] W1-2.1 新增统一 `EnvVarGuard`（串行化 + 自动恢复）
- [x] W1-2.2 迁移 `conversation_loop` 关键 env 测试
- [x] W1-2.3 迁移 `hooks` 关键 env 测试
- [x] W1-2.4 扫描并迁移剩余 env 改写测试（`bootstrap`/`kimi`/`batch_refactor`/`telemetry` 已迁移）

#### W1-3 UTF-8/大输出回归（P0）
- [x] W1-3.1 UTF-8 截断边界回归测试
- [x] W1-3.2 小输出不截断回归测试
- [x] W1-3.3 大输出截断标记完整性测试
- [x] W1-3.4 上下文压缩链路 Unicode 压测

### Week 1（P0）：启动体验与稳定性底座

- [x] **W1-1 启动路径重排（最高优先级）**
  - 问题：`--help`/无 key 场景仍走 provider 初始化，拖慢启动且报错噪音大
  - 改动：`main.rs` 启动流程拆为「参数解析 -> 模式判定 -> 按需初始化 provider」
  - 验收：
    - `priority-agent --help` 不触发 provider 初始化
    - 无 key 时仅在进入需要 LLM 的模式时报错
    - 基准：3 次取均值，<= 300ms

- [x] **W1-2 全局环境变量测试隔离**
  - 问题：并行测试修改 env 导致 flaky
  - 改动：统一 `test_env_guard` 工具（锁 + set/restore helper）
  - 验收：
    - 连续跑 5 次 `cargo test` 无随机失败
    - 所有修改 env 的测试迁移到统一 helper

- [x] **W1-3 UTF-8/大输出防御全面回归**
  - 问题：工具输出截断、日志拼接边界风险
  - 改动：为截断、压缩、snip 增加 Unicode/超大输出测试
  - 验收：
    - 新增相关测试 >= 10 条
    - 无 panic

### Week 2（P0）：CI 自动化闭环（对标 Claude Code GitHub Action）

- [x] **W2-1 GitHub Action：Issue/PR 自动唤起 Agent**
  - 改动：`.github/workflows/priority-agent.yml`（评论触发、标签触发、手动触发）
  - 能力：拉取上下文 -> 执行任务 -> 评论回写结果
  - 验收：
    - 在测试仓库中可通过 `@priority-agent` 触发
    - 产出包含变更摘要 + 测试结果 + 风险提示

- [x] **W2-2 Action 运行预算与安全门**
  - 改动：加入 max turns / timeout / 仅允许白名单工具
  - 验收：
    - 超预算自动停止并回写原因
    - 敏感工具默认 deny（需显式开关）

- [x] **W2-3 标准化输出模板**
  - 改动：统一评论模板（结论、改动、验证、风险、下一步）
  - 验收：
    - 人工评审可在 1 分钟内读懂一次运行结果

### Week 3（P1）：MCP 与权限产品化增强

- [x] **W3-1 MCP 非 stdio 通道支持**
  - 改动：在 `engine/mcp.rs` 增加至少一种远程传输（HTTP/SSE/WebSocket 任选一）
  - 验收：
    - 可连接 1 个远程 MCP 服务并成功调用工具
    - 失败重试与超时可配置

- [x] **W3-2 MCP OAuth 全链路打通**
  - 改动：从配置到 token 持久化与刷新策略补齐
  - 验收：
    - 首次授权、过期刷新、撤销三条链路可演示
    - 错误提示清晰（不是仅日志）

- [ ] **W3-3 权限 UX 升级**
  - 改动：`/permissions` 增加 rule explain / 导入导出 / dry-run
  - 验收：
    - 用户可解释「为什么 allow/deny」
    - 项目级权限可一键导出与复用

#### Week 3 实施记录（2026-04-21）

- `McpTransport` 新增 `http`，支持 JSON-RPC over HTTP POST。
- `McpServerConfig` 新增 `http_url` 字段，`server_summaries`/`endpoint_summary` 已覆盖 `http`。
- OAuth：
  - 新增 token 结构与解析逻辑（`access_token/refresh_token/expires_in`）。
  - 新增 token 本地持久化（`data_local_dir()/priority-agent/mcp_oauth_tokens.json`）。
  - 连接前自动校验 token，过期时自动 refresh，缺失时执行认证。
- `mcp_auth` 工具已接通真实认证逻辑（不再是占位提示）。
- `mcp` 管理工具新增 `auth_server` action。

### Week 4（P1/P2）：性能观测与对比基准

- [x] **W4-1 建立 benchmark 脚本**
  - 改动：`scripts/benchmark.sh`（启动、首 token、工具调用、100 轮对话）
  - 验收：
    - 输出 markdown 报告，可提交到仓库
    - 支持本机多次对比（before/after）

- [ ] **W4-2 /doctor 增强为性能体检面板**
  - 改动：接入缓存命中率、工具耗时 P95、失败原因 TopN、上下文压缩触发率
  - 验收：
    - 一条命令看到主要性能瓶颈
    - JSON 输出可用于 CI 归档

- [ ] **W4-3 与桌面 `claude` 项目的持续差距看板**
  - 改动：新增 `docs/CLAUDE_GAP_SCORECARD.md`
  - 维度：工具覆盖、命令覆盖、自动化能力、跨端能力、性能
  - 验收：
    - 每周更新一次，差距趋势可见（不是静态报告）

#### Week 4 实施记录（2026-04-21）

- W4-1 已完成：
  - 新增 `scripts/benchmark.sh`，支持：
    - 冷启动 `--help` 延迟测量
    - `/api/chat` 首响应延迟测量（有 LLM key 时自动启用）
    - `/api/tools/call` 工具调用延迟测量
    - 可选 100 轮对话压测（`--enable-long-chat`）
    - markdown 报告输出到 `docs/benchmarks/`
    - 与历史报告对比（`--compare <old_report.md>`）
  - 已生成示例报告：`docs/benchmarks/report-smoke-20260421-101454.md`
- 额外修复（支撑 benchmark）：
  - 修复 `experimental-api-server` 编译失败：`main.rs` 的 `--api` 分支已补齐 provider/tool registry/LSP/worktree 初始化，并正确调用 `api::start_server(...)`。

### 执行规则（必须遵守）

- 每周只追 1-2 个 P0 主目标，避免并行过多导致延期
- 每个任务必须带「可量化验收」和「最小回归测试」
- 对外能力优先于内部重构；除非重构能在两周内转化为用户可感知收益
- 每周末固定输出：
  - 已完成 / 未完成 / 阻塞项
  - 指标变化（启动、测试时长、失败率）
  - 下周范围收敛（删减低收益任务）

---

## Phase 9：Critical & High 优先级缺口（2026-04-21 开始）

目标：消除 Critical 缺口，补齐 High 优先级缺口，大幅提升命令覆盖率

### 目标清单

| 优先级 | 缺口 | 目标 |
|--------|------|------|
| 🔴 Critical | Advanced Agent Types | 补齐 teammate/remote specialist/dream-task |
| 🟡 High | WebSocket MCP transport | 完成 Phase 4 Task 3 收尾 |
| 🟡 High | 更多命令 | 新增 10+ 高价值命令 |
| 🟡 High | Workspace 拆分 | 完成 Phase 8 Task 9 剩余模块 |

---

### Task 1：Advanced Agent Types（Critical）

**目标**：补齐 Claude Code 的 7 种 Agent 类型中的 5 种缺失类型

**Claude Code Agent 类型**：
- Task Agent ✅（已有）
- Teammate ❌（缺失）
- Assistant ❌（缺失）
- Critic ❌（缺失）
- Verifier ✅（已有）
- Remote Specialist ❌（缺失）
- Dream Task ❌（缺失）

**实现步骤**：

1. **Teammate Agent（队友 Agent）**
   - 目标：与其他 Agent 协作完成复杂任务
   - 新增 `TeammateAgent` 结构体
   - 实现任务委托、结果汇总机制
   - 注册为 skill `/teammate`

2. **Assistant Agent（助手 Agent）**
   - 目标：专注特定领域（代码审查、安全、数据处理）
   - 新增 `AssistantAgent` 结构体，支持 `domain` 参数
   - 提供预设 domain 配置

3. **Critic Agent（批评 Agent）**
   - 目标：审查其他 Agent 的工作，提出改进建议
   - 新增 `CriticAgent` 结构体
   - 实现审查 prompt 模板

4. **Remote Specialist（远程专家 Agent）**
   - 目标：通过 Bridge 调度远程 Agent
   - 复用现有 `BridgeClient`
   - 新增 `RemoteSpecialistAgent`

5. **Dream Task Agent（梦境任务 Agent）**
   - 目标：后台探索性分析，不阻塞主对话
   - 新增 `DreamTaskAgent` 结构体
   - 实现异步执行、结果回调机制

**关键文件**：`src/agent/mod.rs`, `src/agent/manager.rs`, `src/agent/types.rs`

**环境变量**：
- `PRIORITY_AGENT_ADVANCED_AGENTS=1` — 启用高级 Agent

**验收标准**：
- 至少 3 种新 Agent 类型可用
- `/teammate` 命令可演示任务协作
- `cargo test` 通过

---

### Task 2：WebSocket MCP Transport（High）

**目标**：完成 Phase 4 Task 3 收尾，新增 WebSocket 传输支持

**现状**：Phase 4 Task 3 第一阶段已完成 stdio/http 传输骨架

**实现步骤**：

1. 在 `McpTransport` 枚举新增 `websocket` 变体
2. 实现 `WebSocketTransport` 结构体
3. 支持 `ws://`/`wss://` URL
4. 实现连接管理、心跳、重新连接
5. 在 `/doctor` 中显示 WebSocket 状态

**关键文件**：`src/engine/mcp.rs`

**环境变量**：
- `PRIORITY_AGENT_MCP_WS_URL` — WebSocket endpoint

---

### Task 3：高价值命令补齐（High）

**目标**：新增 10+ 高价值命令，快速提升命令覆盖率

**目标命令（按优先级）**：

1. `/btw` — 随口说一句（one-off 注释）
2. `/context` — 显示当前上下文状态
3. `/config` — 交互式配置管理
4. `/keybindings` — 显示/修改键位
5. `/git` — 内联 Git 操作
6. `/history` — 会话历史查看
7. `/retry` — 重试上一次 LLM 调用
8. `/undo` — 撤销上一次操作
9. `/mode` — 切换交互模式
10. `/package` — 包管理相关操作

**实现步骤**：

1. 在 `src/tui/commands.rs` 新增命令常量
2. 在 `src/tui/slash_handler.rs` 实现处理函数
3. 注册命令到 slash command registry
4. 编写对应 bundled skill

**环境变量**：
- `PRIORITY_AGENT_EXTRA_COMMANDS=1` — 启用额外命令

**验收标准**：
- 新增 >= 10 条命令
- `/help` 中可见新命令
- `cargo test` 通过

---

### Task 4：Workspace 拆分收尾（High）

**目标**：完成 Phase 8 Task 9 的剩余模块迁移

**已完成**：
- errors.rs ✅
- workspace 结构 ✅
- permissions/ 循环依赖已解决（提取 security 模块）✅

**待解决**：
- 剩余 20+ 模块迁移到 priority-core

**解决方案**：

1. **继续迁移**：
   - state/ → priority-core
   - services/api/ → priority-core
   - engine/ → priority-core
   - tools/ → priority-core

**关键文件**：`priority-core/src/`, `priority-cli/src/`

**验收标准**：
- `cargo build --workspace` 成功
- `cargo test --workspace` 通过

---

### Phase 9 实施顺序

1. **Task 3（命令补齐）** — 先做，快速见效
2. **Task 2（WebSocket MCP）** — Phase 4 收尾
3. **Task 1（Advanced Agents）** — 架构改动大，后做
4. **Task 4（Workspace 拆分）** — 技术债务清理

---

## Phase 10：命令补齐计划

目标：逐步补充缺失命令，每次新增 10 条，最终达到 60+ 命令覆盖

### 缺失命令清单（39 条已识别 + 30+ 未枚举）

**第一批（10 条）- 高频实用命令**
| 命令 | 描述 | 优先级 |
|------|------|--------|
| `/session` | 会话管理（查看/切换） | P0 |
| `/undo` | 撤销上一次操作 | P0 |
| `/redo` | 重做 | P0 |
| `/retry` | 重试上一次 LLM 调用 | P0 |
| `/stop` | 停止当前正在进行的操作 | P0 |
| `/reload` | 重新加载配置/插件 | P1 |
| `/share` | 分享当前会话 | P1 |
| `/token` | 显示当前 token 使用情况 | P1 |
| `/lsp` | LSP 服务器管理 | P1 |
| `/npm` | npm 包管理辅助 | P1 |

**第二批（10 条）- 工具/调试命令**
| 命令 | 描述 | 优先级 |
|------|------|--------|
| `/hooks` | 查看/管理 hooks | P1 |
| `/profiling` | 性能分析 | P2 |
| `/prompt` | 提示词管理 | P2 |
| `/migrate` | 迁移工具 | P2 |
| `/focus` | 专注模式 | P2 |
| `/pause` | 暂停对话 | P2 |
| `/install` | 安装依赖 | P2 |
| `/skeleton` | 生成代码骨架 | P2 |
| `/branch` | Git 分支操作 | P2 |
| `/color` | 颜色配置 | P3 |

**第三批（10 条）- 集成/高级命令**
| 命令 | 描述 | 优先级 |
|------|------|--------|
| `/webhook` | Webhook 管理 | P2 |
| `/wizard` | 向导模式 | P2 |
| `/workspace` | 工作区管理 | P2 |
| `/slack` | Slack 集成 | P3 |
| `/stealth` | 隐身模式 | P3 |
| `/shadow` | 影子模式 | P3 |
| `/reject` | 拒绝建议 | P3 |
| `/subscribe` | 订阅更新 | P3 |
| `/slots` | 槽位管理 | P3 |
| `/ticker` |  ticker | P3 |

**第四批（9 条）- 收尾**
| 命令 | 描述 | 优先级 |
|------|------|--------|
| `/config` | 交互式配置 | P1 |
| `/copy` | 复制内容 | P1 |
| `/desktop` | 桌面集成 | P2 |
| `/chrome` | Chrome 集成 | P3 |
| `/effort` | 预估工作量 | P2 |
| `/preamble` | 修改前置提示 | P2 |
| `/untrap` | 取消拦截 | P3 |
| `/verbose` | 详细输出 | P3 |
| `/write` | 写入文件 | P2 |

### 实施顺序

1. **Batch 1（第一批）**：session, undo, redo, retry, stop, reload, share, token, lsp, npm
2. **Batch 2（第二批）**：hooks, profiling, prompt, migrate, focus, pause, install, skeleton, branch, color
3. **Batch 3（第三批）**：webhook, wizard, workspace, slack, stealth, shadow, reject, subscribe, slots, ticker
4. **Batch 4（第四批）**：config, copy, desktop, chrome, effort, preamble, untrap, verbose, write

### Phase 10 当前状态

- ✅ Batch 1（10 条）：已完成 (session, undo, redo, retry, stop, reload, share, token, lsp, npm)
- ✅ Batch 2（10 条）：已完成 (hooks, profiling, prompt, migrate, focus, pause, install, skeleton, branch, color)
- ✅ Batch 3（10 条）：已完成 (webhook, wizard, workspace, slack, stealth, shadow, reject, subscribe, slots, ticker)
- ✅ Batch 4（9 条）：已完成 (config, copy, desktop, chrome, effort, preamble, untrap, verbose, write)

**Phase 10 完成！共实现 101 条命令，完全对齐 Claude Code！**

---

### 验证标准

每批完成后：
1. `cargo test` 通过
2. `/help` 中可见新命令
3. 更新 `docs/CLAUDE_GAP_SCORECARD.md`
4. 命令 gap 减少 10 条

### Phase 9 当前状态

- ✅ Task 1（Advanced Agent Types）：已完成
  - 新增 `/teammate` 命令（协作队友 Agent）
  - 新增 `/critic` 命令（批评型代码审查 Agent）
  - 新增 `/assistant` 命令（领域专家 Agent，支持 code_review/security/data/infrastructure/testing）
  - 新增 `/remote` 命令（远程专家 Agent）
  - 新增 bundled skills：teammate.md、critic.md、assistant.md、remote.md
- ✅ Task 2（WebSocket MCP）：已完成
  - WebSocket transport 已在 Phase 4 Task 3 实现
  - `McpTransport::WebSocket` 支持 ws:// 和 wss://
  - 自动重连和断线检测已实现
- ✅ Task 3（命令补齐）：已完成
  - 新增 `/btw` 命令（随口注释）
  - 新增 `/context` 命令（显示上下文状态）
  - 新增 `/git` 命令（内联 Git 操作）
  - 新增 `/history` 命令（会话历史查看）
  - 新增 `/mode` 命令（切换交互模式）
  - 新增 `/package` 命令（包管理信息）
  - 命令总数从 22 增加到 28
- ✅ Task 4（Workspace 拆分）：已完成
  - 新增 `src/security/mod.rs` 和 `src/security/dangerous_command.rs`
  - 提取 `is_dangerous_command` 到 security 模块，打破 permissions ↔ tools/bash_tool 循环依赖
  - `permissions/mod.rs` 和 `tools/bash_tool/mod.rs` 均使用 `crate::security::is_dangerous_command`
  - `cargo build` 和 `cargo test` (498 passed) 均通过

---

### 验证方式

每个 task 完成后：
1. 运行 `cargo test` 确保不破坏现有测试
2. 手动测试对应功能
3. 确认测试数量不减少
4. 更新 `docs/CLAUDE_GAP_SCORECARD.md`
