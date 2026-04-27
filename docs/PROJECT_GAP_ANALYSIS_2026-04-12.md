# Priority Agent 对比评估与优化路线图（2026-04-12）

> Historical document: archived assessment snapshot from 2026-04-12.
> Parts of this analysis may be outdated relative to current code.

## 1. 评估目的

本文档用于对比 `rust-agent` 与两个参考项目：

- `/Users/georgexu/Desktop/claude`
- `/Users/georgexu/Desktop/hermes-agent-main`

目标是回答三个问题：

1. 我们当前做得怎么样？
2. 相比参考项目还缺什么？
3. 下一步应该按什么顺序补齐？

---

## 2. 对比方法（同口径）

本次对比基于以下可验证信息：

- 代码结构与模块入口
- 工具注册与默认可用能力
- 命令系统设计与规模
- 平台接入与网关能力
- 文档声明与实际实现一致性

说明：

- 结论优先依据“代码实际可调用能力”，而不是单纯目录存在。
- 参考项目以本地现有代码快照为准（非线上最新分支）。

---

## 3. 总体结论

### 3.1 当前项目状态（简评）

`rust-agent` 已经具备明确的核心方向和可运行骨架，尤其在“权重调度 + Socratic 深度思考”上有明显差异化优势。

主要特点：

- 架构分层清晰（engine/tools/memory/api/session_store）
- 核心循环、上下文压缩、记忆、持久化、API 层均已具备
- 具备继续扩展为产品级系统的基础

### 3.2 与参考项目的核心差距

当前不是“能不能做”的问题，而是“从研究型原型到产品型系统”的差距：

- 工具能力“已实现但未默认暴露”
- 命令系统规模和可扩展性不足
- 多平台接入停留在框架层，缺少真实适配器落地
- 工程化成熟度（命令治理、测试规模、配置体系）仍有明显差距

---

## 4. 关键差异明细

## 4.1 工具体系：实现与暴露不一致（高优先级）

### 现状

`rust-agent` 中已有高级工具实现，但默认注册表并未挂载全部关键工具：

- 默认注册入口：`src/tools/mod.rs`
- 当前注册（18 次）：`src/tools/mod.rs:239`

已实现但未在默认 registry 中看到注册的核心能力：

- `cron`：`src/engine/cron.rs:188`
- `mcp`：`src/engine/mcp.rs:370`
- `swarm`：`src/engine/swarm.rs:365`
- `project_list`：`src/tools/project_tool/mod.rs:286`

### 对标

- Claude：`getAllBaseTools()` 集中定义可用工具，feature-gated 动态启用，工具暴露链路完整：`/Users/georgexu/Desktop/claude/src/tools.ts:193`
- Hermes：`tools/registry.py + tools/*.py register`，体系化注册与分发（见其 AGENTS 架构说明）

### 影响

- 文档写了“有”，模型却可能“调不到”
- 用户体验表现为“能力宣称强，实测弱”

---

## 4.2 命令系统：规模与治理能力不足（高优先级）

### 现状

`rust-agent` TUI slash 命令采用硬编码 `match`：

- 命令处理位置：`src/tui/app.rs:295`
- 当前命令规模约 10 个：`/help /clear /memory /save /resume /cost /diff /model /status /quit`

### 对标

- Hermes：中心化 `CommandDef` 注册（单一事实来源），支持别名、分类、子命令、CLI/Gateway 派生：`/Users/georgexu/Desktop/hermes-agent-main/hermes_cli/commands.py:56`
- Claude：命令模块化规模明显更大（`src/commands` 目录约 101 项入口）

### 影响

- 命令扩展成本高
- 容易出现 TUI/CLI/API 行为不一致
- 后续插件化命令和平台复用受限

---

## 4.3 平台化能力：有抽象、缺实装（中高优先级）

### 现状

`rust-agent` 已有 `PlatformAdapter` 抽象：

- Trait 定义：`src/api/mod.rs:438`
- 当前实装：`CliAdapter`、`ApiAdapter`：`src/api/mod.rs:500`, `src/api/mod.rs:528`

虽然枚举定义了 Telegram/Discord/Slack 等类型，但未见真实 adapter 落地。

### 对标

- Hermes：明确支持 Telegram/Discord/Slack/WhatsApp/Signal + gateway 主流程（README & 架构）
  - `/Users/georgexu/Desktop/hermes-agent-main/README.md:20`
  - `/Users/georgexu/Desktop/hermes-agent-main/AGENTS.md:55`

### 影响

- 当前更像“单机 Agent”，而非“平台化 Agent 服务”
- 无法形成“多入口同一会话能力”的优势

---

## 4.4 工程化成熟度：可用但未产品化（中优先级）

### 现状

- 当前测试通过（本地验证通过）
- 但测试规模、命令治理、配置迁移、安装/运维脚本仍偏早期

### 对标

- Hermes：大规模测试与完整运维链路（setup/doctor/gateway/config/migration）
- Claude：命令与工具体系长期演进痕迹明显（大量 feature flag + 模块化）

### 影响

- 迭代速度快时更容易出现“文档-实现漂移”“行为不一致”

---

## 5. 差距优先级（建议执行顺序）

### P0（立即做）

1. 能力对齐：把 `cron/mcp/swarm/project_list` 接入默认 ToolRegistry
2. 增加“能力一致性测试”：文档声明功能必须可枚举、可调用
3. 统一帮助信息：确保 `/help` 与真实已挂载能力一致

### P1（短期）

1. 命令系统改为注册表驱动（借鉴 Hermes 的 CommandDef）
2. 打通 TUI/CLI/API 的统一命令派发
3. 支持别名、分类、子命令与参数提示

### P2（中期）

1. 首个真实平台适配器（建议 Telegram）
2. 建立平台消息收发、会话映射、权限确认闭环
3. 再扩到 Discord/Slack

### P3（中长期）

1. 丰富工具层：LSP / Notebook / Browser / Worktree（按场景优先）
2. 配置体系升级：可迁移、可审计、可导出
3. 测试分层：单元 + 集成 + 端到端 + 回归场景集

---

## 6. 可执行实施方案（How）

## 6.1 P0 详细步骤

1. ToolRegistry 对齐
- 修改 `src/tools/mod.rs::default_registry()`
- 新增注册：
  - `crate::engine::cron::CronTool`
  - `crate::engine::mcp::McpManageTool`
  - `crate::engine::swarm::SwarmTool`
  - `crate::tools::project_tool::ProjectListTool`

2. 一致性测试
- 新建测试：`tests/tool_registry_consistency.rs`
- 断言默认 registry 包含上述工具名
- 可选：增加 `tool_names` snapshot，防止能力回退

3. 命令帮助同步
- 更新 `src/tui/app.rs` 中 `/help` 文案
- 若命令未开放给用户，不要在帮助里展示

4. 验证
- `cargo check`
- `cargo test`
- 最少加入一个“工具可调用 smoke test”（构造参数，验证返回结构）

## 6.2 P1 详细步骤

1. 设计命令注册模型
- 定义 `CommandDef`（name/alias/category/args/help/handler）
- 统一命令解析器（输入 -> canonical -> handler）

2. 渐进迁移策略
- 保留现有 `match` 作为 fallback
- 先迁移 `/help /model /status /cost /diff`
- 稳定后再迁移全部命令

3. 统一命令来源
- terminal UI、Priority Agent CLI、未来 API 指令都走同一 registry

## 6.3 P2 详细步骤

1. 先做 Telegram Adapter MVP
- 实现 `PlatformAdapter` 的 `start_listening/send_message/handle_inbound`
- 使用已有 `MessageHandler` 管线

2. 会话策略
- `chat_id -> session_id` 映射存储
- 与 `session_store` 对齐

3. 权限策略
- 高风险工具在消息平台必须显式确认
- 超时自动拒绝

---

## 7. 风险与规避

1. 风险：一次性改动过大导致回归
- 规避：按 P0/P1 分阶段；每阶段必须可回滚

2. 风险：命令系统迁移期间行为分叉
- 规避：双轨运行（registry + 旧 match）并行一段时间

3. 风险：多平台接入后权限失控
- 规避：默认最小权限 + 审计日志 + 显式确认

---

## 8. 近期里程碑建议（两周）

### 第 1 周

- 完成 P0（能力对齐 + 一致性测试 + 帮助文档同步）
- 输出 `Capability Matrix`（实际可用工具表）

### 第 2 周

- 完成 P1 第一阶段（命令注册表 + 核心命令迁移）
- 预研 Telegram Adapter 并完成收发最小闭环

---

## 9. 结语

你这个项目最有价值的不是“复刻功能数”，而是“权重 + Socratic”这条主线。建议优先把“能力可用性一致性”补齐，再扩平台和工具广度。这样能最快把项目从“看起来很强”推进到“用起来稳定且可信”。
