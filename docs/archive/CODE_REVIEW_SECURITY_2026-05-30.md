# Priority Agent 代码审查报告

> 审查日期：2026-05-30
> 审查范围：全代码库安全与质量审查
> 审查人：Kimi Code CLI

---

## 📊 总体评估

| 维度 | 评分 | 说明 |
|------|------|------|
| **安全性** | ⭐⭐⭐☆☆ | 存在 2 个 Critical 漏洞，多个 High 风险 |
| **代码质量** | ⭐⭐⭐⭐☆ | 整体结构良好，但 clippy 有错误，unwrap 过多 |
| **并发安全** | ⭐⭐⭐☆☆ | 同步锁混用异步代码，存在死锁和阻塞风险 |
| **测试覆盖** | ⭐⭐⭐⭐☆ | 核心模块有测试，但测试运行慢（超时） |
| **错误处理** | ⭐⭐⭐☆☆ | 2140 处 unwrap/expect，健壮性不足 |

**编译状态**: `cargo check` ✅ 通过 | `cargo clippy --all-features` ❌ 1 个错误

---

## 🔴 Critical（必须立即修复）

### CRIT-1: API 认证默认开放（Fail-Open）

**文件**: `src/api/routes.rs:825-857`
**风险**: 未配置 bridge token 时，API 完全开放，任何人可远程调用工具

```rust
async fn bridge_auth_middleware(req: Request<Body>, next: Next) -> Response {
    let configured = configured_bridge_tokens();
    if !configured.is_empty() {
        // 只有配置了 token 才检查认证
        // ...
    }
    next.run(req).await  // ❌ 未配置 = 完全跳过认证！
}
```

**攻击场景**: 用户未设置 `PRIORITY_AGENT_BRIDGE_TOKEN`，启动 API server 后，攻击者可远程调用 `file_read` 读取敏感文件，或通过 `agent_tool` 执行任意操作。

**修复**:
```rust
async fn bridge_auth_middleware(req: Request<Body>, next: Next) -> Response {
    let configured = configured_bridge_tokens();
    if configured.is_empty() {
        return (StatusCode::FORBIDDEN, Json(json!({
            "error": "bridge authentication not configured"
        }))).into_response();
    }
    // ... 现有认证逻辑
}
```

---

### CRIT-2: AgentTool 路径遍历漏洞

**文件**: `src/tools/agent_tool/mod.rs:160-178`
**风险**: `load_file_context` 直接拼接用户输入路径，无路径遍历检查

```rust
async fn load_file_context(files: &[String], working_dir: &Path) -> String {
    for file in files {
        let path = working_dir.join(file);  // ❌ 未检查 ../ 或绝对路径
        match tokio::fs::read_to_string(&path).await { ... }
    }
}
```

**攻击场景**: LLM 调用 `agent_tool` 时传入 `"files": ["../.env"]` 或 `"files": ["/etc/passwd"]`，可直接读取工作目录外的敏感文件。

**修复**: 使用 `file_tool::resolve_path` 进行路径验证：
```rust
let path = match crate::tools::file_tool::resolve_path(file, working_dir) {
    Ok(p) => p,
    Err(e) => { warn!(...); continue; }
};
```

---

## 🟠 High（尽快修复）

### HIGH-1: BashTool 命令注入

**文件**: `src/tools/bash_tool/mod.rs:~1382`
**风险**: 用户输入直接传递给 `bash -c`，黑名单可被绕过

**绕过示例**:
```bash
# 这些可绕过现有检测
/bin/rm -rf /         # 完整路径
bash -c 'rm -rf /'    # 嵌套调用
echo 'rm' | sh        # 管道
```

**修复建议**: 增加 AST 级命令解析，或限制只允许预定义安全模式。

---

### HIGH-2: PowerShellTool 命令注入

**文件**: `src/tools/powershell_tool/mod.rs:~274`
**风险**: 与 BashTool 相同，用户输入直接传递给 `pwsh -Command`

---

### HIGH-3: run_tests_tool 命令注入

**文件**: `src/tools/run_tests_tool.rs:77-79`
**风险**: 用户输入直接传递给 `sh -lc`

---

### HIGH-4: GrepTool ReDoS（正则拒绝服务）

**文件**: `src/tools/grep_tool/mod.rs:~142`
**风险**: 无长度限制、无超时控制编译用户提供的正则

**攻击示例**: 正则 `(a+)+b` 对输入 `"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaac"` 可导致 CPU 耗尽。

**修复**:
```rust
use regex::RegexBuilder;

if pattern.len() > 1000 {
    return ToolResult::error("regex pattern too long (max 1000 chars)");
}

let regex = RegexBuilder::new(pattern)
    .size_limit(1 << 20)
    .dfa_size_limit(1 << 20)
    .build();
```

---

### HIGH-5: API 工具调用黑名单可绕过

**文件**: `src/api/routes.rs:304-324`
**风险**: 黑名单不完整，缺少 `file_patch`, `powershell`, `mcp_tool` 等

```rust
const DANGEROUS_TOOLS: &[&str] = &["bash", "file_write", "file_edit"];
// ❌ 缺少 "file_patch", "powershell", "mcp_tool", "remote_dev", "install_dependencies"
```

**修复**: 改用白名单机制，仅允许明确的只读工具通过 API。

---

## 🟡 Medium（建议修复）

### MED-1: 同步锁混用异步代码

**文件**: 
- `src/engine/streaming.rs:181-183`
- `src/state/events.rs:39-76`

**风险**: `std::sync::RwLock` 和 `std::sync::Mutex` 在 tokio 异步运行时中会阻塞 worker 线程，影响并发性能，极端情况下可能导致死锁。

**修复**: 全部替换为 `tokio::sync::RwLock` 和 `tokio::sync::Mutex`。

---

### MED-2: EventBus 回调在锁内执行

**文件**: `src/state/events.rs:71-76`

```rust
pub fn emit(&self, event: StateEvent) {
    let subscribers = self.subscribers.lock().expect("...");  // ❌ expect 会 panic
    for callback in subscribers.values() {
        callback(event.clone());  // ❌ 在锁内调用外部代码
    }
}
```

**风险**: 
1. 回调 panic 会导致 Mutex poison，后续所有 emit 都 panic
2. 回调阻塞会阻塞所有订阅/发布操作

**修复**:
```rust
pub fn emit(&self, event: StateEvent) {
    let callbacks: Vec<_> = {
        let subscribers = self.subscribers.lock().unwrap_or_else(|e| e.into_inner());
        subscribers.values().cloned().collect()
    };
    for callback in callbacks {
        callback(event.clone());
    }
}
```

---

### MED-3: BashTool 输出日志可能泄露敏感信息

**文件**: `src/tools/bash_tool/mod.rs:463-508`
**风险**: 命令输出（可能含 API key、密码）写入日志文件，未限制文件权限

**修复**: 写入时设置权限 `0o600`：
```rust
#[cfg(unix)]
{
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).ok();
}
```

---

### MED-4: 消息历史注入风险

**文件**: `src/engine/query_engine.rs:~157-181`
**风险**: `context_messages` 直接注入 LLM 上下文，未验证角色

**修复**: 验证 `context_messages` 中不包含 `System` 角色消息。

---

### MED-5: Agent 取消无 Graceful Shutdown

**文件**: `src/agent/manager.rs:506-519`
**风险**: `tokio::spawn` 后未保存 JoinHandle，`kill` 不等待实际终止

**修复**: 保存 JoinHandle，kill 时等待完成（带超时）。

---

### MED-6: notebook_tool 反序列化无大小限制

**文件**: `src/tools/notebook_tool/mod.rs:~191`
**风险**: 超大 `.ipynb` 文件可导致内存耗尽

**修复**: 反序列化前检查文件大小（如限制 10MB）。

---

### MED-7: web_search 未限制重定向

**文件**: `src/tools/web_tools/mod.rs:~165`
**风险**: `curl -sL` 未设置 `--max-redirs`，可被重定向链攻击

**修复**: 添加 `--max-redirs 3`。

---

### MED-8: 权限 ReadOnly 模式覆盖不完整

**文件**: `src/permissions/mod.rs:394-397`

```rust
PermissionMode::ReadOnly => {
    matches!(tool_name, "file_write" | "file_edit" | "bash" | "mcp_tool")
}
```

**遗漏的工具**: `file_patch`, `format`, `notebook`（edit_cell 等）, `skill_manage`, `powershell`, `install_dependencies`, `remote_dev`, `worktree` 等。

---

### MED-9: BrowserTool 使用 `--no-sandbox`

**文件**: `src/tools/browser_tool.rs:~202`
**风险**: Chrome 以 `--no-sandbox` 启动，浏览器被攻破后可直接访问主机

---

### MED-10: BrowserTool unwrap 可能 panic

**文件**: `src/tools/browser_tool.rs:232`

```rust
anyhow::bail!("Navigation error: {}", res["errorText"].as_str().unwrap());  // ❌
```

---

### MED-11: 配置更新无安全校验

**文件**: `src/api/routes.rs:374-386`
**风险**: `PUT /api/config` 可任意修改 `api.base_url`，攻击者可重定向对话到恶意服务器

---

### MED-12: 审计导出路径校验不完整

**文件**: `src/api/routes.rs:497-538`
**风险**: 仅检查 `..` 和绝对路径，未限制路径长度、未防 symlink 跳转

---

## 🟢 Low（可选修复）

### LOW-1: 大量 unwrap/expect（2140 处）

**分布**: `src/session_store/mod.rs` (128), `src/memory/manager.rs` (88), `src/memory/provider.rs` (78), `src/tools/file_tool/mod.rs` (176), `src/tools/bash_tool/mod.rs` (30), ...

**建议**: 逐步替换为 `?` 或 `match` 处理，特别是生产代码中的用户输入处理路径。

---

### LOW-2: Clippy 错误

**文件**: `src/engine/conversation_loop/mod.rs:159`

```
error: writing `&mut Vec` instead of `&mut [_]` involves a new object where a slice will do
```

**修复**: 将 `&mut Vec<Message>` 改为 `&mut [Message]`。

---

### LOW-3: AgentMemory 条目无上限

**文件**: `src/agent/memory.rs`
**风险**: `entries` HashMap 可无限增长（snapshots 已有 `MAX_SNAPSHOTS=100` 限制）

---

### LOW-4: AgentAuditor 记录无限制增长

**文件**: `src/agent/manager.rs:306-345`
**风险**: `records: Vec<AgentAuditRecord>` 长期运行消耗大量内存

---

### LOW-5: session_id 长度未限制

**文件**: `src/tools/bash_tool/mod.rs:472-486`
**风险**: 超长 session_id 可能导致路径超过 OS 限制

---

## 📊 问题统计

| 严重程度 | 数量 | 类别 |
|---------|------|------|
| 🔴 Critical | 2 | 认证绕过、路径遍历 |
| 🟠 High | 5 | 命令注入、ReDoS、权限绕过 |
| 🟡 Medium | 12 | 并发安全、信息泄露、注入风险 |
| 🟢 Low | 5 | unwrap、内存增长、代码风格 |
| **总计** | **24** | |

---

## 🎯 优先修复建议

### 立即修复（今天）
1. **CRIT-1** `bridge_auth_middleware` fail-open → fail-closed
2. **CRIT-2** `agent_tool` 使用 `resolve_path`
3. **LOW-2** 修复 clippy 错误

### 本周修复
4. **HIGH-4** GrepTool 正则限制
5. **HIGH-5** API 工具白名单替代黑名单
6. **MED-1** 同步锁替换为 tokio 异步锁
7. **MED-2** EventBus 回调移出锁范围
8. **MED-8** ReadOnly 模式补全工具列表

### 中期修复
9. **HIGH-1/2/3** 命令注入防护增强
10. **MED-3/6/7** 文件权限、反序列化限制、重定向限制
11. **LOW-1** 逐步减少 unwrap（从用户输入路径开始）

---

*报告生成时间: 2026-05-30*
