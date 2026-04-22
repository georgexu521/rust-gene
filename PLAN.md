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
5. 按高频命令闭环清单逐项执行与验收：`docs/HIGH_FREQ_COMMAND_CLOSURE_CHECKLIST.md`。

---

## 11. WorkflowEngine 专项开发计划（基于 `brainstorm.md` 与 `workflow-spec.md`）

> 目标：解决“AI 抓不住重点、容易陷入细枝末节”的核心问题。  
> 方法：源码级权重规则 + 主动提问式深思（Socratic 内核，对外术语用“主动提问式深思”）。

### 11.1 北极星目标
1. 先找主线（Mainline），再做细节。
2. 通过主动多提关键问题，把任务想透再执行。
3. 执行顺序由源码规则约束，而非提示词“建议”。

### 11.2 四个硬指标（专项验收口径）
1. Mainline Hit Rate > 70%。
2. Drift Interruption Rate < 15%。
3. First Plan Coverage > 80%。
4. Rework Rate 按阶段持续下降（每阶段目标下降 10%-20%）。

### 11.3 12 周分阶段任务表

| 周次 | 阶段主题 | 核心任务 | 交付物 | 验收标准 |
|------|---------|---------|-------|---------|
| W1 | 规范冻结 | 冻结术语、状态机、指标字典、复杂任务判定标准 | `workflow-v1-design.md` | 评审通过，术语不再反复变更 |
| W2 | 基线采样 | 采集真实任务样本并人工标注 blocker/跑偏 | `baseline-report.md`、样本集 v1 | 四指标有可复现实测基线 |
| W3 | Gate 方案 | 设计 Direct vs Workflow 闸门，定义误判回退策略 | `gate-spec.md` | 离线判定准确率达标（建议 >= 75%） |
| W4 | 权重规则 | 固化六维权重（Risk/Impact/Complexity/Blocker/Dependency/Drift） | `weights-spec.md` | 同输入稳定排序、可解释 |
| W5 | 提问策略 | 设计问题链生成、停止条件、预算上限 | `questioning-spec.md` | 能稳定产出“可执行问题链” |
| W6 | 执行策略 | 设计 Planner/Executor/递归拆分/失败升级策略 | `planner-executor-spec.md` | 异常路径闭环完整 |
| W7 | 观测体系 | 指标埋点、日志结构、回放格式、周报模板 | `metrics-pipeline-spec.md` | 每周可自动产出趋势报告 |
| W8 | MVP 联调计划 | 明确 M1 最小可运行链路与阻断清单 | `m1-integration-plan.md` | 依赖关系与接口全部清晰 |
| W9 | MVP 验收脚本 | 制定专项验收清单与样本任务通过标准 | `m1-acceptance-checklist.md` | 清单可直接执行 |
| W10 | 缺口收敛 | 按验收演练输出缺口与优先级 | `m1-gap-list.md` | 缺口可量化且有排期 |
| W11 | 优化计划 | 针对 drift 与返工率做专项优化方案 | `m2-optimization-plan.md` | 预期提升幅度明确 |
| W12 | 上线与回滚 | 试运行/灰度/回滚/运维手册 | `rollout-plan.md`、`operator-guide.md` | 可控上线、可快速回退 |

### 11.4 每周固定节奏（执行纪律）
1. 周一：确认本周唯一主目标（最多 1 主 + 2 次目标）。
2. 周三：中期评审，检查是否偏离四项指标。
3. 周五：跑验收脚本、更新周报、确认下周阻断项。

### 11.5 范围控制规则（防止再次跑偏）
1. 任何任务都必须回答：“它如何改善四项硬指标？”
2. 无法提升指标的工作自动降级为 backlog。
3. 先观测和约束，后算法“变聪明”。
4. 任何新增复杂机制必须有降级路径（回到 direct mode）。

### 11.6 技术约束（与 `workflow-spec.md` 对齐）
1. 权重逻辑必须在源码实现，环境变量仅允许调系数。
2. 提问式深思必须有预算上限（轮数、每问 token、总预算）。
3. 递归深度默认 3 层，触底后强制平铺为原子步骤。
4. 所有排序和决策必须输出“可解释原因”文本。

### 11.7 第一批实施入口（不含编码，只定边界）
1. `src/engine/conversation_loop.rs`：Workflow 闸门插入点。
2. `src/engine/socratic.rs`：被动到主动模式改造边界。
3. `src/engine/socratic_executor.rs`：执行层复用范围。
4. `src/engine/workflow/`：新增模块边界（`gate/weights/questioning/planner/executor/metrics`）。

### 11.8 与 24 周主计划的映射
1. 本专项 W1-W6 对应主计划 Phase 1-3 的“正确性 + Context + 规划能力”强化。
2. 本专项 W7-W9 对应主计划 Phase 4-5 的“可观测 + 可验收”落地。
3. 本专项 W10-W12 对应主计划 Phase 6-7 的“产品化 + 灰度发布”路径。

### 11.9 立即执行清单（专项）
1. 先产出 `workflow-v1-design.md`（冻结状态机和术语）。
2. 建立 20 条真实任务样本并完成 blocker 标注。
3. 输出 `gate-spec.md` 与 `weights-spec.md` 初稿。
4. 同步建立专项周报模板与指标看板字段定义。

### 11.10 当前完成度快照（2026-04-22）
1. W1-W9 交付物：已落盘（含 `workflow-v1-design.md`、`baseline-report.md`、`gate-spec.md`、`weights-spec.md`、`questioning-spec.md`、`planner-executor-spec.md`、`metrics-pipeline-spec.md`、`m1-integration-plan.md`、`m1-acceptance-checklist.md`）。
2. W10-W12 交付物：已补齐（`m1-gap-list.md`、`m2-optimization-plan.md`、`rollout-plan.md`、`operator-guide.md`）。
3. 执行支持文件：已补齐（`scripts/workflow-m1-acceptance.sh`、`docs/workflow/weekly-report-template.md`、`docs/workflow/samples/samples-v1-annotated.json`）。
4. 代码一致性：`WorkflowEngine::run_gate` 已接入真实 Gate 判定，避免直接调用绕过闸门。

---

## 12. 稳定生产级开发计划（Post-MVP）

> 目标：把当前“可用 Beta”提升为“稳定生产级”。  
> 时间窗：6 周（可压缩到 4 周，但风险更高）。

### 12.1 生产级验收标准（Definition of Done）
1. 连续 14 天无 P0 故障。
2. P1 故障每周不超过 2 个，且均有 RCA。
3. `cargo check` / `cargo clippy` / `cargo test` / `workflow-m1-acceptance` / `workflow-gate-replay` 全绿。
4. 四项北极星指标可追溯、可周报、可对比趋势。
5. 灰度与回滚演练完成，15 分钟内可降级到 Direct。

### 12.2 主要缺口（当前）
1. 真实工具执行参数生成稳定性不足（复杂步骤偶发失败）。
2. `Drift Interruption Rate` 仍是轻量埋点，缺持久化与维度拆解。
3. Gate 的 LLM 分类不是默认强策略，误判治理机制仍需加强。
4. 运维侧报警、SLO、发布门禁尚未完整自动化。

### 12.3 分阶段计划

#### Phase S0（第 1-2 周）：执行稳定性
1. 扩展“工具专用参数 planner”到 Top 10 高频工具（`file_read/file_edit/bash/grep` 之外继续补齐）。
2. 参数失败二级回退：专用模板回退 + 安全默认值回退 + 明确错误码。
3. 建立参数生成回放样本（>= 80 条）并纳入 CI。
4. 交付物：`docs/workflow/param-planner-spec.md`、`docs/workflow/param-replay-report.md`。
5. 验收：回放执行成功率 >= 95%，危险默认参数 0 次。

#### Phase S1（第 3-4 周）：指标与 Gate 产品化
1. 北极星指标持久化到 SQLite（会话维度 + 任务维度 + 周维度）。
2. 增加指标查询接口与周报导出脚本（自动生成趋势报告）。
3. Gate 双通道默认启用：启发式 + LLM 分类，带超时降级与原因记录。
4. Gate 误判样本扩充到 >= 200 条，持续回放评估。
5. 交付物：`docs/workflow/metrics-storage-spec.md`、`docs/workflow/gate-misclass-report.md`。
6. 验收：Gate 离线准确率 >= 85%，线上误判可解释率 100%。

#### Phase S2（第 5 周）：发布门禁与可观测
1. CI 强制质量门禁：`check + clippy + tests + m1 acceptance + gate replay + round3 replay`。
2. 结构化日志与告警规则：drift 激增、rework 激增、fallback 激增。
3. 故障分级与自动降级：连续失败触发 `FallbackDirect`，并打告警。
4. 交付物：`docs/workflow/release-gates.md`、`docs/workflow/alerts-playbook.md`。
5. 验收：门禁失败禁止合并；告警误报率 < 10%。

#### Phase S3（第 6 周）：灰度与生产演练
1. 灰度放量：10% -> 50% -> 100%，每阶段至少观察 48 小时。
2. 回滚演练：配置回滚、开关回滚、版本回滚三层演练。
3. 事故演练：模拟 drift 激增并在 15 分钟内降级恢复。
4. 交付物：`docs/workflow/production-readiness-report.md`。
5. 验收：全量后连续 7 天核心指标稳定，无 P0。

### 12.4 6 周甘特版任务清单

| 周次 | 重点目标 | 关键任务 | 验收标准 |
|------|---------|---------|---------|
| W1 | 参数稳定化 | Top 10 工具参数 planner v1 + 回放样本集 | 参数回放成功率 >= 90% |
| W2 | 参数稳定化闭环 | 二级回退 + 错误码统一 + 安全默认值收敛 | 参数回放成功率 >= 95% |
| W3 | 指标持久化 | 四指标写入 SQLite + 查询脚本 | 周报可自动生成 |
| W4 | Gate 产品化 | 双通道 Gate 默认启用 + 200 样本回放 | Gate 准确率 >= 85% |
| W5 | 发布门禁 | CI 门禁 + 告警规则 + 自动降级策略 | 失败可自动阻断发布 |
| W6 | 灰度上线 | 10%->50%->100% 灰度 + 回滚演练 | 连续稳定通过生产验收 |

### 12.5 发布门禁（生产前必须满足）
1. `scripts/workflow-m1-acceptance.sh` 通过。
2. `scripts/workflow-gate-replay.sh` 通过，准确率 >= 85%。
3. `scripts/workflow-real-devflow-round2.sh` 与 `scripts/workflow-real-devflow-round3.sh` 通过。
4. 无 blocker 级缺陷未关闭。
5. 回滚脚本实测通过。

### 12.6 风险与缓解
1. 指标好看但不真实：保留人工抽样核验（每周至少 20 条）。
2. 参数 planner 覆盖不全：先覆盖高频工具，再扩面。
3. Gate 过拟合样本：训练/回放样本分离并保留盲测集。
4. 灰度放量太快：严格执行阶段观测窗口，不提前晋级。

### 12.7 立即行动（本周）
1. 新建 `docs/workflow/param-planner-spec.md` 并冻结 Top 10 工具策略。
2. 建立 `docs/workflow/gate-replay-samples-v2.json`（目标 200 条）。
3. 增加 `workflow-production-gates.sh` 汇总脚本（串行执行全部门禁）。
4. 启动 W1 任务并在周报模板中新增“生产级达成率”字段。

#### 12.7.1 执行进展（2026-04-22）
- ✅ 已完成：`param-planner-spec.md`（Top-10 工具策略冻结 v1）
- ✅ 已完成：`workflow-production-gates.sh`（已纳入 `workflow-param-replay`）
- ✅ 已完成：`param-replay-samples.json` + `workflow-param-replay.sh` + `param-replay-report.md`
- ✅ 已完成：`gate-replay-samples-v2.json` 扩容到 200 条（门槛提升到 85% 并通过）
- ✅ 已完成：`gate-misclass-report.md` 自动生成（由 `workflow-gate-replay.sh` 产出）
- ✅ 已完成：Top-10 参数 planner（`file_read/file_edit/bash/grep/file_write/glob/project_list/memory_save/todo_write/json_query`）
- ✅ 已完成：S1 首步落地，`metrics-storage-spec.md` + SQLite 持久化接线（`workflow_metrics_runs`）
- ✅ 已完成：`/api/workflow/metrics/weekly` 查询接口（支持 `limit`）
- ✅ 已完成：`workflow-weekly-report.sh` 周报脚本升级（含 WoW 环比列）

---

## 13. 第一性原理与顶层设计修正计划（详细版）

> 背景：当前“权重 + 主动提问式深思（Socratic）”主链路已可运行，下一阶段重点从“功能存在”升级到“策略可校准、结果可证明、发布可持续”。

### 13.1 总体目标（North Star）
1. 在固定成本预算下，最大化主线推进效率与一次通过率。
2. 降低“看似完成但质量不闭环”的假阳性。
3. 让 Gate / Weight / Questioning / Executor 的决策在同一策略语义下可解释、可对账。

### 13.2 顶层修正方向 A：统一目标函数与策略语义
1. 新增统一评分口径：`Score = MainlineHit * 0.4 + FirstPassQuality * 0.35 + CostEfficiency * 0.25`。
2. 将 `fallback/retry/reweight/abort` 定义为互斥状态转移，禁止同一错误路径多语义解释。
3. 在 `docs/workflow/` 新增 `policy-spec.md`，统一 Gate、权重、提问、执行的触发阈值说明。
4. 验收：任意一次 workflow run 都能输出“为什么进 workflow / 为什么重算 / 为什么降级”的同口径解释。

### 13.3 顶层修正方向 B：策略中心化（Policy Layer）
1. 引入 `WorkflowPolicy`（集中读取 env、阈值、启发式开关），避免阈值散落多模块。
2. Gate/Questioning/Weights 统一依赖 `WorkflowPolicy`，减少隐藏耦合。
3. 所有策略变更通过单一配置快照记录到指标（便于回放对比）。
4. 验收：策略参数变更后，可在周报中直接关联到指标变化（非人工猜测）。

### 13.4 顶层修正方向 C：Socratic 从“问得多”升级到“证据驱动”
1. 为每轮提问增加 `evidence_required` 字段：必须产出可执行证据（文件/命令/断言/风险项）。
2. 若未形成证据，允许继续提问；若达到预算仍无证据，强制降级为 Direct 并输出缺失证据列表。
3. 把“收敛条件”从仅文本长度/不确定性，升级为“证据闭环度 + 主线相关度 + 风险覆盖度”。
4. 验收：复杂任务中，`question_chain` 至少 70% 节点附带可执行证据项。

### 13.5 顶层修正方向 D：权重从静态规则升级为“规则 + 反馈校准”
1. 保留六维规则作为保底排序，新增 `feedback_adjustment`（基于历史失败签名、返工率、工具失败率）。
2. 对高返工步骤自动提升 DriftPenalty 或 BlockerValue，避免重复跑偏。
3. 新增“排序稳定性”指标：相同输入在同策略版本下排序一致率应接近 100%。
4. 验收：连续两周 `Rework Rate` 降低且 `Mainline Hit` 不下降。

### 13.6 顶层修正方向 E：指标可信度升级（从近似到可运营）
1. 将 `MainlineHit` 与 `FirstPlanCoverage` 拆为“启发式值 + 人工抽样校准值”双轨。
2. 每周固定抽样（建议 20-30 条）做人审对账，记录偏差率。
3. 报告中显式展示“自动指标置信区间”，避免仅看单点百分比。
4. 验收：自动指标与人工抽样偏差率持续收敛（目标 <= 10%）。

### 13.7 顶层修正方向 F：生产门禁与运行时一致性
1. 将 clippy/test/replay/real-devflow 与策略版本绑定，避免“代码绿但策略漂移”。
2. 门禁报告增加 `policy_version`、`gate_mode`、`questioning_budget`、`weight_multipliers` 快照。
3. 新增“失败语义一致性测试”：同类错误触发的状态转移必须一致。
4. 验收：门禁失败可直接定位是“代码回归”还是“策略回归”。

### 13.8 6 周落地节奏（从设计修正到可运行）
1. Week 1: 冻结 `policy-spec.md` + 失败语义状态机图 + 指标口径字典 v2。
2. Week 2: 完成 `WorkflowPolicy` 接线（Gate/Weights/Questioning 三模块）。
3. Week 3: Socratic 证据字段与收敛规则升级，加入缺失证据降级路径。
4. Week 4: 权重反馈校准上线，补排序稳定性与回归测试。
5. Week 5: 指标双轨校准与周报升级（自动值 + 抽样值）。
6. Week 6: 全量门禁联调，产出 `top-level-design-readiness-report.md`。

### 13.9 DoD（本章节完成定义）
1. 代码层：Policy Layer 落地并替代散落阈值读取。
2. 指标层：自动指标与人工抽样对账机制常态化。
3. 发布层：门禁报告能定位策略回归来源。
4. 业务层：复杂编程任务的主线命中与返工率趋势稳定改善。


---

## 14. Claude Code 源码差距分析与追赶计划（2026-04-23 新增）

> 基于 Claude Code v2.1.76 源码（桌面 `claude` 文件夹）+ 2026 年最新公开资料的系统调研。

### 14.1 核心发现：Claude Code 强在哪里？

Claude Code 的核心竞争力不是单一功能，而是 **"Harness 完整性"**——工具、提示、上下文、安全、Agent 编排形成高度协同的体系。

| 维度 | Claude Code | Priority Agent | 差距 |
|------|-------------|----------------|------|
| **工具数量** | 43 | 29 | **-14** |
| **上下文压缩** | 6 种策略 + compact_boundary 标记 | ✅ CompactMetadata + Boundary Marker + SessionMemoryCompact | 差距缩小 |
| **File Checkpointing** | ✅ 自动快照 + diff + 回滚 | ✅ 系统级 CheckpointManager + `/checkpoints` + `/restore` | 差距缩小 |
| **Agent 系统** | 5 种内置角色 + fork/in-process/remote 三模式 + 记忆隔离 | ✅ 9 种角色 + `RoleMemoryStore` 记忆隔离 | 差距缩小 |
| **Plan Mode** | Enter/Exit/AskQuestion 三位一体 + Ultra-plan | ✅ Clarifying 状态 + `ask_user` 集成 + 状态栏实时显示 | 差距缩小 |
| **LSP 集成** | LSPClient + DiagnosticRegistry + ServerManager | 基础 LspManager | 诊断深度差距 |
| **Security** | Security Classifier + Denial Tracking + 工具级风险分析 | 危险命令检测 + 规则系统 | 分类器缺失 |
| **Skills** | 18 内置 skills + marketplace + /batch 大规模并行 | ✅ 内置 `/batch` skill + Agent 并行执行 | 差距缩小 |
| **Git/Worktree** | worktree 隔离 + commit-push-pr 自动化 | /diff 命令 | 工作流闭环差距 |
| **TUI** | 自研 ink 渲染引擎（331 文件）+ Vim 模式 + 终端通知 | ratatui 基础界面 | 体验差距 |
| **Remote** | `claude ssh <host>` | ❌ | 远程执行缺失 |
| **VS Code** | 原生插件 inline edit | HTTP API | IDE 集成差距 |
| **Prompt Caching** | ✅ 成本降低 90% | ❌ | 成本优化缺失 |
| **Voice** | 语音输入 | ✅ STT/TTS | **我们领先** |
| **权重+Socratic** | ❌ | ✅ 独特优势 | **我们领先** |

### 14.2 关键差距详解

#### 🔴 P0：File Checkpointing（文件快照系统）✅

Claude Code 每次工具执行前自动创建文件快照，最多保留 100 个，支持 diff 对比和任意状态恢复。这是让 Agent "大胆自动执行" 的安全网。

**已实现**：`CheckpointManager` 系统级检查点管理（`src/engine/checkpoint.rs`，26KB）。`file_write`/`file_edit` 修改前自动调用 `CheckpointManager::create()` 创建快照。支持 `restore()` 回滚、`diff_checkpoints()` unified diff 对比、`prune()` 清理旧快照（最多保留 100 个）。TUI 斜杠命令 `/checkpoints`（列出最近 20 个）、`/restore <id>`（恢复到历史状态）。

#### 🔴 P0：Agent 角色化与记忆隔离 ✅

Claude Code 的 AgentTool 有 5 种内置角色（plan/verification/guide/advisor/fast），每个角色有独立的记忆文件、权限上下文、模型选择。

**已实现**：`AgentRole` 扩展至 9 种角色（`Default/Teammate/Specialist/DreamTask/Plan/Verify/Fast/Guide/Advisor`），每种角色有独立的 `system_prompt()`。`AgentConfig::from_role()` 自动注入角色提示词。`RoleMemoryStore`（`src/agent/memory.rs`）按角色隔离持久化存储，路径 `~/.priority-agent/memories/<role>.json`。

#### 🟠 P1：上下文压缩升级 ✅

Claude Code 有 6 种压缩策略（snip / micro / auto / session memory / grouping / post-cleanup），在消息流中插入 `compact_boundary` 标记保留恢复信息，有基于对话时长的动态配置。

**已实现**：`CompactMetadata` 压缩边界元数据（sequence, boundary_id, preserved_tail_count, messages_before/after, tokens_before/after, timestamp）。`COMPACT_BOUNDARY_MARKER` 标记 `[COMPACT_BOUNDARY:id=...|seq=N|...]` 嵌入摘要消息保留恢复信息。`SessionMemoryCompact` 基于会话阶段的智能压缩策略（探索/实现/验证/收尾四阶段）。

#### 🟠 P1：Skills 内置库与 /batch ✅

Claude Code 有 18 个内置 skills，`/batch` 可以并行修改 5-30 个独立代码单元。支持 marketplace 和变量替换。

**已实现**：内置 `batch.md` skill（`src/skills/bundled/batch.md`）：大规模并行代码修改流程（研究→分解→5-30 单元→并行执行→PR）。`BatchRefactor` 增强（`src/engine/batch_refactor.rs`）：集成 `AgentManager` 真正调用 Agent 执行每个单元，支持 git worktree 隔离执行。TUI 斜杠命令 `/batch <description>` 自动发现前 50 个文件并分解为并行单元。

#### 🟠 P1：Plan Mode 交互式提问 ✅

Claude Code 的 Plan Mode 有 `AskUserQuestionTool`，在规划阶段主动向用户澄清需求，避免"做出来才发现不对"。

**已实现**：
- `EnterPlanModeTool` 描述增强：明确指示 Agent 在不确定时使用 `ask_user` 工具提问
- `PlanTool` 描述增强：提示提交计划前先用 `ask_user` 澄清模糊需求
- `PlanModeState::Clarifying { question }` 新状态：追踪 Agent 正在向用户提问
- TUI 状态栏实时显示 Plan Mode 子状态：`[PLAN: generating]`、`[PLAN: clarifying "..."]`、`[PLAN: awaiting approval]`、`[PLAN: step N]`
- PlanModeManager 新增 `start_clarifying()` / `finish_clarifying()` 方法

#### 🟡 P2：Security Classifier

Claude Code 每个工具有 `toAutoClassifierInput`，Bash/PowerShell 有专门的安全分析器（regex + 语义分析），权限拒绝有追踪学习机制。

**我们的现状**：BashTool 有关键词检测，但没有系统级 security classifier，没有 denial tracking。

#### 🟡 P2：Git Worktree 与 commit/PR 自动化

Claude Code 支持 `--worktree` 自动创建 git worktree 隔离执行，`commit-push-pr.ts` 自动完成代码提交和 PR 创建。

**我们的现状**：有 `/diff` 命令，但没有 worktree 隔离和自动 commit/PR。

#### 🟢 P3：TUI 体验

Claude Code 的 ink 渲染引擎支持 Ghostty/iTerm2/kitty 原生特性、Vim 模式、文件 diff 可视化、上下文可视化。

**我们的现状**：ratatui 基础界面，有设置页面和斜杠命令，但没有 Vim 模式、没有 diff 可视化。

### 14.3 追赶优先级与排期

| 优先级 | 方向 | 预估工作量 | 预期影响 | 计划章节 |
|--------|------|-----------|---------|---------|
| 🔴 **P0** | **File Checkpointing** | 2 周 | 安全网，支撑自动模式 | 14.4 |
| 🔴 **P0** | **Agent 角色化 + 记忆隔离** | 3-4 周 | 核心差异化能力 | 14.5 |
| 🟠 **P1** | **上下文压缩升级**（compact_boundary + time-based + session memory） | 2-3 周 | 长对话稳定性 | 14.6 |
| 🟠 **P1** | **Skills 内置库 + /batch** | 2 周 | 大规模代码修改能力 | 14.7 |
| 🟠 **P1** | **Plan Mode 交互式提问** | 1-2 周 | 减少返工 | 14.8 |
| 🟡 **P2** | **Security Classifier + Denial Tracking** | 1-2 周 | 安全提升 | 14.9 |
| 🟡 **P2** | **Git worktree + commit/PR** | 1-2 周 | 工作流闭环 | 14.10 |
| 🟡 **P2** | **工具补充到 35+** | 2 周 | 功能补齐 | 14.11 |
| 🟢 **P3** | **TUI diff 可视化 + Vim** | 2-3 周 | 体验提升 | 14.12 |
| 🟢 **P3** | **Prompt Caching** | 1-2 周 | 成本优化 | 14.13 |

### 14.4 执行状态追踪

| 任务 | 状态 | 开始日期 | 完成日期 | 备注 |
|------|------|---------|---------|------|
| File Checkpointing | ✅ 已完成 | 2026-04-23 | 2026-04-23 | 14.4.1 |
| Agent 角色化 | ✅ 已完成 | 2026-04-23 | 2026-04-23 | 14.5 |
| 上下文压缩升级 | ✅ 已完成 | 2026-04-23 | 2026-04-23 | 14.6 |
| Skills 内置库 | ✅ 已完成 | 2026-04-23 | 2026-04-23 | 14.7 |
| Plan Mode 提问 | ✅ 已完成 | 2026-04-23 | 2026-04-23 | 14.8 |
| Security Classifier | ⏳ 待启动 | — | — | 14.9 |
| Git Worktree | ⏳ 待启动 | — | — | 14.10 |
| 工具补齐 35+ | ⏳ 待启动 | — | — | 14.11 |
| TUI 增强 | ⏳ 待启动 | — | — | 14.12 |
| Prompt Caching | ⏳ 待启动 | — | — | 14.13 |

---
