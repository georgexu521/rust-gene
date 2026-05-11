# Priority Agent 核心编码质量下一阶段计划

日期：2026-05-11

本文档承接：

- `docs/NEXT_AGENT_PRODUCTIZATION_PLAN_2026-05-10.md`
- `docs/LLM_RUNTIME_SIMPLIFICATION_PLAN_2026-05-08.md`
- `docs/PROJECT_STATUS.md`
- 本地 Claude Code 源码：`/Users/georgexu/Desktop/claude`
- 本地 opencode 源码：`/Users/georgexu/Desktop/opencode-dev`

这份计划不是再写一个总路线，而是把下一阶段执行顺序收敛为三条主线：

1. 先拆分主循环，降低行为副作用。
2. 再把 shell / terminal 做成一等编码能力。
3. 最后补齐文件编辑质量，尤其是 stale read、编码、换行、锁、diff、LSP 和回滚。

## 当前判断

Priority Agent 的基础编码能力已经不再是空白：

- 有 `file_read`、`grep`、`glob`、`file_edit`、`file_write`、`bash`、`git`、`format`、`lsp`。
- 有 route-scoped tools、权限上下文、closeout、EvidenceLedger、live eval、provider retry 和 provider-safe tool result work。
- 最近全量本地测试基线是 `1204 passed; 0 failed`。

但还没有完全赶上 Claude Code / opencode 的核心编码质量。差距主要不是功能数量，而是运行时产品化程度：

- 主循环仍然过重，`src/engine/conversation_loop/mod.rs` 还有 5600+ 行。
- shell 仍是普通工具，不是完整终端运行时。
- 文件编辑工具已经有 stale-read 检测和路径身份修复，但还缺成熟产品里的编码、换行、锁、diff、LSP、历史恢复等细节。

## 参考结论

### Claude Code 值得借鉴的语义

参考文件：

- `/Users/georgexu/Desktop/claude/src/query.ts`
- `/Users/georgexu/Desktop/claude/src/Tool.ts`
- `/Users/georgexu/Desktop/claude/src/tools/BashTool/BashTool.tsx`
- `/Users/georgexu/Desktop/claude/src/tasks/LocalShellTask/`
- `/Users/georgexu/Desktop/claude/src/tools/FileEditTool/FileEditTool.ts`

借鉴点：

- query loop 负责会话推进和 context budget，不把所有工具细节塞在主循环里。
- `ToolUseContext` 和 `ToolPermissionContext` 把工具执行、权限、文件读取状态、UI 状态、agent 状态分开。
- BashTool 不只是执行命令，还处理命令语义、权限、timeout、background、sandbox、输出展示和任务状态。
- FileEditTool 强制 read-before-edit，检查外部修改，保留 encoding / line endings，更新 file history 和 LSP。

### opencode 值得借鉴的语义

参考文件：

- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/session/processor.ts`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/session/prompt.ts`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/tool/tool.ts`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/tool/registry.ts`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/tool/shell.ts`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/tool/truncate.ts`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/tool/edit.ts`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/permission/`
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/pty/`

借鉴点：

- `SessionProcessor` 管 tool call lifecycle：pending、running、completed、error、cleanup、snapshot。
- `Tool.define` 统一 schema decode、执行、truncation 和 metadata。
- shell 会扫描命令、路径和权限，输出过长时落盘并给模型可继续读取的路径。
- edit tool 有 per-file lock、BOM、换行、format、LSP diagnostics 和 snapshot diff。
- permission 是 ruleset 和 runtime ask，不是提示词里的提醒。

## 第一性原则

1. LLM 负责理解、判断、写代码和解释。
2. runtime 负责工具、终端、权限、文件事实、证据、回滚和验收。
3. 硬约束必须放到 tool schema、permission、file state、terminal state、EvidenceLedger 和 tests。
4. 不用长 prompt 修补工具契约问题。
5. 简单任务不要被 workflow 框架绑住，复杂任务才启用 repair、validation、closeout。

## 非目标

- 不新增一个更大的 coordinator。
- 不把所有能力都默认暴露给模型。
- 不为了单个 live eval case 写特殊分支。
- 不把 Claude/opencode 的 TypeScript/React/Effect 结构机械翻译到 Rust。
- 不把“功能数量多”当作赶上 Claude Code 的证明。

## Phase 1：拆分主循环

目标：让 `ConversationLoop::run_inner` 退回到会话编排层，把具体职责移动到可测试的小模块。

当前已有拆分：

- `session_processor.rs`
- `tool_execution_controller.rs`
- `tool_result_controller.rs`
- `tool_orchestrator.rs`
- `repair_controller.rs`
- `closeout_controller.rs`
- `validation_runner.rs`
- `action_checkpoint.rs`
- `patch_recovery.rs`
- `patch_repair_rules.rs`
- `runtime_diet.rs`
- `runtime_timeouts.rs`
- `turn_recording.rs`

问题是这些模块很多仍然是 `impl ConversationLoop` 的横向切片，状态和职责还耦合在 `ConversationLoop` 上。下一步不是继续随便抽函数，而是建立清晰边界。

### Batch 1.1：主循环职责地图和行为冻结

任务：

- 给 `conversation_loop/mod.rs` 当前职责分区：
  - prompt/context assembly
  - intent route 和 resource policy
  - model request/streaming
  - tool exposure
  - tool call lifecycle
  - tool result normalization
  - evidence ledger
  - repair/action checkpoint
  - validation
  - closeout
  - memory/learning persistence
  - trace/session store
- 给每个职责标注目标模块和迁移状态。
- 写一个很小的 architecture note 或直接补到本文件的进度区。

验收：

```bash
cargo fmt --check
git diff --check
```

完成标准：

- 后续每个拆分 commit 都能说明“从哪个职责区搬到哪个模块”。
- 不再通过猜测判断主循环能不能继续拆。

### Batch 1.2：建立 `TurnRuntimeState`

参考：

- Claude `query.ts` 在每轮顶部显式拆出 state。
- opencode `ProcessorContext` 明确保存 toolcalls、snapshot、blocked、needsCompaction、currentText、reasoningMap。

任务：

- 新增或完善 `TurnRuntimeState`，承载一轮运行中状态：
  - route
  - resource policy
  - exposed tools
  - pending tool calls
  - pre-executed read-only results
  - changed files before/after
  - validation labels
  - closeout visibility
  - trace handles
- 把散落在 `run_inner` 的局部变量逐步收进 state。
- 第一批只移动数据承载，不改变行为。

验收：

```bash
cargo test -q route_scoped_tools -- --test-threads=1
cargo test -q runtime_diet -- --test-threads=1
cargo check -q
```

完成标准：

- `run_inner` 的局部状态数量明显减少。
- tool exposure、repair、closeout 读取 state，而不是互相传长参数串。

### Batch 1.3：让 `SessionProcessor` 成为状态机，而不是 helper 文件

参考：

- opencode `session/processor.ts` 用事件处理 `tool-input-start`、`tool-call`、`tool-result`、`finish-step`、cleanup。
- Claude `query.ts` 不依赖 `stop_reason` 判断工具调用，而是看实际 streamed tool_use block。

任务：

- 定义 `SessionProcessor` 的输入和输出：
  - input：messages、tools、route、runtime state、provider handle。
  - output：assistant text、tool calls/results、usage、finish reason、evidence events。
- 把 provider request、stream handling、tool-call collection 迁出 `mod.rs`。
- 给 tool call lifecycle 建立状态枚举：
  - pending
  - running
  - completed
  - failed
  - denied
  - provider_executed
- 把 streaming fallback、pre-executed read-only tool result、tool result attach 统一进状态机。

验收：

```bash
cargo test -q prompt_context -- --test-threads=1
cargo test -q route_scoped_tools -- --test-threads=1
cargo test -q tool_result -- --test-threads=1
cargo check -q
```

完成标准：

- `run_inner` 不再直接管理 streamed tool call 的内部生命周期。
- provider 400 类 tool result schema 问题有固定归属，不再散落在主循环。

### Batch 1.4：抽出 `ToolCallLifecycle` 和 `ToolResultNormalizer`

参考：

- opencode `Tool.define` 统一 decode、execute、truncate、metadata。
- Claude `ToolDef` 有 strict、max result size、validateInput、outputSchema。

任务：

- 从 `tool_execution_controller.rs` 里拆出：
  - 参数 schema 校验和错误格式化。
  - execution metadata。
  - provider-facing tool result content。
  - user-facing tool result summary。
  - truncation/large output handling。
- 让 bash/file/edit/git 的 result schema 进入统一 normalizer。
- 所有 tool result 都必须区分：
  - model content
  - UI content
  - structured metadata
  - evidence facts

验收：

```bash
cargo test -q provider -- --test-threads=1
cargo test -q evidence_ledger -- --test-threads=1
cargo test -q closeout -- --test-threads=1
cargo check -q
```

完成标准：

- provider-safe serialization 是 normalizer 的职责。
- closeout 不再从原始 stdout/stderr 里临时猜事实。

### Batch 1.5：拆 `RepairController` 和 deterministic repair 边界

任务：

- 把 repair 入口统一成：
  - failure evidence in
  - allowed repair budget in
  - proposed next action out
- deterministic patch synthesis 只能作为明确 fallback，并记录 owner/reason。
- action checkpoint 只约束“此刻允许哪些工具”，不要注入大量模型规则。

验收：

```bash
cargo test -q focused_repair -- --test-threads=1
cargo test -q action_checkpoint -- --test-threads=1
bash scripts/workflow-production-gates.sh
```

完成标准：

- repair 失败时能说明是 tool boundary、model reasoning、validation、provider 还是 harness。
- 不再通过增加 prompt 段落修 repair 行为。

### Phase 1 完成标准

- `conversation_loop/mod.rs` 从 5600+ 行降到 3500 行以内。
- 后续目标是 2500 行以内，但第一阶段不为行数破坏清晰度。
- 主循环只负责高层顺序：route、prompt、session processor、tool lifecycle、closeout。
- 每个核心行为都有独立测试入口。

## Phase 2：shell / terminal 一等化

目标：让 Priority Agent 在基本编程任务上像 Claude Code / opencode 一样可靠使用终端。

当前问题：

- `bash` 已可执行命令，但仍像普通工具。
- 长命令、后台任务、输出继续读取、交互式 PTY、取消、输出落盘还不完整。
- route/permission 隐藏 bash 时，模型有时只能给命令文本，用户体验会倒退。

### Batch 2.1：终端可见性和诊断

任务：

- 完善 `tool_exposure` 诊断：
  - registry 是否注册
  - tool 是否 available
  - permission 是否暴露
  - route 是否允许
  - provider/tool schema 是否兼容
- 在 `/status` 或 `/doctor` 暴露当前 bash 状态。
- 对用户问题“检查/安装/运行/测试/启动/默认 python/package”强制走 terminal-capable route。

验收：

```bash
cargo test -q tool_exposure -- --test-threads=1
cargo test -q intent_router -- --test-threads=1
```

完成标准：

- 用户问“帮我看看默认 python 有没有安装 pygame，帮我安装一下”时，模型能看到 `bash`。
- 如果不能看到，UI/诊断能说清楚具体原因。

### Batch 2.2：统一 `ShellCommandClassification`

参考：

- Claude `BashTool/commandSemantics.ts`
- Claude `BashTool/bashPermissions.ts`
- opencode `tool/shell.ts`

任务：

- 把现有 bash classifier 和 destructive scope 共享到一个语义层。
- 命令分类至少包括：
  - read
  - list
  - search
  - validation
  - package_install
  - dev_server
  - test_run
  - file_mutation
  - git_mutation
  - destructive
  - unknown
- 分类结果进入：
  - permission
  - progress label
  - EvidenceLedger
  - closeout
  - UI summary

验收：

```bash
cargo test -q bash_tool -- --test-threads=1
cargo test -q destructive_scope -- --test-threads=1
cargo test -q progress -- --test-threads=1
```

完成标准：

- shell 语义只维护一份，不在 bash tool、permission、trace、closeout 各写一套。

### Batch 2.3：Shell result schema 和输出落盘

参考：

- opencode `tool/truncate.ts`
- Claude tool result storage / BashTool output handling

任务：

- 标准化 shell result：
  - command
  - cwd
  - exit_code
  - stdout_preview
  - stderr_preview
  - output_path
  - duration_ms
  - timed_out
  - truncated
  - classification
  - evidence_status
- 超过阈值的 stdout/stderr 写入 `.priority-agent/tool-results/` 或 session artifact 目录。
- tool result 给模型的是预览和可继续读取路径，不直接塞完整长输出。
- `file_read` / `grep` 可以读取 output artifact。

验收：

```bash
cargo test -q bash_tool -- --test-threads=1
cargo test -q evidence_ledger -- --test-threads=1
cargo test -q closeout -- --test-threads=1
```

完成标准：

- 大输出不会污染上下文。
- 模型可以用工具继续检查完整输出。
- closeout 只引用结构化 evidence，不从截断文本猜。

### Batch 2.4：前台、后台、取消和继续读取

参考：

- Claude `LocalShellTask`
- opencode `pty/` 和 `shell/shell.ts`

任务：

- 新增 terminal task abstraction：
  - task id
  - command
  - cwd
  - status
  - started_at / ended_at
  - output artifact
  - cancel handle
- 支持：
  - foreground command
  - background command
  - read output by task id
  - stop task
  - timeout kill process group
- UI 显示 active shell task。

验收：

```bash
cargo test -q bash_tool -- --test-threads=1
cargo test -q terminal -- --test-threads=1
cargo check -q
```

完成标准：

- dev server、watch test、长安装命令不再卡死主 loop。
- 用户可以让 agent 启动服务，再继续读输出或停止。

### Batch 2.5：PTY 能力和交互式终端边界

任务：

- 先做非交互式 PTY smoke，避免一开始就扩大范围。
- 明确哪些命令应该走普通 `bash`，哪些应该走 PTY：
  - 普通测试、安装、脚本运行：bash
  - REPL、交互式 CLI、需要持续读取屏幕：PTY
- 如果 PTY 不可用，给出可诊断原因。

验收：

```bash
cargo test -q terminal -- --test-threads=1
cargo check -q
```

完成标准：

- terminal 能力有明确边界，不再出现“bash 工具不可用，只能让用户手动运行”的退化。

### Phase 2 完成标准

- 用户要求检查环境、安装包、运行脚本、启动项目时，agent 默认能实际执行。
- 长输出可落盘并继续读取。
- 长命令可后台运行、取消、读取输出。
- bash 结果进入 EvidenceLedger，最终回答不和命令事实矛盾。

## Phase 3：文件编辑质量追上成熟编码 agent

目标：把文件编辑从“能替换文本”提升到“长期安全写代码”的产品级能力。

当前已有能力：

- 路径边界和只读根。
- 文件大小限制。
- stale-read 检测。
- line_start / line_end 编辑。
- 多 occurrence guard。
- checkpoint。
- 最近新增：read/edit 状态使用解析后的规范路径。

缺口：

- encoding / BOM / line ending 保真。
- per-file edit lock。
- atomic write。
- read-before-edit 默认策略还不够清晰。
- LSP/format feedback 和 edit result 没有深度集成。
- file history / rollback 和用户可见 diff 还没有达到 Claude/opencode 级别。

### Batch 3.1：文件身份和 read state 整理

任务：

- 把 `file_state_key`、read state、file cache、checkpoint 统一到一个 `FileStateTracker`。
- 明确 path identity：
  - lexical path
  - resolved path
  - canonical path
  - display path
- read state 记录：
  - full read vs partial read
  - content hash
  - mtime
  - line range
  - session id

验收：

```bash
cargo test -q file_tool -- --test-threads=1
```

完成标准：

- `./a.rs`、`a.rs`、`/abs/a.rs` 不再绕过 stale-read 检测。
- partial read 的编辑策略明确，不误认为完整上下文已经读过。

### Batch 3.2：encoding、BOM、line ending 保真

参考：

- Claude `FileEditTool` 的 encoding / line endings 处理。
- opencode `Bom.readFile`、`detectLineEnding`、`convertToLineEnding`。

任务：

- 读取文件时记录：
  - utf8 / utf16le / unknown
  - BOM
  - LF / CRLF
- 编辑写回时保留原编码和换行。
- 对 binary/unknown encoding 给出清晰错误。

验收：

```bash
cargo test -q file_tool -- --test-threads=1
```

测试用例：

- CRLF 文件编辑后仍是 CRLF。
- UTF-8 BOM 文件编辑后仍保留 BOM。
- binary 文件拒绝文本编辑。

### Batch 3.3：per-file lock 和 atomic edit

参考：

- opencode `edit.ts` 的 file lock。
- Claude FileEditTool 的读写临界区。

任务：

- 为每个 canonical path 建立 async lock。
- staleness check 和 write 在同一临界区完成。
- 写文件用临时文件加 rename，避免半写入。
- checkpoint 在写前创建，失败时不污染 read state。

验收：

```bash
cargo test -q file_tool -- --test-threads=1
cargo test -q checkpoint -- --test-threads=1
```

完成标准：

- 并发编辑同一文件不会互相覆盖。
- 写失败不会把文件状态标成成功编辑。

### Batch 3.4：diff、format、LSP diagnostics 进入 edit result

参考：

- Claude FileEditTool 的 patch、LSP notify、diagnostics。
- opencode edit tool 的 diff、format、LSP diagnostic report。

任务：

- `file_edit` result 返回：
  - file path
  - replacements
  - changed line range
  - additions/deletions
  - unified diff preview
  - diagnostics summary
- 若项目有 formatter，可按配置或 route 运行。
- LSP diagnostics 不阻塞所有编辑，但必须进入 evidence。

验收：

```bash
cargo test -q file_tool -- --test-threads=1
cargo test -q lsp -- --test-threads=1
cargo test -q closeout -- --test-threads=1
```

完成标准：

- 模型和最终回答都能引用真实 diff / diagnostics，而不是猜代码是否正确。

### Batch 3.5：文件历史和 rollback 产品化

任务：

- 把 checkpoint、file history、diff viewer、rollback 统一。
- 每次 edit/write 记录：
  - before hash
  - after hash
  - diff
  - tool call id
  - user/session id
  - timestamp
- `/rollback` 能按最近 edit/write 选择恢复。

验收：

```bash
cargo test -q checkpoint -- --test-threads=1
cargo test -q rollback -- --test-threads=1
cargo check -q
```

完成标准：

- 用户可以信任 agent 写代码，因为每次修改都有可解释、可恢复路径。

### Phase 3 完成标准

- file edit 对编码、换行、并发和外部修改安全。
- 文件修改结果有 diff、diagnostics 和 evidence。
- rollback 是正常产品路径，不是 debug fallback。

## 推荐执行顺序

严格按下面顺序推进：

1. Phase 1 Batch 1.1 到 1.3：先把主循环边界稳住。
2. Phase 1 Batch 1.4 到 1.5：让工具结果和 repair 有清晰归属。
3. Phase 2 Batch 2.1 到 2.3：先让 bash 可见、可诊断、结果可靠。
4. Phase 2 Batch 2.4 到 2.5：再做后台任务和 PTY。
5. Phase 3 Batch 3.1 到 3.3：先做文件身份、编码、锁。
6. Phase 3 Batch 3.4 到 3.5：再做 diagnostics、history、rollback。

原因：

- 不先拆主循环，后面 terminal 和 file edit 会继续往上叠补丁。
- 不先让 terminal 可靠，基本编程任务还是会退化成“给用户命令”。
- 文件编辑质量很重要，但它依赖更清晰的 tool result、evidence、rollback 路径。

## 每批通用验收

每个 batch 至少跑：

```bash
cargo fmt --check
cargo check -q
```

涉及工具、文件、终端、closeout 时补充：

```bash
cargo test -q file_tool -- --test-threads=1
cargo test -q bash_tool -- --test-threads=1
cargo test -q provider -- --test-threads=1
cargo test -q evidence_ledger -- --test-threads=1
cargo test -q closeout -- --test-threads=1
```

涉及 workflow / live eval 时补充：

```bash
bash -n scripts/run_live_eval.sh
bash scripts/workflow-production-gates.sh
```

大批次完成后跑：

```bash
cargo clippy --all-features -- -D warnings
cargo test -q
```

## 风险控制

- 每次只拆一个职责边界。
- 先移动代码，再改行为。
- 每个行为变化都要有回归测试。
- 如果某个改动需要新增大量 prompt 规则，先暂停，改成 tool/runtime contract。
- 如果某个 eval case 只能靠 special-case 通过，先记录为产品差距，不直接编码分支。

## 成功标准

下一阶段完成后，Priority Agent 应该达到下面状态：

- 主循环足够薄，新增工具或修复 closeout 不会误伤 streaming/provider/repair。
- shell 是可靠的一等能力，能运行、后台、取消、读输出、解释失败。
- file edit 具备成熟编码 agent 的基本安全性：read state、stale check、encoding、line ending、lock、diff、diagnostics、rollback。
- EvidenceLedger 从“评测辅助”变成日常回答和 closeout 的事实来源。
- 用户看到的是自然的编码 agent，而不是被规则和框架牵着走的模型。

