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
9. **上下文折叠** — 高难度，未完成
10. **权限系统增强** — 可改进
