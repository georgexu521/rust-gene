# Docs Index
Status: Index

Priority Agent 文档导航。从这里出发，找到你需要的文档。这个目录保留了大量
历史计划；判断当前实现状态时，先看 canonical 文档，再看具体代码和测试。

## Canonical（权威当前文档）

这些是 `PROJECT_STATUS.md` 列为 canonical 的文档，代表项目当前状态的权威来源：

| 文档 | 用途 |
|------|------|
| [PROJECT_STATUS.md](PROJECT_STATUS.md) | 项目当前状态、runtime spine、产品表面、剩余工作 |
| [PROJECT_MAP.md](PROJECT_MAP.md) | 代码导航地图，模块/工具/入口点索引 |
| [CLAUDE_CODE_GAP_MATRIX_2026-05-03.md](CLAUDE_CODE_GAP_MATRIX_2026-05-03.md) | 与 Claude Code 的 gap 矩阵，P0/P1/P2 优先级 |
| [CLAUDE_CODE_ALIGNMENT_PLAN.md](CLAUDE_CODE_ALIGNMENT_PLAN.md) | Claude Code 对齐计划 |
| [CLAUDE_CODE_PARITY_IMPLEMENTATION_PLAN_2026-05-20.md](CLAUDE_CODE_PARITY_IMPLEMENTATION_PLAN_2026-05-20.md) | Parity 实现计划（12 阶段，当前实施路线） |
| [REMAINING_CLOSURE_PLAN.md](REMAINING_CLOSURE_PLAN.md) | 剩余收尾计划 |
| [LLM_RUNTIME_SIMPLIFICATION_PLAN_2026-05-08.md](LLM_RUNTIME_SIMPLIFICATION_PLAN_2026-05-08.md) | Runtime 简化：减少对 LLM 的过度控制 |
| [RUNTIME_DIET_UPDATE_2026-06-02.md](RUNTIME_DIET_UPDATE_2026-06-02.md) | Runtime diet 更新：移除 pseudo-intelligent 干预 |
| [UNIFIED_RUNTIME_ENTRYPOINTS_2026-06-01.md](UNIFIED_RUNTIME_ENTRYPOINTS_2026-06-01.md) | 统一 runtime 入口点：StreamingQueryEngine 为 canonical |
| [NEXT_DEVELOPMENT_PLAN_2026-05-09.md](NEXT_DEVELOPMENT_PLAN_2026-05-09.md) | 下一阶段开发计划 |
| [PERSONAL_AGENT_PRODUCT_PRINCIPLES_2026-05-18.md](PERSONAL_AGENT_PRODUCT_PRINCIPLES_2026-05-18.md) | 产品原则：窄、深、个人化、可验证 |
| [AGENT_LEARNING_NOTES_PROJECT_ALIGNMENT_2026-05-24.md](AGENT_LEARNING_NOTES_PROJECT_ALIGNMENT_2026-05-24.md) | Agent 学习笔记与项目对齐 |
| [AGENT_TESTING_MATRIX_2026-05-08.md](AGENT_TESTING_MATRIX_2026-05-08.md) | Agent 测试矩阵 |

## Recent Active Notes（近期活跃记录）

这些文档是 2026-06-07 到 2026-06-09 附近的最近工作记录或下一步计划。它们有用，
但仍要以 `PROJECT_STATUS.md` 和代码为准。

| 文档 | 日期 | 用途 |
|------|------|------|
| [CONTROLLER_MERGE_PLAN.md](CONTROLLER_MERGE_PLAN.md) | 06-09 | `conversation_loop` 控制器合并结果和后续边界 |
| [MESSAGE_PERSISTENCE_PLAN_2026-06-08.md](MESSAGE_PERSISTENCE_PLAN_2026-06-08.md) | 06-08 | 会话消息持久化计划 |
| [LLM_COMPACTION_PLAN_2026-06-08.md](LLM_COMPACTION_PLAN_2026-06-08.md) | 06-08 | LLM compaction 计划 |
| [ROUTING_AND_CONTEXT_ANALYSIS_2026-06-08.md](ROUTING_AND_CONTEXT_ANALYSIS_2026-06-08.md) | 06-08 | 路由与上下文分析 |
| [WEIGHTING_SYSTEM_NEXT_PHASE_PLAN_2026-06-08.md](WEIGHTING_SYSTEM_NEXT_PHASE_PLAN_2026-06-08.md) | 06-08 | 权重系统下一阶段 |
| [OPENCODE_PROGRAMMING_PARITY_NEXT_PLAN_2026-06-07.md](OPENCODE_PROGRAMMING_PARITY_NEXT_PLAN_2026-06-07.md) | 06-07 | opencode programming parity 下一步 |
| [OPENCODE_THIRD_ALIGNMENT_PLAN_2026-06-07.md](OPENCODE_THIRD_ALIGNMENT_PLAN_2026-06-07.md) | 06-07 | opencode 第三轮对齐 |
| [AGENT_PROMPT_ENTRYPOINT_ALIGNMENT_PLAN_2026-06-07.md](AGENT_PROMPT_ENTRYPOINT_ALIGNMENT_PLAN_2026-06-07.md) | 06-07 | prompt 入口对齐 |

## Active Plans（仍可能相关的计划）

这些是仍可能相关的计划或刚完成的计划；日期较早的计划不要直接当作当前状态：

| 文档 | 日期 | 用途 |
|------|------|------|
| [AGENT_SKILLS_OPTIMIZATION_PLAN_2026-06-01.md](AGENT_SKILLS_OPTIMIZATION_PLAN_2026-06-01.md) | 06-01 | Agent skills 优化计划 |
| [LIVE_CODING_TEST_PLAN_2026-06-01.md](LIVE_CODING_TEST_PLAN_2026-06-01.md) | 06-01 | Live coding 测试计划 |
| [NEW_FEATURE_EVAL_PLAN_2026-06-01.md](NEW_FEATURE_EVAL_PLAN_2026-06-01.md) | 06-01 | 新功能评估计划 |
| [REAL_WORLD_TEST_PLAN_2026-06-01.md](REAL_WORLD_TEST_PLAN_2026-06-01.md) | 06-01 | 真实世界测试计划 |
| [TEST_LANES_2026-05-29.md](TEST_LANES_2026-05-29.md) | 05-29 | 测试通道规划 |
| [DEVELOPMENT_REFACTORING_PLAN_2026-05-28.md](DEVELOPMENT_REFACTORING_PLAN_2026-05-28.md) | 05-28 | 开发重构计划 |
| [SELF_EVOLUTION_EVAL_LOOP_PLAN_2026-05-28.md](SELF_EVOLUTION_EVAL_LOOP_PLAN_2026-05-28.md) | 05-28 | 自我进化评估循环计划 |
| [CODING_FLOW_POLISH_OBSERVABILITY_PLAN_2026-05-28.md](CODING_FLOW_POLISH_OBSERVABILITY_PLAN_2026-05-28.md) | 05-28 | Coding flow 可观测性打磨 |
| [BEHAVIOR_ASSERTION_NO_EFFECTIVE_DIFF_REPAIR_PLAN_2026-05-28.md](BEHAVIOR_ASSERTION_NO_EFFECTIVE_DIFF_REPAIR_PLAN_2026-05-28.md) | 05-28 | 行为断言 / 无效 diff 修复 |
| [FLOW_STABILIZATION_TEST_PLAN_2026-05-27.md](FLOW_STABILIZATION_TEST_PLAN_2026-05-27.md) | 05-27 | Flow 稳定性测试计划 |
| [FLOW_STABILIZATION_TEST_RUN_2026-05-27.md](FLOW_STABILIZATION_TEST_RUN_2026-05-27.md) | 05-27 | Flow 稳定性测试运行结果 |
| [HERMES_MEMORY_FEATURE_FOLLOWUP_PLAN_2026-05-27.md](HERMES_MEMORY_FEATURE_FOLLOWUP_PLAN_2026-05-27.md) | 05-27 | Hermes memory 功能跟进 |
| [RUNTIME_SPINE_STABILITY_REVIEW_PLAN_2026-05-26.md](RUNTIME_SPINE_STABILITY_REVIEW_PLAN_2026-05-26.md) | 05-26 | Runtime spine 稳定性审查 |

## Reference（参考文档）

设计笔记、架构描述、原则讨论。不是执行计划，但提供有用的上下文：

| 文档 | 用途 |
|------|------|
| [AGENTIC_DESIGN_PATTERNS_REVIEW.md](AGENTIC_DESIGN_PATTERNS_REVIEW.md) | Agentic 设计模式审查 |
| [AGENT_RUNTIME_CONTRACT_PLAN.md](AGENT_RUNTIME_CONTRACT_PLAN.md) | Agent runtime 契约设计 |
| [CODING_AGENT_WORKFLOW_DISCUSSION.md](CODING_AGENT_WORKFLOW_DISCUSSION.md) | Coding agent 工作流讨论 |
| [CONVERSATION_LOOP_RESPONSIBILITY_MAP_2026-05-11.md](CONVERSATION_LOOP_RESPONSIBILITY_MAP_2026-05-11.md) | Conversation loop 职责映射 |
| [FUNCTIONAL_REALITY_AUDIT.md](FUNCTIONAL_REALITY_AUDIT.md) | 功能现实审计 |
| [MEMORY_CONTROLLED_SELF_EVOLUTION_DESIGN.md](MEMORY_CONTROLLED_SELF_EVOLUTION_DESIGN.md) | Memory 控制的自我进化设计 |
| [PROJECT_FLOW_AND_RUNTIME_ARCHITECTURE_2026-05-26.md](PROJECT_FLOW_AND_RUNTIME_ARCHITECTURE_2026-05-26.md) | 项目流与 runtime 架构 |
| [HERMES_MEMORY_SELF_EVOLUTION_REVIEW.md](HERMES_MEMORY_SELF_EVOLUTION_REVIEW.md) | Hermes memory 自我进化审查 |
| [ACTIVE_MEMORY_PROTOTYPE.md](ACTIVE_MEMORY_PROTOTYPE.md) | Active memory 原型 |
| [SKILL_ROOTS_AND_TRUST.md](SKILL_ROOTS_AND_TRUST.md) | Skill 根目录与信任模型 |
| [SOUL_USER_TOOLS_CONTEXT.md](SOUL_USER_TOOLS_CONTEXT.md) | SOUL/USER/TOOLS 上下文层 |
| [CODING_AGENT_REAL_TASK_REGRESSION_PLAN.md](CODING_AGENT_REAL_TASK_REGRESSION_PLAN.md) | 真实任务回归计划 |

## Archive（已归档）

以下文档已完成使命，移至 [`archive/`](archive/)。详见 [`archive/ARCHIVE_INDEX.md`](archive/ARCHIVE_INDEX.md)。

## 其他目录

| 目录 | 内容 |
|------|------|
| [`benchmarks/`](benchmarks/) | Live eval 运行快照、报告、对比数据 |
| [`workflow/`](workflow/) | 工作流相关文档 |
| [`generated/`](generated/) | 自动生成的文档 |
| [`proposal_assets/`](proposal_assets/) | 提案附件 |

## Root Docs

| 文档 | 用途 |
|------|------|
| [`../README.md`](../README.md) | 仓库入口、快速状态、架构摘要 |
| [`../QUICKSTART.md`](../QUICKSTART.md) | 安装、provider 配置、运行和基础验证 |
| [`../AGENTS.md`](../AGENTS.md) | prompt-injected runtime guidance |
| [`../CLAUDE.md`](../CLAUDE.md) | Claude Code 兼容的紧凑项目说明 |
| [`../TESTING.md`](../TESTING.md) | 测试命令手册 |
| [`../QUALITY_GATES.md`](../QUALITY_GATES.md) | 发布和阶段门禁 |

## 阅读顺序建议

**新加入项目**：`PROJECT_STATUS.md` → `PROJECT_MAP.md` → `PERSONAL_AGENT_PRODUCT_PRINCIPLES_2026-05-18.md`

**了解当前开发方向**：`PROJECT_STATUS.md` → `CONTROLLER_MERGE_PLAN.md` → 最近的 06-07/06-08 active notes

**理解 runtime 设计**：`LLM_RUNTIME_SIMPLIFICATION_PLAN_2026-05-08.md` → `RUNTIME_DIET_UPDATE_2026-06-02.md` → `UNIFIED_RUNTIME_ENTRYPOINTS_2026-06-01.md`

**对比 Claude Code**：`CLAUDE_CODE_GAP_MATRIX_2026-05-03.md` → `CLAUDE_CODE_PARITY_IMPLEMENTATION_PLAN_2026-05-20.md`
