# 核心编码质量阶段结论 - 2026-05-13

本文档用于收束当前这一轮“基础编码能力”优化：记录已经验证过的
8/8 核心案例、对照本地 Claude Code / opencode 源码后的判断，以及下一阶段
最应该推进的工程工作。

## 当前结论

Priority Agent 现在已经达到一个可信的基础编程 agent 基线。

当前 `core-coding-quality` 回归集 8 个 live-eval case 全部通过，覆盖了：

- 基于工具证据回答本地文件事实，不乱编文件夹元数据；
- 写代码前先读当前文件，避免 stale edit；
- 单次 patch 同步修改多文件；
- terminal / bash 路由能执行安装和运行类任务；
- 长输出能保存为 artifact，再基于 artifact 检查；
- provider tool-call / tool-result 协议能诚实区分离线验证和真实在线成功；
- 用户拒绝危险操作后，agent 能换路恢复并完成正确修改；
- rollback 是正常产品路径，而不是只靠 git fallback 或 debug 命令。

但这还不等于已经整体追平 Claude Code / opencode。更准确的说法是：

- 基础编程任务的主链路已经站住了；
- 之前“规则太多、限制模型发挥、最后还幻觉”的问题有明显改善；
- 现在的主要差距不再是缺某一个工具，而是 runtime 架构和产品化深度。

不要把工具数量、命令数量、单次 replay 成功当成“已经追平 Claude Code”的证据。
当前可验证的结论要窄一点：基础编码质量已经有可复现基线，下一步要把这个基线
扩展到复杂项目、长会话、多 provider、真实权限交互和更成熟的 shell/edit 产品路径。

## 已验证基线

权威来源：

- `docs/AGENT_TESTING_MATRIX_2026-05-08.md`
- `docs/NEXT_AGENT_CORE_CODING_QUALITY_PLAN_2026-05-11.md`
- `scripts/run_live_eval.sh --list --case core-coding-quality`

| Case | 最新 run | 结果 | 证明的能力 |
| --- | --- | --- | --- |
| `core-inspection-grounding` | `core-quality-smoke-20260513-133437` | passed | 能检查本地文件事实，不编造文件夹大小、创建时间、内容数。 |
| `core-terminal-install-run` | `core-quality-terminal-fix-20260513-141842` | passed | terminal 任务能暴露 bash，不强行写文件或生成假 diff。 |
| `core-simple-stale-edit` | `core-quality-stale-fix-20260513-150307` | passed | 能先读文件再做聚焦单文件修改，并运行要求的验证。 |
| `core-multi-file-edit` | `core-quality-multifile-fix2-20260513-152308` | passed | 能先读两个文件，再用一次 `file_patch` 同步修改代码和文档。 |
| `core-long-output-artifact` | `core-quality-long-output-20260513-152851` | passed | 800 行输出落盘为 artifact，最终回答保持简洁，并能验证关键行。 |
| `core-provider-roundtrip` | `core-quality-provider-fix-20260513-154942` | passed | provider 协议验证不再伪装成真实在线 provider 成功。 |
| `core-permission-rejection-recovery` | `core-quality-permission-runtime-fix-20260513-162318` | passed | 拒绝危险 cleanup 后能恢复，required validation 能抓住漏改事实。 |
| `core-rollback-product-path` | `core-quality-rollback-20260513-163404` | passed | rollback/checkpoint/file-history 是正常产品路径，并有测试覆盖。 |

注意：更早的全量 live-eval aggregate 里仍然混有 stale / legacy case。它们不能直接代表
当前产品质量，需要重新分层为 current、stale、retired 后再统计。现在最可信的是这 8 个
基础编码质量 case。

## 这一轮具体变好了什么

- 文件事实不再只靠模型“看起来合理”的回答，required validation 和 tool evidence 开始约束最终结论。
- terminal 类任务可以保持 terminal-only，不再因为安装副产物或 audit 任务误触发合成代码修改。
- `.venv`、`*.egg-info` 这类 runtime artifact 不再被当成产品代码变更。
- required validation 现在能吸收安全的正向 `rg` / `grep` 断言，避免“内部 closeout passed，但验收事实没满足”。
- 长输出有 artifact 路径，模型不需要把大量输出塞进最终回答。
- provider roundtrip 能诚实说明“这是离线协议验证”，不再把它说成真实在线 provider 成功。
- rollback 已经不是一句建议，而是有 `last-file` / `fc_*` file-history 路径和 checkpoint 测试支撑。

## 对照 Claude Code

本轮重新看了本地 Claude Code 源码：`/Users/georgexu/Desktop/claude`。

Claude Code 的核心经验不是“prompt 更长”，而是把硬约束放进工具契约和 runtime
修复路径里。模型仍然自由解决问题，但 runtime 对 edit identity、permission state、
stale data、provider message shape 很严格。

关键源码锚点：

- `/Users/georgexu/Desktop/claude/src/tools/FileEditTool/FileEditTool.ts`
  - 125-178 行附近：edit tool 内部做权限和输入校验。
  - 316-336 行附近：old string 找不到、多处匹配会返回明确错误。
  - 398-553 行附近：edit 路径包含 file history、stale read state、diff 通知和日志。
- `/Users/georgexu/Desktop/claude/src/hooks/toolPermission/PermissionContext.ts`
  - 55-70 行附近：permission request 是 queue，并且有 resolve-once 防竞态。
  - 217-331 行附近：hook/user allow-deny、权限持久化和输入修正都在同一个产品路径中。
- `/Users/georgexu/Desktop/claude/src/utils/messages.ts`
  - 213-274 行附近：拒绝权限时，会明确告诉模型什么没发生、什么时候必须停下来。
  - 5119-5454 行附近：tool_use / tool_result pairing 会被修复或在 strict mode 直接拒绝。

对我们的启发：

- 不要继续靠 prompt 写很多“你应该怎样做”。
- 文件编辑、权限、tool-result 配对、provider 协议这些硬约束应该由工具和 runtime 保证。
- 用户拒绝、stale file、tool_result 缺失这类情况要变成结构化错误，而不是让模型猜。

## 对照 opencode

本轮也重新看了本地 opencode 源码：`/Users/georgexu/Desktop/opencode-dev`。

opencode 的核心经验是 service-level productization。permission、shell、truncate、
edit metadata、session revert 都有独立服务和明确状态，而不是堆在一个主循环里。

关键源码锚点：

- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/permission/index.ts`
  - 57-107、143-261 行附近：permission reply 有 `once` / `always` / `reject`，
    同时维护 pending request 和 approved ruleset。
  - 291-315 行附近：从 config 生成 permission rules，并据此禁用工具。
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/tool/edit.ts`
  - 58-206 行附近：edit 执行包含文件锁、权限 metadata、BOM/换行处理、diff metadata、LSP diagnostics。
  - 674-710 行附近：replacement 有多种匹配策略，并给出明确 not-found / multiple-match 错误。
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/tool/shell.ts`
  - 262-332 行附近：shell command 用 tree-sitter 解析 bash / PowerShell。
  - 377-411 行附近：基于解析树提取路径意图和外部目录权限。
  - 548-578 行附近：截断 shell 输出时，metadata 里带 full output path。
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/tool/truncate.ts`
  - 21-43、81-140 行附近：大输出会保存、预览，并提示用 grep/read 或 task agent 继续检查。
- `/Users/georgexu/Desktop/opencode-dev/packages/opencode/src/session/revert.ts`
  - 43-101 行附近：revert / unrevert 是 session 产品路径，背后有 snapshot 和 diff。

对我们的启发：

- 质量来自小而清晰的状态服务，不是来自更复杂的总控 prompt。
- shell 必须从“能跑命令”升级到“能理解命令结构、路径意图、风险和长输出生命周期”。
- rollback/revert、truncate、permission reply 这些都应该是用户能理解的产品路径。

## Priority Agent 现在已经接近的部分

在狭义基础编码面上，Priority Agent 已经接近成熟 agent 的基本形态：

- **本地事实 grounding**：route-scoped tools + required validation 能防住之前的元数据幻觉。
- **聚焦文件修改**：`file_patch` 写入前创建 checkpoint，并记录 file change 和 read state。
- **rollback 路径**：file-history rollback 与 checkpoint rollback 已有独立测试。
- **terminal 基础能力**：bash 在 terminal route 可见，并已有 local/restricted/external backend、PTY、background task 模块。
- **长输出处理**：tool-result truncation 与 live-eval artifact 检查已经能证明大输出不污染最终回答。
- **required validation**：显式要求的命令会由 runtime 执行，包含安全的正向 search assertion。
- **provider 诚实性**：provider-health 能区分确定性协议测试和真实网络 provider 成功。

目前这个项目最有价值的优势是 evaluation / trace loop：可以把一次主观体验问题转成
可复现 live-eval case，然后修 runtime，再跑同一个 case 验证。这一点应该保留并强化。

## 仍然存在的重要差距

### 1. 主循环仍然太重

`src/engine/conversation_loop/mod.rs` 已经拆出不少模块，但仍然有 6198 行。
整个 `conversation_loop` 目录超过 22000 行。`run_inner` 仍然同时碰 prompt assembly、
tool execution、required validation、repair、closeout、trace、workflow state。

这是当前最高优先级工程风险。继续靠小补丁能修单点问题，但副作用很难判断。

### 2. shell 语义还没追上 Claude Code / opencode

Priority Agent 已有 `command_classifier`、backend selection、timeout、PTY、background task。
但还没有达到：

- Claude Code 那种更完整的 shell permission / semantic rejection；
- opencode 那种 tree-sitter bash/PowerShell 解析、路径模式提取、外部目录权限推导。

shell 是编程 agent 的核心能力。下一版不能继续主要依赖启发式命令分类来做重要权限和变更判断。

### 3. edit engine 的失败诊断还不够成熟

`file_patch` 现在有 checkpoint 和 state update，这是明显进步。但失败质量还不够：

- Claude Code 会明确区分 no-op、not-found、multiple-match、ignored-path、stale-state、diff evidence。
- opencode 会处理 BOM/换行、文件锁、diff metadata，并在 edit 后给 LSP diagnostics。

Priority Agent 需要让失败 edit 自带下一步诊断信息，而不是靠 repair prompt 反推。

### 4. provider/tool-result 协议还需要统一合同

MiniMax 400 和后续 provider-roundtrip 修复说明：provider 协议不能每个 provider 单独补。
Claude Code 有 defensive pairing；opencode 有 provider transform 和 truncation service。

Priority Agent 现在有 `tool_result_controller`、`tool_metadata`、provider health tests，
但还需要跨以下情况建立统一矩阵：

- streaming / non-streaming；
- missing / duplicate / orphaned / reordered tool results；
- resume / compaction 边界；
- OpenAI-compatible provider 的细微差异；
- strict failure 和 safe repair 的明确边界。

### 5. 用户可见产品路径还不够成熟

核心 case 能过，但用户体验还没完全成熟：

- permission ask/reject/corrected messaging；
- diff preview 和 file history review；
- rollback / unrevert 可见性；
- terminal background job 生命周期；
- 简洁但可追溯的最终证据；
- live-eval report 清晰区分 product failure 和 harness failure。

这些是 Claude Code / opencode 仍然领先的地方。

## 下一阶段开发计划

### Phase A：先拆主循环，建立生命周期合同

目标：降低 `run_inner` 复杂度，把真正的决策边界拆进可测试 controller，而不是机械移动代码。

具体工作：

- 抽出 `RequiredValidationController`，负责提取、安全过滤、执行、记录证据、接入 closeout。
- 抽出 `ToolTurnController`，负责 provider tool result append、metadata normalize、truncation observation、runtime-diet evidence。
- 抽出 `CloseoutEvaluator`，负责从 validation、acceptance、changed files、no-diff audit 语义计算最终状态。
- `ConversationLoop` 只保留 sequencing：构造 prompt、调用 provider、分发 tool turn、询问 controller 下一步。

建议验证：

```bash
cargo test -q prompt_context
cargo test -q route_scoped_tools
cargo test -q closeout
cargo test -q validation_runner
```

每完成一个 extraction batch，至少 rerun 一个 `core-coding-quality` live case。
第一批最该 rerun `core-permission-rejection-recovery`，因为它最能防止 closeout 再次幻觉。

### Phase B：把 shell / terminal 做成真正一等能力

目标：terminal 不只是“有 bash 工具”，而是能理解命令结构、路径意图、风险、长输出和后台任务。

具体工作：

- 先实现 bash/zsh parser-backed command scan。PowerShell 可作为后续 Windows milestone。
- 从解析结果推导 command kind、mutation risk、path read/write、external-directory access、background/dev-server intent。
- 把扫描结果接入 permission metadata 和 route-scoped tool exposure。
- 让长运行命令、后台任务、PTY session 都有稳定 metadata 和用户可见 summary。

建议验证：

- command parse / path extraction 单测；
- external path 和 destructive command 权限测试；
- rerun `core-terminal-install-run`、`core-long-output-artifact`；
- 新增一个 dev-server/background-job live case。

### Phase C：增强 edit / diff 质量

目标：让 edit 失败能自诊断，成功 edit 能提供足够 diff、history、diagnostic 证据。

具体工作：

- 改进 `file_edit` / `file_patch` 的 no-op、not-found、multiple-match、stale-read、binary/encoding、line-ending 错误输出。
- 补 BOM 和 line-ending preservation 测试。
- 让 edit、patch、write 的 diff metadata 一致。
- 在语言栈支持时，把 LSP 或 syntax diagnostics 作为可选 post-edit evidence。
- 让 rollback 和 unrevert/reapply 成为更清楚的产品操作。

建议验证：

- targeted file-tool tests；
- rollback/checkpoint tests；
- rerun `core-simple-stale-edit`、`core-multi-file-edit`、`core-rollback-product-path`；
- 新增一个 failed-edit-recovery live case。

### Phase D：provider/tool-result 协议矩阵化

目标：不再等真实 provider 400 才发现协议问题。

具体工作：

- 定义 provider-neutral tool-turn transcript invariant。
- 测 missing、orphaned、duplicate、reordered tool result。
- 增加 OpenAI-compatible、Anthropic-style、MiniMax-like strict validation fixtures。
- repair 行为必须显式：能安全修就记录 trace，不能安全修就 strict failure 并给出可读错误。
- 把重复的 live-eval 解析规则继续收敛到 `scripts/live_eval_report_parser.py`，不要散在 shell script 片段里。

建议验证：

```bash
cargo test -q provider_health -- --test-threads=1
cargo test -q provider -- --test-threads=1
```

有 API/network credentials 时，再补一个真实在线 smoke。

### Phase E：重置更大的评测叙事

目标：不要把旧 stale 失败和当前产品质量混在一个数字里。

具体工作：

- 把旧 live-eval case 标为 current、stale、retired。
- 用当前 release binary 成组重跑 `core-coding-quality`。
- 增加第二组真实项目任务：已有项目 bug fix、多文件 feature、test failure repair、terminal dependency setup、bad edit 后 rollback。
- 选一小组共享任务手动对比 Claude Code / opencode，记录行为差异，而不是只记 pass/fail。

产物：

- 更新 `docs/AGENT_TESTING_MATRIX_2026-05-08.md`。
- 为 current cases 记录可复现 run id。
- 写一份短 gap report，明确哪些接近、哪些没接近，避免过度宣称。

## 推荐立即下一步

下一步应该从 Phase A 开始，先抽 `RequiredValidationController`。

理由：

- 它直接关联这轮最关键的“不要乱 closeout / 不要幻觉通过”问题。
- 边界相对清楚：extract、filter、run、record、closeout integration。
- 风险比直接拆整个 tool loop 小。
- 有 `core-permission-rejection-recovery` 这个高价值 live case 可以回归。

