# TUI 深度优化计划（第二轮）

> 2026-06-09 — 基于 opencode 对比后的架构/产品/体验三类差距
> 在 Ratatui 框架约束下，逐项补齐到生产级

---

## 0. 当前基线

- cargo test --lib: 2443 passed, 0 failed
- cargo clippy --all-features -- -D warnings: clean
- 第一轮 TUI 优化（5 Phase）已交付，核心交互功能对齐 opencode

---

## 1. 架构层优化

### 1.1 对话框叠加系统

**目标**: 支持多个弹窗叠加（如 CommandPalette 叠 PermissionApproval 叠 Chat），而不是当前的单层 AppMode 切换。

**设计方案（Ratatui 兼容）**:

```rust
// src/tui/app.rs

pub struct TuiApp {
    // 当前活跃的模式栈（栈顶 = 最上层弹窗）
    pub mode_stack: Vec<AppMode>,
    // Chat 模式始终在栈底
    // ...
}
```

**渲染逻辑**:
```
fn render(f, app):
    遍历 mode_stack:
        render 对应模式（底层先渲染，上层覆盖）
```

**键盘处理**:
```
Esc 关闭栈顶弹窗（回到上一层）
其他按键传递给栈顶模式处理
```

**改动文件**:
- `src/tui/app.rs` — `mode: AppMode` → `mode_stack: Vec<AppMode>`
- `src/tui/mod.rs` — 渲染循环 + 键盘分发逻辑
- `src/tui/screens/main_screen.rs` — 适配多模式渲染

**工作量**: ~200 行

---

### 1.2 持久侧边栏 + 实时面板

**目标**: 侧边栏不仅显示会话列表，还可以切换为实时面板（Context / Files / Todo），像 opencode 那样持久可见。

**设计方案**:

```rust
// 侧边栏面板类型
enum SidebarPanel {
    Sessions,  // 当前：会话列表
    Context,   // Token 用量 / 成本 / 模型
    Files,     // 最近修改的文件
}

// TuiApp 新增字段
pub sidebar_panel: SidebarPanel,  // 当前面板
pub sidebar_width: u16,          // 可调整宽度
```

**渲染逻辑**:
- `Ctrl+Tab` 切换面板类型
- `←/→` 调整宽度
- Context 面板显示：token 用量、cache hit rate、model/provider
- Files 面板从 `app.diff_content` / file changes 中提取

**改动文件**:
- `src/tui/screens/main_screen.rs` — 重写 render_sidebar 支持多面板
- `src/tui/app.rs` — 新增字段
- `src/tui/mod.rs` — 键盘处理

**工作量**: ~150 行

---

## 2. 体验层优化

### 2.1 Diff 语法高亮

**目标**: Diff 查看器中，变更行的代码内容使用 syntect 做语法高亮，而不只是统一的 `+`/`-` 颜色。

**设计方案**:

```rust
// 在 build_diff_line 中，对添加/删除行：
// 1. 去掉开头的 + 或 - 符号
// 2. 用 syntect 对剩余代码做语法高亮
// 3. 把高亮结果加上 diff 颜色 tint

fn highlight_code_line(
    raw: &str,
    file_ext: &str,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let syntax = syntax_set.find_syntax_by_extension(file_ext)
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
    // ... syntect 高亮逻辑
}
```

**文件扩展名推断**: 从 diff 的 `+++ b/path/to/file.rs` 行提取扩展名。

**改动文件**:
- `src/tui/components/diff_viewer.rs` — 增强 build_diff_line

**工作量**: ~100 行

---

### 2.2 权限面板内联 Diff

**目标**: 审批文件修改时，直接在审批弹窗中看到 diff 预览，不需要按 `d` 切到单独 DiffViewer。

**设计方案**:

```rust
// 在 ToolApprovalRequest 中新增字段
pub struct ToolApprovalRequest {
    // ... 现有字段
    pub diff_preview: Option<String>,  // 截断的 diff 文本（前 10 行）
}
```

审批弹窗渲染时，如果 `diff_preview` 存在，在决策选项之前渲染 6-10 行 diff 预览。

**改动文件**:
- `src/engine/conversation_loop/approval.rs` — 新增 diff_preview 字段
- `src/engine/conversation_loop/permission_controller.rs` — 填充 diff_preview
- `src/tui/screens/main_screen/approvals.rs` — 渲染 diff 预览

**工作量**: ~80 行

---

### 2.3 Which-Key 快捷键发现

**目标**: 按前缀键（如 `Ctrl+O`）后弹出一个小窗口，提示该前缀下的所有快捷键。

**设计方案（简单版）**:

在 ShortcutHelp 模式下，支持"先按前缀键，然后显示该前缀下的快捷键"。

```
用户按 g → 弹出:
  g → scroll to top
  G → scroll to bottom
```

更简单的实现：在 ShortcutHelp 中按模式分组，过滤只显示当前模式的快捷键。

**改动文件**:
- `src/tui/screens/main_screen/popups.rs` — ShortcutHelp 增强
- `src/tui/keybindings.rs` — 添加按模式过滤方法

**工作量**: ~60 行

---

## 3. 产品层优化

### 3.1 交互式 Onboarding

**目标**: 首次启动时不只是静态幻灯片，而是引导用户完成实际配置。

**设计方案**:

在 onboarding 流程中增加：
1. **Provider 选择**: 从 catalog 中列出可配置的 provider，用户选择后展示 `/connect` 指引
2. **API Key 输入引导**: 显示具体的 `export XXX=...` 命令
3. **主题选择**: 展示 9 套主题的预览，用户选择默认主题
4. **首次对话提示**: 给出建议的第一个问题

**改动文件**:
- `src/onboarding/mod.rs` — 增强步骤内容
- `src/tui/screens/main_screen.rs` — render_onboarding 适配

**工作量**: ~120 行（主要是文字内容）

---

## 4. 实施顺序

| 顺序 | 方向 | 工作量 | 理由 |
|:---:|------|:---:|------|
| 1 | **Diff 语法高亮** | ~100 行 | 用户感知最强 |
| 2 | **Which-Key** | ~60 行 | 最快交付 |
| 3 | **权限内联 Diff** | ~80 行 | 审批体验闭环 |
| 4 | **对话框叠加** | ~200 行 | 架构改进 |
| 5 | **持久侧边栏面板** | ~150 行 | 上下文可见性 |
| 6 | **交互式 Onboarding** | ~120 行 | 首次体验 |

---

## 5. 窄门禁

```bash
cargo test -q --lib
cargo clippy --all-features -- -D warnings
cargo fmt --check
```

---

> 文档版本: v1.0, 2026-06-09
> 基于: 第一轮 TUI 优化后的剩余差距分析
