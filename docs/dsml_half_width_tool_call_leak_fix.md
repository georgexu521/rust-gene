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
