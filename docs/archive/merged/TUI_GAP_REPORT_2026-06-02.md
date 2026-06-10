# TUI Gap Report: Priority Agent vs Reasonix

Date: 2026-06-02

对比 Priority Agent 当前 TUI（本次改造后）与 Reasonix 源码 (`src/cli/ui/`)。
差距按严重程度分 P0/P1/P2 三档。

---

## P0 — 影响日常使用体验的关键缺失

### 1. 虚拟滚动 (CardStream)

**Reasonix**: 完整的虚拟滚动系统。`CardStream` 通过 `useBoxMetrics` 测量每张 card 高度，
只渲染视口 ±30 行 buffer 内的 cards，其余以 spacer 替代。滚动指示器显示 "N lines above · M remaining"。
`StaticCardStream` 用 Ink `<Static>` 将已 settled 的 cards 冻结为不可变输出，避免重复渲染。

**PA 当前**: 简单 `scroll_offset` + 全量遍历消息。`transcript_window()` 做了一些截断但远不及虚拟滚动。
长对话时每帧重绘所有可见消息，性能随消息数线性下降。

**建议**: 核心架构改进。ratatui 没有 `useBoxMetrics`，需要自己实现高度估算和视口裁剪。

### 2. 流式输出实时反馈

**Reasonix**: `StreamingCard` 在流式输出时显示：
- 实时 token 速率（t/s），通过 `estimateLiveTokenCount` + 时间戳计算
- 默认预览最后 4 行，`Ctrl+O` 展开到 60 行
- 脉冲动画圆圈 `●`（writing 状态）
- 完成后切换到 `‹` glyph + 总计 token 数 pill

**PA 当前**: 流式输出时消息区域实时更新文本，但没有：
- token 速率显示
- 预览/展开切换
- 脉冲动画
- 完成后的 token 统计

**建议**: 需 streaming engine 侧暴露 token 事件给 TUI。

### 3. Reasoning / Thinking 显示

**Reasonix**: `ReasoningCard` 独立渲染模型思考过程。3 行 streaming 预览，
settled 后显示 head+tail，>800 token 折叠为 "scroll-past summary"。
`◆` 脉冲菱形 + token 数 + 段落数 + 耗时。

**PA 当前**: 无 reasoning 显示。思考过程和回复混在一起，或根本不可见。

**建议**: 如果使用的模型支持 reasoning tokens，需要 engine 侧分离并传递给 TUI。

---

## P1 — 产品深度差距

### 4. Tool Card 分区渲染

**Reasonix**: `ToolCard` 渲染：
- 文件编辑工具：显示文件路径、行范围、add/del 计数、内联 diff
- Bash 工具：exit code、elapsed、尾部输出（2 行读工具 / 5 行其他）
- Read 工具：搜索匹配、输入字节数
- 状态指示：运行中 `▣` 脉冲、完成 ✓、失败 ✗、中止 ⊘
- 折叠/展开长输出

**PA 当前**: Tool 消息是单行 `▣ Tool` header + 纯文本内容。没有按工具类型区分的渲染。

**建议**: 引入 `ToolCard` 渲染，根据 tool name 区分显示格式。

### 5. 实时活动区 (LiveRows)

**Reasonix**: `LiveRows` 渲染当前正在进行的活动：
- `ThinkingRow`：脉冲圆圈 + token 速率
- `ModeStatusBar`：编辑模式 pill（auto/review/plan）
- `OngoingToolRow`：脉冲方块 + 进度条 + args 摘要
- `SubagentLiveStack`：1→显示详情，2+→compact 列表，>max→折叠
- `Countdown` 倒计时

**PA 当前**: 全靠 spinner 帧动画 + "Thinking..." 文本。没有结构化的实时活动卡片。

**建议**: 在 status bar 上方增加一行活动区，显示当前 tool 运行状态。

### 6. Toast / 通知系统

**Reasonix**: `ToastRail` 自动消失的通知条。每种 toast 有 glyph (✓/ⓘ/⚠/✗)、title、detail、
TTL 自动消失、<1/3 TTL 时颜色渐隐。

**PA 当前**: 无 toast 系统。系统消息以永久消息形式出现在对话中。

**建议**: 在 TUI 底部或顶部增加 toast rail，用于非关键的瞬态通知。

### 7. Context Usage 可视化

**Reasonix**: `CtxCard` 显示上下文窗口使用情况：32 格柱状图（system/tools/log/input），used/cap 比例。
`StatusRow` 中也有 8 格上下文 bar + 百分比，>=80% 红、>=50% 黄。

**PA 当前**: 无上下文使用可视化。

**建议**: 在 status bar 增加 ctx usage bar（已在 plan 中提到）。

### 8. BootSplash 启动画面

**Reasonix**: `BootSplash` 启动时显示：logo、版本、模型信息、API 余额、session 信息。
带旋转进度指示器。

**PA 当前**: 有 `render_onboarding` 引导弹窗，但非启动画面。启动直接进聊天界面。

**建议**: 增加启动画面，显示关键状态信息。

### 9. Session Intro

**Reasonix**: `SessionIntro` 在每个 session 顶部渲染：
`◈ {session id} · {branch} · {workspace} · {model}` 的 faint 分隔线。

**PA 当前**: 无 session intro。

**建议**: 在对话顶部添加 session 信息行。

### 10. Plan Card 结构化显示

**Reasonix**: `PlanCard` 渲染计划进度：5 步窗口，每步有状态图标
(queued ○ / running ● / done ✓ / failed ✗ / blocked ⚠ / skipped ·)。
running 步用脉冲 `◆`。显示进度计数和 variant tag。

**PA 当前**: `render_plan_approval` 是简单的段落渲染，无步骤状态追踪。

**建议**: 将 plan 渲染改为结构化步骤卡片。

### 11. Memory / Context Card

**Reasonix**: `MemoryCard` 按类别（user/feedback/project/reference）分组显示记忆条目，
每类最多 5 条，其余折叠。显示 token 数。

**PA 当前**: 无 memory 可视化。

**建议**: 当 memory retrieval 活跃时，用 card 展示加载的上下文条目。

### 12. Usage Card（Token 消耗分解）

**Reasonix**: `UsageCard` 每次 turn 后显示 token 消耗柱状图：
prompt/reason/output/cache 分段，cost + elapsed。有全宽/紧凑两种布局。

**PA 当前**: 仅在 status bar 中以文本形式显示 token 数。

**建议**: 每次 turn 后在消息流末尾插入 Usage card。

---

## P2 — 打磨和锦上添花

### 13. 多主题运行时切换

**Reasonix**: 8 个命名主题（graphite/ember/aurora/sandstone/porcelain/linen/glacier/midnight），
通过 Proxy 实现零开销运行时切换。用户在 settings 或 `/theme` 切换。

**PA 当前**: 编译时选主题。无运行时切换 UI。

**建议**: 在 settings 面板或 slash command 中增加主题切换。

### 14. Model Picker UI

**Reasonix**: `ModelPicker` 带搜索过滤的模型选择器。显示模型名、badge (flash/pro/r1)、
描述。支持键盘导航。

**PA 当前**: `render_model_select` 是简单的选项列表，无描述、无搜索。

**建议**: 增加模型描述和搜索过滤。

### 15. Edit Confirm / Edit Picker

**Reasonix**: 编辑前确认 UI，显示 diff preview 和 apply/skip/reject 选项。
`EditPicker` 在多个编辑候选之间选择。

**PA 当前**: 有 PermissionApproval 和 PlanApproval 弹窗，但无编辑确认。

**建议**: 在 file_edit 执行前展示 diff 预览和确认。

### 16. Search Card（搜索结果高亮）

**Reasonix**: `SearchCard` 按文件分组显示 grep 结果，匹配文本高亮。≤10 条预览，其余折叠。

**PA 当前**: grep 结果以普通 tool 消息显示，无结构化渲染。

**建议**: 为 grep/glob 结果增加结构化渲染。

### 17. SubAgent Card（子代理状态树）

**Reasonix**: `SubAgentCard` 树形渲染子代理的 cards。
`⌬` 脉冲六边形 + 状态 + 运行中 child 计数。每个 child 有独立状态行。

**PA 当前**: agent 结果以普通消息显示，无树形结构。

**建议**: 为 agent 执行增加结构化渲染。

### 18. Tip Card（快捷提示）

**Reasonix**: `TipCard` 在对话开头插入一次性键盘快捷键提示，按 section 分组。
`ⓘ` glyph + topic + "shown once" tag。

**PA 当前**: 有 `render_shortcut_help` 弹出面板，但无内联 tip。

**建议**: 在新 session 开头插入内联快捷键提示。

### 19. 滚动时自动贴底/解锁

**Reasonix**: 用户向上滚动 → unpin（不再自动滚到底）。用户滚到底或新 card 出现 → pin。
滚动通过 16ms coalescing window 批量处理。

**PA 当前**: 有 `scroll_to_bottom()` 和 `scroll_offset`，但无 pin/unpin 语义。

**建议**: 增加自动贴底和手动滚动的切换逻辑。

### 20. Error Recovery 显示

**Reasonix**: `ErrorCard` 显示错误消息 + 最后 5 行 stack trace。
有 retry count badge。

**PA 当前**: 错误以红色文本在输入区显示。

**建议**: 将严重错误渲染为独立的 error card。

---

## PA 已有但可继续打磨的功能

| 功能 | PA 当前 | 改进方向 |
|------|---------|---------|
| Diff Viewer | `render_diff_viewer` 全屏弹窗 | 内联 diff card（像 Reasonix DiffCard） |
| Command Palette | `render_command_palette` 模糊搜索 | 分类显示、快捷键标注 |
| Sidebar | `render_sidebar` 文件树 | 集成 context/cache/MCP 信息 |
| Settings | `render_settings` 键值编辑 | 分类导航、搜索 |
| Tool Viewer | `render_tool_viewer` 查看 tool 输出 | 分区渲染（reason/output/shell）像 Reasonix pill sections |
| Message Search | `render_message_search` | 高亮匹配文本 |

---

## 汇总

| 优先级 | 数量 | 主要缺口 |
|--------|------|---------|
| P0 | 3 | 虚拟滚动、流式反馈、Reasoning 显示 |
| P1 | 9 | Tool card 分区、LiveRows、Toast、Ctx bar、BootSplash、SessionIntro、Plan card、Memory card、Usage card |
| P2 | 8 | 运行时主题切换、Model picker、Edit confirm、Search card、SubAgent card、Tip card、Scroll pin/unpin、Error card |
| 已有可打磨 | 6 | Diff、Command palette、Sidebar、Settings、Tool viewer、Message search |
