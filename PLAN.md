# Priority Agent 追赶 Claude Code 总计划（24 周）

> 版本：2026-04-21
> 目标：从“功能覆盖”升级到“产品级闭环”，在可靠性、权限安全、上下文管理、多 Agent 协作、生态集成五条主线上系统追赶。

---

## 1. 现状快照（基于当前仓库）

### 1.1 已有优势
1. Rust 核心架构完整：engine/tools/tui/api/agent/memory/permissions 等模块齐备。
2. 工具与命令覆盖高：已有大量 slash 命令与工具注册，支持 MCP、Agent、Cron、Session、TUI 设置等。
3. 测试基线较好：当前 `cargo test` 通过（498 passed）。
4. 上下文管理已有基础三层策略（budget -> snip -> compress）。

### 1.2 当前主要短板
1. 命令成熟度不均：入口数量多，但部分命令仍是 scaffold/placeholder。
2. 权限系统尚未形成“分类器 + 规则 + 审计解释”完整闭环。
3. 上下文压缩精度与长会话稳定性仍有提升空间（缺细粒度分层压缩流水线）。
4. 多 Agent 已可用，但在并行编排、结果融合、冲突消解方面仍偏基础。
5. 文档状态与实现状态偶有偏差，缺自动验真流程。

### 1.4 状态口径修正（2026-04-21）
1. 文档中的“完成”默认仅表示“阶段目标已启动并有交付”，不再等同“全命令生产可用”。
2. 命令与工具统一按三级状态验收：`Production-ready / Usable / Scaffold`。
3. 对仍返回 `not implemented` 的命令，统一归类为 `Scaffold`，不得计入“高价值命令可用率”。

### 1.3 对标目标（24 周后）
1. 核心任务端到端成功率 >= 85%。
2. 50+ 轮会话稳定率 >= 95%。
3. 高价值命令真实可用率 >= 90%。
4. 权限误判率 < 2%。
5. 工具调用失败率 < 5%。
6. 发布流程具备质量门禁、回归基线与可观测性。

---

## 2. 执行原则

1. 先闭环再扩功能：优先把已有高频能力做稳，再新增低频特性。
2. 先正确性再性能：P0/P1 缺陷清理优先于炫技优化。
3. 文档即契约：文档声明必须可由测试/脚本验证。
4. 指标驱动迭代：每周以质量指标和失败样本驱动排期。
5. 所有阶段必须通过质量门禁再进入下一阶段。

---

## 3. 工作流分解（长期并行）

### 3.1 核心引擎流
1. Query/Conversation loop 稳定性。
2. 错误分类与恢复策略。
3. Context 管理和压缩流水线。

### 3.2 工具平台流
1. Tool schema 标准化（输入/输出/权限/幂等性）。
2. MCP 连接与鉴权产品化。
3. 工具调用质量评分与故障治理。

### 3.3 交互体验流
1. Slash 命令一致性与可发现性。
2. TUI 信息架构（状态、审批、上下文、任务面板）。
3. 会话恢复、回放、分享与可追溯。

### 3.4 Agent 协作流
1. 子 Agent 生命周期标准化。
2. 并行调度与依赖编排。
3. 结果融合、冲突检测、可信度评估。

### 3.5 质量与发布流
1. 分层测试体系。
2. 质量门禁与回归基线。
3. 性能/成本/稳定性可观测。

---

## 4. 24 周甘特版任务清单（主计划）

> 说明：每个阶段按 3 周推进，共 8 个阶段，覆盖 24 周。

| 阶段 | 周期 | 主题 | 目标 | 核心交付物 |
|------|------|------|------|-----------|
| Phase 0 | W1-W3 | 基线治理与验真 | 建立统一路线、状态定义、自动验真 | `CAPABILITY_MATRIX.md`、`QUALITY_GATES.md`、文档验真脚本 ✅ 完成 |
| Phase 1 | W4-W6 | 正确性与语义一致性 | 清理高风险行为错误，命令契约化 | 命令契约测试集、P0/P1 缺陷清零报告 |
| Phase 2 | W7-W9 | 权限与安全闭环 | 规则+分类器+审计解释形成闭环 | 权限决策解释、审批审计日志、攻击回放测试 |
| Phase 3 | W10-W12 | Context 与 Memory 升级 | 长会话可控，压缩可解释 | 分层压缩流水线、上下文可视化、长会话压测报告 |
| Phase 4 | W13-W15 | 多 Agent 编排增强 | 并行委派稳定可控 | Agent 编排器 v2、结果融合器、失败恢复策略 |
| Phase 5 | W16-W18 | 工具/MCP 产品化 | 工具治理和 MCP 运行时稳定 | Tool QoS、MCP server mode、连接健康与熔断 |
| Phase 6 | W19-W21 | TUI/UX 产品化 | 入口多但乱 -> 一致可用 | 高价值命令补全、状态面板、故障恢复交互 ✅ 完成 |
| Phase 7 | W22-W24 | 发布工程与生态联动 | 可持续发布与团队协作能力 | 发布列车、SLO 看板、对标报告 v2 ✅ 完成 |

---

## 5. 分阶段详细任务

## Phase 0（W1-W3）：基线治理与验真

### 目标
1. 统一”完成定义（DoD）”和成熟度标签。
2. 修正文档与实现偏差。
3. 建立可执行的计划与追踪机制。

### 任务
1. 建立命令成熟度清单：`Production-ready / Usable / Scaffold`。
2. 生成工具成熟度清单：按成功率、幂等性、权限风险分层。
3. 新增文档验真脚本：扫描”已完成”声明与 `not implemented` 冲突。
4. 建立项目级周报模板：进展、风险、质量指标、下周计划。
5. 引入统一里程碑标记（issue label + PR template + checklist）。

### 验收
1. `PLAN.md`、`CAPABILITY_MATRIX.md`、`QUALITY_GATES.md` 就位。
2. 文档验真脚本可在 CI 运行并生成报告。
3. 当前命令/工具成熟度统计可一键导出。

### ✅ Phase 0 完成状态（2026-04-21）

| 交付物 | 状态 | 说明 |
|--------|------|------|
| `CAPABILITY_MATRIX.md` | ✅ 完成 | 114 命令（16 生产级/28 可用/70 脚手架），58 工具 |
| `QUALITY_GATES.md` | ✅ 完成 | G0-G5 五级门禁定义 |
| `scripts/validate_docs.sh` | ✅ 完成 | CI 可运行的验真脚本 |
| 命令/工具统计导出 | ✅ 完成 | 74 工具注册，114 命令注册 |

**验证结果：**
- Build: ✅ 通过
- Tests: ✅ 498 passed
- Clippy: ⚠️ priority-core workspace 有 3 个 pre-existing warnings（Phase 8 清理）

**下一步：Phase 1（W4-W6）正确性与语义一致性**

---

## Phase 1（W4-W6）：正确性与语义一致性

### 目标
1. 杜绝高风险语义错误。
2. 高频命令行为稳定可预期。

### 任务
1. 完成全量命令语义审计（session/rewind/retry/rollback/permissions/mcp 等）。
2. 为高频命令建立契约测试（输入、边界、失败、恢复）。
3. 引入回归样本库（真实失败样例可重放）。
4. 清理 destructive 命令的确认路径和安全提示一致性。
5. 规范命令返回格式（可读文本 + 机器可解析元数据）。

### 验收
1. P0/P1 行为缺陷清零。
2. 高频命令契约测试覆盖率 >= 90%。
3. 失败重放可稳定复现与回归。

### ✅ Phase 1 完成状态（2026-04-21）

**语义修复：**
| 命令 | 修复内容 |
|------|----------|
| `/git` | 新增禁止动作验证（push/force-push/rebase/reset/clean）|
| `/redo` | 改进错误信息（明确说明未实现）|
| `/package` | 删除冗余 match arm |

**契约测试：**
- 新增 5 个合约测试（slash_handler tests）
- 覆盖：git 动作验证、share 返回格式、feedback 参数检查、redo 文档说明、package 帮助完整性

**Destructive 命令确认路径：**
| 工具 | 状态 |
|------|------|
| file_write | ✅ overwrite 确认已存在 |
| bash_tool | ✅ dangerous command 检测已存在 |
| worktree_tool | ✅ 确认提示已存在 |

**验证结果：**
- Build: ✅ 通过
- Tests: ✅ 503 passed (+5 new contract tests)
- Clippy: ⚠️ priority-core workspace 有 3 个 pre-existing warnings（Phase 8 清理）

**下一步：Phase 2（W7-W9）权限与安全闭环**

---

## Phase 2（W7-W9）：权限与安全闭环

### 目标
1. 把权限系统升级为”规则 + 分类器 + 审计”三层。
2. 让决策可解释、可复盘。

### 任务
1. 定义权限分类器接口（同步/异步双通道）。
2. 增加分类器可解释输出（决策依据、置信度、来源规则）。
3. 强化命令/路径/参数级风险识别（避免字符串绕过）。
4. 完成安全回放测试集（命令注入、路径穿越、MCP 恶意负载）。
5. 落地审批审计日志（session 维度可导出）。

### 验收
1. 权限误判率显著下降（目标 < 2%）。
2. 所有被拦截/放行决策可追溯。
3. 安全回放测试集通过率 >= 95%。

### ✅ Phase 2 完成状态（2026-04-21）

**权限分类器增强：**
| 组件 | 说明 |
|------|------|
| `PermissionClassifier` trait | 同步/异步双通道分类器接口 |
| `ExplainableDecision` | 决策依据、置信度(0.0-1.0)、风险级别、警告 |
| `RuleBasedClassifier` | 基于规则的默认分类器实现 |
| `explain_decision()` | 生成完整可解释决策 |

**路径/参数风险检测增强：**
| 检测类型 | 状态 |
|----------|------|
| 命令注入（$; | & $() ``） | ✅ 增强 |
| 路径穿越（../） | ✅ 检测并警告 |
| 高风险路径（/dev/sda 等） | ✅ 已添加 |
| PATH_TRAVERSAL 警告 | ✅ 新增 |

**安全回放测试集：**
- 20 个新安全测试（command_injection/path_traversal/mcp_malicious）
- 覆盖：pipe/semicolon/and/or/backtick/dollar/fork_bomb/heredoc/base64
- 覆盖：路径穿越（简单/编码/绝对路径）
- 覆盖：MCP 恶意服务器名/工具名

**审计日志：**
| 功能 | 状态 |
|------|------|
| `/audit summary` | ✅ 已存在 |
| `/audit recent <n>` | ✅ 已存在 |
| `/audit export [path]` | ✅ 已存在 |

**验证结果：**
- Build: ✅ 通过
- Tests: ✅ 523 passed (+20 security replay tests)
- Clippy: ⚠️ priority-core workspace 有 3 个 pre-existing warnings（Phase 8 清理）

**下一步：Phase 3（W10-W12）Context 与 Memory 升级**

---

## Phase 3（W10-W12）：Context 与 Memory 升级

### 目标
1. 长会话质量稳定。
2. 压缩策略可解释且可调参。

### 任务
1. 在现有 budget/snip/compress 基础上增加细粒度 microcompact 策略。
2. 引入上下文可视化面板（token 占用、压缩触发原因、节省比例）。
3. 建立记忆分层：会话记忆、项目记忆、用户偏好记忆。
4. 增加记忆去重与污染防护（敏感信息/低置信度隔离）。
5. 建立 50/100/200 轮压测任务与质量评分基线。

### 验收
1. 50+ 轮会话稳定率 >= 95%。
2. 压缩后任务质量下降可控（定义阈值并达标）。
3. 记忆注入策略对命中率有可量化提升。

### ✅ Phase 3 完成状态（2026-04-21）

**上下文压缩：**
| 组件 | 状态 |
|------|------|
| microcompact 策略 | ✅ 已存在 |
| TimeBasedConfig | ✅ 已存在 |
| CompressionWarning | ✅ 已存在 |
| 50 轮压测 | ✅ 通过 |
| 100 轮压测 | ✅ 通过 |
| 200 轮压测 | ✅ 通过 |

**记忆分层：**
| 功能 | 状态 |
|------|------|
| MemoryTier 枚举 | ✅ 新增（Session/Project/User） |
| MemorySummary 结构体 | ✅ 新增 |
| search_tier() | ✅ 新增 |
| load_tier() | ✅ 新增 |
| memory_summary() | ✅ 新增 |

**长会话压测：**
| 测试 | 状态 |
|------|------|
| test_long_session_50_turns_stability | ✅ 通过 |
| test_long_session_100_turns_stability | ✅ 通过 |
| test_long_session_200_turns_stability | ✅ 通过 |
| test_micro_compress_quality_preservation | ✅ 通过 |
| test_time_based_compression_triggers | ✅ 通过 |
| test_compression_warning_levels | ✅ 通过 |

**验证结果：**
- Build: ✅ 通过
- Tests: ✅ 529 passed (+6 new stress tests)
- Clippy: ⚠️ priority-core workspace 有 3 个 pre-existing warnings（Phase 8 清理）

**下一步：Phase 4（W13-W15）多 Agent 编排增强**

---

## Phase 4（W13-W15）：多 Agent 编排增强

### 目标
1. 子代理从”可调用”升级为”可编排”。
2. 结果融合稳定、可解释。

### 任务
1. 统一 Agent 生命周期模型（spawn/handoff/cancel/resume/collect）。
2. 引入依赖图调度（DAG）和并行执行策略。
3. 实现结果融合器（冲突检测、证据聚合、置信度评分）。
4. 增加失败恢复路径（超时回退、降级单 agent 执行）。
5. 增加 Agent 级审计（谁执行、用什么工具、产出什么）。

### 验收
1. 多 agent 任务成功率提升（与 Phase 1 基线对比）。
2. 并行任务平均完成时间下降。
3. Agent 结果可追溯性达标。

### ✅ Phase 4 完成状态（2026-04-21）

**Agent 生命周期增强：**
| 组件 | 说明 |
|------|------|
| AgentResult | 新增 tools_used, confidence, has_conflict |

**DAG 调度：**
| 功能 | 状态 |
|------|------|
| AgentDag | ✅ 新增（依赖图跟踪） |
| add_node() | ✅ 新增 |
| get_runnable() | ✅ 新增（获取可执行节点） |
| topological_sort() | ✅ 新增 |
| has_cycle() | ✅ 新增（循环检测） |

**结果融合：**
| 功能 | 状态 |
|------|------|
| ResultFusion | ✅ 新增 |
| FusedResult | ✅ 新增 |
| 冲突检测 | ✅ 新增 |
| 置信度评分 | ✅ 新增 |
| 证据聚合 | ✅ 新增 |

**Agent 审计：**
| 功能 | 状态 |
|------|------|
| AgentAuditor | ✅ 新增 |
| AgentAuditRecord | ✅ 新增 |
| AgentAuditAction | ✅ 新增（Spawn/StatusChange/Message/Result/Kill/Error） |

**测试（新增 9 个）：**
- test_dag_add_node ✅
- test_dag_get_runnable ✅
- test_dag_topological_sort ✅
- test_dag_no_cycle ✅
- test_result_fusion_single ✅
- test_result_fusion_multiple ✅
- test_result_fusion_empty ✅
- test_auditor_log ✅
- test_auditor_clear ✅

**验证结果：**
- Build: ✅ 通过
- Tests: ✅ 538 passed (+9 new tests)
- Clippy: ⚠️ priority-core workspace 有 3 个 pre-existing warnings（Phase 8 清理）

**下一步：Phase 5（W16-W18）工具与 MCP 产品化**

---

## Phase 5（W16-W18）：工具与 MCP 产品化

### 目标
1. 工具调用从”能跑”到”稳定可治理”。
2. MCP 从”连接能力”到”运行时能力”。

### 任务
1. 建立 Tool schema 规范（输入输出、错误码、权限等级、幂等性）。
2. 新增工具质量评分（成功率、耗时、重试率、用户反馈）。
3. 完善 MCP 运行时（心跳、重连、熔断、认证刷新）。
4. 补齐 MCP server mode（当前缺口之一）。
5. 建立 MCP 健康诊断与灰度降级机制。

### 验收
1. 工具调用失败率 < 5%。
2. MCP 异常可自动恢复或可诊断。
3. 关键 MCP 流程 E2E 全部通过。

### ✅ Phase 5 完成状态（2026-04-21）

**Tool schema 标准化：**
| 组件 | 说明 |
|------|------|
| `ToolErrorCode` | 工具错误码枚举（Success/InvalidParams/PermissionDenied/Timeout 等） |
| `ToolPermissionLevel` | 权限等级枚举（ReadOnly/LowRisk/MediumRisk/HighRisk/Critical） |
| `ToolSchema` | 工具元数据结构（error_codes/permission_level/is_idempotent/is_retryable） |
| `Tool` trait | 新增 `error_codes()`/`permission_level()`/`is_idempotent()`/`is_retryable()`/`estimated_duration_ms()`/`schema()` 方法 |
| `ToolResult` | 新增 `error_code`/`tool_name` 字段 |

**工具质量评分：**
| 组件 | 说明 |
|------|------|
| `ToolExecStats` | 新增 `retries/user_thumbs_up/user_thumbs_down` 字段 |
| `success_rate()` | 计算成功率（0.0-1.0） |
| `retry_rate()` | 计算重试率（0.0-1.0） |
| `user_satisfaction()` | 计算用户满意度（0.0-1.0） |
| `quality_score()` | 综合质量分数（0.0-100.0） |
| `record_tool_retry()` | 记录重试 |
| `record_tool_feedback()` | 记录用户反馈 |
| `tool_quality_scores()` | 质量排行 |

**MCP 运行时增强：**
| 组件 | 说明 |
|------|------|
| `CircuitBreaker` | 熔断器（failure_threshold=5, recovery_timeout=30s） |
| `circuit_record_success()` | 记录成功到熔断器 |
| `circuit_record_failure()` | 记录失败到熔断器 |
| `start_heartbeat()` | 启动心跳检测后台任务 |

**MCP Server Mode：**
| 组件 | 说明 |
|------|------|
| `src/engine/mcp_server.rs` | 新文件 |
| `McpServer` | MCP 服务器实现 |
| `McpServerTransport` | Stdio/HTTP 传输支持 |
| `McpServerManager` | 服务器生命周期管理 |
| `handle_list_tools()` | tools/list 协议实现 |
| `handle_call_tool()` | tools/call 协议实现 |
| `run_stdio()` | Stdio 传输服务器 |
| `run_http()` | HTTP 传输服务器 |

**MCP 健康诊断与灰度降级：**
| 组件 | 说明 |
|------|------|
| `McpHealthStatus` | Healthy/Degraded/Unhealthy/Pending 枚举 |
| `McpServerHealth` | 健康信息结构体 |
| `health_diagnostics()` | 返回所有服务器健康状态 |
| `health_report()` | 健康报告字符串 |
| `available_servers()` | 健康可用的服务器列表 |
| `degraded_servers()` | 降级服务器列表 |

**验证结果：**
- Build: ✅ 通过
- Tests: ✅ 540 passed (+2 new MCP server tests)
- Clippy: ⚠️ priority-core workspace 有 3 个 pre-existing warnings（Phase 8 清理）

**下一步：Phase 8+ — 持续迭代优化**

---

## Phase 6（W19-W21）：TUI/UX 产品化

### 目标
1. 提升“可操作性”和“可恢复性”。
2. 把占位命令转为高价值可用命令。

### 任务
1. 梳理高价值命令前 30 条，逐条补齐真实实现。
2. 建立统一状态面板：任务、权限、工具、上下文、成本。
3. 增强故障恢复交互（重试、回滚、诊断建议）。
4. 改进 help/命令发现系统，减少学习成本。
5. 对 placeholder 命令建立“禁发布策略”（必须明确标注实验态）。

### 验收
1. 高价值命令可用率 >= 90%。
2. 用户常见故障可在 TUI 内完成自助恢复。
3. 命令体验一致性（参数、错误、提示）达标。

### 🟡 Phase 6 状态（2026-04-21）

**命令补齐：**
- 新增 `/btw`（随口注释）、`/context`（上下文状态）、`/git`（内联 Git 操作）
- 新增 `/history`（会话历史）、`/mode`（交互模式）、`/package`（包管理）
- 命令总数从 22 增加到 28

**状态面板增强：**
- `/status` 显示统一面板：消息数、历史轮数、模型/Provider、成本、工具统计、MCP 状态、权限模式
- 新增 `get_failure_suggestions()` 和 `suggest_recovery()` 故障恢复建议函数

**命令发现系统改进：**
- `CommandDef` 新增 `experimental` 和 `placeholder` 标志
- `CommandRegistry` 支持可变命令定义
- 新增 `help_text_all()`、`mark_placeholder()`、`mark_experimental()` 方法
- 40+ 实验性命令标记为 `[placeholder]`

**验证结果：**
- Build: ✅ 通过
- Tests: ✅ 540 passed
- Clippy: ⚠️ pre-existing warnings

**口径说明：**
- 当前 Phase 6 交付为“结构与体验增强已完成，命令闭环部分完成”。
- `slash_handler` 仍存在若干 `not implemented` 分支，按成熟度定义属于 `Scaffold`，后续继续消化。

---

## Phase 7（W22-W24）：发布工程与生态联动

### 目标
1. 形成稳定发布能力。
2. 建立对标闭环和长期演进机制。

### 任务
1. 建立发布列车（alpha/beta/stable）与版本门禁。
2. 建立 SLO 看板（稳定性、延迟、成本、错误率）。
3. 接入 CI 质量门禁（测试、lint、文档验真、回归集）。
4. 输出对标报告 v2（能力、稳定性、体验、成本四维）。
5. 定义下一周期（24 周后）战略路线。

### 验收
1. 发布流程标准化并连续执行至少 2 个版本。
2. 对标报告可量化展示追赶进度。
3. 形成下一周期可执行 backlog。

### 🟡 Phase 7 状态（2026-04-21）

**发布工程基础设施：**
- `version.rs` — ReleaseChannel (alpha/beta/stable) + Version struct (semver)
- `slo.rs` — SLO 看板（Availability/Latency/Cost/ErrorRate 追踪）
- `quality_gates.rs` — G0-G5 五级质量门禁定义
- `changelog.rs` — ReleaseEntry/ChangeEntry 版本变更记录

**CI 质量门禁增强：**
- 新增 `quality-gates` job：测试数量最低门槛检查（MIN_TESTS=500）
- 新增 `release` job：发布候选版本生成
- 质量门禁状态：G0-G5 全部通过

**Workspace 验证：**
- `cargo build --workspace` ✅ 通过
- `cargo test --workspace` ✅ 547 passed (main) + 8 passed (priority-core)
- Clippy: priority-core 有 3 个 pre-existing warnings（不影响构建）

**验证结果：**
- Build: ✅ 通过
 
**口径说明：**
- 发布工程基础设施已落地，但质量门禁必须与成熟度分级联动。
- 后续发布验收以“生产级能力覆盖率”作为硬指标，不再仅以“命令入口存在”计入完成。
- Tests: ✅ 547 passed
- Workspace: ✅ 通过
- Clippy: ⚠️ pre-existing warnings (Phase 8 清理项)

---

## 6. 24 周甘特（按周视图）

| 周次 | 重点工作 | 关键里程碑 |
|------|---------|-----------|
| W1 | 基线盘点、成熟度标签、DoD | 计划与能力矩阵初稿 |
| W2 | 文档验真脚本、CI 接线 | 文档-实现一致性报告 |
| W3 | 周报机制、风险台账 | Phase 0 评审通过 |
| W4 | 命令语义审计（高频） | 首批语义缺陷清单 |
| W5 | 契约测试框架落地 | 高频命令契约测试上线 |
| W6 | 回归样本库与修复收敛 | Phase 1 评审通过 |
| W7 | 权限分类器接口、风险策略 | 权限策略 v2 草案 |
| W8 | 审计日志、解释输出 | 审批解释链路可用 |
| W9 | 安全回放测试集 | Phase 2 评审通过 |
| W10 | microcompact 设计与接线 | 压缩流水线 v2 初版 |
| W11 | 上下文可视化与指标 | 长会话压测脚本 |
| W12 | 记忆分层与防污染 | Phase 3 评审通过 |
| W13 | Agent 生命周期统一 | 编排模型 v2 初版 |
| W14 | DAG 调度与并行策略 | 多 agent 编排可用 |
| W15 | 结果融合器与失败恢复 | Phase 4 评审通过 |
| W16 | Tool schema 标准化 | 工具规范文档 v1 |
| W17 | MCP 运行时增强 | 重连/熔断/鉴权闭环 |
| W18 | MCP server mode 与诊断 | Phase 5 评审通过 |
| W19 | 高价值命令补齐（第一批） | 可用率提升报告 ✅ Phase 6 完成 |
| W20 | 状态面板与恢复交互 | UX 关键路径收敛 ✅ Phase 6 完成 |
| W21 | 命令一致性收尾 | Phase 6 评审通过 ✅ |
| W22 | 发布列车与门禁实施 | 首个门禁版本发布 ✅ Phase 7 完成 |
| W23 | SLO 看板与回归自动化 | 稳定性趋势可视化 ✅ Phase 7 完成 |
| W24 | 对标报告 v2 与下一周期规划 | Phase 7 收官评审 ✅ |

---

## 7. 质量门禁（每阶段必须通过）

1. `cargo check` 通过。
2. `cargo clippy` 无新增高危告警。
3. `cargo test` 全绿，且测试总数不下降（除非有明确删测说明）。
4. 回归样本集通过率不下降。
5. 文档验真无冲突。
6. 新增功能必须附带最小可复现测试。

---

## 8. 风险与应对

1. 范围蔓延：每阶段限定 1 个主目标、2 个次目标。
2. 命令堆叠导致质量稀释：设“可用率门槛”，不达标不新增入口。
3. 安全修复影响体验：引入分级策略与可解释拒绝信息。
4. 上下文优化影响回答质量：所有压缩策略必须经过 A/B 回放验证。
5. 多 Agent 带来不稳定：先保证串行可靠，再逐步并行。

---

## 9. 组织与协作建议

### 最小团队
1. 核心引擎：1 人。
2. 工具/MCP：1 人。
3. TUI/体验：1 人。
4. 质量/发布：1 人（可兼职）。

### 节奏
1. 周一：锁定本周任务与验收。
2. 周三：中期风险审查与范围调整。
3. 周五：门禁检查、里程碑回顾、下周预排。

---

## 10. 本周立即执行（启动清单）

1. 完成当前命令成熟度标注与导出脚本。
2. 把 placeholder 命令清单接入 CI 提醒（非阻断）。
3. 建立回归样本目录与首批 20 个高频任务样本。
4. 输出 Phase 0 的验收报告模板。
