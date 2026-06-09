# TUI 体验优化计划

> 2026-06-09 — 基于 opencode-dev 源码的 TUI 层深度对比
> 聚焦 5 个可独立交付的改进方向，按用户感知价值排序

---

## 0. 总体判断

Priority-Agent 的 TUI 在功能深度上并不输 opencode——14 种模式、9 套主题、Vim 模式、tool 视图分类、125+ 命令面板都是实实在在的。
差距主要在**交互流畅度**和**信息可见性**上：对话框不能叠加、侧边栏是静态文本而不是实时面板、diff 查看器缺少语法高亮和行号、会话列表不能 pin/重命名。

好消息是：这些问题都是**增量式的**，不需要重构架构。每个方向都可以独立交付。

---

## 1. Diff 查看器升级

**用户感知**: ★★★★★（每次 code review 都用）
**工作量**: ~300 行 Rust

### 现状 vs 目标

| 功能 | 现状 | 目标 |
|------|:--:|:--:|
| 语法高亮 | ❌ 只有 +/- 颜色 | ✅ syntect 语法高亮 |
| 行号显示 | ❌ | ✅ 新旧行号 |
| 侧边模式 | ❌ 只有 unified | ✅ unified |
| Hunk 导航 | ❌ 逐行滚动 | ✅ `n`/`p` 跳转 hunk |
| 多文件 diff | ❌ 单文件 | ✅ Tab 切换文件 |
| Git 集成 | ❌ 只接收字符串 | ✅ 直接调用 `git diff` |
| 滚动位置 | ❌ 无指示 | ✅ "Line 42/150" 或滚动条 |

### 实现方案

```rust
// src/tui/components/diff_viewer.rs 增强

// 1. 旧/新行号计算
struct DiffLine {
    text: String,
    kind: DiffLineKind, // Add, Remove, Context, Header
    old_line: Option<usize>,
    new_line: Option<usize>,
}

// 2. Hunk 边界检测
struct DiffHunk {
    header: String,
    old_start: usize,
    new_start: usize,
    lines: Vec<DiffLine>,
}

// 3. syntect 语法高亮
fn highlight_diff_line(line: &str, language: &str, theme: &Theme) -> Vec<Span> {
    let syntax = SyntaxSet::load_defaults_newlines();
    let syntax = syntax.find_syntax_by_extension(language).unwrap();
    // ...
}

// 4. Hunk 导航
// n: 跳到下一个 @@ header
// p: 跳到上一个 @@ header
// 搜索下一个以 @@ 开头的行
```

### 关键改动文件
- `src/tui/components/diff_viewer.rs` — 主要增强
- `src/tui/theme.rs` — 添加 diff 专用主题色（已有部分）
- `src/tui/screens/main_screen.rs` — 渲染 diff 模式时传入文件语言

---

## 2. 会话选择器增强

**用户感知**: ★★★★★（每天切换会话用）
**工作量**: ~200 行 Rust

### 现状 vs 目标

| 功能 | 现状 | 目标 |
|------|:--:|:--:|
| Pin 会话 | ❌ | ✅ `P` 固定到列表顶部 |
| 内联重命名 | ❌ | ✅ `R` 在侧边栏中重命名 |
| 侧边栏删除 | ❌ | ✅ `D` 删除（二次确认） |
| 侧边栏切换 | ❌ | ✅ `Enter` 直接切换 |
| 搜索筛选 | `/resume` 命令 | ✅ 侧边栏内实时筛选 |
| 元数据显示 | 只有 title | ✅ model + msg count + 时间 |
| 预览卡片 | ❌ | ✅ `Tab` 展开预览 |

### 实现方案

```rust
// src/tui/session_manager.rs 增强

impl TuiSessionManager {
    // 已有方法，需要增强
    fn pinned_sessions(&self) -> Vec<&SessionInfo> { ... }
    fn toggle_pin(&mut self, session_id: &str) { ... }
    fn rename_session(&mut self, session_id: &str, new_title: &str) { ... }
}

// src/tui/screens/main_screen.rs render_sidebar 增强
fn render_sidebar(f, app, area) {
    // 搜索栏（输入 / 开始筛选）
    // 已固定区域（Pinned）
    // 今日区域（Today）
    // 更早区域（日期分组）
    // 每个条目：title | model | msg_count | time
    // 快捷键提示：Enter=switch R=rename D=delete P=pin
}
```

### 关键改动文件
- `src/tui/session_manager.rs` — 添加 pin/toggle/rename 方法
- `src/tui/screens/main_screen.rs` — 重写 render_sidebar
- `src/tui/slash_handler/session.rs` — 添加 `/session pin` `/session rename` 子命令
- `src/tui/commands/catalog.rs` — 注册新命令

---

## 3. 权限面板内联 Diff

**用户感知**: ★★★★（每次审批文件修改时用）
**工作量**: ~150 行 Rust

### 现状 vs 目标

| 功能 | 现状 | 目标 |
|------|:--:|:--:|
| 内联 diff | ❌ `d` 键切换到 DiffViewer 模式 | ✅ 审批面板内直接显示 diff |
| 风险分解 | ❌ 只在 panel 文本中 | ✅ 审批面板底部显示 risk facts |
| 规则匹配原因 | ❌ | ✅ 显示匹配的规则 + 来源 |
| 作用域可视化 | ❌ 平铺字母选项 | ✅ 分组：一次性 / 会话级 / 项目级 / 全局 |

### 实现方案

在 `render_permission_approval` 中：
1. 如果 `has_diff_preview`，在审批面板下半部分嵌入 diff 预览（截断到 10-15 行）
2. Diff 使用现有 diff_viewer 的渲染逻辑（+/-/@@ 颜色）
3. 底部显示 risk_facts（如 "修改文件: src/main.rs", "在工作区外: /etc/hosts"）
4. 选项分组渲染

### 关键改动文件
- `src/tui/screens/main_screen/approvals.rs` — 主要增强
- `src/engine/human_review.rs` — 增强 `permission_review_data` 返回更多上下文

---

## 4. 命令面板增强

**用户感知**: ★★★★（高频使用）
**工作量**: ~100 行 Rust

### 现状 vs 目标

| 功能 | 现状 | 目标 |
|------|:--:|:--:|
| 匹配高亮 | ❌ | ✅ 匹配字符用 accent 色 |
| 最近使用 | ✅ 8 条 | ✅ 显示 "Recently Used" 分区 |
| 无结果建议 | ❌ | ✅ "Did you mean...?" |
| model/provider 集成 | ❌ 独立弹窗 | ✅ 在面板中切换 |

### 实现方案

在 `render_command_palette` 中：
1. 匹配字符高亮：遍历搜索结果，对匹配位置的字符用 `theme.accent` 渲染
2. 最近使用分区：当输入为空时，显示 "Recently Used" 分组（已有数据，只需 UI）
3. 无结果时计算编辑距离，推荐最接近的命令

### 关键改动文件
- `src/tui/app/palette.rs` — 增强渲染逻辑
- `src/tui/screens/main_screen/popups.rs` — 命令面板 UI

---

## 5. 快捷键可发现性（Which-Key）

**用户感知**: ★★★（降低学习成本）
**工作量**: ~80 行 Rust

### 实现方案

最简单的方案：增强现有的 `ShortcutHelp` 弹窗。

1. **搜索筛选**：在 ShortcutHelp 模式下，输入 `/` 可以筛选快捷键
2. **上下文感知**：根据当前 AppMode 过滤显示的快捷键
   - Chat 模式：显示消息/导航/输入快捷键
   - Approval 模式：显示审批快捷键
   - Diff 模式：显示 diff 导航快捷键
3. **分组表头**：用加粗/颜色区分不同分组

### 关键改动文件
- `src/tui/screens/main_screen/popups.rs` — ShortcutHelp 增强
- `src/tui/keybindings.rs` — 添加按模式过滤的方法

---

## 实施顺序建议

| 顺序 | 方向 | 理由 |
|:---:|------|------|
| 1 | **Diff 查看器** | 用户感知最高，每次 code review 都用，独立性强 |
| 2 | **会话选择器** | 日常高频操作，pin/rename 是基础功能 |
| 3 | **权限面板内联 diff** | 审批体验的直接提升 |
| 4 | **命令面板高亮** | 快速优化，改动量小 |
| 5 | **ShortcutHelp 搜索** | 降低学习成本 |

---

## 暂不纳入的方向

| 方向 | 原因 |
|------|------|
| 对话框 Stack（多弹窗叠加） | 需要重构 AppMode 架构，工程量大，边际收益低 |
| Sidebar 实时面板（持久侧边栏） | Ratatui 没有原生 sidebar 组件，需要大量手写布局逻辑 |
| Onboarding 交互化 | 更依赖产品定义而非代码，需要先明确交互流程 |
| 国际化 | 产品方向是个人工具，暂不需要 |
| 动画/过渡效果 | Ratatui 限制，投入产出比低 |

---

> 文档版本: v1.0, 2026-06-09
> 基于: opencode-dev TUI (SolidJS) vs priority-agent TUI (Ratatui) 深度对比
