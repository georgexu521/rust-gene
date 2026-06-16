# DSML 半角工具调用泄漏修复记录

## 问题描述

DeepSeek 系列模型（如 `deepseek-v4-flash`）在 TUI/非流式输出中，会把工具调用以**半角 DSML markup** 的形式直接泄漏到 assistant content 中，而不是以 JSON `tool_calls` 返回。例如用户看到的原始回复里出现：

```text
我来检查。
<| | DSML | | tool_calls>
<| | DSML | | invoke name="bash">
<| | DSML | | parameter name="command" string="true">ls -la ~/Desktop/phageGPT/<| | DSML | | parameter>
<| | DSML | | parameter name="description" string="true">List project root<| | DSML | | parameter>
</| | DSML | | invoke>
</| | DSML | | tool_calls>
Done.
```

这导致：
1. **UI 上把工具调用结构当成普通文本渲染**，用户看到杂乱 markup。
2. **运行时没有把工具调用真正解析为 `ToolCall`**，因此不会执行 bash 等工具。
3. 清理后的 visible content 中仍可能残留 DSML 块。

此前代码只支持全角 DSML 分隔符（如 `〈DSML｜function_calls〉`），不支持 DeepSeek 实际泄漏的半角变体。

## 涉及的半角 DSML 形态

| 形态 | 示例 |
|------|------|
| 紧凑半角 open | `<|DSML|tool_calls>`、`<|DSML|invoke name="bash">` |
| 紧凑半角 close | `</|DSML|tool_calls>`、`<|/DSML|parameter>` |
| 带空格半角 open | `<| | DSML | | tool_calls>`、`<| | DSML | | parameter name="command" string="true">` |
| 带空格半角 close | `</| | DSML | | invoke>`、`<| | DSML | | parameter>`（与 open 同形，当作 self-closing） |

此外，wrapper tag 名称可能是 `function_calls` 或 `tool_calls`。

## 修复目标

1. 在 `repair_response` 路径中把半角 DSML 正确解析为 `ToolCall`，包括参数。
2. 在 `sanitize_assistant_content`/`strip_hidden_blocks` 路径中把 DSML 工具调用块从可见 content 中移除。
3. 保持全角 DSML 的向后兼容。
4. 新增单元测试覆盖紧凑和带空格两种半角形态。

## 已做的修改

### 1. `src/services/api/tool_call_repair.rs`

#### 1.1 新增 `normalize_dsml_markup`
把半角 DSML 归一化为全角形态，让后续 parser 统一处理：
- open tag: `<|...|...>` / `<| | ... | | ...>` → `〈DSML｜{name}{attrs}〉`
- close tag: `</|...|...>` / `<|/...|...>` / `<| | ... | | ...>` → `〈/DSML｜{name}{attrs}〉`

#### 1.2 `scavenge_dsml_function_calls` 适配
- 先调用 `normalize_dsml_markup`。
- 早期返回条件从只检查 `function_calls` 改为同时检查 `function_calls` 和 `tool_calls`。
- wrapper regex 从只匹配 `function_calls` 改为同时匹配 `function_calls|tool_calls`。

#### 1.3 `parse_dsml_invoke_block` 适配
- invoke 和 parameter 的 close delimiter 同时接受 `〈/DSML｜...〉` 和 `〈DSML｜...〉`，以处理 DeepSeek 用与 open 同形 tag 作 self-close 的情况。

#### 1.4 新增/调整测试
- `normalize_tests::normalizes_compact_half_width_dsml`
- `normalize_tests::normalizes_spaced_half_width_dsml`
- `scavenges_half_width_spaced_dsml_tool_call`
- `scavenges_compact_half_width_dsml_tool_call`

### 2. `src/services/api/content_sanitizer.rs`

#### 2.1 新增 `strip_dsml_blocks`
在原有 `strip_hidden_blocks` 之前先移除 DSML 工具调用外层块。实现方式：
- 用 open regex 找到 `<|...|tool_calls>` / `<|...|function_calls>` 等。
- 用 close regex 找到对应的 close tag（支持 `</|...>`、`<|/...>`、`<| | ... | | ...>` 等）。
- 循环移除从 open 到 close 的整块内容。

#### 2.2 调整 `strip_hidden_blocks`
先调用 `strip_dsml_blocks`，再执行原有的 thinking/invoke/tool_call 清理。

#### 2.3 新增测试
- `strips_spaced_half_width_dsml_blocks`
- `strips_compact_half_width_dsml_blocks`

## 当前状态（截至文档创建时）

- `content_sanitizer` 相关测试：**已通过**。
- `tool_call_repair` 相关测试：工具调用和参数解析已能正确工作，但**带空格形态的 content 断言已从严格相等放宽为 `ends_with("Done.")`**，因为原始文本开头有前置普通文本。
- 尚未运行完整 `cargo test` / `cargo clippy` / `workflow-production-gates.sh`。

## 复查补充（2026-06-16）

后续复查发现两个遗漏：

1. **全角 DSML 仍可能泄漏到可见 content**
   - `scavenge_dsml_function_calls` 能解析全角 `〈DSML｜tool_calls〉...`。
   - 但 `content_sanitizer::strip_dsml_blocks` 只移除了半角 `<|...>` wrapper，未移除全角 wrapper。
   - 结果是工具调用可能会执行，但 assistant 可见正文仍残留全角 DSML markup。
   - 已补 `strips_full_width_dsml_blocks` 和 `scavenges_full_width_dsml_without_visible_markup`。

2. **DSML scavenged 计数被双计数**
   - `repair_response` 会统一按 scavenged calls 数量累加 `report.scavenged_tool_calls`。
   - `parse_dsml_invoke_block` 内部又额外累加了一次，导致 DSML 场景 report 显示 `2` 而不是 `1`。
   - 已移除内部累加，并把半角 DSML 测试收紧为严格校验 cleaned content 和 `scavenged_tool_calls == 1`。

复查后目标测试：

```bash
cargo fmt --check
cargo test -q tool_call_repair
cargo test -q content_sanitizer
cargo check -q
```

## 调试过程中发现的关键原因

1. `scavenge_dsml_function_calls` 的早期返回只检查 `function_calls`，导致 `tool_calls` wrapper 被直接忽略。
2. `parse_dsml_invoke_block` 的 close regex 只接受 `〈/DSML｜parameter〉`，而 DeepSeek 的 self-closing parameter 用的是与 open 同形的 `〈DSML｜parameter〉`，导致参数体为空。
3. `content_sanitizer::strip_dsml_blocks` 最初的 close regex 要求 `</|DSML|...>` 开头，没有处理 `<|/DSML|...>` 和同形 self-close 情况。

## 后续待办

1. 运行 `cargo test -q` 全量测试。
2. 运行 `cargo clippy --all-targets --all-features -- -D warnings`。
3. 运行 `bash scripts/workflow-production-gates.sh`。
4. 提交修改并刷新 workflow 报告。

## 新增：TUI assistant 文本重复输出修复（2026-06-16）

### 现象

在 TUI 中，同一条 assistant 回复的文本内容会出现两次（如用户截图中 `## 📌 phageGPT 项目概览` 到 `核心目标 / 工作原理` 段落在页面上方和下方各出现一次）。

### 根因

TUI 的 message-part 投影允许一条 assistant message 存在多个 `Text` part：
- **Streaming 增量**（`AssistantTextDelta`）通过 `assistant_part_for_message` 创建/追加 text part，id 形如 `msg:text:0`。
- **权威性更新**（`AssistantTextUpdated`，来自 persisted replay 或最终状态同步）通过 `set_message_text_part` 设置 text part，id 固定为 `msg:text`（`part_id_for` 生成）。

两者 id 不一致，导致同一条 assistant message 同时存在：
- `msg:text:0`：streaming 累积的文本
- `msg:text`：authoritative 更新写入的完整文本

渲染时 `append_part_lines` 会把所有 text parts 依次渲染，于是用户看到重复内容。

### 修复

在 `src/tui/sync_store.rs` 的 `set_message_text_part` 中，对 `TuiPartKind::Text` 使用与 streaming 相同的 id 方案：
- 如果该 message 已存在 text part，复用最后一个 text part 的 id。
- 否则创建 `msg:text:0`。

这样 `AssistantTextUpdated` 会覆盖当前 text part，而不是创建新的重复 part。

### 验证

- `cargo test -q tui::sync_store::tests`：通过。
- `cargo test -q tui::view_model::timeline::tests`：通过。
- `cargo test -q tui::components::message::tests`：通过。
- `cargo fmt --check`：通过。
- `cargo check -q`：通过。

### 备注

本次只修改了 `src/tui/sync_store.rs`。另有两个与 tool-part message 归属相关的既有测试失败，与本次重复输出修复无关：
- `tui::render_session::tests::live_projection_and_persisted_hydration_produce_same_render_session`
- `tui::app::tests::cancel_active_run_interrupts_query_and_marks_tool_cancelled`

这些失败源于近期“把 tool call 作为 assistant message ordered parts”的改动，导致 persisted replay 与 live streaming 对 tool part 的 `message_id` 归属不一致，需要单独处理。
