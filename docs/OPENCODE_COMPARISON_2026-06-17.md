# OpenCode 对比分析报告

日期：2026-06-17  
更新：2026-06-18

## 审计范围

本文对比 priority-agent 与 opencode 在提示构建、上下文组装、指令加载、压缩、记忆、计划模式、技能、会话运行和工具体验上的差异。

本次更新使用两类证据：

- priority-agent 当前代码：`/Users/georgexu/Desktop/rust-agent`
- opencode 本地源码快照：`/Users/georgexu/Downloads/opencode-dev`

注意：本地 opencode 快照目录不是 git checkout，无法标注精确 commit；它只能作为 2026-06-04 左右的本机源码快照。若后续要做 release-level parity 结论，需要重新拉取 opencode 当前源码并记录 commit。

## 审计结论

原文方向有价值，但不能直接作为当前路线图使用。它的主要问题是：

- 对 opencode 的部分结论缺少源码版本标注。
- 对 priority-agent 的描述停留在较早状态，漏掉了已经实现的 goal mode、plan mode tools、session todos、`@` 文件补全/附件、run coordinator、route-scoped tools、provider/tool schema 诊断等能力。
- 对 priority-agent 的 5 区域上下文模型写得过满：代码确实有 zone 命名、fingerprint、预算报告和 trace，但不是所有区域都已经被一个统一 builder 强制预算和装配。
- 对压缩触发写得过强：time/message/idle 判断存在或可配置，但当前生产主链路主要是 preflight、streaming pre-query、manual compact、API reactive 和 request-local selective compression。
- “priority-agent 没有显式计划/构建模式”已经不准确；当前已有 `PlanModeManager`、`enter_plan_mode`、`exit_plan_mode`、`plan` tool、`/goal` durable goal runner，只是产品体验和 opencode 的 agent plan/build 切换不一样。

更准确的总评是：

- opencode 更强在产品化的 session/agent/model surface、模型自适应提示、plan/build 体验、reference 语法、plugin hooks、compaction continuation 和成熟 UI。
- priority-agent 更强在本地个人化记忆、runtime evidence、validation/permission/checkpoint 边界、cache/zone 诊断、route-scoped tool exposure、closeout proof 和可解释失败状态。
- 后续不应“照搬 opencode 架构”，而应借鉴它的产品收敛方式：更清楚的 provider/model/agent catalog，更轻的 plan/build 入口，更好的 reference/composer 体验，更标准的 extension hook。

## 一、系统提示构建

### opencode

opencode 有模型自适应提示模板。当前本地快照中 `packages/opencode/src/session/system.ts` 会按模型 ID 选择：

| 模型/关键词 | 提示模板 |
|-------------|----------|
| `gpt-4` / `o1` / `o3` | `beast.txt` |
| `gpt` | `gpt.txt` |
| `codex` | `codex.txt` |
| `gemini-` | `gemini.txt` |
| `claude` | `anthropic.txt` |
| `trinity` | `trinity.txt` |
| `kimi` | `kimi.txt` |
| 其他 | `default.txt` |

此外，opencode 将环境信息、skills、instructions、reminders、tools resolution 分在不同服务中处理，prompt 形状更像服务管道组合。

### priority-agent

priority-agent 当前以统一基础提示为主，但并不只是“单一基础提示 + `<task-focus>`”：

- `instructions::compose_system_prompt()` 组合 base prompt、AGENTS.md 和 root context layers。
- `PromptContextAssembler` 建模 stable prefix 与 task-aware prompt。
- `context_assembly.rs` 定义 `stable_prefix`、`task_state`、`relevant_material`、`recent_observation`、`current_decision_request` 五个 zone。
- `request_preparation_controller` 在请求准备阶段记录 zone trace、cache fingerprint 和 dynamic context envelope。
- `tool_exposure.rs` 和 route-scoped tools 按 route/permission/provider schema 决定工具是否暴露。

### 结论

opencode 的模型自适应提示值得借鉴，但 priority-agent 不应把 provider quirks 全塞进 always-on prompt。更合适的路线是：

- 对弱提供者增加极小的 provider profile prompt delta；
- 将工具 schema、权限、输出格式和 closeout proof 继续放在 runtime/tool contract；
- 为 MiniMax/Kimi 等模型补 provider-specific tool schema/输出修复测试，而不是增加大段提示。

## 二、上下文组装架构

### opencode

opencode 的上下文组装靠服务管道：

- `SystemPrompt`：模型提示、环境、skills。
- `Instruction`：AGENTS.md / CLAUDE.md / CONTEXT.md，以及 URL fetch。
- `SessionReminders`：plan/build reminder 和 build-switch。
- `SessionTools`：工具解析、schema transform、permission ask、plugin hook。
- `Reference` / prompt parts：解析 `@reference` 和文件附件。

它还有 `SystemContext` / `SessionSystemContext` 这样的 component/checkpoint 模型，用 hash 判断系统上下文变化。

### priority-agent

priority-agent 有显式 zone 模型和 cache 诊断：

| 区域 | 当前作用 |
|------|----------|
| `stable_prefix` | base prompt、AGENTS.md、root context 等稳定前缀 |
| `task_state` | task-aware prompt / task state |
| `relevant_material` | retrieval context、memory、project/session/web/MCP 等材料 |
| `recent_observation` | 最近工具观察、repair/failure context |
| `current_decision_request` | 当前用户请求 |

但需要准确表述：当前 zone 模型主要承担命名、fingerprint、预算报告和 trace。动态材料仍分散在 turn bootstrap、retrieval controller、request preparation、memory snapshot、repair/controller 等路径里；还没有一个统一的 dynamic context block builder 对所有动态块做强制排序、dedupe、预算和 provenance 管理。

### 建议

priority-agent 不需要改成 Effect 服务管道，但可以借鉴 opencode 的“组件化上下文服务”：

- 保留 5-zone 诊断模型；
- 增加统一 dynamic context block builder；
- 将 retrieval、memory、skills、repair、validation evidence 的注入顺序和 provenance 收敛到一个小接口；
- 保持 stable prefix 和 dynamic tail 的 cache boundary 清晰。

## 三、指令加载

### opencode

本地快照中 `Instruction` 服务支持：

- 全局 `AGENTS.md`；
- 可选 `~/.claude/CLAUDE.md`；
- 项目向上查找 `AGENTS.md`、`CLAUDE.md`、deprecated `CONTEXT.md`；
- HTTP/HTTPS fetch；
- 对 read tool 加载过的文件做附近 instruction resolve；
- `@reference`/prompt parts 路径解析。

### priority-agent

priority-agent 的指令加载更偏安全和缓存稳定：

- 全局、项目根、目录层 AGENTS.md；
- 优先提取 `## Agent Runtime Guidance`；
- per-layer 选段、字符限制和总限制；
- root context：`SOUL.md`、`USER.md`、`TOOLS.md`；
- prompt-visible safety scan；
- XML escape；
- root context/source/trust 标签。

### 建议

不建议直接打开远程 AGENTS.md 默认加载。可以考虑更受控的版本：

- 只允许用户显式配置的 remote instruction URL；
- 拉取后经过 safety scan、size cap、source label 和 cache；
- 默认作为 untrusted/background context，而不是 stable runtime policy；
- 对每文件 instruction resolve 做 opt-in，避免读文件时隐式改变模型规则。

## 四、压缩系统

### opencode

本地快照中的 `SessionCompaction` 有这些关键点：

- overflow 检测使用模型可用上下文窗口；
- `select()` 保留最近 turn 或 token budget；
- anchored summary：新摘要会包含 previous summary，并要求保留仍正确、删除过时、合并新事实；
- `TOOL_OUTPUT_MAX_CHARS = 2000`；
- `PRUNE_PROTECT = 40_000` token，旧工具输出超过保护窗口才 prune；
- `PRUNE_PROTECTED_TOOLS = ["skill"]`；
- `experimental.session.compacting` 可以提供额外 context 或替换 compaction prompt；
- `experimental.compaction.autocontinue` 控制压缩后是否合成 “Continue if you have next steps...”；
- overflow 场景会 replay 原用户消息。

### priority-agent

priority-agent 当前压缩主链路包括：

- preflight compression；
- streaming pre-query compression；
- manual `/compact`；
- API reactive compression；
- request-local selective compression；
- compact boundary 持久化；
- `CompactionRuntimeRecord`、`CompactionDecision`、cache/zone trace；
- `StructuredSummary::merge()`；
- runtime continuity facts；
- tool pair sanitize；
- consecutive failure/no-gain circuit。

需要修正原文：time/session/message/idle 触发判断不应写成当前稳定生产主路径；`ContextCollapseService` 本体也未接入主对话循环。

### 建议

可借鉴 opencode 的点：

- protected tool output policy：skills、validation、permission、checkpoint、failure_owner、required proof 不应被普通工具输出裁剪策略误删；
- overflow replay contract：明确 provider context error 后是否重放原用户请求；
- compaction hook：先做内部 trait/hook，不急着开放插件 API；
- anchored summary wording：当前 merge 已有类似目标，但可以让 LLM compaction prompt 明确“保留仍正确、删除过时”。

## 五、记忆与检索

### opencode

opencode 没有 priority-agent 这种长期记忆系统。它主要依赖：

- instruction files；
- skills；
- references / prompt parts；
- session history；
- compaction summary；
- MCP 和工具。

### priority-agent

priority-agent 的检索/记忆是独立优势，但需要分清 read 与 write：

- RetrievalSource 当前包括 `Memory`、`Project`、`Session`、`Web`、`Mcp`、`File`、`Tool`。
- memory snapshot 可以注入 stable prefix。
- dynamic retrieval context 可以进入 relevant material。
- active memory 是 opt-in、read-only、gated worker。
- 长期记忆写入默认 review-only，`narrow`/`legacy` 需要显式配置。
- memory proposals 有 review queue 和 `/memory-proposals` 操作链路。

### 建议

继续保持 review-first 长期记忆边界。比“默认自动写记忆”更值得做的是：

- memory proposal badge / nudge；
- memory status 直接显示 proposal queue；
- protected evidence 与 memory proposal 的可视化；
- import/export 可以做，但优先级低于 review flow 可用性。

## 六、计划、目标与任务管理

### opencode

opencode 有 plan/build agent 体验：

- `SessionReminders` 注入 `plan.txt`、`plan-mode.txt`、`build-switch.txt`；
- plan 模式可以创建/编辑 plan file；
- build 切换时提醒不再处于只读计划；
- TodoWrite 被强力提示用于复杂任务。

### priority-agent

原文说 priority-agent “没有显式计划/构建模式”已经不准确。当前已有：

- `PlanModeManager`；
- `enter_plan_mode` / `exit_plan_mode` tools；
- `PlanTool`；
- `todo_write`，并持久化到 session store；
- `/goal <objective>`、`GoalRunner`、`GoalDecisionEngine`、goal persistence；
- `/quick`、`/active-task`、desktop goal progress row；
- goal steer/queue；
- goal drift checks。

差距不在“有没有”，而在产品形态：

- opencode 的 plan/build 是 agent-level mode，用户感知更直接；
- priority-agent 的 plan/goal/todo 分散在工具、slash command、runtime state 中；
- plan mode 和 goal runner 还需要更统一的用户入口和状态展示。

### 建议

不要新增一套完全独立 `/plan` 系统。更合适的是整合现有能力：

- `/mode plan|build|review` 与 `enter_plan_mode`/`exit_plan_mode` 对齐；
- `/goal` 负责多轮持久目标；
- `todo_write` 负责本轮执行清单；
- UI 明确显示当前处于 plan、build、goal-running 还是 review 状态。

## 七、技能系统

### opencode

opencode skills 支持：

- `SKILL.md` / `<name>.md`；
- skill discovery；
- remote skill index pull；
- `skill` tool；
- system prompt 中列出可用 skills；
- protected skill tool output in compaction prune。

### priority-agent

priority-agent 当前有：

- `SkillRuntime::load(...).search(...)`；
- skill trigger 注入 retained context；
- source/trust metadata；
- skill allowlist / scanner；
- route-scoped skill tool exposure；
- compression 过程中的 preserved skills marker。

需要注意：`has_active_skills` 当前不是源自真实技能状态，前面的缓存/压缩审计已把它列为未完成事项。

### 建议

- 增加 YAML frontmatter 支持可以做，但不是最高优先级；
- 更高优先级是 skill active state 源头、protected skill outputs、skill provenance 和 UI 可见性；
- remote skill index 可以借鉴 opencode，但必须经过安全扫描和 allowlist。

## 八、其他重要差距

原文漏掉了一些比“模型提示模板”更重要的 opencode 对齐项。

### 1. Provider/model catalog

opencode 有更产品化的 provider/model catalog、model selector、provider auth 和 OpenAPI surface。priority-agent 虽然已有 provider adapters 和 provider/model unification 计划，但 UI/配置/状态面还不如 opencode 成熟。

建议：继续推进 provider/model catalog 作为单一事实源，而不是把 provider 差异散在 prompt、tools 和配置里。

### 2. Session event / projection / parts

priority-agent 已经有 session events、session parts、message ops、compact boundary、todo store、goal store 等投影能力。原文没有覆盖这一块，导致对“opencode 架构优势”的总结偏窄。

建议：后续对比应单独做 session spine 对照：event、part、tool output、diff、revert、queue、export。

### 3. File mutation and diff UX

opencode 的产品感很大一部分来自更直接的 edit/write diff、session timeline、tool part rendering。priority-agent 在 permission/checkpoint/validation 上更硬，但 TUI diff review 和 tool rendering 仍是体验差距。

建议：这个方向比“远程指令加载”优先级更高。

## 九、建议优先级

### P0：修正文档和事实口径

- [x] 标注 opencode 本地快照来源。
- [x] 修正 priority-agent 已有 plan mode / goal mode / todos / `@` 文件能力的事实。
- [x] 修正 priority-agent 5-zone 模型“强制预算”过度表述。
- [x] 修正压缩触发路径过度表述。
- [x] 将“没有计划模式”改为“产品入口分散”。

### P1：值得实际推进

- [ ] Provider/model catalog：统一模型能力、提示 delta、tool schema transform、timeout、context window、成本。
- [ ] Dynamic context block builder：统一动态上下文的排序、dedupe、预算和 provenance。
- [ ] Protected output policy：保护 skill、validation、permission、checkpoint、failure evidence。
- [ ] Plan/build UX 收敛：把 `/mode`、plan tools、goal runner、todo 状态统一展示。
- [ ] Reference/composer 体验：在已有 `@` 文件补全/附件基础上增加更清晰的 reference provenance 和多源引用。

### P2：可借鉴但不急

- [ ] Provider-specific prompt delta，先针对 MiniMax/Kimi 做小范围测试。
- [ ] Internal compaction hook，不急着开放外部插件 API。
- [ ] Remote instruction/skill source，必须 gated、scanned、cached、labeled。
- [ ] Anchored summary wording 调整。

### P3：不建议作为近期重点

- [ ] 默认远程 AGENTS.md 加载。
- [ ] 为了“像 opencode”重写成 Effect/service 架构。
- [ ] 默认自动写长期记忆。
- [ ] 在 always-on prompt 中堆大量弱模型规则。

## 总结

这份对比文档原来的结论“priority-agent 在记忆、区域上下文、安全扫描、多级压缩上更强；opencode 在模型自适应提示、计划模式、reference、插件上更强”大方向可以保留，但需要更精确：

- priority-agent 当前最核心优势是硬 runtime 边界和 repo-backed evidence：验证、权限、checkpoint、closeout proof、memory review、cache/zone trace。
- opencode 当前最核心优势是产品化一致性：agent/mode/provider/model/session/tool part/reference/compaction hook 的用户体验更完整。
- 下一步最值得做的不是照搬 opencode，而是把 priority-agent 已经具备的硬能力收敛成更清晰的产品入口和状态面。

## 建议验证

本文是文档更新，未改运行时代码。若后续按本文推进实现，建议按对应范围运行：

```bash
cargo fmt --check
cargo test -q prompt_context
cargo test -q request_preparation_controller
cargo test -q route_scoped_tools
cargo test -q plan_mode
cargo test -q goal --lib
cargo test -q todo_store
cargo test -q context_compressor
cargo check -q
```
