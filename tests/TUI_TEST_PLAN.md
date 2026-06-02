# Priority Agent TUI 真实案例测试计划

日期：2026-06-02

## 测试环境要求

```bash
# 确保 API key 已配置
export MOONSHOT_API_KEY="your-key"
# 或
export OPENAI_API_KEY="your-key"

# 启动 TUI
cargo run -- --tui
# 或
./target/debug/priority-agent --tui
```

## 测试原则

**区分 PA 流程问题 vs LLM 能力问题：**

| 症状 | 可能是 PA 流程问题 | 可能是 LLM 能力问题 |
|------|-------------------|---------------------|
| 工具调用失败、返回错误 | ✅ 检查工具实现 | ❌ |
| 模型拒绝使用工具、选择错误的工具 | ❌ | ✅ |
| closeout 虚假通过（没验证就说过了） | ✅ 检查验证闭环 | ❌ |
| 模型写的代码有 bug | ❌ | ✅ |
| 权限检查不合理（该拒绝的过了，该过的拒绝了） | ✅ 检查权限逻辑 | ❌ |
| 模型反复读同一个文件不干活 | ❌ | ✅（弱模型常见） |
| 修复循环跑了很多轮才修好 | ❌ | ✅ |
| 流式输出卡住、不显示 | ✅ 检查 streaming | ❌ |
| 缓存命中率低 | ✅ 检查 cache_stability | ❌ |

**核心判断标准**：如果换个更强的模型（如 kimi-k2.5 → DeepSeek V3）问题消失，那是 LLM 能力问题。如果换模型问题依旧，是 PA 流程问题。

---

## 测试 1：基础代码阅读

**提示词**：
```
Read the file src/tui/theme.rs and tell me:
1. What theme presets are available?
2. Which one is the default?
3. What is the fg.strong color in graphite theme?
```

**预期行为**：
- ✅ 调用 file_read 读取 src/tui/theme.rs
- ✅ 回答包含 graphite, porcelain, nord, dracula, gruvbox-dark, catppuccin-mocha 等
- ✅ 回答默认是 graphite
- ✅ fg.strong 颜色值正确

**判断**：如果 file_read 成功但回答错误 → LLM 问题。如果 file_read 失败 → PA 问题。

---

## 测试 2：简单代码修改

**提示词**：
```
Add a "hello world" comment at the top of src/tui/theme.rs.
The comment should say: // Hello from Priority Agent test

Then run cargo check -q to verify it compiles.
```

**预期行为**：
- ✅ 调用 file_read 读取文件
- ✅ 调用 file_edit 添加注释
- ✅ 调用 bash 执行 cargo check -q
- ✅ closeout 显示 verified + evidence

**判断**：
- 编辑后文件损坏 → PA 的 file_edit 问题
- cargo check 失败但 closeout 说过了 → PA 的验证闭环问题
- 模型拒绝编辑 → LLM 问题
- 编辑正确但验证命令拼错 → LLM 问题

---

## 测试 3：缓存命中率验证

**提示词（第一轮）**：
```
Read src/tui/components/message.rs and tell me how many functions it has
```

**提示词（第二轮，在同一 session 中）**：
```
Now read src/tui/components/message.rs again and tell me if there's a function called render_user_message
```

**预期行为**：
- 两轮都正常工作
- 在 TUI 中输入 `/doctor` 或查看 status bar 的 cache hit%

**判断**：
- 第二轮响应明显比第一轮快 → 缓存命中正常
- 两轮速度差不多 → 缓存可能未命中，需检查 cache_stability
- 第二轮报错 → PA 问题

---

## 测试 4：多工具协作

**提示词**：
```
Find all places in the codebase that call the function "prepend_to_last_user_message".
For each caller, tell me what file it's in and what dynamic zone it injects.
```

**预期行为**：
- ✅ 调用 grep 搜索
- ✅ 返回 file:line 引用
- ✅ 正确识别各个 zone（task-state, context-pack 等）

**判断**：
- grep 没找到 → 检查 grep_tool
- 找到但模型没正确总结 → LLM 问题

---

## 测试 5：验证闭环

**提示词**：
```
Modify the file src/tui/theme.rs: change the graphite theme's bg color
from Color::Rgb(0x0b, 0x10, 0x20) to Color::Rgb(0x10, 0x20, 0x30).

IMPORTANT: after making the change, run "cargo test -q --lib tui::theme::tests"
to verify your change doesn't break anything.
```

**预期行为**：
- ✅ 读取文件
- ✅ 精确编辑（只改颜色值，不改其他）
- ✅ 运行 cargo test
- ✅ closeout 显示 test 结果

**判断**：
- 测试失败了但 closeout 说 passed → PA 的 validation closeout 问题（严重）
- 测试通过了 → 一切正常
- 编辑了错误的行 → LLM 问题

---

## 测试 6：权限和安全

**提示词**：
```
Run this command: rm -rf /tmp/test-pa-deleteme
```

**预期行为**：
- ✅ 弹出权限确认（因为 rm 是高风险命令）
- ✅ 用户可以选择 allow/deny

**判断**：
- 没有任何确认就执行了 → PA 权限问题（严重）
- 有确认 → 正常

---

## 测试 7：Rollback

**提示词**：
```
Edit src/tui/theme.rs to add a deliberate syntax error
(change "Color::Rgb" to "Color::Rgbx" somewhere).

Then run cargo check -q. It should fail.

Then fix your mistake and verify it compiles again.
```

**预期行为**：
- ✅ 模型编辑并引入错误
- ✅ cargo check 失败
- ✅ 模型修复错误
- ✅ 再次 cargo check 通过
- ✅ closeout 显示最终 verified

**判断**：
- 模型无法修复 → LLM 能力问题
- 修复了但 PA 没有正确记录 evidence → PA 问题
- 整个流程顺畅 → 正常

---

## 执行后检查

每个测试结束后，在 TUI 中输入 `/trace last` 查看 turn trace：

1. 工具调用序列是否正确
2. 是否有 unexpected error
3. cache hit 状态
4. closeout evidence 是否完整

---

## 总结

| 测试 | 验证什么 | 预计时间 |
|------|---------|---------|
| 1. 代码阅读 | file_read + 基础工具 | 30s |
| 2. 简单修改 | file_edit + bash validation | 1min |
| 3. 缓存命中 | 静态前缀缓存效果 | 1min |
| 4. 多工具 | grep + 结果分析 | 1min |
| 5. 验证闭环 | edit + test + closeout | 1-2min |
| 6. 权限 | 高风险命令拦截 | 30s |
| 7. Rollback | 错误修复循环 | 1-2min |
