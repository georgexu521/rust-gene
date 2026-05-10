# Priority Agent 下一阶段产品化计划

日期：2026-05-10

本文档承接：

- `docs/LLM_RUNTIME_SIMPLIFICATION_PLAN_2026-05-08.md`
- `docs/NEXT_DEVELOPMENT_PLAN_2026-05-09.md`
- `docs/PROJECT_STATUS.md`
- `docs/AGENT_PRODUCTIZATION_REFERENCE_AUDIT_2026-05-10.md`
- 本地 Claude Code 源码：`/Users/georgexu/Desktop/claude`
- 本地 opencode 源码：`/Users/georgexu/Desktop/opencode-dev`

这个阶段不再把重点放在继续增加 workflow、规则、提示词或实验能力。
下一阶段的重点是把 Priority Agent 从“能力很多的实验型编码 agent”
收敛成“可靠、自然、可长期使用的本地编码终端产品”。

## 核心结论

Priority Agent 现在已经不是基础能力缺失的问题。

当前项目已经具备：

- 交互式 CLI。
- 文件、bash、grep、git、MCP、memory、skill、agent 等工具面。
- intent routing、route-scoped tools、role-scoped subagent profiles。
- trace、closeout、required validation、failure owner、live eval 报告。
- runtime diet、AGENTS.md 压缩、memory/skill 背景化。
- 针对“工具不可用幻觉”“本地文件系统事实幻觉”的部分 guardrail。

但和 Claude Code / opencode 相比，主要差距仍然是产品化运行时：

- Claude Code 的强点是成熟的 tool permission context、BashTool、文件历史、
  app state、session message、MCP、remote/session、UI progress 和恢复路径。
- opencode 的强点是清晰的 service 分层：`SessionPrompt`、`SessionProcessor`、
  `ToolRegistry`、permission、PTY/shell、plugin、server、storage、sync、LSP、
  project/worktree。
- Priority Agent 的能力覆盖面已经很广，但核心路径还偏集中，
  `conversation_loop/mod.rs` 仍承担过多职责，workflow/repair/closeout
  容易进入模型可见路径，影响 LLM 自然解决问题。

下一阶段的总方向：

> 模型负责理解、判断、写代码和沟通；运行时负责工具、权限、终端、状态、证据、回滚和验收。

## 参考优先原则

下一阶段不能只靠我们自己想方案。每个核心改造都要先做
Claude Code / opencode 参考审查，再落到 Priority Agent 的 Rust 实现。

参考不是照抄。参考的目的是回答四个问题：

1. 成熟项目把这个问题放在哪个层解决？
2. 它们靠 prompt、tool schema、permission、session state、UI，还是 runtime check？
3. 哪些设计可以直接借鉴，哪些只是 TypeScript/Bun/React 生态下的实现细节？
4. 我们的 Rust 版本怎样保持同等语义，但不引入新的上帝对象？

每个 Batch 开始前，先写一个很短的 reference note，包含：

- 参考文件。
- 借鉴点。
- 不照抄的原因。
- Priority Agent 的落点文件。
- 验收用例。

参考 note 可以先写在对应 PR/commit 说明里；如果设计较大，再补到本文档或新的
architecture note。

## 参考源码地图

### Claude Code 重点参考

| 主题 | 参考文件 | 借鉴点 |
|------|----------|--------|
| 会话主循环 | `/Users/georgexu/Desktop/claude/src/QueryEngine.ts` | 会话状态、工具上下文、file history、permission context、abort/retry 组合方式 |
| 系统提示组合 | `/Users/georgexu/Desktop/claude/src/utils/systemPrompt.ts` | prompt override、agent prompt、custom prompt、default prompt 的优先级组合 |
| 工具上下文 | `/Users/georgexu/Desktop/claude/src/Tool.ts` | `ToolUseContext` 和 `ToolPermissionContext` 把工具执行、权限、文件缓存、agent 状态分开 |
| Bash 工具 | `/Users/georgexu/Desktop/claude/src/tools/BashTool/BashTool.tsx` | schema、timeout、background、sandbox、命令语义、结果展示、权限反馈 |
| Shell 任务 | `/Users/georgexu/Desktop/claude/src/tasks/LocalShellTask/LocalShellTask.tsx` | 前台/后台 shell task、输出读取、清理、通知 |
| Bash 权限和语义 | `/Users/georgexu/Desktop/claude/src/tools/BashTool/bashPermissions.ts`、`commandSemantics.ts`、`readOnlyValidation.ts` | command 分类、读写判断、危险命令解释 |
| 文件历史 | `/Users/georgexu/Desktop/claude/src/utils/fileHistory.ts` | 编辑前后 snapshot、diff、rewind/restore |
| 权限 UI | `/Users/georgexu/Desktop/claude/src/components/permissions/` | 用户可理解的权限解释和 approval panel |

### opencode 重点参考

| 主题 | 参考文件 | 借鉴点 |
|------|----------|--------|
| Prompt 服务 | `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/session/prompt.ts` | `SessionPrompt` 把 prompt、tool resolving、permission、subtask、shell 串起来 |
| Session Processor | `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/session/processor.ts` | tool call lifecycle、snapshot、permission rejection、stream processing、cleanup |
| Tool Registry | `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/tool/registry.ts` | builtin/custom/plugin tool 统一注册，按 agent permission 过滤 |
| Agent profiles | `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/agent/agent.ts` | build/plan/general/explore 等角色用数据化 permission ruleset 表达 |
| Shell Tool | `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/tool/shell.ts` | shell 命令 parse、路径扫描、权限 ask、输出截断、执行环境 |
| PTY | `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/pty/` | 交互式终端、ticket、平台实现分层 |
| Permission | `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/permission/` | request/reply、ruleset、pending approval、持久化权限 |
| Snapshot | `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/snapshot/index.ts` | session 前后状态跟踪 |
| Storage | `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/storage/` | session/message/part 持久化、迁移、跨运行恢复 |

## 借鉴方式

### 1. 先复制语义，不复制结构

Claude Code 和 opencode 的实现语言、UI 框架、运行时和包生态不同。
我们不应该把 TypeScript 的类、Effect service、React panel 机械翻译成 Rust。

应该复制的是语义：

- prompt 组合有优先级。
- tool context 和 permission context 分离。
- shell 是一等执行能力。
- tool result 有稳定 schema。
- session processor 管 lifecycle，不管工具细节。
- permission denial 是产品路径，不是异常噪声。
- snapshot/file history 是恢复能力，不是 debug 附属品。

### 2. 每个核心模块都要有参考对照

| Priority Agent 模块 | Claude 参考 | opencode 参考 | 我们的目标 |
|---------------------|-------------|---------------|------------|
| `PromptContextAssembler` | `systemPrompt.ts` | `session/system.ts`、`session/prompt.ts` | 短 prompt、可组合、按 route 注入 |
| `SessionProcessor` | `QueryEngine.ts` | `session/processor.ts` | 控制 turn lifecycle，不实现具体工具 |
| `ToolExecutionController` | `Tool.ts`、各 Tool 实现 | `tool/registry.ts`、`tool/tool.ts` | 统一执行、schema、错误、结果规范化 |
| `TerminalController` | `BashTool.tsx`、`LocalShellTask` | `tool/shell.ts`、`pty/`、`shell/shell.ts` | shell/PTY、长命令、后台、取消、输出读取 |
| `PermissionController` | `utils/permissions/`、`components/permissions/` | `permission/`、`agent/agent.ts` | ruleset、ask/reply、解释、持久化 |
| `EvidenceLedger` | file history、tool result message | snapshot、storage、session parts | 文件事实、命令事实、验证事实、closeout 依据 |
| `CloseoutController` | tool summaries、session messages | processor completed/failed tool parts | 基于证据生成简短可信最终回答 |

### 3. 防止“自创框架”回潮

任何新设计如果出现下面信号，要先停下来回看 Claude/opencode：

- 需要新增大量模型可见规则。
- 需要新增一个大而全 coordinator。
- 需要让模型输出复杂 JSON 再由 runtime 解读。
- 简单任务也被拉进 planning/repair/review。
- 为了一个 eval case 新增特殊分支。

优先问：成熟项目是怎么用工具契约、权限、session state、UI 或测试解决的？

## 第一性原理

### 1. Agent 产品的本质是执行环境，不是规则手册

LLM 本身已经会读代码、推理和写代码。项目真正要补足的是：

- 给它真实的本机状态。
- 给它稳定的文件和终端工具。
- 给它清晰且可执行的权限边界。
- 给它快速反馈和失败修复路径。
- 防止它把没有证据的内容说成事实。

如果模型表现差，优先检查工具、权限、上下文、路由、验证和输出契约，
而不是继续增加长提示词。

### 2. 事实必须来自 runtime

下面这些内容不能由模型猜：

- 文件或目录是否存在。
- 目录里有什么。
- 包是否安装。
- 命令是否运行成功。
- 测试是否通过。
- 文件大小、数量、时间、路径、diff。

模型可以解释事实，但事实本身必须来自工具输出、trace 或 runtime 状态。

### 3. 硬约束必须是可执行检查

提示词只能作为软指导。真正的硬约束应该落在：

- tool schema。
- permission rules。
- destructive scope contract。
- stale-read detection。
- validation evidence ledger。
- tool result verifier。
- closeout truth checker。
- route-scoped tool exposure。

尤其是用户已经暴露过的失败：

- 已经删除 `abc.txt` 后，模型又建议删除父文件夹。
- 检查桌面目录时，模型编造大小、数量、创建时间。
- 用户要求安装/运行时，模型只给命令而没有实际用终端。
- provider 因 tool result schema 不匹配返回 400。

这些不应该靠“提醒模型不要这样”来修，应该靠运行时边界和工具结果契约修。

### 4. 简单任务要简单，复杂任务才启用复杂机制

Priority Agent 之前的风险是把所有任务都拉进同一套 workflow 轨道。
下一阶段必须坚持：

- 删除单个文件，不需要规划框架。
- 查看目录，不需要 workflow contract。
- 创建小脚本，不需要多轮验收审查。
- 修复杂代码、失败测试、高风险操作，才需要 repair/validation/closeout 加强。

默认体验应该像 Claude Code / opencode：模型自然完成任务，
runtime 在背后提供 guardrail，而不是把 guardrail 都展示给模型。

### 5. 可维护性直接决定 agent 行为稳定性

当前 `conversation_loop/mod.rs` 仍是最大工程风险。
当一个文件同时处理 prompt、routing、tool execution、permission、repair、
validation、trace、closeout、patch synthesis、memory persistence 时，
任何小修复都可能改变别的行为。

下一阶段必须通过模块边界降低副作用，而不是继续在主循环上叠补丁。

## 目标架构

目标不是照抄 Claude Code 或 opencode，而是吸收它们的结构优点。

### Runtime Spine

建议收敛成以下主干：

```text
User Turn
  -> PromptContextAssembler
  -> IntentRouter
  -> SessionProcessor
      -> ToolExposurePolicy
      -> ToolExecutionController
      -> PermissionController
      -> TerminalController
      -> EvidenceLedger
      -> RepairController
      -> CloseoutController
  -> Assistant Response
  -> Trace / Session Store / Eval Artifacts
```

### 模块责任

| 模块 | 责任 | 不应该负责 |
|------|------|------------|
| `PromptContextAssembler` | 组装短 prompt、AGENTS runtime guidance、必要上下文 | 工具执行、验证、repair |
| `IntentRouter` | 判断任务路线和推荐工具 | 业务执行、最终验收 |
| `SessionProcessor` | 单轮会话状态机和事件编排 | 具体工具实现 |
| `ToolExposurePolicy` | 按 route/role/permission 暴露工具 | 提示词教学 |
| `ToolExecutionController` | 执行工具、规范化结果、捕获错误 | 编写修复策略 |
| `TerminalController` | shell/PTY、长命令、后台任务、取消、输出读取 | 文件编辑 |
| `EvidenceLedger` | 记录验证、diff、工具事实、失败证据 | 美化结论 |
| `RepairController` | 只在失败/卡住时给最小修复引导 | 常规任务规划 |
| `CloseoutController` | 生成基于证据的最终状态 | 替模型写长汇报 |

## 非目标

下一阶段明确不做：

- 不再新建一个更重的 workflow 框架。
- 不把多 agent 作为主线卖点，先把单 agent coding loop 做稳。
- 不用长 prompt 修补工具契约问题。
- 不用功能数量证明接近 Claude Code。
- 不把 live eval 的单个 case 调到通过就当作产品成熟。
- 不默认让模型看到所有历史 memory、skill、workflow 细节。

## 阶段计划

### Phase 0 - 参考审查、当前基线和架构地图

目的：先把下一阶段的参考对象、当前基线和架构地图说清楚，
避免继续在旧结论或主观设计上推进。

任务：

1. 建立 reference audit。
   - Claude Code：会话主循环、BashTool、ToolUseContext、permission UI、
     file history。
   - opencode：SessionPrompt、SessionProcessor、ToolRegistry、Agent profiles、
     ShellTool、PTY、Permission、Storage/Snapshot。
   - 对每个主题写清楚：借鉴语义、不能照抄的实现细节、Priority Agent 落点。

2. 刷新 `docs/PROJECT_STATUS.md`。
   - 当前测试基线。
   - 当前 live eval 可信 run。
   - 当前已知失败类型。
   - 当前 `conversation_loop` 拆分进度。

3. 建立架构地图。
   - 列出 `conversation_loop/mod.rs` 当前职责块。
   - 标记已经抽出的模块：tool orchestrator、repair helpers、closeout helpers、
     validation helpers、patch recovery 等。
   - 标记下一批可抽出的稳定边界。

4. 建立“当前产品能力表”。
   - 文件读写。
   - shell/terminal。
   - 权限。
   - session。
   - memory/skill。
   - MCP。
   - eval。
   - CLI panels。

验收：

```bash
cargo fmt --check
git diff --check
```

完成标准：

- 每个后续 Batch 都有明确参考文件和借鉴点。
- 文档里能一眼看出“现在已完成什么，下一步先动哪里”。
- 不再用过期的历史 pass rate 作为当前状态。

### Phase 1 - 拆主循环，建立 SessionProcessor 边界

目的：降低 `conversation_loop/mod.rs` 的行为风险。

参考：

- Claude Code：`QueryEngine.ts`。
- opencode：`session/processor.ts`、`session/prompt.ts`。

借鉴点：

- session processor 管 lifecycle 和 tool call 状态，不直接承载所有业务策略。
- tool call 有 pending/running/completed/failed/rejected 的明确状态。
- permission rejection、tool failure、snapshot cleanup 是正常路径。

优先拆出：

1. `session_processor.rs`
   - 单轮状态推进。
   - 模型响应和工具调用轮次。
   - turn-level control flow。

2. `tool_execution_controller.rs`
   - 工具调用前准备。
   - 工具执行。
   - tool result normalization。
   - provider-safe tool result formatting。

3. `evidence_ledger.rs`
   - 文件变化证据。
   - validation evidence。
   - acceptance evidence。
   - tool facts。
   - failure facts。

4. `repair_controller.rs`
   - action checkpoint。
   - focused lookup budget。
   - patch-only fallback。
   - deterministic patch synthesis boundary。

5. `closeout_controller.rs`
   - concise closeout。
   - debug/full closeout。
   - not_verified/partial/failed 处理。

设计要求：

- 先搬代码，少改行为。
- 每次只拆一个职责边界。
- 每次拆完跑窄测试。
- 不把新模块设计成新的大对象。

验收：

```bash
cargo test -q route_scoped_tools
cargo test -q closeout
cargo test -q focused_repair_prompt
cargo check -q
```

阶段完成标准：

- `conversation_loop/mod.rs` 明显变薄。
- 工具执行、repair、closeout 可以独立测试。
- provider 400 类 tool result schema 问题有专门测试覆盖。

### Phase 2 - Terminal / Shell 成为一等能力

目的：解决“编码 agent 不能可靠使用终端”的产品缺口。

Claude Code 和 opencode 都把 shell 当作核心产品能力，而不是普通工具。
Priority Agent 下一阶段必须做到：

- 用户要求检查/安装/运行时，agent 默认能使用 bash。
- 如果 bash 不可用，必须说明具体原因。
- 长命令有进度、超时、取消、输出读取。
- 命令输出进入 EvidenceLedger。
- 最终回答不能和命令结果矛盾。

参考：

- Claude Code：`tools/BashTool/BashTool.tsx`、`tasks/LocalShellTask/`、
  `tools/BashTool/bashPermissions.ts`、`tools/BashTool/commandSemantics.ts`。
- opencode：`tool/shell.ts`、`shell/shell.ts`、`pty/`、`permission/arity.ts`。

借鉴点：

- shell tool 需要 parse command，而不是只透传字符串。
- foreground/background、timeout、output truncation、cancel 都是产品语义。
- bash 可见不等于无条件允许，权限应由 command kind 和 path scope 决定。
- 终端不可用必须有可诊断原因，不能让模型猜。

任务：

1. 审计 bash 暴露策略。
   - CodeChange、BugFix、ProjectInspection、Debugging、LocalFilesystem、
     EnvironmentCheck 路由应按权限暴露 bash。
   - 简单“查看目录”可以用 read/list 类工具，但“安装/运行/检查 python 包”
     必须能走 bash。

2. 增加 terminal diagnostics。
   - `/status` 或 `/doctor` 显示 bash 是否对当前 route/model 暴露。
   - 标明隐藏原因：permission、route、platform、provider、registry profile。

3. 强化 shell result schema。
   - stdout/stderr/status/command/cwd/duration/truncated。
   - command kind：read/list/search/validation/install/run/destructive/unknown。
   - provider-safe result serialization，避免 tool result 不符合 provider schema。

4. 支持长命令基础控制。
   - timeout。
   - progress event。
   - cancellation marker。
   - output follow-up。

5. 增加回归用例。
   - “帮我看看默认 python 有没有安装 pygame，帮我安装一下。”
   - “运行这个 Python 脚本。”
   - “cargo test 报错了，帮我修。”
   - “npm install 后跑测试。”

验收：

```bash
cargo test -q bash
cargo test -q route_scoped_tools
cargo test -q tool_result
cargo check -q
```

手工 smoke：

```text
帮我看看我电脑默认的 python 有没有安装 pygame，帮我安装一下吧
```

期望：

- 不应该只打印命令建议。
- 要么真实执行检查/安装，要么给出具体不可执行原因。

### Phase 3 - 事实约束和幻觉防线

目的：让模型不能把没有证据的本地事实说成事实。

参考：

- Claude Code：`Tool.ts`、`BashToolResultMessage.tsx`、`fileHistory.ts`。
- opencode：`session/processor.ts`、`storage/`、`snapshot/index.ts`。

借鉴点：

- tool result 是后续回答的事实来源。
- session part / message / snapshot 应该能还原事实链。
- 文件变化和命令结果要进入独立证据层，而不是散落在 prompt 或最终回答里。

任务：

1. 文件系统事实 grounding。
   - 当用户问“有没有”“里面有什么”“大小/数量/时间”等本地事实时，
     最终回答只能引用工具返回字段。
   - 如果工具没有返回创建时间，不允许补充创建时间。
   - 如果只做 grep，不允许编造完整目录统计。

2. Destructive scope contract。
   - 从用户请求中提取明确批准的 destructive target。
   - 删除后禁止建议删除父目录、兄弟文件或更大范围，除非用户新授权。
   - shell/file delete 都走同一个 scope checker。

3. EvidenceLedger 接入 closeout。
   - `Status: passed` 必须有对应证据。
   - code-generation 任务不能因为验证为空而 passed。
   - required command 失败不能被最终回答写成成功。

4. Tool output truth checker。
   - 针对本地事实回答做轻量 post-check。
   - 检查 assistant final 是否包含工具未提供的高风险字段。
   - 高风险字段包括 size、count、mtime/ctime、installed version、
     test passed、file exists。

验收：

```bash
cargo test -q filesystem_grounding
cargo test -q destructive_scope
cargo test -q closeout
cargo test -q auto_verify
```

回归案例：

- “请帮我看看桌面有没有 gex 文件夹。”
- “里面有什么东西？”
- “帮我把这个文件删了吧。”
- “我该怎么运行刚才创建的 Python 游戏？”

阶段完成标准：

- 不再出现从 `ls -la | grep` 推断大小、数量、创建时间的回答。
- 删除单文件后不会主动扩大删除范围。
- 验证失败和验证缺失不会被包装成成功。

### Phase 4 - 权限和工具 UX 产品化

目的：让安全边界对用户清楚，对模型简单。

参考：

- Claude Code：`components/permissions/`、`utils/permissions/`、
  `tools/BashTool/destructiveCommandWarning.ts`。
- opencode：`permission/`、`agent/agent.ts`、`config/permission.ts`。

借鉴点：

- permission 是用户可理解的产品交互，不是底层错误。
- agent/profile 的默认权限应该是数据化 ruleset。
- destructive/install/network/publish 等风险分类要进入权限解释和 UI。

任务：

1. Permission explanation。
   - 每次拒绝/询问权限，都说明具体原因。
   - 给出最小可批准范围。
   - 避免把权限问题伪装成“工具不可用”。

2. Risk classification。
   - read-only。
   - local edit。
   - validation command。
   - install/network。
   - destructive。
   - external/publish。

3. CLI review panels。
   - bash approval panel。
   - file write/edit preview。
   - destructive target preview。
   - long command progress。

4. Trace-first debugging。
   - `/trace last` 能显示：
     - route。
     - exposed tools。
     - hidden tools and reasons。
     - permission decisions。
     - validation evidence。
     - closeout basis。

验收：

```bash
cargo test -q permissions
cargo test -q tui
cargo test -q trace
cargo check -q
```

阶段完成标准：

- 用户能看懂为什么 agent 能或不能执行某个命令。
- 模型不需要靠长提示词记住所有安全规则。

### Phase 5 - Session、文件历史和恢复能力

目的：让 agent 更接近日常可用产品，而不是一次性脚本执行器。

参考方向：

- Claude Code 有 file history、session messages、tool progress、abort/retry。
- opencode 有 storage、snapshot、sync、worktree、session processor。

参考：

- Claude Code：`utils/fileHistory.ts`、`QueryEngine.ts` 的 file history wiring。
- opencode：`storage/`、`snapshot/index.ts`、`session/processor.ts`。

借鉴点：

- 修改前后 snapshot 是恢复能力的基础。
- session/message/tool part 持久化要支持重启后解释和恢复。
- restore/revert 需要有明确用户动作，不能让模型自由猜回滚范围。

任务：

1. Session store 梳理。
   - 当前会话消息。
   - tool events。
   - file changes。
   - validation runs。
   - permission decisions。

2. File history / snapshot。
   - 修改前快照。
   - 修改后 diff。
   - revert 指令。
   - failed edit recovery。

3. Resume behavior。
   - 重新启动后能看到上次 session summary。
   - 能恢复未完成任务或明确丢弃。

4. Compact context。
   - 长会话压缩保留任务目标、文件变化、验证、失败点。
   - 不把大量 memory/skill 文本重新塞回模型。

验收：

```bash
cargo test -q session
cargo test -q checkpoint
cargo test -q context_collapse
cargo check -q
```

阶段完成标准：

- 一次失败的代码修改可以被用户理解、恢复或回滚。
- 长会话不因为上下文噪音导致模型变笨。

### Phase 6 - 真实任务评测和对标

目的：避免继续优化单个 eval case，建立真实产品能力反馈。

参考：

- Claude Code：以日常 UX 为对照，观察是否主动用工具、是否诚实报告、
  是否有清楚的 permission/terminal UI。
- opencode：以 architecture/service completeness 为对照，观察 session/tool/
  permission/storage/shell 是否形成稳定产品闭环。

借鉴点：

- 对标不是比命令数量，而是比同一真实任务的完成路径。
- 同一任务要记录工具使用、终端行为、失败恢复、最终证据和用户理解成本。

任务：

1. 建立三层 eval。

| 层级 | 目标 | 示例 |
|------|------|------|
| Deterministic unit/replay | 防回归 | route、tool exposure、closeout、schema |
| Local live smoke | 检查真实 agent loop | dashboard、todo api、frontend localstorage |
| Product comparison | 对标 Claude/opencode | 同一任务人工/半自动对比 |

2. 设计 12 个推荐真实任务。

优先覆盖：

- existing Rust bug fix。
- Python script creation and run。
- Node/React small feature。
- backend CRUD。
- failing test repair。
- dependency install and verify。
- filesystem inspection。
- destructive exact-scope action。
- MCP/resource visibility。
- session resume。
- memory/skill low-noise retrieval。
- permission denial recovery。

3. 报告格式统一。

每个任务至少记录：

- eval intent。
- changed files。
- first write turn。
- validation commands。
- validation status。
- closeout status。
- failure owner。
- tool errors。
- hallucination flags。
- whether model made its own patch。

4. 对标方式。

Claude/opencode 对标不追求自动化完全一致。
先用同一任务手工记录：

- 是否主动用终端。
- 是否准确使用文件工具。
- 是否少说废话。
- 是否能从失败修回来。
- 是否诚实报告。
- 用户是否容易理解当前状态。

验收：

```bash
bash -n scripts/run_live_eval.sh
python3 -m py_compile scripts/live_eval_report_parser.py
cargo test -q eval
```

阶段完成标准：

- 至少 12 个真实任务有稳定报告。
- 最新结果和历史 aggregate 分开显示。
- 不再用一个 dashboard case 代表整体能力。

### Phase 7 - MCP、插件和外部产品面

目的：等核心 coding loop 稳定后，再扩产品面。

任务：

1. MCP productization。
   - server health。
   - auth/config diagnosis。
   - prompt/resource/tool visibility。
   - approval UX。

2. Plugin/skill boundary。
   - skill 只作为可加载指导，不默认污染上下文。
   - plugin 工具遵守相同 permission 和 trace。

3. API/server surface。
   - 保持 experimental API 可检查。
   - 不让 API 侧行为绕过 terminal/permission/evidence。

验收：

```bash
cargo check --features experimental-api-server -q
cargo test -q mcp
cargo test -q skill
```

## 推荐执行顺序

### Batch 1：Reference audit、状态文档和架构地图

Status: completed on 2026-05-10.

目标：先把 Claude Code/opencode 的成熟做法对齐清楚，再把当前状态、
已拆模块、下一步拆分点写清楚。

Primary artifact:

- `docs/AGENT_PRODUCTIZATION_REFERENCE_AUDIT_2026-05-10.md`

改动范围：

- `docs/PROJECT_STATUS.md`
- `docs/NEXT_AGENT_PRODUCTIZATION_PLAN_2026-05-10.md`
- 新增或更新 architecture map 文档。

必须产出：

- SessionProcessor 参考 note。
- Bash/PTY 参考 note。
- Permission 参考 note。
- Evidence/session storage 参考 note。
- 每个 note 都要写明“借鉴语义”和“不照抄边界”。

验证：

```bash
cargo fmt --check
git diff --check
```

### Batch 2：Tool result schema 和 provider-safe serialization

Status: completed on 2026-05-10.

目标：优先解决 MiniMax 400 这类基础稳定性问题。

参考：

- Claude Code：`Tool.ts` 和 Bash/File 工具结果结构。
- opencode：`session/processor.ts` 的 tool call lifecycle、`tool/registry.ts`
  的统一工具接口。

改动范围：

- tool result normalization。
- shell/file/read/edit result schema。
- provider adapter 测试。

First completed slice:

- OpenAI-compatible and Kimi request conversion now omit empty assistant
  `content` when the assistant message is purely a `tool_calls` payload. This
  matches the schema rule that assistant content is only required when no
  tool/function call is present, and avoids strict compatible providers
  misreading a tool-call turn.

Second completed slice:

- Tool execution now has an internal `ToolExecutionRecord` that separates
  provider-facing text from machine metadata. Provider-visible tool messages
  are generated through one shared function, while command classification,
  duration, path, and error previews stay available as structured runtime
  metadata instead of being mixed into the text sent back to the model.

验证：

```bash
cargo test -q tool_result
cargo test -q provider
cargo check -q
```

### Batch 3：Terminal availability 和 bash route exposure

Status: completed on 2026-05-10.

目标：让编码任务稳定看到并使用 bash。

参考：

- Claude Code：`BashTool.tsx`、`LocalShellTask/`、Bash permission helpers。
- opencode：`tool/shell.ts`、`pty/`、`shell/shell.ts`。

改动范围：

- route-scoped tools。
- bash diagnostics。
- `/status` 或 `/doctor`。

First completed slice:

- Learning feedback no longer removes recently failing tools from the route
  recommendation list. It still records the failure signal in the route reason,
  but terminal/runtime requests keep `bash` exposed when bash is the tool needed
  to inspect, install, run, or validate local state.

Second completed slice:

- `/doctor` route exposure diagnostics now use the current session's learning
  events, matching the real turn routing path more closely. The diagnostic tests
  cover the prior failure mode where repeated bash failures could otherwise make
  terminal tasks look as if bash were unavailable.

验证：

```bash
cargo test -q route_scoped_tools
cargo test -q bash
cargo check -q
```

### Batch 4：EvidenceLedger 第一版

Status: completed on 2026-05-10.

目标：把文件事实、命令事实、验证事实和 closeout 分开。

参考：

- Claude Code：file history 和 tool result message。
- opencode：snapshot、storage、session parts。

改动范围：

- 新模块 `evidence_ledger.rs`。
- closeout controller 接入。
- filesystem grounding tests。

First completed slice:

- Added `src/engine/evidence_ledger.rs` as the first runtime-owned evidence
  ledger. It records file facts, changed files, shell command facts, and
  validation facts separately from model-visible tool result text.
- `ConversationLoop` now writes tool, file-change, validation, diff, and code
  review facts into the ledger. `FinalCloseoutContext` reads the ledger for the
  runtime validation evidence label while keeping the user-facing closeout
  compact.

Second completed slice:

- Added filesystem grounding checks to catch answers that add unsupported
  metadata such as creation time, item count, or size after a local filesystem
  inspection. If the ledger does not contain evidence for those fields,
  `ConversationLoop` requests one evidence-grounded retry instead of letting the
  model present inferred metadata as fact.

验证：

```bash
cargo test -q closeout
cargo test -q filesystem_grounding
cargo test -q auto_verify
```

### Batch 5：ConversationLoop 第一轮拆分

Status: completed on 2026-05-10.

目标：行为保持不变，先拆出最稳定职责。

参考：

- Claude Code：`QueryEngine.ts` 的会话级编排。
- opencode：`SessionPrompt` + `SessionProcessor` 分层。

改动范围：

- `session_processor.rs`
- `tool_execution_controller.rs`
- `repair_controller.rs`

First completed slice:

- Moved runtime diet accounting and `RuntimeDietReport` emission out of
  `conversation_loop/mod.rs` into `conversation_loop/runtime_diet.rs`. This is a
  behavior-preserving extraction and keeps prompt/tool/memory/retrieval
  accounting separate from the turn execution loop.

Second completed slice:

- Added `conversation_loop/tool_result_controller.rs` for the shared path that
  records tool-result evidence, appends provider-safe tool result text, and
  writes the matching `Message::Tool`. This keeps repeated tool-result plumbing
  out of the main loop while leaving streaming UI completion events unchanged.

Third completed slice:

- Added `conversation_loop/workflow_trace.rs` and moved workflow feedback,
  stage-validation, and adaptive-trigger trace helpers out of the main loop.
  This keeps trace serialization near workflow telemetry instead of embedding it
  in turn execution control flow.

Fourth completed slice:

- Moved changed-file diff evidence generation into `evidence_ledger`, so the
  main loop no longer builds git diff evidence directly. This keeps file-change
  evidence collection with the runtime-owned evidence layer.

Fifth completed slice:

- Added `conversation_loop/runtime_timeouts.rs` for LLM request and stream idle
  timeout configuration. The main loop now consumes timeout policy instead of
  owning environment parsing details.

Sixth completed slice:

- Added `conversation_loop/tool_execution_controller.rs` and moved
  `execute_tools_parallel` out of the main loop. Tool exposure checks, resource
  policy limits, destructive-scope blocking, approval prompts, hooks,
  pre-executed read-only results, trace events, and provider-safe tool-result
  completion are preserved in the extracted controller.

Seventh completed slice:

- Added `conversation_loop/session_processor.rs` and moved the non-streaming
  and streaming LLM request lifecycle out of the main loop. API timeout
  handling, streaming text sanitization, usage events, read-only pre-execution,
  fallback to non-streaming, cost tracking, recovery-plan tracing, and trace
  finish persistence are preserved behind the same `ConversationLoop` methods.

验证：

```bash
cargo test -q runtime_diet_report_is_recorded_for_real_loop_turn
cargo test -q test_coding_quality_tracks_fail_then_repair_cycle
cargo test -q test_tool_specific_confirmation_blocks_git_push_without_approval
cargo test -q test_unexposed_tool_call_is_denied_before_execution
cargo test -q destructive_scope_blocks_parent_delete_before_bash_execution
cargo test -q focused_repair_prompt
cargo test -q closeout
cargo test -q route_scoped_tools
cargo check -q
```

### Batch 6：真实任务评测套件

Status: started on 2026-05-10.

目标：用真实任务证明产品变好，而不是只证明某个 case 过了。

参考：

- Claude Code：同一任务的用户可见交互质量。
- opencode：同一任务的 service/tool/session 结构完整性。

改动范围：

- `docs/AGENT_TESTING_MATRIX_2026-05-08.md`
- eval case definitions。
- report parser / aggregate summary。

First completed slice:

- Added `scripts/run_live_eval.sh --case recommended` as a first-class entry
  for the recommended live suite, with `--case recommended --list` for a
  deterministic view of the suite. Expanded the testing matrix from the old
  six-case product signal to a 12-case suite covering real implementation,
  validation/repair, eval reporting, memory, permissions, skill promotion,
  resume, and CLI scrollback behavior.

Second completed slice:

- Ran the first two recommended live evals after the ConversationLoop split:
  `code-change-verification-repair-loop` and `live-eval-dashboard-summary`.
  Both produced real code diffs, passed required commands including full
  `cargo test -q -- --test-threads=1` with `1174 passed; 0 failed`, generated
  summary reports, and recorded `failure_owner=none`. The dashboard case still
  shows a non-fatal workflow-judgment JSON parse warning before runtime
  fallback and recovery, so parse-noise reduction remains a follow-up.

Third completed slice:

- Reduced workflow-judgment parse noise by downgrading recoverable non-JSON
  judgment responses to debug-level fallback while still surfacing schema
  errors. Added a MiniMax success-body fallback parser for cases where the
  async client rejects a 200 OK body that still contains valid content or
  `tool_calls`. The dashboard rerun `batch6-parsefix-20260510-141148` passed
  with real diff, required commands ok, full `1178 passed; 0 failed`,
  `failure_owner=none`, and no workflow-judgment parse warning in stderr.

Fourth completed slice:

- Ran `backend-todo-api-crud` as the next recommended live eval:
  `batch6-smoke-20260510-142800` passed with a real backend diff, required
  unittest and no-TODO checks ok, `closeout_status=passed`, and
  `failure_owner=none`. The report recorded earlier verification and
  stage-validation failures before repair, then final required commands passed;
  this makes the case useful as both a backend implementation guard and a
  repair-after-failed-validation guard.

Fifth completed slice:

- Ran `frontend-book-notes-localstorage` as the next recommended live eval:
  `batch6-smoke-20260510-143451` passed with a real frontend diff, required
  Node behavior test ok, no-TODO check ok, `acceptance_accepted=True`,
  `closeout_status=passed`, and `failure_owner=none`. This keeps the frontend
  persistence/product-completeness guard current after the loop split.

验证：

```bash
bash -n scripts/run_live_eval.sh
scripts/run_live_eval.sh --case recommended --list
python3 -m py_compile scripts/live_eval_report_parser.py
cargo test -q eval
cargo test -q minimax
cargo test -q workflow_contract
cargo check -q
scripts/run_live_eval.sh --case code-change-verification-repair-loop --mode agent-run --run-tests --timeout 1800 --idle-timeout 300 --label batch6-smoke
scripts/run_live_eval.sh --case live-eval-dashboard-summary --mode agent-run --run-tests --timeout 1800 --idle-timeout 300 --label batch6-smoke
scripts/run_live_eval.sh --case live-eval-dashboard-summary --mode agent-run --run-tests --timeout 1800 --idle-timeout 300 --label batch6-parsefix
scripts/run_live_eval.sh --case backend-todo-api-crud --mode agent-run --run-tests --timeout 1800 --idle-timeout 300 --label batch6-smoke
scripts/run_live_eval.sh --case frontend-book-notes-localstorage --mode agent-run --run-tests --timeout 1800 --idle-timeout 300 --label batch6-smoke
scripts/run_live_eval.sh --mode summary --run-id batch6-smoke-20260510-133309
scripts/run_live_eval.sh --mode summary --run-id batch6-smoke-20260510-133944
scripts/run_live_eval.sh --mode summary --run-id batch6-parsefix-20260510-141148
scripts/run_live_eval.sh --mode summary --run-id batch6-smoke-20260510-142800
scripts/run_live_eval.sh --mode summary --run-id batch6-smoke-20260510-143451
```

## 验收指标

### 产品体验指标

- 本地文件系统问题必须先查再答。
- 终端/安装/运行问题默认使用 bash，除非有明确不可用原因。
- 简单任务最终回答不出现流程化 closeout。
- 复杂任务 final answer 能给出真实验证证据。
- 失败时能明确说失败原因和下一步。

### 架构指标

- `conversation_loop/mod.rs` 继续下降。
- tool execution、repair、closeout、evidence 可以单测。
- 新增行为优先进入 runtime/tool contract，而不是 base prompt。
- route-scoped tools 保持默认开启。

### 评测指标

- 推荐真实任务集至少 12 个。
- 最近一次 recommended suite 单独展示，不和历史失败混在一起。
- failure_owner 覆盖率 100%。
- code-change 任务不能空 diff 通过。
- required validation 失败不能 closeout passed。

## 风险和应对

### 风险 1：继续 overfit eval

表现：

- dashboard case 越来越强，但真实任务仍然不自然。

应对：

- 每个修复都要问：这是通用 runtime 缺口，还是只修一个 case？
- 新增真实任务套件。
- 记录 hallucination flags 和 terminal behavior。

### 风险 2：拆主循环时引入行为回归

表现：

- 编译过了，但 live agent 行为变差。

应对：

- 每次只拆一个职责块。
- 先搬代码再改行为。
- 保留窄测试和一条 live smoke。

### 风险 3：又把规则加回 prompt

表现：

- base prompt 重新变长。
- 模型输出越来越像流程报告。

应对：

- prompt budget gate 保持。
- 新规则优先实现为 tool contract/runtime check。
- 只在失败/高风险/显式 debug 时注入额外指导。

### 风险 4：terminal 权限过宽

表现：

- bash 可用了，但危险命令更容易执行。

应对：

- bash 可见不等于 bash 全放行。
- destructive/install/network/publish 分级权限。
- approval panel 给用户看具体命令、cwd、风险、范围。

## 当前最应该先做的三件事

1. 先做 Batch 1 的 reference audit，不要跳过。
   - 这一步不是写长文档，而是给后续代码改造定方向。
   - 尤其要先看 Claude/opencode 的 session、shell、permission、tool result。

2. 再做 Batch 2：tool result schema 和 provider-safe serialization。
   - 这是 MiniMax 400 这类硬错误的根因方向。
   - 不解决这个，用户会遇到“问一句就挂”的体验。

3. 接着做 Batch 3：Terminal availability 和 bash route exposure。
   - 编码 agent 必须能稳定用终端。
   - 这也是 Claude Code / opencode 的基本能力。

4. 同步推进 Batch 4：EvidenceLedger 第一版。
   - 解决文件系统事实幻觉、验证空证据、closeout 虚假成功。
   - 这是“少提示词、多运行时约束”的核心落地点。

Batch 1 不应该拖很久，但应该先完成最小 reference audit。
否则后面的实现容易又变成自创框架。

## 最终目标

下一阶段完成后，Priority Agent 应该达到这个状态：

- 普通用户感觉它像一个可靠的本地 coding terminal agent。
- 简单任务完成得直接、短、准。
- 复杂任务有读-改-测-修闭环。
- 终端、权限、证据、回滚是产品能力，不是模型口头承诺。
- runtime 不再过度操控模型，而是在关键位置提供硬 guardrail。
- 和 Claude Code / opencode 的差距从“基础产品能力缺口”收敛到
  “生态、 polish、平台覆盖和长期稳定性”。
