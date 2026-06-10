# TUI 差距分析：PA vs Reasonix（2026-06-02 深度对比）

## 总体判断

经过前几轮优化（token themes、card messages、status bar mode glyph、context bar、scroll indicator），
PA 的 TUI 骨架已经和 Reasonix 对齐了大约 **60%**。剩余差距主要在**渲染细节丰富度**和**交互反馈**上，
不是架构问题，是"打磨"问题。

---

## 逐组件对比

### 1. User Card

| 细节 | Reasonix | PA | 差距 |
|------|----------|-----|------|
| Header glyph | `◇` + "you" | `◇` + "You" | ✅ 一致 |
| 时间戳 | `formatRelativeTime()`: "just now" / "5s ago" / "3m ago" | **无** | ❌ 缺失 |
| Body 前缀 | `↳` (FG.sub) | 无前缀，2空格缩进 | ⚠️ 建议加 `↳` |
| 背景色 | `MESSAGE_BG.user` (#373737) | `MESSAGE_BG.user` (#373737) | ✅ 一致 |

**改进**: 在 card header 的 meta 参数中传入相对时间戳。

---

### 2. Assistant / Streaming Card

| 细节 | Reasonix | PA | 差距 |
|------|----------|-----|------|
| 脉冲动画 | `PULSE_CIRCLE` 6帧: ◌◐◑◒◓● | `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` 10帧 braille | ⚠️ 不同风格，都可接受 |
| 实时 token 速率 | `{tps} t/s` pill | 无（只有总 token 数） | ❌ 缺失 |
| 展开/折叠 | `⌃o` 切换 4行↔60行 | 无 | ❌ 缺失 |
| Model badge | `flash`/`pro`/`r1` pill | 无 | ❌ 缺失 |
| 完成后 | `‹` glyph + `{tokens} tok · {tps} t/s` | `‹` glyph + `{N} tok` | ⚠️ 缺 tps |
| 截断提示 | "earlier N lines" faint | 无 | ❌ 缺失 |

**改进优先级**: 实时 t/s → model badge → 展开/折叠

---

### 3. Tool Card

| 细节 | Reasonix | PA | 差距 |
|------|----------|-----|------|
| 脉冲动画 | `PULSE_SQUARE` 4帧: ▢▣▤▣ | 静态 ▣ | ❌ 无动画 |
| Args 摘要 | 格式化显示关键参数 | 无 | ❌ |
| 状态图标 | ✓/✗/⊘ 按结果 | ●/✓/✗ (在 tool runs 区域) | ⚠️ 分散在两处 |
| Exit code | 显示退出码 | 无 | ❌ |
| 耗时 | elapsed seconds | 无 | ❌ |
| 内联 diff | 编辑工具显示 diff | 无（单独的 diff viewer） | ⚠️ 不同方式 |
| Retry badge | ↻ N/M | 无 | ❌ |
| 长输出折叠 | "earlier N lines" | 无 | ❌ |

**改进优先级**: exit code + elapsed → args 摘要 → 内联 diff

---

### 4. Status Bar

| 细节 | Reasonix | PA | 差距 |
|------|----------|-----|------|
| Mode + network | ● auto / ◌ slow / ✗ disconnect | ● auto (无 network 状态) | ⚠️ 缺 network |
| Session | `{id} · {branch}` | `{short_id}` | ⚠️ 缺 branch |
| Turn cost | `▸ ¥0.003 turn` | 无 | ❌ 缺失 |
| Cache hit% | `cache 85%` | 无 | ❌ 缺失 |
| Context bar | 8-cell `█░` + pct + token count | 8-cell `█░` + pct（无 token 数） | ⚠️ 缺 token 数 |
| Wallet | `⛁ ¥0.15 spent / ¥12.34 left` | 无 | ❌ 缺失 |
| Version | `v2.5.0` | 无 | ❌ 缺失 |
| Feedback hint | `⚑ Ctrl+O expand tool` | `? shortcuts` | ⚠️ 不同内容 |
| Responsive | 根据 cols 动态显示/隐藏 pills | 无 | ❌ |
| Recording | 🔴 录音状态 | 无 | ❌ |
| MCP loading | `⌁ MCP loading 2/5` | `mcp:2/5` | ⚠️ 图标不同 |

**改进优先级**: turn cost → cache hit% → version → wallet → responsive

---

### 5. Composer / Input

| 细节 | Reasonix | PA | 差距 |
|------|----------|-----|------|
| Prompt 字符 | `›` (非 shell) / `$` (shell) | `›` (统一) | ✅ 基本一致 |
| Mode 颜色 | brand/warn/accent | brand/warn/accent/info | ✅ 一致 |
| Placeholder | "Message Reasonix..." | "Message Priority Agent..." | ✅ 一致 |
| HintRow | 键盘快捷键提示 | Ctrl+O hint（部分） | ⚠️ 建议加 HintRow |
| Shell mode | `$` 前缀 | 无 | ❌ 缺失 |
| 多行 | 自动扩展 | 自动扩展 | ✅ 一致 |
| 边框 | 无（纯内容） | 上下分隔线 | ⚠️ 建议去掉边框 |

**改进优先级**: 去掉上下分隔线 → 加 HintRow

---

### 6. Live Activity Area（Reasonix 有，PA 无）

Reasonix 在消息区和输入框之间有 **LiveActivityArea**：

- **ThinkingRow**: 脉冲圆圈 + token 速率 + model badge（在等待 LLM 响应时显示）
- **ModeStatusBar**: edit mode pill (yolo/auto/review) + plan mode indicator
- **OngoingToolRow**: 脉冲方块 + 进度条 + args 摘要（当前运行的 tool）
- **SubagentLiveStack**: 子代理状态（1→详情，2+→compact，>max→折叠）
- **UndoBanner**: undo/pause 快捷键提示
- **Countdown**: 自动审批倒计时

PA 完全没有这个区域。tool 运行状态分散在 status bar（◌ label）和 tool cards 中。

**改进优先级**: 高。这是 Reasonix 最有辨识度的 UI 元素之一。

---

### 7. Scroll / Virtual List

| 细节 | Reasonix | PA | 差距 |
|------|----------|-----|------|
| 虚拟化 | CardStream 按高度裁剪视口 | 按消息行数估算 | ⚠️ Reasonix 更精确 |
| 滚动指示器 | `{N} above · {M} remaining` + PgUp | `{N} above · {M} remaining` + PgUp | ✅ 一致 |
| 自动贴底 | pin/unpin | pin/unpin | ✅ 一致 |
| Static rendering | StaticCardStream（已 settled 卡片不可变） | 无 | ❌ |

**改进优先级**: 低。当前实现可用。

---

### 8. Diff Display

| 细节 | Reasonix | PA | 差距 |
|------|----------|-----|------|
| 显示位置 | 内联 card（在消息流中） | 独立全屏弹窗 | ⚠️ 不同策略 |
| 颜色 | add=ok, del=err | add=ok, del=err | ✅ 一致 |
| 文件路径 | 在 card header | 在弹窗标题 | ⚠️ |
| Apply/skip 操作 | Footer 显示快捷键 | 无 | ❌ |

**改进优先级**: 中。弹窗可用，但内联更自然。

---

### 9. Session Intro

| 细节 | Reasonix | PA | 差距 |
|------|----------|-----|------|
| 格式 | `◈ {id} · {branch} · {workspace} · {model}` | `◈ {id} · {model} · {mode}` | ⚠️ 缺 branch/workspace |
| 颜色 | 全部 faint | 全部 faint | ✅ 一致 |
| 显示时机 | 每个 session 顶部 | 有消息时 | ✅ 一致 |

---

### 10. 缺失的 Card 类型

Reasonix 有但 PA 没有的 card：

| Card | 用途 | 优先级 |
|------|------|--------|
| UsageCard | 每轮 token 消耗分解（prompt/reason/output/cache 柱状图） | 中 |
| MemoryCard | 加载的记忆条目（按类别分组） | 低 |
| CtxCard | 上下文窗口使用详情（32格柱状图 + top tools） | 低 |
| ReasoningCard | 模型思考过程（流式预览/折叠） | 中（需 engine 支持） |
| TipCard | 一次性快捷键提示 | 低 |
| CompactionCard | 历史压缩摘要 | 低 |

---

### 11. Toast 通知系统

Reasonix 有 `ToastRail`：自动消失的通知条（glyph + title + TTL，渐隐）。
PA 完全没有。系统消息以永久 message 形式出现在对话中。

**改进优先级**: 中。

---

## 改进优先级排序

### P0 — 显著提升体验
1. **Live Activity Area** — 显示当前运行的 tool、thinking 状态
2. **Tool card 增强** — exit code、elapsed、args 摘要
3. **Status bar 增强** — turn cost、cache hit%、version

### P1 — 锦上添花
4. **Streaming 增强** — 实时 t/s、model badge
5. **内联 diff** — 替代弹窗
6. **Toast 通知** — 瞬态消息

### P2 — 后续版本
7. **User card 时间戳**
8. **Composer 去边框 + HintRow**
9. **Usage card**
10. **Responsive breakpoints**
11. **StaticCardStream**
