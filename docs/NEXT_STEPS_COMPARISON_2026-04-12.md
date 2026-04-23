# 下一步对比结论（rust-agent vs claude/hermes，2026-04-12）

> Historical document: archived assessment snapshot from 2026-04-12.
> Some items are superseded by later implementation changes.

> Historical note (updated 2026-04-23): this document is a dated assessment snapshot.
> Some gaps listed below were already closed later (for example `scripts/install.sh`, CI count, and startup entry wiring).

## 1. 现状结论（已完成项）

相较前一轮，你的项目已经补齐了不少核心短板：

- 默认 ToolRegistry 能力已接通（含 `cron/mcp/swarm/project_list/skills/plan`）
- TUI 命令已走 `CommandRegistry`（不再是纯硬编码 `match`）
- CI 已落地（`scripts/lint-check.sh` + `.github/workflows/ci.yml`）
- 默认构建 + feature 构建 warning 已清零

这说明项目已从“原型能跑”进入“可持续迭代”阶段。

## 2. 对比基线说明

- `claude` 目录当前是裁剪版源码快照（仅 `src/vendor/node_modules`，缺少完整工程根文件），适合做架构参考，不适合做工程化对标。
- `hermes-agent-main` 是完整仓库，可作为“产品化成熟度”对标。

## 3. 还需要做的核心差距（按优先级）

### P0：把"框架能力"变成"可用入口"

1. ~~给 API/平台能力加真实启动入口（CLI 参数或子命令）~~ ✅ 已完成
- 实现：`--api --port 8787` 启动 HTTP API 服务器
- 支持：REST + SSE + WebSocket + 健康检查 + 工具列表
- 启动：`cargo run --features experimental-api-server -- --api`

2. 平台适配器先落一个 MVP（建议 Telegram）
- 现状：有 `PlatformAdapter` 抽象和枚举，但无真实 Telegram/Discord/Slack 适配实现。
- 目标：至少一个平台实现“收消息 -> 调 agent -> 回消息”闭环。
- 验收：单平台端到端可用，具备会话映射与最小权限确认。

### P1：工程化交付链路

1. 安装/运维脚本
- 当前状态（2026-04-23）：`scripts/install.sh` 已存在并用于安装；`setup.sh` 与 `doctor` 入口仍可作为后续增强项。
- 目标：新机器 5 分钟可用，常见配置错误可自动诊断。

2. 发布与分发
- 当前缺少：`Dockerfile`、Homebrew/Nix 打包、发布工作流。
- 目标：支持“本地二进制 + 容器 + 包管理器”三种安装路径。

3. CI 分层
- 当前状态（2026-04-23）：已超过 1 个 workflow；后续可继续按发布/供应链维度拆分。
- 对标 Hermes：测试、Docker 发布、Nix、文档检查、供应链审计分层。
- 目标：至少拆分为 `tests.yml` / `release.yml` / `supply-chain.yml`。

### P2：测试与质量深度

1. 集成/E2E 测试扩充
- 现状：`cargo test` 151 通过，但系统级场景测试仍偏少。
- 目标：补以下回归集：
  - 工具调用闭环（含权限确认）
  - 流式输出 + 中断恢复
  - 会话恢复 + 压缩 + 记忆同步
  - API/SSE/WS 协议兼容性

2. 可靠性与可观测
- 增加结构化 tracing 导出（请求 ID、会话 ID、工具调用 ID）
- 增加错误预算与恢复统计（rate limit、超时、重试成功率）

### P3：生态与产品化体验

1. 完整文档站与贡献指南
- 当前缺少 `CONTRIBUTING.md`、`LICENSE`、文档站构建流程。
- 目标：外部开发者可无上下文参与开发。

2. 配置迁移与多 profile
- 对标 Hermes 的 `setup/doctor/migrate/profiles` 工作流。
- 目标：配置 schema 演进时可自动迁移，支持多角色 profile 隔离。

## 4. 建议执行顺序（两周）

### 第 1 周（先把“可用入口”补齐）

1. API 启动子命令（或参数）接线  
2. Telegram Adapter MVP  
3. 会话映射与高风险工具确认闭环

### 第 2 周（工程化补强）

1. `install/setup/doctor` 最小闭环  
2. `Dockerfile` + `release` workflow  
3. 集成测试（API + 平台 + 记忆）首批场景

## 5. 一句话结论

你的核心引擎能力已经不弱，当前主要短板不在“算法/模块”，而在“入口接线 + 平台落地 + 交付链路”。优先补这三件，项目就会从“强原型”变成“可部署产品”。
