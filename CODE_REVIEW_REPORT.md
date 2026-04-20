# Priority Agent — 全面代码审查 & 架构评估报告

Date: 2026-04-13
Reviewer: Hermes Agent (Nous Research)
Project: ~/Desktop/rust-agent/ (27,968 LOC, 101 源文件, 185 tests passing)

================================================================================
## 1. 代码审查报告（按 P0/P1/P2 分级）

================================================================================
### P0 — 必须立即修复

[P0-1] READ_ONLY_TOOLS 定义不一致（逻辑漏洞）
  文件: src/engine/conversation_loop.rs
  问题: 第 22-25 行 READ_ONLY_TOOLS 常量包含 "web_search"，
        但第 439-441 行 is_read_only() 函数不包含 "web_search"。
        这导致：
        - 迭代预算退还逻辑使用 READ_ONLY_TOOLS（包含 web_search）
        - 并行执行逻辑使用 is_read_only()（不包含 web_search）
        - web_search 工具会被串行执行而非并行，且会消耗迭代预算
  修复: 统一两处定义，建议删除 is_read_only()，全部引用 READ_ONLY_TOOLS。

[P0-2] tool_results_text 构建后未被使用
  文件: src/engine/conversation_loop.rs 第 252-261 行
  问题: tool_results_text 变量在循环中拼接所有工具结果，
        但在第 275 行传给 sync_turn() 后，再无其他用途。
        而 messages 已经包含了所有 tool result。
        这不是严重 bug，但 tool_results_text 与 messages 中的
        Tool 消息内容重复，sync_turn 接收的参数格式可能不匹配预期。

[P0-3] compressor lock 在循环中反复获取/释放
  文件: src/engine/conversation_loop.rs 第 140-152 行
  问题: preflight 循环中先 lock → check → drop → 再 lock → compress。
        这是正确做法（避免持有锁做异步操作），但两次 lock 之间
        没有原子性保证，理论上另一个并发调用可能在中间插入。
        实际上 ConversationLoop 是单线程顺序执行的，所以影响不大，
        但设计上应添加注释说明。

================================================================================
### P1 — 应尽快修复

[P1-1] 大量 #[allow(dead_code)] — 84 处
  文件: src/main.rs (9处), src/engine/mod.rs (11处), 遍布全项目
  问题: 84 处 #[allow(dead_code)] 表明大量模块虽然编译但未被实际使用。
        包括：agent, errors, cost_tracker, github, memory, permissions,
        session_store, skills, state 等。
        风险：代码腐烂 — 未使用的代码不会被测试覆盖，容易积累 bug。
  建议: 清理或标记为 Phase N 待集成，用 feature gate 控制。

[P1-2] 大量 unwrap() 在非测试代码中（59处）
  文件: src/engine/swarm.rs, src/engine/mcp.rs, src/tools/cache.rs 等
  问题: swarm.rs 中多处 .unwrap() 如 map.get(&session_key).unwrap()，
        如果 key 不存在会 panic。cache.rs 中大量 .expect("Cache lock poisoned")。
  建议: 对 swarm/mcp 中的 unwrap 改用 ? 或 if let，
        cache 中的 expect 可以保留（lock poisoning 确实是严重错误）。

[P1-3] default_system_prompt 重复定义
  文件: src/engine/query_engine.rs:201, src/engine/streaming.rs:324
  问题: 完全相同的函数在两个文件中各定义一次。
        如果需要修改 prompt，必须同时改两处，容易遗漏。
  建议: 提取到公共模块（如 engine/system_prompt.rs）。

[P1-4] QueryEngine 和 StreamingQueryEngine 功能重叠
  文件: src/engine/query_engine.rs, src/engine/streaming.rs
  问题: 两个引擎都持有 provider + tool_registry + cost_tracker，
        都有 with_model / with_max_iterations / with_agent_manager 等方法。
        StreamingQueryEngine 内部的 StreamingEngineInner 再次持有相同字段。
        大量重复代码。
  建议: QueryEngine 只保留非流式简单查询，流式/工具调用全部走
        StreamingQueryEngine（或 ConversationLoop）。

[P1-5] MemoryManager 中的同步文件 I/O
  文件: src/memory/manager.rs
  问题: add_learning() 使用 std::fs::write() 同步写文件，
        在 tokio 异步上下文中可能阻塞线程。
        freeze_snapshot() 使用 std::fs::read_to_string() 同步读。
  建议: 改用 tokio::fs 异步版本，或用 spawn_blocking 包裹。

[P1-6] 记忆预取搜索算法过于简单
  文件: src/memory/manager.rs 第 209-268 行
  问题: extract_keywords 只做停用词过滤，search_memory 按段落关键词匹配。
        无语义理解能力，对于 "authentication" 和 "登录" 不会关联。
        关键词过滤按长度 >= 2，会丢失很多有意义的短词。
  建议: 至少添加同义词表，或者与外部 embedding 服务集成。

================================================================================
### P2 — 建议改进

[P2-1] Token 估算过于粗略
  文件: src/engine/context_compressor.rs 第 134 行
  问题: estimate_tokens 使用 (len + 3) / 4，即 4 字符 ≈ 1 token。
        对 CJK 文字、代码、JSON 等估算偏差大。
  建议: 集成 tiktoken 或用更精确的估算公式。

[P2-2] summarize_middle 使用启发式而非 LLM
  文件: src/engine/context_compressor.rs 第 735-813 行
  问题: 上下文压缩的摘要生成只做启发式提取（取第一句用户消息），
        而非调用 LLM 生成真正有意义的摘要。
        注释和模板都提到 "给 LLM 的压缩 prompt"，但实际没用 LLM。
  建议: 实现 LLM 摘要路径（可以是可选的 fallback 模式）。

[P2-3] Compress 函数中的 head 分离可能吞掉非 system 消息
  文件: src/engine/context_compressor.rs 第 634-642 行
  问题: split_head 将所有 Message::System 放入 head。
        但如果用户手动插入了 User 消息在 System 前面，
        那个 User 消息也会被归入 rest。
        实际上这不太可能发生，但边界 case 没处理。

[P2-4] ToolContext 每次工具调用都新建
  文件: src/engine/conversation_loop.rs 第 97-110 行, 457-458 行
  问题: execute_tools_parallel 中每次工具调用都调用 create_tool_context()
        创建新的 ToolContext。创建过程包含 uuid 生成、HashMap 初始化等。
        并行工具调用时创建 N 个 context。
  建议: 在循环开始前创建一个 context 并 clone。

[P2-5] Session 相关功能与 TUI 紧耦合
  文件: src/engine/streaming.rs
  问题: StreamingQueryEngine 直接持有 session_store + session_id，
        但 ConversationLoop 不持有。两个入口的 session 管理不一致。
  建议: session 管理提升到更上层或统一注入。

================================================================================
## 2. 第一性原理分析

================================================================================
### 2.1 Agent 的本质 = "感知 → 思考 → 行动" 循环

评估: 7/10

当前架构支持度:
+ 感知: 通过 25 个工具（file_read, grep, web_fetch 等）收集信息 ✓
+ 思考: Socratic 引擎 + LLM 推理 ✓
+ 行动: 工具执行（bash, file_write 等）+ 并行优化 ✓
- 反馈循环: 工具结果直接送回 LLM，无中间质量检查 ✗
- 自我校正: 无错误检测 → 重试的自动机制，依赖 LLM 自身判断 ✗

建议:
1. 添加 tool result 质量评估层 — 如果工具返回错误，自动触发修复
2. 实现 "observe → plan → act → verify" 四步循环，而非当前的三步
3. 引入 execution trace — 记录每步的输入/输出，供后续推理参考

================================================================================
### 2.2 上下文的本质 = "有限窗口内的信息压缩"

评估: 6/10

当前架构支持度:
+ 3 层压缩: ToolResultBudget → Snip → Compress ✓
+ 8 段结构化摘要模板（Hermes 风格）✓
+ 迭代式摘要更新（累积而非丢失）✓
+ 工具调用对完整性校验（sanitize_tool_pairs）✓
+ Token-budget 尾部保护（soft_ceiling）✓
- 摘要生成是启发式的，不是 LLM 驱动的 ✗
- Token 估算精度不足（4 字符 ≈ 1 token）✗
- 无多粒度压缩 — 只有 "全部保留" 或 "摘要" 两级 ✗

建议:
1. 实现 LLM 摘要路径作为默认选项（启发式作为 fallback）
2. 引入分层摘要 — 对旧对话做粗粒度摘要，对近期做细粒度
3. 用 tiktoken 精确计数 token

================================================================================
### 2.3 工具的本质 = "扩展 Agent 能力的接口"

评估: 8/10

当前架构支持度:
+ Tool trait + ToolRegistry — 清晰的插件化架构 ✓
+ 25 个工具覆盖文件/搜索/网络/Agent/MCP 等 ✓
+ 并行执行（只读工具）+ 串行（读写工具）✓
+ 权限系统（AutoLowRisk 模式）✓
+ 缓存层（CachedToolExecutor）✓
- ToolContext 包含过多字段（agent_manager, llm_provider, mcp_manager...）✗
  这违反了接口隔离原则 — 大多数工具只需要 working_dir
- 无工具版本管理 — schema 变更可能导致兼容问题 ✗
- 无工具使用统计反馈循环 ✗

建议:
1. 拆分 ToolContext 为基础版（BaseToolContext）和扩展版
2. 添加工具版本字段到 Tool trait
3. 利用 cost_tracker 的统计做动态工具裁剪

================================================================================
### 2.4 记忆的本质 = "跨会话的知识持久化"

评估: 5/10

当前架构支持度:
+ 冻结快照 — 会话内一致性 ✓
+ 预取机制 — 注入相关记忆 ✓
+ XML 围栏包裹 — 防止记忆被模型误读 ✓
+ 会话结束批量提取 ✓
- 搜索基于关键词匹配，无语义理解 ✗
- 记忆存储是 markdown 文件，无结构化查询能力 ✗
- 无记忆衰减/遗忘机制 — 旧的不重要记忆不会被清理 ✗
- pending_learnings 的提取规则过于简单（关键词匹配）✗
- MEMORY.md 文件可能无限增长 ✗

建议:
1. 引入 embedding-based 搜索（如用本地 embedding 模型或 API）
2. 添加记忆优先级/时间衰减机制
3. 限制 MEMORY.md 大小 + 定期合并压缩
4. 结构化存储（SQLite）替代纯文本文件

================================================================================
## 3. 顶层设计评估

================================================================================
### 3.1 架构图（简化版）

```
main.rs
  ├── TUI 模式 ──→ StreamingQueryEngine ──→ ConversationLoop ──→ LLM Provider
  ├── API 模式 ──→ QueryEngine ──→ ConversationLoop ──→ LLM Provider
  └── Legacy 模式 ──→ QueryEngine (直接调用)

ConversationLoop (核心)
  ├── Preflight Compression (ContextCompressor)
  ├── Memory Fence Injection (MemoryManager)
  ├── Iteration Loop:
  │   ├── API Call (streaming or non-streaming)
  │   ├── Tool Execution (parallel read-only, serial read-write)
  │   ├── Iteration Budget Refund (read-only tools)
  │   └── Memory Sync (MemoryManager)
  └── Return LoopResult

工具系统:
  Tool trait → ToolRegistry → 25 个实现
  ├── 文件工具: file_read/write/edit
  ├── 搜索工具: glob, grep, project_list
  ├── 系统工具: bash
  ├── 高级工具: agent, swarm, socratic, mcp, cron
  └── 辅助工具: web, memory, todo, calculate, etc.
```

================================================================================
### 3.2 模块边界清晰度

评分: 6/10

优点:
- engine/ 聚集核心逻辑（query, streaming, compression, tools）
- services/api/ 抽象 LLM provider（LlmProvider trait）
- tools/ 模块化良好（每个工具独立文件）
- memory/ 独立模块

缺点:
- engine/ 内部有 12 个子模块，职责不够清晰
  - query_engine.rs 和 streaming.rs 功能大量重叠
  - conversation_loop.rs 作为新的统一入口，但旧的两个引擎仍存在
- main.rs 中有 887 行代码，包含大量 legacy CLI 逻辑
- 全项目 84 处 #[allow(dead_code)] — 模块边界不清晰

================================================================================
### 3.3 扩展性评估

评分: 7/10

添加新 Provider: 容易 (实现 LlmProvider trait)
添加新工具: 容易 (实现 Tool trait + register)
添加新功能模块: 中等 (需要修改多个地方)
添加新压缩策略: 中等 (ContextCompressor 设计合理但不支持插件化)

================================================================================
### 3.4 与 Hermes Agent / Claude Code 的架构差距

| 维度          | Claude Code        | Priority Agent      | 差距   |
|---------------|--------------------|--------------------|--------|
| 语言          | TypeScript         | Rust               | Rust 优势 |
| 工具数量      | ~20                | 25                 | PA 领先 |
| 上下文压缩    | LLM 摘要           | 启发式摘要          | 差距大 |
| 记忆系统      | 向量搜索           | 关键词匹配          | 差距大 |
| 流式支持      | 原生               | 实现但有重复代码     | 中等  |
| 错误恢复      | 完善 (retry/backoff)| 有 ErrorClassifier  | 中等  |
| 测试覆盖      | 高                 | 185 tests          | 中等  |
| 模块化        | 优秀               | 模块多但边界不清     | 中等  |
| 权重调度      | 无                 | 有 (差异化优势)     | PA 领先 |
| Socratic 推理 | 无                 | 有 (差异化优势)     | PA 领先 |

核心差距:
1. 上下文压缩质量 — Claude Code 用 LLM 生成高质量摘要，PA 用启发式
2. 记忆搜索精度 — Claude Code 用 embedding，PA 用关键词
3. 代码一致性 — PA 有大量重复代码和 dead code

核心优势:
1. 权重调度系统 — 独特差异化
2. Socratic 推理引擎 — 独特差异化
25 个工具 — 数量领先
Rust 性能 — 原生性能优势

================================================================================
### 3.5 商业化/开源评估

核心竞争力:
1. 权重优先级调度 — 市场上唯一显式权重驱动的 Agent
2. Socratic 深度推理 — 高密度思考优于直接执行
3. Rust 实现 — 性能和安全优势
4. 25 个工具生态 — 丰富的开箱即用能力

短板:
1. 上下文压缩质量不足（启发式 vs LLM）
2. 记忆系统能力有限（关键词 vs 向量）
3. 代码有 84 处 dead code 注解 — 给人 "半成品" 感觉
4. 缺少 LLM 摘要 — 这是 Agent 的核心能力之一
5. main.rs 过大（887 行含 legacy 代码）

================================================================================
## 4. 最终评分和 Top 5 建议

================================================================================
### 最终评分: 6.5 / 10

细分:
- 代码质量: 6/10 (有 bug, dead code 多, 重复代码)
- 架构设计: 7/10 (模块化不错但边界不清)
- 功能完整: 8/10 (25 个工具, 权重, Socratic, 流式)
- 测试覆盖: 6/10 (185 tests, 但核心模块测试偏少)
- 文档: 7/10 (AGENTS.md 很好)

================================================================================
### Top 5 建议

1. [P0] 修复 READ_ONLY_TOOLS 不一致 bug
   文件: src/engine/conversation_loop.rs
   删除 is_read_only() 函数，统一使用 READ_ONLY_TOOLS 常量。
   影响: 修复 web_search 工具的并行执行和迭代预算计算。

2. [P1] 清理 dead code 和重复代码
   - 移除 84 处 #[allow(dead_code)] 中真正不需要的模块
   - 将 default_system_prompt 提取到公共模块
   - 合并 QueryEngine 和 StreamingQueryEngine 的重复逻辑
   影响: 减少 ~2000 行无用代码，提升可维护性。

3. [P1] 实现 LLM 驱动的上下文压缩
   当前 summarize_middle 是启发式的，这是与 Claude Code 的最大差距。
   在 ContextCompressor 中添加可选的 LLM 摘要路径，
   使用 8 段模板 prompt 生成高质量摘要。
   影响: 从 6/10 提升到 8/10 的上下文管理能力。

4. [P2] 增强记忆系统
   - 用 tokio::fs 替换同步 I/O
   - 添加同义词/语义搜索（至少同义词表）
   - 添加记忆大小限制和衰减机制
   影响: 记忆系统从 5/10 提升到 7/10。

5. [P2] 拆分 main.rs + 统一引擎入口
   - 将 legacy CLI 移到独立 crate 或 feature gate 更严格的模块
   - 确保 TUI 模式和 API 模式共享同一套初始化逻辑
   - 将 main.rs 控制在 100 行以内
   影响: 项目结构更专业，降低新贡献者上手难度。

================================================================================
报告结束。
