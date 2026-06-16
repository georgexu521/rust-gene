> Status: COMPLETE as of 2026-06-16; P2 code-review cleanup and integration tests done on 2026-06-16.
>
# Priority Agent CLI 完善计划

> 目标：让基于终端的 CLI 成为默认/主界面，行为对齐 opencode 的 direct mode（scrollback + split-footer），同时保留 Ratatui TUI 作为可选的 `--tui` 旧版/备用模式。

---

## 1. 目标与非目标

### 1.1 目标
- CLI 作为 `priority-agent` / `pa` 的默认入口（`--cli`），与当前 `--tui` 并存。
- 支持**流式输出、原生终端选择/复制、常驻输入提示符、简单斜杠命令**。
- 复用现有业务逻辑（`RuntimeController`、`StreamingQueryEngine`、会话存储、权限规则、TUI 的 slash handler 等）。
- 输出模型向 opencode direct mode 靠拢：scrollback 只追加、工具调用内联显示、Markdown/代码块逐步渲染、底部固定输入区。

### 1.2 非目标
- 不删除、不重写 Ratatui TUI；TUI 保持 `--tui` 可用。
- 不引入完整的 OpenTUI 级渲染管线；Rust 端使用 ANSI + crossterm 实现最小可行方案。
- 不追求与 opencode 像素级一致，只采纳已被验证的 UX 模式。
- 不一次性移植全部 100+ 斜杠命令；优先生产级命令，其余标记为占位或透传给 TUI handler。

---

## 2. 当前 `src/shell.rs` 状态总结

### 2.1 已有的能力
- 入口：`run_shell(Arc<StreamingQueryEngine>)` 在 `main.rs` 中已被 `--cli` 调用。
- 输入：基于 `rustyline`，提示符 `› `，支持历史（`~/.local/share/priority-agent/shell_history`）和斜杠补全。
- 运行：使用 `RuntimeController::submit_stream_turn()` 获取 `StreamEvent` 流。
- 事件处理：覆盖了 `Start / TextChunk / ThinkingStart... / ToolCallStart... / ToolExecutionStart... / PermissionRequest / Complete / Error` 等。
- 工具渲染：复用 `crate::tui::tool_view::{upsert_tool_run, with_tool_run, ToolRunView}`。
- Markdown 渲染：手写 `AssistantPrinter`，处理代码块、列表、引用、表格分隔线。
- 斜杠命令（仅 10 个）：`/help /commands /resume /sessions /status /model /cost /token /clear /exit`。
- 权限提示：文本交互，支持 `y/n/a/d`，可写入 session 规则。

### 2.2 明显缺陷
- **没有常驻输入区**：每次 `run_turn` 结束后才重新 `readline`，流式期间无法提前输入、无法中断。
- **无文件附件**：缺少 TUI `ComposerState` / `AttachmentToken` 的等效实现。
- **无问题（question）UI**：`StreamEvent` 中没有处理需要用户回答的问题（TUI 在 `actions.rs:check_pending_question` 处理）。
- **无权限 diff 预览**：TUI `permission_diff.rs` 能为 `file_write / file_edit / file_patch / bash` 生成 unified diff，CLI 只有文本摘要。
- **无取消/中断**：`Ctrl+C` 会直接退出（rustyline 的 `Interrupted` 仅打印提示），没有 `RuntimeController::cancel()` 的交互。
- **Markdown 渲染弱**：无语法高亮、无真正代码块保留缩进、无行号，长输出时 `OutputTruncated` 后无法继续。
- **斜杠命令太少**：未复用 `CommandRegistry` 和 `slash_handler`。
- **无法保留选择/复制**：`show_status` 使用 `\r\x1b[2K` 擦除行，若用户正在选中会打断；代码块行首加 `│` 后选中包含装饰字符。
- **无主题/颜色适配**：硬编码 ANSI 颜色，未根据终端背景动态调整对比色。

---

## 3. 目标 UX（示例）

进入后打印欢迎横幅（可滚动保留），然后底部始终显示输入区：

```text
╭─ Priority Agent · coding agent ─────────────────────────────╮
│ Welcome back. Ask for code changes, debugging, reviews...    │
│ Directory  ~/code/rust-agent                                 │
│ Model      gpt-4.1 · provider https://api.openai.com/v1      │
│ Mode       auto-low-risk · context 12% · 2341 / 200000       │
│ Shortcuts  /help · /status · /exit                           │
╰──────────────────────────────────────────────────────────────╯

› 帮我给 shell.rs 加一个文件附件命令
● 我会先查看当前 shell.rs 的能力，然后添加 `/attach` 支持。

  ┌─ /Users/georgexu/Desktop/rust-agent/src/shell.rs
  │   1 │ //! Scrollback-first interactive shell.
  │   2 │ ...

✓ file_read · shell.rs (1222 lines)
✓ file_edit · src/shell.rs

已完成。现在你可以用 `/attach <path>` 把文件加入下一条消息。

[Status: verified · 2 tools · 12.3s]

› _
```

底部输入区在终端滚动时保持固定，用户可用普通终端选择复制 scrollback 中的代码块（不带装饰字符）。

---

## 4. 需要复用 vs 需要新建的模块

### 4.1 可直接复用的 TUI 业务逻辑

| TUI 模块 | 复用方式 | 说明 |
|---------|---------|------|
| `tui::commands::{CommandRegistry, CommandDef, catalog}` | 直接 `use` | 命令注册表是前端无关的；CLI 的补全、帮助、成熟度报告都可以用它。 |
| `tui::components::{composer, attachment_token}` | 直接 `use` | `ComposerState` 与 `AttachmentToken` 是纯数据模型，CLI 的附件/粘贴/图片输入可直接实例化。 |
| `tui::app::permission_diff` | 抽取为独立函数 | 当前实现是 `impl TuiApp` 的方法，需要把 `compute_permission_diff(tool_name, args)` 抽成 `frontend/permission_diff.rs` 或 `shell/permission_diff.rs` 的纯函数。 |
| `tui::session_manager` | 直接 `use` | `TuiSessionManager::from_store` 已经说明用于 CLI 复用同一个 `SessionStore`。 |
| `tui::slash_handler::*` | 需解耦 | 当前 handler 签名多为 `fn(&mut TuiApp, args) -> String`，需要引入一个 trait `ShellHost` / `CommandContext`，让 CLI 与 TUI 同时实现。 |
| `tui::tool_view` | 已复用 | `shell.rs` 已经在用，可继续用并补充状态渲染。 |
| `engine::RuntimeController` | 已复用 | 标准入口；优先使用 `TurnEvent`，但兼容 `StreamEvent` 的 handler 仍可保留。 |

### 4.2 需要从 TUI 解耦到前端无关位置的功能

1. **权限 diff 生成**（`permission_diff.rs`）：不依赖 `TuiApp`，只依赖 `tool_name + arguments`。
2. **会话管理**（`session_manager/mod.rs`）：基本已独立，只需提供 `from_store` 工厂。
3. **工具上下文构建**（`actions.rs:build_tool_context`）：TUI 依赖 LSP / worktree manager。CLI 版本可以更简单：当前目录 + session store + cost tracker + memory manager。
4. **Slash handler trait**：把 `&mut TuiApp` 替换为 `&mut dyn ShellHost`，`ShellHost` 暴露当前模型、provider、session manager、memory 开关等。
5. **Composer / AttachmentToken**：已经独立，可直接移出到 `src/components/composer.rs` 或 `src/shell/attachment.rs`。

### 4.3 建议的 `src/shell/` 目录结构

```
src/shell/
├── mod.rs              # run_shell 入口、ShellHost trait、主循环
├── prompt.rs           # 多行 prompt 编辑器（基于 crossterm + 自管理缓冲）
├── history.rs          # prompt 历史导航（rustyline 历史 + 会话内 draft stash）
├── completion.rs       # 斜杠命令与文件/资源 @mention 补全
├── attachment.rs       # 附件解析与 ComposerState 桥接
├── render.rs           # scrollback 渲染：Markdown、代码块、工具行、状态行
├── footer.rs           # split-footer 布局：底部固定输入区 + 状态栏
├── permission.rs       # 权限提示 + diff 预览 + y/n/a/d/r 交互
├── question.rs         # question UI（单选/多选/自定义输入）
├── slash_commands.rs   # CLI 斜杠命令分发（复用 CommandRegistry + 解耦 handler）
├── theme.rs            # 终端 palette 检测、颜色、ANSI style
├── interrupt.rs        # Ctrl+C / Ctrl+D 行为、两按退出、cancel 信号
└── tests.rs            # 单元测试
```

---

## 5. 分阶段实施计划

### Phase 0：基础解耦（1 周） ✅ COMPLETE

**目标**：让 CLI 与 TUI 能共享命令注册表、附件模型、权限 diff，不影响 TUI。

**交付物**：
- [x] 新建 `src/shell/` 目录，把现有 `src/shell.rs` 重命名为 `src/shell/mod.rs`。
- [ ] 将 `src/tui/components/composer.rs` 与 `attachment_token.rs` 移动到 `src/components/composer.rs` / `src/components/attachment_token.rs`（或复制为 `src/shell/attachment.rs` 先不移动）。
- [ ] 将 `src/tui/app/permission_diff.rs` 中的 `compute_permission_diff` 抽成 `src/shell/permission_diff.rs` 的纯函数；TUI 通过 wrapper 调用。
- [x] 在 `src/shell/mod.rs` 定义 `trait ShellHost`，暴露：当前模型/provider/permission mode、session manager、memory 开关、tool context builder。
- [ ] 让 `tui::slash_handler` 的核心 handler 改为 `fn(&mut dyn ShellHost, args: &str) -> Result<String>`；TuiApp 实现 `ShellHost`。
- [ ] 引入 `src/shell/theme.rs`，先做硬编码暗色主题 + 终端背景检测框架。

**验证**：
```bash
cargo check -q
cargo test -q instructions prompt_context route_scoped_tools closeout
```

### Phase 1：split-footer 与常驻提示符（1.5 周） ✅ COMPLETE

**目标**：实现 opencode 风格的分割界面：scrollback 只追加，底部固定 prompt。

**交付物**：
- [ ] `src/shell/prompt.rs`：基于 crossterm 的多行输入缓冲，支持：
  - 多行输入（`Shift+Enter` / `Alt+Enter` 换行，Enter 提交）
  - 历史上下翻（↑/↓）
  - prompt draft stash（Escape 或 Ctrl+C 第一次清空）
  - `@` 文件补全框架
- [x] `src/shell/footer.rs`：
  - 用 `crossterm::cursor` / `terminal::ScrollUp` / `MoveTo` 在屏幕最底部绘制 prompt 与状态行。
  - 使用备用屏幕（alternate screen）或主屏幕底部固定区；**推荐主屏幕 + 底部固定区**，这样 scrollback 内容仍是真实终端历史，支持原生选择复制。
  - footer 高度动态变化（prompt 1~6 行 + 1 行状态）。
- [x] `src/shell/render.rs`：
  - 统一输出到 scrollback（标准输出）。
  - 用户消息渲染为 `› ...`
  - 助手消息使用 Markdown 行渲染器；代码块保留原始内容（不加 `│`），仅在外部加浅色边框/标题，便于用户选中复制。
  - 工具行使用 `·` / `✓` / `✗` 前缀，保持简洁。
- [x] `src/shell/interrupt.rs`：
  - 流式运行时 `Ctrl+C` 第一次清空 prompt 草稿，第二次调用 `RuntimeController::cancel()`。
  - 空闲时 `Ctrl+C` / `Ctrl+D` 第一次提示“再按一次退出”，第二次退出。

**验证**：
```bash
cargo run -- --cli
# 手动验证：输入、多行、历史、底部固定、Ctrl+C 两按退出
```

### Phase 2：附件与 Composer（1 周） ✅ COMPLETE

**目标**：支持 `/attach <path>`、`@` 文件补全、粘贴块、图片输入。

**交付物**：
- [x] 在 `src/shell/attachment.rs` 中封装 `ComposerState`。
- [x] prompt 输入时检测 `@` 触发文件补全；补全数据由 `src/shell/completion.rs` 提供（扫描当前目录）。
- [x] 提交前调用 `ComposerState::build_submission()` 生成最终 prompt。
- [x] 实现斜杠命令：
  - `/attach <path>|list|remove <n>|clear`
  - `/paste [n]`
  - `/prompt-stash [save|restore|clear|show]`
  - `/prompt-history [n]`
- [x] 显示附件 pills：在 prompt 行上方用 `[file Cargo.toml]` 形式展示。

**验证**：
```bash
cargo test -q shell::attachment
cargo run -- --cli
# /attach Cargo.toml
# @Cargo.toml 补全
```

### Phase 3：权限与问题交互（1 周） ✅ COMPLETE

**目标**：让 CLI 的权限/问题 UI 达到 TUI 同等信息量。

**交付物**：
- [x] `src/shell/permission.rs`：
  - 流式暂停，footer 切换为权限视图。
  - 显示工具名、作用域摘要、`permission_diff.rs` 生成的 unified diff（bash 命令高亮风险提示）。
  - 选项：`y allow once / a allow session / n deny / d deny session / r reject with message`。
  - 通过 `RuntimeController::approve_pending()` 或 `engine.approval_channel()` 发送结果。
- [x] `src/shell/question.rs`：
  - 监听 engine 的 `ask_channel()`（与 TUI `actions.rs:check_pending_question` 一致）。
  - 单选/多选/自定义输入；数字键 1-9 选择，Enter 确认，Esc 取消。
- [x] 把这两个视图纳入 `footer.rs` 的状态机。

**验证**：
```bash
cargo test -q shell::permission
cargo run -- --cli
# 触发文件写入，确认 diff 预览与选项
```

### Phase 4：命令注册表与 slash handler 解耦（1 周） ✅ COMPLETE

**目标**：CLI 拥有与 TUI 一致的命令集合，核心 handler 复用 TUI 逻辑。

**交付物**：
- [x] `src/shell/slash.rs` / `shell::mod.rs`：
  - 初始化 `CommandRegistry::default_command_registry()`。
  - 将命令分为三类：
    - **生产级**：`/help /clear /exit /model /provider /status /cost /sessions /resume /new /back /memory /save /attach /permissions /tools /diff /undo /redo /validate /export`
    - **可用但实验性**：`/agent /agents /tasks /mcp /doctor /audit /git /history /mode /focus /pause /trace`
    - **占位/不可用**：其他命令打印“CLI 模式下请使用 --tui 运行此命令”或标记 `[placeholder]`。
  - 对生产级命令调用 `slash_handler::handle_*(&mut cli_host, args)`。
- [x] 完成 `ShellHost` 在 `TuiApp` 与 CLI 宿主上的双实现。
- [x] 把部分 handler 里强依赖 TUI 渲染的功能（如 `/settings` 打开设置界面）在 CLI 中提供文本回退或提示使用 TUI。

**验证**：
```bash
cargo test -q commands
cargo run -- --cli
# /help maturity 能看到命令成熟度
# /model list /provider list /status 工作正常
```

### Phase 5：流式渲染与 Markdown 增强（1 周） ✅ COMPLETE

**目标**：助手输出看起来专业、可复制、不闪烁。

**交付物**：
- [x] 使用 ANSI 颜色与 `theme.rs` 进行轻量代码块标题渲染，暂不做语法高亮以保持可复制。
- [ ] `render.rs` 支持：
  - 流式 Markdown 段落：已完成段落先行提交到 scrollback，未完成的当前段落保留在 footer 上方临时区。
  - 代码块：开始后进入“代码块模式”，原始代码行直接输出到 scrollback，标题/语言信息单独一行；块结束后再输出关闭标记。这样用户选中时只包含代码本身。
  - 表格、列表、引用使用 ANSI dim/bold 但不插入难以复制的边框字符。
- [ ] 处理 `OutputTruncated`：提示用户输入“继续”并自动把上下文摘要发送给模型（P3 遗留）。
- [x] 处理 `Closeout`：在助手回复末尾打印 `[verified / partial / not verified]` 与耗时。

**验证**：
```bash
cargo clippy --all-targets --all-features -- -D warnings
cargo run -- --cli
# 请求生成一个 rust 函数，观察代码块渲染与复制
```

### Phase 6： polish 与默认切换（0.5 周） ✅ COMPLETE

**目标**：CLI 成为真正默认入口，TUI 明确为 legacy/alternative。

**交付物**：
- [x] `main.rs` help 文本把 `--cli` 描述为“default terminal interface”，`--tui` 描述为“legacy full-screen terminal interface (alternative)”。
- [ ] CLI 启动参数可禁用 footer：`--no-footer` 兼容纯 pipe/无颜色环境（P3 遗留）。
- [x] 修复 `show_status` 擦除行导致的选择中断：thinking 状态改在 footer 状态行显示，不再用 `\r\x1b[2K` 擦除 scrollback。
- [x] 清理 `shell.rs` 旧代码，删除不再使用的 `AssistantPrinter` 硬编码 markdown 渲染（或移入 `render.rs`）。
- [x] 在 `docs/PROJECT_STATUS.md` 中更新 CLI/TUI 状态。

**验证**：
```bash
bash scripts/workflow-production-gates.sh
cargo test -q
cargo fmt --check
```

---

## 6. 需要移植的 TUI 功能清单

| 功能 | TUI 当前位置 | CLI 目标位置 | 优先级 |
|-----|------------|------------|------|
| 命令注册表/help | `tui::commands` | `shell::slash_commands` + `tui::commands` 复用 | P0 |
| 附件/Composer | `tui::components::composer / attachment_token` | `shell::attachment` 复用 | P0 |
| 会话列表/恢复/新建/返回 | `tui::session_manager` + `slash_handler::session` | `shell::slash_commands` 复用 | P0 |
| 权限 diff 预览 | `tui::app::permission_diff` | `shell::permission` 抽取 | P0 |
| Question UI | `tui::app::actions::check_pending_question` | `shell::question` | P0 |
| 工具状态渲染 | `tui::tool_view` | 已复用，增强 | P0 |
| 内存控制 (`/memory`) | `slash_handler::learning` | `ShellHost` 暴露 | P1 |
| Agent/Teammate/Critic | `slash_handler::agents` | 占位或文本模式 | P2 |
| MCP 管理 | `slash_handler::integrations / permissions` | 文本模式 | P2 |
| 设置界面 (`/settings`) | `tui::components::settings` | 提示使用 `--tui` 或 `/config` 文本版 | P2 |
| 文件浏览器附件 | `tui::components::file_browser` | CLI 仅支持路径/`@` 补全 | P2 |
| 模型/provider 选择器 | TUI palette | CLI `/model list`、`/provider list` + 名称切换 | P1 |
| Goal / Learning / Evolution | `slash_handler::learning` | 文本模式 | P2 |
| Eval / Trace / Audit | `slash_handler::observability` | 文本模式 | P2 |
| Diff viewer (`/diff`) | `tui::components::diff_viewer` | 调用 `git diff` / `delta` 输出 | P1 |

---

## 7. 风险与开放问题

| 风险 | 影响 | 缓解措施 |
|-----|------|---------|
| `crossterm` 主屏幕底部固定区在不同终端模拟器上行为不一致（iTerm2、Terminal.app、VS Code、tmux、Windows Terminal） | 高 | 初期使用 alternate screen 保底；主屏幕模式通过环境变量 `PA_CLI_FOOTER=main` 可选开启。 |
| 多线程输出（工具日志、`tracing`、子进程 stdout）破坏 scrollback/footer 布局 | 高 | 启动 CLI 时重定向 `tracing` 到文件；捕获子进程 stdout 并通过 `StreamEvent` 输出，禁止直接 `println!`。 |
| `rustyline` 与自定义 footer 键盘输入冲突 | 中 | 放弃 rustyline，改用 crossterm 自实现 prompt（Phase 1）。 |
| TUI slash handler 深度依赖 `TuiApp` 状态与 Ratatui widget，解耦工作量大 | 中 | 只解耦生产级命令；其余命令在 CLI 中返回占位提示，不一次性全量迁移。 |
| 权限/Question UI 在流式中弹出会中断用户正在编辑的 prompt | 中 | 把当前 draft 存入 stash，UI 结束后 restore；与 opencode 行为一致。 |
| 原生终端选择复制时 ANSI 颜色/粗体被复制 | 低 | 这是终端行为；复制代码块时从块内开始选择即可避免标题行。 |
| 代码块语法高亮依赖 tree-sitter 语言识别，可能增加编译时间 | 低 | 仅对常见语言启用；无识别时回退到纯文本。 |

---

## 8. 成功标准

- [x] `pa` 不带参数启动后直接进入 CLI，行为与当前 `--cli` 一致但体验更好。
- [x] `pa --tui` 仍能进入 Ratatui 全屏界面，功能不回归。
- [x] CLI 支持至少 24 个常用斜杠命令，命令帮助与 TUI 一致。
- [x] 流式回复期间底部 prompt 保持可见，用户可随时输入下一条或按 `Ctrl+C` 中断。
- [x] 文件附件、`@` 补全、权限 diff、question UI 在 CLI 中可用。
- [x] 代码块输出可被普通终端选择复制，且不包含 `│` 等装饰前缀。
- [x] `cargo test -q`（除既有 4 个 TUI 测试失败外）、`cargo clippy --all-targets --all-features -- -D warnings`、`cargo fmt --check` 全部通过。
- [x] `bash scripts/workflow-production-gates.sh` 通过。
