# 下一步开发计划
Status: Active

> 2026-06-09 — 综合 opencode 源码对比、产品生态差距分析、TUI 体验对比后的统一路线图
> 已完成的 23 个任务见 `docs/archive/OPENCODE_GAP_ANALYSIS_AND_DESIGN_2026-06-09.md` 和 `docs/archive/NEXT_PHASE_PRODUCT_ECOSYSTEM_GAP_PLAN_2026-06-09.md`

---

## 0. 当前基线

- cargo test --lib: 2437 passed, 0 failed, 1 ignored
- cargo clippy --all-features -- -D warnings: clean
- cargo fmt --check: clean

---

## 1. TUI 体验优化（当前阶段）

### 1.1 Diff 查看器升级

**目标文件**: `src/tui/components/diff_viewer.rs`

| 任务 | 说明 |
|------|------|
| 1.1.1 行号显示 | 解析 unified diff 的 `@@ -old,count +new,count @@` header，为每行计算旧/新行号，渲染到左侧 gutter |
| 1.1.2 Hunk 导航 | `n` 键跳到下一个 `@@` header，`p` 键跳到上一个；在 footer 显示 "Hunk 2/5" |
| 1.1.3 多文件切换 | `Tab` 键在 `diff --git a/... b/...` 分割的多文件 diff 间切换 |
| 1.1.4 滚动位置指示 | footer 显示 "Line 42/150" 及滚动百分比 |
| 1.1.5 Git 集成 | 当 diff_text 为空时，自动调用 `git diff` 获取工作区变更；支持 `--staged` |

**Narrow gates**: `cargo test -q diff_viewer --lib`

---

### 1.2 会话选择器增强

**目标文件**: `src/tui/session_manager.rs`, `src/tui/screens/main_screen.rs`

| 任务 | 说明 |
|------|------|
| 1.2.1 Pin 会话 | 在侧边栏按 `P` 固定/取消固定；已固定会话显示在 "Pinned" 分组顶部，最多 9 个 |
| 1.2.2 内联重命名 | 侧边栏中按 `R` 进入重命名模式，Enter 确认 |
| 1.2.3 侧边栏操作 | 侧边栏中按 `Enter` 直接切换会话，按 `D` 删除（需二次确认） |
| 1.2.4 搜索筛选 | 侧边栏中按 `/` 开始筛选，实时过滤会话标题 |
| 1.2.5 元数据显示 | 每个会话条目显示：title (截断), model (缩写), msg 数量, 最后活跃时间 |

**Narrow gates**: `cargo test -q session_manager --lib`

---

### 1.3 权限面板内联 Diff

**目标文件**: `src/tui/screens/main_screen/approvals.rs`

| 任务 | 说明 |
|------|------|
| 1.3.1 内联 diff | 当审批对象是 file_write/file_edit 时，在审批面板下半部分嵌入 10-15 行的 diff 预览 |
| 1.3.2 风险分解 | 审批面板底部显示 risk_facts 列表（如 "修改文件", "在工作区外", "网络访问"） |
| 1.3.3 规则匹配说明 | 显示匹配的权限规则及其来源（System/Global/Project/User） |

**Narrow gates**: `cargo test -q approvals --lib`

---

### 1.4 命令面板增强

**目标文件**: `src/tui/app/palette.rs`, `src/tui/screens/main_screen/popups.rs`

| 任务 | 说明 |
|------|------|
| 1.4.1 匹配字符高亮 | 在搜索结果中，用 accent 色高亮匹配的字符位置 |
| 1.4.2 最近使用分区 | 当输入为空时，显示 "Recently Used" 分组（已有 recency 数据，只需 UI） |
| 1.4.3 无结果建议 | 当匹配数为 0 时，用编辑距离计算最接近的 3 个命令并显示 "Did you mean...?" |

**Narrow gates**: `cargo test -q palette --lib`

---

### 1.5 快捷键可发现性

**目标文件**: `src/tui/screens/main_screen/popups.rs`, `src/tui/keybindings.rs`

| 任务 | 说明 |
|------|------|
| 1.5.1 ShortcutHelp 搜索 | 在 ShortcutHelp 弹窗中，按 `/` 可以输入关键词筛选快捷键 |
| 1.5.2 上下文感知 | 根据当前 AppMode 过滤显示的快捷键（Chat/Approval/Diff 各自只显示相关快捷键） |

**Narrow gates**: `cargo test -q keybindings --lib`

---

## 2. 后续方向（TUI 完成后）

| 方向 | 优先级 | 说明 |
|------|:---:|------|
| API 生产化 + SDK 发布 | 高 | `experimental-api-server` 去 feature gate，生成 OpenAPI spec |
| VS Code 扩展 | 高 | 已有 `DesktopRunContext` + `@file` 解析，扩展约 100 行 |
| CI 多平台发布 | 中 | 解锁 Linux/Windows 用户 |
| 桌面自动更新 | 中 | Tauri updater 配置简单 |
| 插件 WASM 沙箱 | 低 | 长期差异化能力 |

---

> 当前阶段: TUI 体验优化（5 个方向，预计 ~830 行 Rust）
