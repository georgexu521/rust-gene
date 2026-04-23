# TUI 全面升级实施计划（方案 B）

> 目标：功能性 + 美观度双提升，工作量约 1-2 周

---

## 新增依赖

```toml
# 语法高亮
syntect = { version = "5.2", default-features = false, features = ["default-fancy"] }
# 中文字符宽度计算
unicode-width = "0.2"
# 剪贴板（消息复制）
arboard = { version = "3.4", default-features = false, features = ["wayland-data-control"] }
```

---

## Phase 1：基础修复（1-2 天）

### 1.1 修复消息高度估算
- **文件**：`src/tui/screens/main_screen.rs`
- **问题**：`estimate_message_height` 用 `content.len() / width`，中文双宽字符、emoji 会导致滚动错位
- **修复**：使用 `unicode-width` 精确计算每行实际显示宽度
- **验证**：添加包含中文和 emoji 的测试消息，确认滚动不错位

### 1.2 接入终端通知系统
- **文件**：`src/tui/notify/mod.rs`, `src/tui/app.rs`, `src/tui/mod.rs`
- **触发点**：
  - 流式响应完成 → `send_notification("Priority Agent", "Response ready")`
  - 工具执行完成（耗时较长时）→ `send_notification(...)`
  - Plan 审批等待时 → `send_notification(...)`
- **注意**：TUI 运行时 stdout 被 ratatui 接管，需要用备用通道或仅在 TUI 退出时发送

### 1.3 激活消息搜索
- **文件**：`src/tui/components/message_search.rs`, `src/tui/app.rs`, `src/tui/mod.rs`
- **实现**：
  - 新增 `AppMode::MessageSearch`
  - Vim Normal 模式下按 `/` 进入搜索
  - 输入关键词实时过滤消息
  - `↑/↓` 或 `n/N` 跳转到上/下一个匹配
  - `Esc` 退出搜索

---

## Phase 2：主题扩展 + 消息视觉区分（1 天）

### 2.1 添加 4 个流行主题
- **文件**：`src/tui/theme.rs`
- **新增主题**：
  - Nord（蓝灰色调， Arctic Ice 风格）
  - Dracula（紫粉色调，开发者最爱）
  - Gruvbox Dark（复古暖色调）
  - Catppuccin Mocha（现代柔和暗色）
- **实现**：`Theme::nord()`, `Theme::dracula()`, `Theme::gruvbox_dark()`, `Theme::catppuccin_mocha()`
- **设置界面**：更新下拉选项

### 2.2 消息添加颜色区分
- **文件**：`src/tui/components/message.rs`, `src/tui/screens/main_screen.rs`
- **设计**：
  - User 消息：左侧 2px 青色竖条 + 轻微背景色差异
  - Assistant 消息：左侧 2px 绿色竖条 + 轻微背景色差异
  - System/Tool 消息：保持当前样式或更弱化
- **实现方式**：在 `render_message` 返回的 Paragraph 外包裹一个带左边框的 Block

### 2.3 输入框视觉区分
- **文件**：`src/tui/screens/main_screen.rs`
- **设计**：输入区添加与聊天区不同的边框样式（active border 颜色）

---

## Phase 3：代码块语法高亮（1-2 天）

### 3.1 接入 syntect
- **文件**：`src/tui/components/markdown.rs`
- **实现**：
  - 在 `parse_markdown` 中识别 fenced code block 的语言标签
  - 用 syntect 的 `SyntaxSet` + `ThemeSet` 解析代码高亮
  - 将 syntect 的 color 转换为 ratatui 的 `Color::Rgb(r, g, b)`
  - 回退：语言不支持时保持当前绿色 dim 样式
- **性能考虑**：`SyntaxSet::load_defaults_newlines()` 在启动时预加载，避免运行时重复加载

### 3.2 主题配色适配
- 为每个主题选择合适的 syntect 主题（Dark→base16-ocean.dark, Light→base16-ocean.light）

---

## Phase 4：状态栏重构 + 空状态美化（1 天）

### 4.1 状态栏三分栏重构
- **文件**：`src/tui/screens/main_screen.rs`
- **设计**：
  ```
  [✓ Ready] | [worktree: main] [VIM] [FOCUS] | MiniMax-M2.7 | 12 msgs | /help
  ```
  - 左：状态图标 + 文字（spinner/ready/error）
  - 中：模式徽章（VIM/PAUSED/FOCUS/PLAN）+ 工作树信息
  - 右：提供商/模型 + 消息数 + 帮助提示
- **实现**：`Layout::horizontal([Constraint::Min(20), Constraint::Min(30), Constraint::Min(30)])`

### 4.2 空状态美化
- **文件**：`src/tui/screens/main_screen.rs`
- **设计**：
  - ASCII art logo（Priority Agent 字样）
  - 欢迎语
  - 快速操作提示（"Type a message to start" / "Press / for commands" / "Press ? for help"）

---

## Phase 5：消息折叠 + 日期分隔线（1 天）

### 5.1 消息折叠
- **文件**：`src/tui/components/message.rs`, `src/tui/app.rs`
- **实现**：
  - `MessageItem` 或 TuiApp 中添加 `collapsed_messages: HashSet<usize>`
  - 长消息（>10 行）在头部显示 `[+]` / `[-]` 折叠按钮
  - Vim Normal 模式下按 `Tab` 折叠/展开当前消息
  - 折叠时只显示前 3 行 + "... (N more lines)"

### 5.2 日期分隔线
- **文件**：`src/tui/screens/main_screen.rs`
- **实现**：
  - 渲染消息时检查相邻消息是否跨天
  - 跨天时插入居中分隔线：`─── 2026-04-23 ───`

---

## Phase 6：多会话侧边栏 + 流式打字机效果（2-3 天）

### 6.1 多会话侧边栏
- **文件**：`src/tui/screens/main_screen.rs`, `src/tui/app.rs`
- **设计**：
  - 左侧 20% 宽度窄栏显示最近会话列表
  - 会话标题（自动生成的标题或时间戳）
  - 当前会话高亮
  - 按 `b`（buffer）切换显示/隐藏侧边栏
  - 按 `↑/↓` 选择，`Enter` 切换会话
- **实现**：需要集成 `session_manager` 获取会话列表

### 6.2 流式打字机效果
- **文件**：`src/tui/app.rs`, `src/tui/screens/main_screen.rs`
- **设计**：
  - Assistant 消息在流式接收时逐字符/逐词渲染
  - 不是等完整响应后再一次性显示
  - 需要与现有的 `current_response` + `stream_done` 机制配合
- **挑战**：ratatui 的渲染是离散的（250ms tick），需要确保流式内容在 tick 间及时更新

---

## Phase 7：测试 + 整合 + 提交（1 天）

- 全量测试 `cargo test`
- 手动测试各功能
- 清理 dead code 标记
- 提交 commit

---

## 执行状态

| Phase | 内容 | 状态 |
|-------|------|------|
| Phase 1 | 基础修复（消息高度、通知接入、消息搜索） | ✅ 完成 |
| Phase 2 | 主题扩展 + 消息视觉区分 | ✅ 完成 |
| Phase 3 | 代码块语法高亮（syntect） | ✅ 完成 |
| Phase 4 | 状态栏重构 + 空状态美化 | ✅ 完成 |
| Phase 5 | 消息折叠 + 日期分隔线 | ✅ 完成 |
| Phase 6 | 多会话侧边栏 + 流式打字机效果 | ✅ 完成 |
| Phase 7 | 测试 + 整合 + 提交 | ✅ 完成 |

**总计**：7 个 commit，722 测试全部通过。
