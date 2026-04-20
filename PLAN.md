# Priority Agent 达标 Claude Code 路线图

> 本计划按 重要性 P0 → P1 → P2 逐步执行。每个 Phase 完成后需保证 `cargo check/clippy/test`
> 全部通过，并更新相关测试。

---

## P0 阶段 — 核心能力洪沟（Coding Agent 生存必备）

### Phase 1: 增强 FileEditTool（精确文件编辑）✅ DONE
**现状**：已有基础版本，仅支持简单的 `replacen(old_string, new_string, 1)`，没有快照、模糊匹配、多处替换或插入操作。

**已完成**：
1. ✅ 支持 `expected_replacements` 参数：默认唯一，但可允许模型声明要替换多少处。
2. ✅ 失败时返回详细上下文（包含匹配位置的前后几行代码），帮助模型纠正。
3. ✅ 支持模糊匹配（空白/缩进容差）作为 fallback。
4. ✅ 添加 `insert_after` / `insert_before` 操作（可选），支持简单增量写入。
5. ✅ 每次编辑前自动保存文件原内容到临时快照目录（为 Phase 3 Rewind 做准备）。

**测试**：新增 7 个单元测试覆盖正常/异常/多处替换/模糊匹配/插入/快照场景。325 tests pass。

---

### Phase 2: AskUserQuestionTool 真正 Wire 到 TUI ✅ DONE
**现状**：`ask_user` 工具已存在，但 TUI 主循环没有检查并显示 pending question。用户看不到问题，工具调用会卡死。

**已完成**：
1. ✅ 在 `TuiApp` 的 `on_tick` 中添加对 `AskChannel::take_pending()` 的轮询（`check_pending_question`）。
2. ✅ 当模型调用 `ask_user` 时，暂停流式输出，在聊天界面中渲染问题和选项（`render_ask_user` popup）。
3. ✅ 用户回复后，将答案作为 tool result 返回给模型，继续对话。
4. ✅ 支持数字键 1-9 选择选项，Enter 确认。

**测试**：手动架构已验证；TUI 事件处理逻辑通过编译检查。

---

### Phase 3: /resume TUI 会话恢复 ✅ DONE
**现状**：`/resume` 命令已存在，但恢复后会添加两条冗余系统消息。

**已完成**：
1. ✅ 修复 `restore_session` 内部重复添加系统消息的问题，统一由 `handle_slash_command` 处理。
2. ✅ 改进恢复提示信息，显示会话 ID 和消息数量。
3. ✅ 新增单元测试覆盖成功恢复和会话不存在场景。

**测试**：`test_restore_session`、`test_restore_session_not_found` 通过。

---

### Phase 4: Rewind / 文件修改回滚 ✅ DONE
**现状**：没有文件级撤销。agent 自动编辑出错后用户无法快速恢复。

**已完成**：
1. ✅ 建立 `FileSnapshot` 系统：在 `file_edit` 和 `file_write` 执行前自动保存原文件内容到 `~/.priority-agent/snapshots/<session_id>/<timestamp>/<path>`。
2. ✅ 在快照系统中记录每次修改的元数据（时间戳、文件路径、工具名、快照路径）到 `edits.json`。
3. ✅ 新增 `/rewind` 命令：支持 `/rewind` 列出编辑历史、`/rewind <n>` 回滚最近 n 次编辑，或 `/rewind <file>` 回滚某个文件到上一个版本。
4. ✅ 在 `TuiSessionManager` 中添加 `list_edits`、`rewind_last_edit`、`rewind_file` 方法。

**测试**：新增 3 个单元测试覆盖空编辑历史、回滚单次编辑、回滚指定文件场景。328 tests pass。

---

### Phase 5: AgentTool 深度化（子代理系统） ✅ DONE
**现状**：`AgentTool` 只有 241 行，是基础 spawn/wait 框架，没有内置代理、没有并行执行、没有 memory snapshot。

**已完成**：
1. ✅ 内置子代理模板：`explore`（代码探索）、`verify`（验证/审查）、`plan`（任务规划）。通过 `template` 参数触发，自动构建专属 system prompt。
2. ✅ 子代理自动装载项目上下文：通过 `files` 参数，在执行前自动读取相关文件内容并以 `## File: path\n```\ncontent\n```格式注入 system prompt。
3. ✅ 支持并行执行多个子代理：新增 `subtasks` 数组参数，支持一次创建多个子 agent 并通过 `futures::future::join_all` 并行等待。结果通过 `synthesize_results` 统一汇总，显示 success/failure 统计和每个 agent 详细输出。
4. ✅ 子代理结果汇总时保留调用链：汇总结果中包含 `parent_session`、`agent_id`、`role`、`template` 等元数据，方便追溯。
5. ✅ 支持 `resume_agent` 基础能力：通过 `agent_id` 参数可以直接查询已完成子代理的结果（`AgentManager::get_result`），无需重新 spawn。

**测试**：新增 7 个单元测试覆盖模板生成、文件上下文加载、resume 路径、subtasks 校验。333 tests pass，clippy clean。

---

### Phase 6: 扩展 TUI 权限/审批流程 ✅ DONE
**现状**：已有基础权限系统，但 TUI 中的交互式审批弹窗是占位符，且仅对 `mcp_tool` 生效。

**已完成**：
1. ✅ 将交互式审批扩展到所有需要确认的工具（`bash`、`file_write`、`file_edit`、`agent`、`mcp_tool` 等）。
2. ✅ `AppMode::PermissionApproval` 渲染实际界面：显示工具名、参数、提示语，以及 `y = Allow / n = Deny` 选项。
3. ✅ `AppMode::DiffViewer` 支持权限审批中的 diff 预览：对 `file_write`/`file_edit`/`bash` 工具按 `d` 键可查看变更预览，看完按 Esc/q 返回审批弹窗。
4. ✅ 在主循环中正确传递权限响应到 `ToolApprovalRequest`（通过 `approval_channel`）。
5. ✅ 记录用户审批决策到 session_store（以 system message 形式写入当前会话）。

**测试**：新增 6 个单元测试覆盖 `respond_to_permission` 状态转换、`compute_permission_diff`（file_write/file_edit/bash/unsupported）。340 tests pass，clippy clean。

---

### Phase 6b: ContextCompressor 接入 LLM Provider [DONE]
**现状**：`ContextCompressor` 已集成到 `conversation_loop`，但只使用启发式摘要。`llm_summarize_middle` 已存在但未被主循环调用。

**目标**：
1. 在 `StreamingQueryEngine` / `ConversationLoop` 初始化时，如果配置了 LLM provider，将其传递给 `ContextCompressor`。
2. 在压缩时，优先尝试调用 `llm_summarize_middle` 异步生成高质量摘要；失败时回退到启发式摘要。
3. 添加压缩质量监控（比如压缩后 token 数、节省率、失败次数）。
4. 调整压缩触发阈值，避免频繁压缩影响用户体验。

**验收**：单元测试验证 LLM 摘要失败时能正确回退；无 provider 时不会崩溃。

---

## P1 阶段 — 产品竞争力（差异化与用户体验）

### Phase 7: 自动记忆提取（Auto Memory Extraction） [DONE]
1. 在每次对话结束后（或达到一定轮数后），用轻量级 prompt 让模型提取 "Critical Context"。
2. 自动写入 `MEMORY.md` 或 SQLite memory 表，无需用户手动 `/save`。
3. 支持记忆去重（基于相似度或 hash），避免重复写入。

### Phase 8: /doctor 深度诊断 [DONE]
1. 扩展 `/doctor` 命令：检测 `git` 可用性、网络连通性（ping Kimi API）、工具链完整性、常见配置错误。
2. 提供可执行的修复建议（如“缺少 MOONSHOT_API_KEY，请设置环境变量”）。
3. 生成诊断报告 JSON，方便远程排错。

### Phase 9: MCP 资源层完善 [DONE]
1. 完善 `list_mcp_resources` 和 `read_mcp_resource` 工具（已注册但可能未完全实现）。
2. 添加 MCP 服务器鉴权/审批流程（类似 Claude 的 `mcpServerApproval`）。
3. 在工具调用前检查 MCP 连接健康状态。

### Phase 10: 文件状态缓存与历史快照 ✅ DONE
**已完成**：
1. ✅ `FileStateCache` 完整实现：`GLOBAL_FILE_CACHE` 懒加载全局缓存，支持元数据（mtime/size）缓存、内容缓存与过期检测。
2. ✅ 项目级文件扫描：`scan_project()` 使用 `ignore::WalkBuilder` 遍历项目树。
3. ✅ 会话读取追踪：记录会话中读取过的文件，支持变更检测统计。
4. ✅ 容量管理与淘汰：基于条目数上限的 LRU 淘汰策略。

**测试**：新增 3 个单元测试覆盖元数据命中/未命中、内容缓存、过期检测。

### Phase 11: TUI Vim 模式与主题系统 ✅ DONE
1. ✅ 添加可选的 Vim 键位模式（可通过 `/vim` 命令开关）。
2. ✅ 实现主题系统（dark/light/high-contrast），并保存用户偏好。
3. ✅ 支持输入模式状态标识（-- INSERT -- / -- NORMAL --）。

**测试**：新增 `theme.rs` 单元测试，所有渲染组件迁移到主题系统，`/vim` 命令响应正确。360 tests pass，clippy clean。

---

## P2 阶段 — 锦上添花（差异化与商业化）

### Phase 12: Voice 语音模式 ✅ DONE
替换当前 placeholder，接入本地 STT（Whisper）和 TTS（系统语音）。

**已完成**：
1. ✅ TTS 系统命令后端：`SystemTtsBackend` 支持 macOS (`say`)、Linux (`espeak`/`spd-say`)、Windows (PowerShell `System.Speech`)。
2. ✅ STT Whisper 后端：`WhisperSttBackend` 支持转写已有音频文件，支持录音 (`rec`/`ffmpeg`/`arecord`) 后转写。
3. ✅ `VoiceTool` 注册到 ToolRegistry，支持 `speak` / `status` / `transcribe` 三种 action。
4. ✅ `/voice` 命令显示 TTS/STT 可用性状态。

**测试**：新增 8 个单元测试覆盖平台检测、backend 默认构造、命令存在性检查。348 tests pass。

### Phase 13: Chrome / 浏览器集成 ✅ DONE
实现 `BrowserTool`，通过 Chrome DevTools Protocol (CDP) 与本地 Chrome 实例通信。

- 无需额外 crate，基于项目已有的 `reqwest` 和 `tokio-tungstenite` 自建轻量级 CDP 客户端
- 支持 5 种 action：`navigate` / `screenshot` / `get_page_content` / `find_elements` / `evaluate_js`
- 自动探测系统 Chrome/Chromium 路径（macOS/Linux/Windows），支持 `CHROME_PATH` 环境变量
- 每次调用启动独立 headless 实例，随机端口，调用结束自动关闭
- `screenshot` 支持 `full_page` 参数，返回 base64 Markdown 图片链接
- `find_elements` 通过 CSS selector 查找元素，返回文本列表
- 完整单元测试，363 tests pass，clippy clean

### Phase 14: 远程开发支持 ✅ DONE
实现 `RemoteEnvDetector` 和 `RemoteSessionManager`，支持远程环境检测与 SSH 会话管理。

- `RemoteEnvDetector` 自动检测 7 种环境类型：Local / SSH / Docker / WSL / Codespaces / GitPod / VS Code Remote
- 通过环境变量 + 系统文件（`/proc/1/cgroup`、`/proc/version`）多重检测，准确率高于单一指标
- `RemoteSessionManager` 支持 CRUD 操作：创建、列出、获取、删除、更新状态远程 SSH 会话
- 支持三种认证方式：SSH Agent、私钥文件、密码（不推荐）
- `build_ssh_command` 生成可直接执行的 SSH 命令
- `execute_remote` 异步执行远程命令并返回 stdout / stderr / exit code
- 会话持久化到 `~/.priority-agent/remote_sessions.json`
- `remote_dev` 工具已注册到 ToolRegistry，支持 6 种 action：`detect` / `list` / `create` / `remove` / `ssh` / `exec`
- 371 tests pass，clippy clean

### Phase 15: 团队协作基础 ✅ DONE
多 agent 之间的邮箱系统（`TeammateMailbox`）。

**已完成**：
1. ✅ `TeammateMailbox` 完整实现：支持点对点消息、广播、未读轮询、消息持久化（JSONL 追加写入）。
2. ✅ 消息模型：`MailboxMessage` 含 id/from/to/content/timestamp/priority/read/kind/reply_to 字段。
3. ✅ `TeamTool` 注册到 ToolRegistry，支持 `send` / `receive` / `poll` / `broadcast` / `mark_read` / `list` 6 种 action。
4. ✅ 未读统计：`UnreadSummary` 按发件人和优先级聚合。

**测试**：`src/team/mod.rs` 6 个单元测试 + `src/tools/team_tool.rs` 4 个单元测试全部通过。

### Phase 16: Telemetry 与性能追踪 ✅ DONE
在用户同意的前提下，收集工具调用耗时、成功率、崩溃日志。

**已完成**：
1. ✅ `TelemetryCollector`：跨会话持久化到 `~/.priority-agent/telemetry.json`，保留最近 100 条会话记录。
2. ✅ 用户同意控制：`PRIORITY_AGENT_TELEMETRY=enabled/disabled`，默认不收集。
3. ✅ `TelemetryTool` 注册到 ToolRegistry，支持 `status` / `summary` / `export` 三种 action。
4. ✅ `/telemetry` 命令显示同意状态和已记录会话数。

**测试**：新增 3 个单元测试覆盖同意解析、默认禁用、会话序列化。355 tests pass。

### Phase 17: 新用户 Onboarding ✅ DONE
第一次启动时的交互式引导流程。

**已完成**：
1. ✅ `OnboardingManager` 检测首次启动：`~/.priority-agent/.onboarded` 标志文件。
2. ✅ 五步交互式引导弹窗：Welcome → API Key Setup → Commands → Permissions → All Set!
3. ✅ 键盘导航：Enter/→ 下一步，← 上一步，Esc 跳过。
4. ✅ TUI 首次启动自动进入引导模式，非首次启动直接显示普通欢迎语。
5. ✅ `/onboarding` 命令可重新触发引导流程。
6. ✅ `/skip` 命令可在引导过程中跳过。

**测试**：新增 3 个单元测试覆盖步骤流转、首次启动检测、状态导航。358 tests pass。

### Phase 18: 会话分享 ✅ DONE
`/share` 命令将对话转换为可分享的 markdown 或 JSON。

**已完成**：
1. ✅ `/share` 命令：将当前 TUI 会话消息导出为 Markdown，写入 `~/.priority-agent/shared/session_<timestamp>.md`。
2. ✅ `ShareTool` 注册到 ToolRegistry，支持 `markdown` / `json` 导出格式，可指定输出路径。
3. ✅ 导出文件包含角色标签（User/Assistant/System/Tool）和消息内容。

**测试**：新增 2 个单元测试覆盖工具名称和文件名清理。355 tests pass。

---

## 当前进行中
> P0–P2 全部完成！项目已覆盖 Claude Code 核心能力（LSP、IDE、Worktree、Skills、Rich TUI、Bridge、Voice、Telemetry、Sharing、Onboarding）。
>
> 如需继续，可考虑：> - 插件生态完善（签名信任链、市场、升级治理）
> - MCP OAuth/审批产品化
> - Voice 模块从系统命令升级到 crate 原生（cpal + whisper-rs）
> - 可配置键位文件持久化到 config.toml
> - Auto-updater、桌面集成等生态体验
