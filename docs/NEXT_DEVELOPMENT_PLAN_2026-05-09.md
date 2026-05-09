# Priority Agent 下一阶段开发计划

日期：2026-05-09

本文档承接 `docs/LLM_RUNTIME_SIMPLIFICATION_PLAN_2026-05-08.md`
和截至 `6df0039` 的 live eval 修复证据。

上一阶段的核心是减少 runtime 对 LLM 的过度控制：压缩提示词、按路由暴露工具、
把规则移到工具契约、降低 closeout 噪音。下一阶段不能再新造一个更重的流程框架。
下一阶段的目标应该是把 Priority Agent 做成可靠的本地编码终端产品：能准确理解本机状态、
能自然使用终端和文件工具、能在真实代码任务中稳定读-改-测-修，并能用证据说明成败。

## 顶层判断

Priority Agent 应该被设计成 **LLM 的可靠执行环境**，不是试图替 LLM 思考的规则引擎。

LLM 应该保留正常的推理、取舍和写代码自由。Runtime 应该负责提供：

1. 来自工具的真实状态。
2. 清晰且可执行的安全边界。
3. 快速测试和命令反馈。
4. 可见的长命令进度和取消能力。
5. 基于证据的诚实 closeout。
6. 可复盘、可统计、可转化为回归测试的 trace。

如果行为错了，优先修工具契约、路由、验证、状态采集、eval 覆盖，而不是继续给模型加长篇规则。

## 第一性原理

### 1. 事实只能来自工具

模型不能凭空回答文件夹是否存在、目录里有什么、命令是否成功、包是否安装、测试是否通过。
这些事实必须来自工具输出。

落地含义：

- 问桌面、文件、目录时，必须走真实 inspect 工具。
- 问 terminal、python、包安装、运行命令时，默认应该走 bash/terminal。
- 最终回答不能补充工具输出里没有的大小、时间、数量、内容。
- grep/file_read 的显示高亮和行号不能被当成源码复制回 patch。

### 2. 硬约束放在 runtime，不放在提示词

提示词是建议，工具契约和 runtime 检查才是约束。

落地含义：

- 文件编辑边界、破坏性范围、stale read、shell 函数边界、validation command 提取、
  closeout 真实性，都应该是可执行检查。
- 默认提示词要短，给模型留正常解决问题的空间。
- 不再用越来越多的模型可见规则去弥补 runtime 缺口。

### 3. 真正的评测是实际代码任务，不是玩具任务

贪吃蛇这类任务只能作为写文件和给运行命令的手工 smoke。它测不出这个项目真正应该有的优势。

真正的目标是：

- 能读现有代码。
- 能找对主线 blocker。
- 能从检查转成真实 diff。
- 能运行 required validation。
- 能从失败测试修回来。
- 能诚实报告证据。

### 4. 产品外壳是能力的一部分

Claude Code 和 opencode 强，不只是因为模型强，也因为产品外壳可靠：终端、文件工具、权限、
session、进度 UI、恢复路径、测试面都很完整。

本地参考项目观察：

- Claude 的 `QueryEngine` 围绕 tool permission context、system prompt parts、app state、
  tool progress、remote permission 和 session message 组织。
- opencode 把 `build`、`plan`、`general`、`explore` 这类 agent mode 做成数据化权限配置，
  而不是靠提示词描述。
- opencode 有大量 pty、shell、file、server/session、permission、plugin、LSP、MCP、
  project/worktree 测试。

Priority Agent 下一步应该向这个方向收敛：更少隐形规则，更多产品化执行面。

### 5. 可维护性就是产品质量

当前核心能力已经很多，但仍然太集中。

当前主要文件体量：

```text
9694  src/engine/conversation_loop/mod.rs
1291  src/tools/mod.rs
1624  scripts/run_live_eval.sh
 577  src/main.rs
 538  src/tui/slash_handler/config.rs
```

大文件不一定错，但现在 `conversation_loop/mod.rs` 同时承载工具执行、validation、repair、
trace、closeout、patch recovery、workflow state，导致任何行为修复都容易产生副作用。

## 当前状态

### 已经变强的部分

- Runtime simplification Phase 0-15 已完成。
- 根目录 `AGENTS.md` 已经压缩成运行指南，历史内容归档。
- route-scoped tools、role-scoped subagent profiles、memory/skill 背景化、
  concise closeout 已落地。
- required validation 已经成为真实证据，不再只靠 assistant 自称验证。
- live eval report 已能区分 `failure_owner` 和 agent-flow stops。
- 最新关键恢复案例通过：
  `checkpoint-function-anchor-20260509-120047`，
  `test-status=ok`，`agent-quality-status=status=ok`。
- 最新本地测试基线：
  `cargo test -q` -> `1139 passed; 0 failed`。

### 仍然薄弱的部分

- 历史 live eval aggregate 仍然差：已提交的 shortfall summary 里是 `32/118` 通过。
  最新 dashboard recovery 已改善关键案例，但 aggregate 还需要刷新。
- 终端能力的产品体验还不够稳定。一个编码 agent 不能可靠使用用户 shell，会直接显得功能不全。
- 简单文件系统事实问答暴露过幻觉风险。
- patch synthesis/recovery 已经能修 dashboard case，但仍然复杂，应该被封装成更小的组件。
- `conversation_loop/mod.rs` 仍是核心风险点。
- `docs/PROJECT_STATUS.md` 已过期，还记录 2026-05-08 的 `1090 passed`。
- Claude/Codex/opencode 外部基线还没有进入同一套 report 格式。

## 北极星目标

Priority Agent 应该成为可信的本地编码伙伴：

- 简单问题可以直接回答。
- 本地文件和目录问题必须真实检查。
- 编码工作默认能使用终端。
- 能读代码、改代码、跑验证、修失败。
- 失败时能明确说失败在哪里，归因给 model、agent flow、tooling、eval harness 或环境。
- 可以反复用真实任务评测，并和 Claude Code、Codex、opencode 对比。

## 下一阶段不做什么

- 不再加新的重量级 workflow 框架。
- 不把多智能体作为主线，先把单 agent 编码可靠性做好。
- 不把长计划文本当成测试和工具证据的替代品。
- 不默认跑所有 live eval。
- 不用命令数量或功能列表声称接近 Claude/opencode。

## 成功指标

### 手工 UX smoke

`docs/AGENT_TESTING_MATRIX_2026-05-08.md` 里的手工 smoke 应该能在安装后的 `pa` 路径下通过：

- 桌面真实检查。
- 目录内容检查。
- 精确破坏性范围。
- terminal 可用性。
- 简单代码创建。
- 无代码解释型回答。

目标：6/6 通过。

### Live eval 五案例

下一阶段不要先跑玩具任务，也不要默认全量跑。先跑五个最能压出问题的案例：

1. `code-change-verification-repair-loop`
2. `live-eval-dashboard-summary`
3. `backend-todo-api-crud`
4. `frontend-book-notes-localstorage`
5. `memory-save-quality-gate`

近期目标：

- 5 个里至少 3 个通过，且需要真实 diff 的任务不能空 diff。
- 当前 run 的 `eval_intent` 和 `failure_owner` 覆盖率达到 100%。
- required command 失败不能被 closeout 说成成功。

后续目标：

- 5 个里至少 4 个通过。
- seeded code-change 任务不能长期停留在 broad inspection。
- 推荐套件里不再出现 `action_checkpoint_no_patch` 或 `action_checkpoint_invalid_tools`。

### 代码健康

近期目标：

- 从 `conversation_loop/mod.rs` 抽出至少 3 个独立模块。
- 每次抽取都保持行为测试通过。
- `cargo clippy --all-features -- -D warnings` 保持干净。

中期目标：

- `conversation_loop/mod.rs` 降到 7000 行以下。
- tool orchestration、validation、repair、closeout 可以分别测试。

## Workstream A：基线重置和证据清理

目的：让仓库文档先反映真实状态。

### 任务

1. 刷新 `docs/PROJECT_STATUS.md`。
   - 日期更新到 2026-05-09。
   - 记录最新提交：
     - `b2ff20c Harden live eval patch recovery`
     - `6df0039 Record live eval recovery evidence`
   - 记录最新测试基线：`1139 passed; 0 failed`。
   - 记录 dashboard-summary 最新恢复通过。

2. 重新生成 live-eval aggregate。
   - 运行 `bash scripts/live-eval-aggregate-summary.sh`。
   - 对比 all-history 和 instrumented slice。
   - 标明历史失败与最新恢复之间的区别。

3. 更新 testing matrix 的当前基线。
   - 哪些是历史报告。
   - 哪些是当前推荐 live suite。
   - 哪个 run 是最新可信恢复证据。

### 验证

```bash
bash scripts/live-eval-aggregate-summary.sh
cargo fmt --check
git diff --check
```

### 完成标准

- 状态文档不再引用过期测试数量。
- 新读者能分清历史失败、当前恢复、下一步评测目标。

## Workstream B：Terminal 和工具真实性

目的：让 terminal 成为编码 agent 的标配能力。

这直接对应用户实测中出现的问题：agent 说 bash 当前不可用，或者只建议命令而不实际检查/安装。

### 设计方向

区分三件事：

1. 建议命令：assistant 告诉用户可以运行什么。
2. 执行 shell：agent 运行有边界的命令并读取输出。
3. 长运行 terminal session：agent 监控、流式输出、取消、汇报进度。

编码任务里，只要权限和平台允许，bash/shell 应该可用。如果不可用，必须说明具体原因。

### 任务

1. 审计 route-scoped bash 暴露。
   - CodeChange、Debugging、Review、ProjectInspection 应该在权限允许时暴露 bash。
   - “检查/安装/运行”这类自然语言请求不能退化成纯文字建议。

2. 增加 terminal 可用性诊断。
   - `/status` 或 `/doctor` 显示 bash 是否对模型暴露。
   - 如果隐藏，说明是路由、权限、平台还是 provider/tool 调用限制。

3. 强化长命令语义。
   - 保留进度提示。
   - 记录取消证据。
   - timeout 和 partial output 进入 trace 和最终回答。

4. 增加回归测试。
   - 检查默认 python 包。
   - 缺包时安装。
   - 运行生成脚本。
   - bash 不可用时给出具体原因。

### 优先检查文件

- `src/tools/bash_tool/`
- `src/tools/mod.rs`
- `src/engine/conversation_loop/tool_metadata.rs`
- `src/engine/conversation_loop/mod.rs`
- `src/engine/intent_router.rs`
- `src/tui/`

### 验证

```bash
cargo test -q bash_tool
cargo test -q command_classifier
cargo test -q route_scoped_tools
cargo test -q tool_metadata
scripts/coding-workflow-gates.sh standard
```

### 完成标准

- 手工 “检查 pygame/python 并安装” smoke 能实际使用 bash。
- 如果 bash 不可用，UI 给出具体策略原因，而不是一句笼统的不可用。

## Workstream C：文件系统真实性和幻觉防线

目的：修掉简单本地事实问答里的幻觉。

### 任务

1. 增加本地事实问题路由和测试。
   - 中文：“桌面有没有 gex 文件夹”“这个文件夹里面有什么”。
   - 英文等价 prompt。
   - 覆盖存在、不存在、隐藏文件、空目录。

2. 简单目录问题优先走结构化文件工具。
   - 能不用 shell 解析就不用 shell。
   - 如果用 shell，回答只引用 shell 输出。

3. 给最终回答增加事实 grounding 检查。
   - 本地 inspect 任务不能补充工具输出里没有的 size、date、count、item。

4. 加一个小型手工 smoke 脚本或 live case。
   - 创建临时目录和文件。
   - 让 agent 检查。
   - 保存报告。

### 优先检查文件

- `src/tools/file_tool/`
- `src/engine/intent_router.rs`
- closeout 相关代码
- `docs/AGENT_TESTING_MATRIX_2026-05-08.md`

### 验证

```bash
cargo test -q file_tool
cargo test -q intent_router
cargo test -q closeout
scripts/coding-workflow-gates.sh quick
```

### 完成标准

- 桌面真实检查不再编造 metadata。
- no-result 回答明确说明检查路径和工具结果。

## Workstream D：Live eval 飞轮

目的：把“感觉变强了”变成可复现的数据。

### 任务

1. 按五案例逐个跑 live suite。
   - 不默认 `--case all`。
   - 有意义的批次提交报告。
   - 遇到清晰 agent-flow failure 就停下来修根因。

2. 统一 report 字段。
   - `eval_intent`
   - `failure_owner`
   - required-command status
   - diff yes/no
   - first write index
   - action checkpoint status
   - closeout status

3. 把 live failure 转成 deterministic test。
   - tool/schema/guard/routing 问题加 unit 或 replay test。
   - 模型推理失败只在 runtime 隐藏了关键信息时调整 evidence 展示。

4. 增加外部基线协议。
   - 同一 task prompt。
   - 同一初始 worktree。
   - 同一 required commands。
   - Claude Code、Codex、opencode 输出保存成可比较 report。

### 验证

```bash
scripts/run_live_eval.sh --case <case> --mode agent-run --run-tests --label capability-now
bash scripts/live-eval-aggregate-summary.sh
```

### 完成标准

- 五案例都有当前 report。
- 每个失败都有 owner 和下一步动作。
- 至少有一个 Claude/Codex/opencode 外部基线 report。

## Workstream E：拆分 conversation loop

目的：降低核心回归风险。

### 目标结构

`conversation_loop/mod.rs` 应该是 turn driver，而不是所有策略的容器。

优先抽取：

1. `tool_orchestrator`
   - exposed tools
   - parallel execution
   - truncation
   - summaries

2. `validation_runner`
   - required validation command extraction
   - sanitized validation env
   - long-running progress
   - validation evidence records

3. `repair_controller`
   - action checkpoint state
   - no-diff detection
   - focused repair prompts
   - retry/fuse rules

4. `patch_recovery`
   - deterministic repair rules
   - patch synthesis validation
   - shell function boundary recovery
   - syntax/semantic guards

5. `closeout_controller`
   - concise/full visibility
   - evidence summary
   - failed/partial/not-verified rendering

6. `turn_trace_adapter`
   - trace event helpers
   - runtime diet metrics
   - live-eval report hooks

### 规则

- 先做行为保持型抽取。
- 能移动测试就跟着移动测试。
- live eval 根因未修完时，不做大重构。
- 每个 commit 要可 review。

### 验证

```bash
cargo fmt --check
cargo test -q patch_synthesis -- --test-threads=1
cargo test -q action_checkpoint -- --test-threads=1
cargo test -q command_classifier -- --test-threads=1
cargo test -q closeout
cargo test -q -- --test-threads=1
cargo clippy --all-features -- -D warnings
```

### 完成标准

- `conversation_loop/mod.rs` 降到 7000 行以下。
- patch recovery、validation、repair、closeout 有独立模块和定向测试。
- 不引入 live eval 回归。

## Workstream F：产品 UX 和 agent mode

目的：让 CLI 像一个可靠编码工具，而不是 debug harness。

### 设计方向

吸收 Claude/opencode 的有效形态：

- 默认 coding/build mode。
- plan/read-only mode。
- explore mode。
- review mode。
- 权限状态可见。
- terminal 进度可见。
- session resume/status 清晰。

这些应该由 permission/config 驱动，不靠 prompt 解释。

### 任务

1. 定义产品 mode。
   - `build`：正常编码。
   - `plan`：只读，允许写计划文件。
   - `explore`：read/search/validation。
   - `review`：read/search/validation/diff，默认不编辑。

2. mode 可见。
   - 启动 banner。
   - `/status`。
   - command palette。
   - trace。

3. 优化命令输出。
   - 默认折叠，可展开。
   - 明确 command/cwd/status/duration。
   - 长命令进度不刷屏。

4. 加强 `/doctor` 或 `/status`。
   - provider/model
   - exposed tools
   - bash availability
   - cwd/workspace
   - permission mode
   - memory/skill context state

### 验证

```bash
cargo test -q quick
cargo test -q status
cargo test -q slash
cargo test -q route_scoped_tools
```

### 完成标准

- 用户在提问前就能知道 agent 当前能不能执行 shell、能不能编辑、在哪个目录。
- 不需要懂内部 env vars 才能判断工具可用性。

## Workstream G：把 memory/skill 变成可测优势

目的：保留项目差异化能力，但避免污染上下文。

### 任务

1. live report 增加 memory usefulness 指标。
   - recalled count
   - manually relevant count
   - conflict count
   - 是否改变计划

2. 继续 route-gated memory。
   - 简单本地事实检查不注入无关 memory。
   - stale memory 不能当作 task fact。

3. skill promotion 绑定真实任务结果。
   - skill 被提升是因为改善任务，不是因为格式像 skill。

4. aggregate report 增加 memory/skill section。

### 验证

```bash
cargo test -q memory
cargo test -q retrieval_context
cargo test -q skills
scripts/coding-workflow-gates.sh standard
```

### 完成标准

- `memory-save-quality-gate` 或 `persistent-memory-planning-context` 中 memory 有正向证据。
- skill promotion 有 replay/live 证据支撑。

## 执行顺序

### Batch 1：基线重置

先做。

- 刷新 `docs/PROJECT_STATUS.md`。
- 重新生成 aggregate summary。
- 把最新 dashboard recovery 写入 testing matrix。

预期提交：

```text
Refresh current agent baseline docs
```

### Batch 2：Terminal 和文件系统真实性

这是用户最直接感知到的信任问题，优先级高于继续大规模 live suite。

- bash availability diagnostics。
- local filesystem truth regression tests。
- manual smoke 文档更新。

预期提交：

```text
Harden terminal and filesystem truth contracts
```

当前进展：

- `d025d6a Add bash exposure diagnostics` 已落地 `/status`/诊断侧的 bash 暴露信息。
- `2b1852e Guard terminal and filesystem grounding` 已落地 runtime 保护：
  bash 已暴露时不允许声称 bash 不可用；本地文件系统事实问答不能在未调用
  `file_read`/`glob` 时直接编造答案。
- 同批修复了 `glob` 的 `**/` 零层匹配和浅层优先排序，避免宽泛搜索截断时隐藏
  `src/main.rs` 这类入口文件。
- 最新确定性验证：`cargo test -q` -> `1139 passed; 0 failed`。

### Batch 3：五案例 live suite

逐个跑，逐个归因。

顺序：

1. `code-change-verification-repair-loop`
2. `backend-todo-api-crud`
3. `frontend-book-notes-localstorage`
4. `memory-save-quality-gate`
5. `live-eval-dashboard-summary`

`live-eval-dashboard-summary` 已有最新通过恢复，但刷新 suite summary 时仍要纳入。

预期提交：

```text
Record live suite baseline
Fix <case> live eval agent-flow gap
```

当前进展：

- `capability-now-20260509-135556/code-change-verification-repair-loop` 已通过：
  `diff=yes`，`required_command_status=ok`，`verification_passed=true`，
  `stage_validation_passed=true`，`closeout_status=passed`，`failure_owner=none`。
- `capability-now-20260509-140733/backend-todo-api-crud` 已通过：
  `diff=yes`，`required_command_status=ok`，`verification_passed=true`，
  `stage_validation_passed=true`，`closeout_status=passed`，`failure_owner=none`；
  但过程有 `tool_errors_seen` 和 patch synthesis old_string 不匹配噪音。
- `capability-now-20260509-141759/frontend-book-notes-localstorage` 已通过：
  `diff=yes`，`required_command_status=ok`，`verification_passed=true`，
  `stage_validation_passed=true`，`closeout_status=passed`，`failure_owner=none`；
  这次修掉了该案例此前的 no-diff 失败形态。
- 当前 aggregate 已刷新到 `38/139`，instrumented slice 为 `16/47`，
  real code-change passes 为 `11`。
- 下一例按顺序跑 `memory-save-quality-gate`。

### Batch 4：conversation loop 抽取

Batch 2 完成并且至少跑过一轮 suite 后再开始，避免无证据重构。

顺序：

1. Extract `patch_recovery`。
2. Extract `validation_runner`。
3. Extract `repair_controller`。
4. Extract `closeout_controller`。
5. Extract `tool_orchestrator`。

预期提交：

```text
Extract patch recovery module
Extract validation runner from conversation loop
Extract repair controller from conversation loop
```

### Batch 5：产品 mode 和 UX

核心真实性和 loop 边界稳定后做。

- build/plan/explore/review modes。
- mode-visible status。
- stronger `/doctor`。
- command output polish。

预期提交：

```text
Add explicit coding agent modes
```

### Batch 6：memory 和 skill 证据化

live suite 当前报告稳定后做。

- memory usefulness metrics。
- skill promotion evidence。
- aggregate memory/skill reporting。

预期提交：

```text
Add memory and skill usefulness reporting
```

## 执行纪律

1. 一个 batch 可以拆多个小 commit。
2. live eval 报告提交和行为修改提交尽量分开。
3. agent-flow failure 尽量转成 deterministic regression test。
4. 每次 prompt 改动都必须说明为什么不能用工具契约解决。
5. 如果修复需要明显增加提示词，先停下来重新判断是不是 runtime 缺口。
6. benchmark artifact 只在能解释决策或建立 baseline 时提交。

## 立即下一步

从 Batch 1 开始。

建议命令：

```bash
bash scripts/live-eval-aggregate-summary.sh
cargo fmt --check
git diff --check
```

然后修改：

- `docs/PROJECT_STATUS.md`
- `docs/AGENT_TESTING_MATRIX_2026-05-08.md`

Batch 1 后的第一个真实实现目标是 Batch 2：terminal 和文件系统真实性契约。
这是修复当前用户感知问题、同时避免重新过度控制 LLM 的最短路径。
