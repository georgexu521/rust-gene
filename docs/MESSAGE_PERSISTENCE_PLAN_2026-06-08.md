# Message Persistence Plan — 2026-06-08
Status: Active

将对话消息从纯内存管理升级为持久化优先。已有 SQLite 基础，不需要重写。

---

## 1. 现状

### 已有基础

| 能力 | 位置 | 状态 |
|------|------|------|
| SQLite 消息存储 | `src/session_store/message_ops.rs` | ✅ `add_message()` / `get_messages()` / `delete_messages_before()` |
| 消息写入 | `src/engine/streaming.rs:1045,1394` | ✅ user 和 assistant 消息**已写入 DB** |
| 消息读取 | 无调用者 | ❌ `get_messages()` **从未被 conversation loop 使用** |
| Trace 存储 | `src/session_store/session_ops.rs` | ✅ `add_turn_trace()` 每轮写入 |
| Learning events | `src/engine/conversation_loop/turn_recording.rs` | ✅ `persist_turn_learning_event()` |

### 核心问题

```
写入路径: LLM 回复 → DB (streaming.rs)         ✅ 工作了
读取路径: 新一轮 → DB (get_messages) → 消息列表  ❌ 从未调用

实际流程:
  新一轮 → Vec::new() → 手动构建 messages → 传给 LLM
  
  而不是:
  新一轮 → get_messages(session_id) → 已有的 messages → 追加新消息 → 传给 LLM
```

消息虽然写入了 DB，但 conversation loop 完全依赖内存中的 `Vec<Message>`。进程重启后，DB 中有完整历史，但 agent 从零开始。

### 和 OpenCode 的差距

| | OpenCode | Priority Agent |
|---|---|---|
| 消息存储 | SQLite，事件溯源 | SQLite，直接 INSERT |
| 消息读取 | 每轮从 DB 加载 | 从内存 Vec 构建 |
| 压缩/裁剪 | 直接修改 DB 记录 | 修改内存 Vec，不写回 DB |
| 进程重启 | 完整恢复对话 | 丢失所有对话 |
| token 计数 | 每消息记录 input/output/reasoning/cache | 不记录 |
| 消息 schema | 结构化 parts（text/tool/reasoning/compaction） | 简单 role+content+tool_calls |

---

## 2. 改进方案

### 目标

不是重写，是对齐现有能力——把"已写入但未读取"的 DB 接入 conversation loop。

### Phase 1: 对话历史从 DB 恢复

**问题**: `get_messages()` 存在但从不被 conversation loop 调用。

**改动**: 在 `session_processor.rs` 或 `streaming.rs` 中，每轮开始时从 DB 加载已有消息，而不是从空 Vec 开始。

```rust
// 当前 (streaming.rs 或 conversation loop):
let messages: Vec<Message> = Vec::new();  // 从零开始

// 改为:
let messages = store.get_messages(&session_id)?;  // 从 DB 恢复
// 追加新消息到 messages
```

### Phase 2: 压缩结果写回 DB

**问题**: `message_compression.rs` 的 `selectively_compress_tool_outputs()` 修改内存 Vec，不写 DB。下次从 DB 恢复时压缩丢失。

**改动**: 在压缩后调用 `store.update_tool_content(message_id, compressed)` 或 `delete_messages_before()` 写回 DB。

### Phase 3: Token 计数持久化

**问题**: 没有每条消息的 token 计数。`ContextTokenBreakdown` 只记 trace event。

**改动**: 在 `add_message()` 时记录 `tokens_input` / `tokens_output` / `tokens_cache_read` 字段（message 表已有扩展能力）。

### Phase 4: 消息 parts 结构化

**问题**: 当前 `messages` 表是扁平的 role+content+tool_calls。OpenCode 用 parts 数组（text/tool/reasoning/compaction）组装消息。

**改动**: 不着急。当前 schema 够用——先把读写闭环做好。parts 是锦上添花。

---

## 3. 改动文件

| 文件 | 改动 |
|------|------|
| `src/session_store/message_ops.rs` | `add_message()` 增加 tokens 字段 |
| `src/session_store/mod.rs` | 无需大改，已有能力 |
| `src/engine/conversation_loop/` | 使用 `get_messages()` 恢复历史 |
| `src/engine/message_compression.rs` | 压缩后调用 `delete_messages_before()` |
| `src/engine/context_compressor/` | 压缩后通过 session_store 持久化 compact boundary |

---

## 4. 验证

| 验证项 | 方法 |
|--------|------|
| 进程重启后恢复对话 | 启动 agent，确认历史消息可见 |
| 压缩后 DB 状态一致 | 压缩前消息数 > 压缩后，但 closeout evidence 保留 |
| 多轮 token 不爆炸 | DB 中旧消息已被裁剪 |

---

## 5. 总结

不是"我们没有消息数据库"——是**有但没用**。`SessionStore` 已经有完整的 SQLite 消息接口，但 conversation loop 从未用 `get_messages()` 恢复历史。需要的不是重写存储层，而是把"已写入"和"未读取"之间的断头路接上。
