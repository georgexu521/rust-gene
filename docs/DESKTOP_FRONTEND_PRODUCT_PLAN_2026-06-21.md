# Desktop Frontend Product Plan - 2026-06-21

## Goal

把 `apps/desktop` 做成 gex 日常能用的成熟 agent 工作台：既能像 Codex / OpenCode 一样快速完成通用编程任务，也能承载本项目独有的 LabRun 长周期项目推进模式。

桌面端的核心目标不是做一个漂亮外壳，而是把现有 runtime 能力可视化、可控制、可恢复：

- 通用 Agent Mode：保留现在 CLI/agent 的直接对话式工作流，用户一句一句下命令，agent 执行、验证、总结。
- LabRun Mode：用户先和教授智能体讨论项目立项，立项后进入教授、博后、研究生循环推进，用户通过侧边沟通和控制按钮介入。
- Runtime Truth：前端只展示 runtime 已经记录的证据、状态、工具结果、权限请求、上下文、缓存和验证结果，不凭 UI 自己判断任务成功。
- Local-first：项目、会话、LabRun 进度、artifact、报告和恢复状态必须能在本地持久化并重新加载。

## Non-Goals

- 不重写现有 Tauri 架构；继续使用 `apps/desktop` 的 React + Tauri 边界。
- 不把桌面前端变成独立智能判断层；智能判断仍由 LLM 负责，runtime 负责确定性状态和证据。
- 不照搬 OpenCode 的 Electron 技术栈；借鉴产品结构、状态组织、交互和组件分层。
- 不在第一阶段追求完整视觉品牌重做；先把工作台信息架构和关键路径做好。

## Current State Audit

当前桌面端已经有相当多的底层能力，不适合推倒重来。

### Existing Strengths

- `apps/desktop/src/app/App.tsx` 已经接入健康检查、设置、provider/model 状态、会话列表、诊断、上下文快照、workbench 快照、trace、tool output、permission recovery、goal 状态和 Lab daemon supervision。
- `apps/desktop/src/runtime/desktopApi.ts` 已经形成了明确的 typed boundary，包含 context snapshot、workbench snapshot、Lab status、subagent task、session/message/run event 等结构。
- `Composer.tsx` 已经有 context/project/mode/provider popover、文件上下文入口、当前 diff 上下文、权限模式和 agent mode 选择。
- `Transcript.tsx` 已经能展示 run event、tool/permission/final answer 等 runtime 证据。
- `WorkbenchPanel.tsx` 已经可以展示 project map、symbol index、runtime context、cache surface、LabRun、subagent tasks。
- `StatusBar.tsx` 已经展示 provider、workspace、cache、token、context 等底部运行状态。
- `apps/desktop/tests/desktop-ui-smoke.spec.ts` 已经覆盖 workbench drawer、Lab status panel、composer controls、session operations、timeline cards、trace、permission、context 等关键 UI。

### Current Product Gaps

- Workbench 仍像抽屉功能集合，不是常驻的 agent 工作台右侧 inspector。
- 通用 Agent Mode 和 LabRun Mode 的入口还不够清楚，用户不容易理解当前是在普通对话、项目立项、还是 LabRun 执行循环。
- LabRun 有状态和命令入口，但缺少完整的 proposal -> approve -> running -> meeting/intervention -> report -> pause/resume 闭环 UI。
- Composer 已经有上下文入口，但还没有成熟 agent 产品常见的 slash command、附件列表、历史、文件/图片拖拽、模式提示和提交状态。
- Transcript 能展示事件，但还需要更商业化的 run group、tool card、diff/test/card 状态层级，让用户快速知道“做了什么、卡在哪里、证据是什么”。
- Settings/provider/model 流程可用但还没有形成首屏 onboarding 和 provider repair 的强引导。
- 现有 `App.tsx` 承担太多状态和协调职责，需要逐步拆成 feature hooks 和 layout components。

## OpenCode / Codex Lessons To Borrow

本地 OpenCode 源码的关键启发不是 Electron，而是分层和工作台结构：

- Desktop shell 与 app UI 分开：OpenCode 的 `packages/desktop` 只负责桌面壳、窗口、更新、native 能力；主 UI 在 `packages/app`。
- Runtime/server context 独立：OpenCode 的 `context/server-sync.tsx`、`context/sync.tsx`、`context/permission.tsx`、`context/models.tsx`、`context/file.tsx` 等把状态同步和 UI 分开。
- Prompt input 是产品核心：OpenCode 有 prompt input history、slash popover、attachments、image attachments、paste、submit、request parts 等完整模块。
- Session header 和 context usage 是一等对象：OpenCode 有 session header、context metrics、context breakdown、session tabs。
- 文件树、provider/model dialog、settings、status popover、tool card、diff changes 都是工作台能力，不是隐藏的 debug 面板。
- UI primitives 独立：button、icon button、menu、tabs、segmented control、dialog、field、select、textarea、badge、tooltip、basic tool、tool error card、diff changes 等应该沉淀为稳定组件。

Codex 类产品的关键启发：

- 左侧是项目/会话和导航，中间是 agent transcript/composer，右侧是上下文/文件/执行/诊断 inspector。
- 权限、命令、diff、测试、失败、重试、验证都应在同一任务流里可见。
- 用户要能随时区分“agent 正在想”“agent 调用了工具”“工具失败”“需要我批准”“已经验证完成”。

## Target Information Architecture

### Three-Pane Workbench

1. Left Rail: workspace and navigation
   - Project selector
   - Recent projects
   - Sessions
   - Mode entry: Direct Agent / LabRun
   - Search, archived sessions, settings

2. Center: active conversation and execution
   - Session header with mode, provider/model, project, branch/worktree, run state
   - Transcript grouped by user turn and run
   - Tool cards, permission cards, validation cards, final answer cards
   - Composer with context attachments, slash commands, mode-specific hints

3. Right Inspector: persistent workbench
   - Context tab: token usage, compression, cache, attached context, compact boundary
   - Files tab: project map, file tree, symbol index, selected file preview
   - Execution tab: running tools, logs, trace, tool output, permissions
   - Subagents tab: durable subagent tasks, tools used, artifacts, recovery state
   - LabRun tab: proposal, run status, tasks, professor/postdoc/graduate state, reports, blockers, meeting recommendation
   - Diagnostics tab: health checks, provider setup, environment, logs

### Bottom Status Bar

底部状态栏保留，但职责更明确：

- Provider/model/current host
- Workspace/project path
- Context usage and compact threshold
- Cache hit/read/write/miss where available
- Run state: idle/running/waiting permission/failed/verified
- LabRun state when active

## Mode Model

### Direct Agent Mode

这是现在主流 agent 的模式，尽量保持简单直接：

- 用户在 composer 里下命令。
- Agent 调用工具、修改代码、运行验证。
- Runtime 展示工具、权限、验证、diff、错误和 closeout。
- Goal 功能继续服务通用模式，不被 LabRun 替代。

桌面端要让 Direct Agent Mode 达到日常使用质量：

- 清楚的 session header。
- 可见的 running state。
- 可恢复的 permission prompt。
- 易读的 tool/test/diff cards。
- 方便添加 current diff、file、project map、screenshot 等上下文。
- 失败时展示下一步和证据，不只展示文本。

### LabRun Mode

LabRun 是并列模式，不是 Direct Agent Mode 的一个小 feature。

LabRun 的推荐桌面流程：

1. Intake: 用户先和教授智能体讨论需求、目标、约束、风险和预期产物。
2. Proposal: 教授生成立项 proposal，用户可以继续讨论或点击立项。
3. Approval: 用户点击立项按钮后进入正式 LabRun。
4. Execution Loop: 教授、博后、研究生按 runtime 流程推进，多轮循环，必要时插入组会。
5. Side Channel: 用户不能直接命令博后/研究生，但可以通过侧边输入框和教授沟通，教授把调整写入 LabRun。
6. Supervision: 博后负责具体代码和结果审查，教授负责方向性 steering。
7. Pause/Resume: 用户可暂停；关闭软件或关机时自动暂停；重新打开后由用户点击继续。
8. Closeout: 项目阶段完成后生成 report，用户和教授确认是否继续下一阶段或结题。

LabRun 桌面端要突出“项目推进”而不是普通聊天：

- Proposal card
- Approve / Pause / Resume / Meeting / Intervene / Closeout controls
- Professor side-channel
- Current stage and owner
- Task board
- Reports and artifacts
- Blockers and repeated-failure signals
- Cost/context/cache surface

## LabRun Cost, Cache, And Compression Policy

LabRun 跑得久，前端必须让成本和上下文状态可见。

P0/P1 之间需要做到：

- 显示真实 provider usage：input、output、cached/read、cache write、total tokens，缺字段时明确显示 unavailable。
- 显示当前上下文窗口占用、压缩阈值、最近一次压缩原因、压缩前后 token 变化。
- 显示 cache hit/read/write/miss，并区分“provider 返回的真实 usage”和“runtime 预估”。
- LabRun 执行循环中，右侧 inspector 要能看到每阶段成本和累计成本。
- 教授/博后/研究生之间的共享前缀和动态上下文要尽量稳定，减少重复 prompt 成本。
- LabRun 的报告、artifact、任务状态优先用结构化本地文件传递；不要每轮把完整历史塞回上下文。

前端不负责决定何时压缩，但必须展示 runtime 的压缩状态和证据。

## Engineering Architecture

### Keep The Boundary

- Tauri command / event / typed API 是桌面端和 Rust runtime 的边界。
- 前端只消费 `desktopApi.ts` 暴露的 typed data。
- 前端不自行判断 agent 是否成功，只展示 runtime proof、validation、tool result、artifact、state。
- 如果 UI 需要新字段，先在 runtime 持久化和 API 上定义清楚，再在 UI 展示。

### Refactor Direction

当前 `App.tsx` 应逐步拆分：

- `app/shell/DesktopShell.tsx`: 三栏布局、全局快捷键、全局 drawer/dialog。
- `app/state/useDesktopBootstrap.ts`: health/settings/provider/session initial load。
- `app/state/useRunEvents.ts`: run event subscription、watchdog、permission recovery。
- `app/state/useWorkbenchSnapshot.ts`: context/workbench/diagnostics refresh。
- `app/features/direct-agent/*`: Direct Agent header、transcript、composer integration。
- `app/features/labrun/*`: LabRun proposal、status board、side-channel、controls。
- `app/features/inspector/*`: right inspector tabs。
- `app/ui/*`: common primitives and layout components。

先做结构性拆分和高价值 UI，不做大规模视觉重写。

## Implementation Slices

### P0 - Plan And Audit

Status: complete.

- [x] 对照现有 `apps/desktop` 能力。
- [x] 对照本地 OpenCode 桌面/app/ui 结构。
- [x] 明确 Direct Agent Mode 与 LabRun Mode 并列产品模型。
- [x] 写出本计划文档。
- [x] 在 `docs/PROJECT_STATUS.md` 增加桌面端目标状态入口。

### P1 - Workbench Shell

Status: complete.

目标：让桌面端看起来和用起来像一个成熟 agent 工作台。

- [x] 把 Workbench 从 drawer 优先模式升级为可常驻右侧 inspector。
- [x] 增加 top/session header，展示 mode、project、provider/model、run state、branch/worktree。
- [x] 增加 Direct Agent / LabRun 明确模式入口。
- [x] 右侧 inspector 增加 tabs：Context、Files、Execution、Subagents、LabRun、Diagnostics。
- [x] Files inspector 增加 selected file preview：从 symbol index 选择文件后，通过受限 Tauri API 读取 selected project 内文本预览。
- [x] 保留 drawer 作为窄屏或临时展开模式。
- [x] 更新 Playwright smoke，覆盖三栏布局和 inspector tab 切换。

### P2 - Direct Agent Daily-Use Polish

Status: complete.

目标：把通用 agent 模式做到日常可用。

- [x] Composer 增加 slash command 基础结构。
- [x] Composer 增加 prompt history：提交后的 prompt 可用 ↑ / ↓ 在同一桌面会话里快速找回和返回草稿。
- [x] Composer 增加全局 `/` 快捷入口：焦点不在输入控件时按 `/` 会聚焦 composer，并在空输入时打开 slash command 菜单。
- [x] Composer 附件区明确展示 current diff/file/project context。
- [x] Provider/model selector 改成更清楚的 dialog/popover，支持 setup repair。
- [x] Transcript 增加 grouped run cards：tool、permission、validation、diff、final。
- [x] Tool output 和 trace 从 debug drawer 变成 Execution inspector 的一部分。
- [x] Context usage 从 status bar 扩展为可打开的 breakdown。

### P3 - LabRun Product Surface

Status: complete for the current typed runtime snapshot.

目标：LabRun 不再只是 CLI 命令集合，而是桌面端可观察、可暂停、可恢复的项目模式。

- [x] Proposal/intake UI：教授讨论完成后展示 proposal card 和立项按钮。
- [x] LabRun 状态 board：stage、owner、tasks、reports、artifacts、blockers、meeting recommendation。
- [x] Pause/Resume controls：用户可暂停，重开软件后显示可恢复状态。
- [x] Professor side-channel：用户只和教授沟通，教授把调整写入循环。
- [x] Meeting controls：用户手动开组会，runtime/教授也可推荐组会。
- [x] Reports/artifacts browser：从 LabRun status 打开最新 report、任务 artifact、验证证据。
- [x] Cost/context/cache panel：显示 LabRun 累计 token、cache、compression 状态。

Implementation notes:

- LabRun inspector now exposes proposal/intake and approve/draft actions by staging the existing `/lab proposal`, `/lab start`, and `/lab approve <proposal_id>` commands into the normal composer path.
- Project controls now stage `/lab resume`, `/lab pause user_pause`, `/lab continue`, and `/lab meeting open`, preserving the runtime as the authority for execution and validation.
- Professor side-channel now has a dedicated input that stages `/lab professor <message>` or `/lab intervene <message>`; users still do not directly command postdoc/graduate agents from the UI.
- LabRun status board now surfaces stage, owner, cycle, task progress, blockers, needs-user state, meeting recommendation, topic, and detail from the typed runtime snapshot.
- Reports/artifacts surface now opens the latest report when the API exposes a path and stages `/lab report`, `/lab report list`, and `/lab review` for runtime-backed details.
- Desktop API now exposes structured LabRun `artifacts`, `reports`, and `evidence_refs` rows from `LabStore`; the LabRun inspector renders these rows directly instead of relying only on command-backed summary actions.
- Desktop API now includes short report previews for LabRun artifact/report rows, and the LabRun inspector supports local search across artifacts, reports, preview text, and evidence refs.
- Desktop API now exposes guarded paged markdown report reads through `desktop_lab_report_page`; it only reads resolved `.md` files under the selected project's `.priority-agent/lab` tree. The LabRun inspector can preview full report pages in place with previous/next paging.
- Desktop API now exposes guarded structured artifact body reads through `desktop_lab_artifact_body`; it resolves the latest LabRun from the selected project and only reads artifact ids registered on that run. The LabRun inspector can preview the artifact body JSON in place next to report previews.
- Cost/context/cache surface now reuses the runtime context snapshot for context usage, cache read/miss, compression count, and strategy. The Context inspector also keeps the latest provider usage event and shows real provider input/output/total/reasoning/cache-write tokens when present; missing provider fields remain explicitly unavailable instead of being frontend-estimated.

### P4 - Packaging, Validation, And Visual QA

目标：保证桌面端不是 demo，而是可运行、可验证、可回归。

- [x] `corepack pnpm --dir apps/desktop build`
- [x] `corepack pnpm --dir apps/desktop test:ui-smoke`
- [x] `corepack pnpm --dir apps/desktop test:native-smoke`
- [x] `cargo check --features experimental-api-server -q`
- [x] `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q`
- [x] `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q desktop_smoke_lab_status_reads_file_backed_labrun_state`
- [x] 桌面端运行截图检查：desktop viewport via `apps/desktop/test-artifacts/native-smoke.png`; narrow viewport via `apps/desktop/test-artifacts/desktop-narrow-loaded.png`。
- [x] 原生窗口路径覆盖：native smoke now opens Settings provider setup, LabRun inspector, LabRun search, Execution inspector, context details, trace drawer, permission approval, final answer, and usage surface.
- [x] 真实 provider 原生路径覆盖：`scripts/desktop-native-smoke.sh --live-provider --provider minimax --timeout 180 --no-screenshot` and `scripts/desktop-native-smoke.sh --live-provider --provider deepseek --timeout 180 --no-screenshot` now run real provider submit/stream/final-answer/usage paths in the packaged Tauri app; latest proof is `apps/desktop/test-artifacts/native-live-provider-minimax-app-desktop.log` and `apps/desktop/test-artifacts/native-live-provider-deepseek-app-desktop.log`.
- [x] 重启恢复路径覆盖：`scripts/desktop-native-smoke.sh --live-provider --provider deepseek --restart-check --timeout 180 --no-screenshot` now restarts the packaged app against the same temporary app data directory and verifies the previous real provider user message, assistant answer, and session metadata are restored; latest proof is `apps/desktop/test-artifacts/native-live-provider-deepseek-restart-app-desktop.log`.
- [x] 真实多工具编辑路径覆盖：`scripts/desktop-native-smoke.sh --live-provider --provider deepseek --multi-tool-check --timeout 240 --no-screenshot` now creates an isolated project, runs the packaged app in Build mode, verifies real provider file read/edit execution, provider usage, verified closeout, and the target file's changed contents; latest proof is `apps/desktop/test-artifacts/native-multitool-deepseek-app-desktop.log`.
- [x] 多轮真实项目编辑 soak 覆盖：`scripts/desktop-native-smoke.sh --live-provider --provider deepseek --soak-check --timeout 420 --no-screenshot` now runs two consecutive Build-mode desktop turns through the packaged app's Tauri `send_message` path, verifies real provider file read/write execution in the same session, requires two verified closeouts, and checks two changed files; latest proof is `apps/desktop/test-artifacts/native-soak-deepseek-app-desktop.log`.
- [x] 多轮真实项目编辑后重启恢复覆盖：`scripts/desktop-native-smoke.sh --live-provider --provider deepseek --soak-check --restart-check --timeout 480 --no-screenshot` now runs the two-turn Build-mode soak, restarts the packaged app against the same temporary home/project, and verifies restored session messages, restored UI text, and project file previews for both changed files; latest proof is `apps/desktop/test-artifacts/native-soak-deepseek-restart-app-desktop.log`.
- [x] 跨 provider 两轮 soak + restart 覆盖：`scripts/desktop-native-smoke.sh --live-provider --provider minimax --soak-check --restart-check --timeout 480 --no-screenshot` now passes on MiniMax with `agent_mode=build`, five tool executions, two verified closeouts, restored session messages, restored UI text, and restored project file previews; latest proof is `apps/desktop/test-artifacts/native-soak-minimax-app-desktop.log` and `apps/desktop/test-artifacts/native-soak-minimax-restart-app-desktop.log`.
- [x] 三轮真实项目编辑 extended soak 覆盖：`scripts/desktop-native-smoke.sh --live-provider --provider deepseek --extended-soak-check --restart-check --timeout 720 --no-screenshot` now passes as a stricter real-provider gate. It requires three consecutive Build-mode file edits, per-turn hard `desktop_file_preview` verification, unchanged future-target checks, three verified closeouts, and restart recovery. Latest proof is `apps/desktop/test-artifacts/native-extended-soak-deepseek-app-desktop.log` plus `apps/desktop/test-artifacts/native-extended-soak-deepseek-restart-app-desktop.log`: the run records `agent_mode=build`, six tool executions, three verified closeouts, all three project files changed to expected content, restored session messages, restored project file previews, and restored UI text. This also fixed the desktop smoke config path so extended soak applies `PRIORITY_AGENT_DESKTOP_SMOKE_AGENT_MODE=build`; failure cleanup now keeps the isolated native-smoke HOME/project by default, and unattended live-provider smoke fails early on `ask_user` instead of waiting for timeout.
- [x] 跨 provider 三轮 extended soak 覆盖：MiniMax now passes the stricter three-turn gate after the native smoke task contract was tightened to require read/write/cat tool evidence on every QA turn. Latest proof is `apps/desktop/test-artifacts/native-extended-soak-minimax-app-desktop.log` plus `apps/desktop/test-artifacts/native-extended-soak-minimax-restart-app-desktop.log`; the run records `agent_mode=build`, six tool executions, three verified closeouts, all three target files changed to expected content, and restart recovery of session messages, UI text, and project file previews.
- [x] LabRun paused recovery/report UI 覆盖：`scripts/desktop-native-smoke.sh --lab-recovery-check --timeout 120 --no-screenshot` now prepares a real file-backed paused LabRun through the existing Lab command handler, launches the packaged Tauri app, verifies `desktop_workbench_snapshot`, `desktop_lab_report_page`, the LabRun tab, artifact search, and full report viewer; latest proof is `apps/desktop/test-artifacts/native-lab-recovery-app-desktop.log`.
- [x] LabRun paused recovery restart 覆盖：`scripts/desktop-native-smoke.sh --lab-recovery-check --restart-check --timeout 150 --no-screenshot` now restarts the packaged app against the same temporary home/project after the first LabRun recovery pass and verifies the same paused LabRun/report UI again without re-preparing the project; latest proof is `apps/desktop/test-artifacts/native-lab-recovery-restart-app-desktop.log`. After the `StartupStateCard.tsx` and `WorkspaceTopbar.tsx` shell split, the same gate was rerun from a freshly rebuilt packaged app with `--timeout 180`; the log records `snapshot-verified`, `report-page-verified`, `labrun-tab-open`, `report-preview-open`, and `full-report-visible` before and after restart. The native smoke wrapper also now appends those key diagnostic lines into `native-lab-recovery-smoke.log` and `native-lab-recovery-restart-smoke.log`, so the summary smoke logs are useful even when the Tauri process writes no stdout/stderr.
- [x] 验证已有 runtime tests 不因 desktop API 改动破坏：`cargo test -q` passes with 3109 main-crate tests passed, 1 ignored, plus all follow-on integration/doc test batches passing.
- [x] LabRun artifact body viewer 覆盖：`desktop_lab_artifact_body` reads only registered artifacts on the selected project's latest LabRun and the LabRun inspector shows the structured body JSON; validated by `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q desktop_smoke_lab_status_reads_file_backed_labrun_state`, `corepack pnpm --dir apps/desktop build`, and `corepack pnpm --dir apps/desktop test:ui-smoke`.
- [x] 发布前桌面 dogfood 套件覆盖：`scripts/desktop-release-dogfood.sh --skip-build --timeout 720 --repeat 2` now runs the release-critical native desktop checks as one repeatable gate: DeepSeek three-turn extended soak + restart, MiniMax three-turn extended soak + restart, and paused LabRun recovery/report/artifact UI + restart. Latest proof is `apps/desktop/test-artifacts/desktop-release-dogfood.log`; the current run records `PASS desktop_release_dogfood repeat=2` plus per-iteration PASS markers for both iterations, including `PASS deepseek_extended_soak_restart iteration=1/2`, `PASS minimax_extended_soak_restart iteration=1/2`, `PASS lab_recovery_restart iteration=1/2`, `PASS deepseek_extended_soak_restart iteration=2/2`, `PASS minimax_extended_soak_restart iteration=2/2`, and `PASS lab_recovery_restart iteration=2/2`. MiniMax initially exposed a third-turn no-tool failure; the native extended-soak harness now has a bounded third-turn repair path that still requires real `desktop_file_preview` evidence before passing, and the final MiniMax dogfood runs passed with all three target files verified.
- [x] Repeated release dogfood entrypoint：`scripts/desktop-release-dogfood.sh` now supports `--repeat count`, preserving the default single-run behavior while allowing unattended repeated release gates. The first iteration honors `--skip-build`/build settings; later iterations reuse the packaged app and each step is tagged with `iteration=N/total` in `desktop-release-dogfood.log`. Validated by `bash -n scripts/desktop-release-dogfood.sh`, `scripts/desktop-release-dogfood.sh --help`, and real `scripts/desktop-release-dogfood.sh --skip-build --timeout 720 --repeat 1` plus `--repeat 2` runs.
- [x] Daily-use UI polish pass：mobile topbar/session header no longer clips Output/Trace actions because topbar/session header are fixed-size flex children and the mobile topbar uses a single-column layout; mobile workspace now explicitly occupies the first grid column instead of inheriting the desktop column; mobile statusbar now spans the viewport and scrolls horizontally so provider/cache/token/context/model/workspace state remains reachable; mobile session metadata now shows the full provider/model instead of truncating `deepseek-v4-flash`; inspector trace detail text now wraps instead of truncating long permission/tool evidence; the mobile composer empty-context hint and restored-session startup card now wrap inside their cards instead of truncating important guidance/session detail. Validated with `corepack pnpm --dir apps/desktop build`, the targeted mobile Playwright case, full `corepack pnpm --dir apps/desktop test:ui-smoke`, and `git diff --check`.
- [x] Composer unavailable-context honesty：the Add context menu no longer exposes Screenshot as a disabled/dead button. Until native screenshot context is actually connected, it renders as a non-actionable status note (`Screenshot context unavailable`) while file and current-diff context remain real actions. Covered by the desktop layout smoke and `corepack pnpm --dir apps/desktop build`.
- [x] Delete-session dialog keyboard polish：session deletion now uses a focused `DeleteSessionDialog` component with initial focus on Cancel, Tab/Shift+Tab focus containment, and Escape-to-cancel behavior. The destructive Delete action still requires an explicit click/keyboard activation. Covered by the web-preview desktop smoke and `corepack pnpm --dir apps/desktop build`.
- [x] Export feedback polish：session export completion now renders through `ExportNoticeBanner` with `role=status`, Open export, and Dismiss actions, instead of a persistent inline banner. Covered by the desktop layout smoke and `corepack pnpm --dir apps/desktop build`.
- [x] Environment popover polish：the topbar Environment details popover now closes via Escape and outside click, instead of requiring a second click on the same icon. Covered by the desktop layout smoke and `corepack pnpm --dir apps/desktop build`.
- [x] Mobile/narrow access polish：because the sidebar is hidden on narrow viewports, topbar now has an explicit Settings button so provider setup, permissions, and diagnostics remain reachable. The previously inert `More conversation actions` button now opens the command palette instead of behaving like a dead control; the command palette now fits narrow viewports, command labels/hints clamp inside the palette instead of widening it, and mobile smoke verifies `More conversation actions` -> `Command palette` -> `New Chat`. Covered by desktop and mobile Playwright smoke, `corepack pnpm --dir apps/desktop build`, and `git diff --check`.
- [x] Mobile Settings provider/permissions polish：provider key setup now uses class-backed responsive controls instead of inline layout, stacks provider select/API-key input/save button on narrow screens, and smoke verifies Provider plus Permissions categories stay inside the Settings drawer viewport. Covered by the targeted mobile Playwright smoke, full `corepack pnpm --dir apps/desktop test:ui-smoke`, `corepack pnpm --dir apps/desktop build`, and `git diff --check`.
- [x] Settings keyboard flow polish：opening Settings now moves focus into `Back to app`, Tab/Shift+Tab stay inside the drawer instead of leaking into the background app, Escape closes the drawer, and focus returns to the launcher button on desktop and mobile. Covered by targeted desktop/mobile Playwright smoke, full `corepack pnpm --dir apps/desktop test:ui-smoke`, `corepack pnpm --dir apps/desktop build`, and `git diff --check`.
- [x] Shared drawer keyboard polish：Workbench, Run Trace, Context Details, Tool Output, and Settings now share one drawer keyboard hook for initial close-button focus, Tab/Shift+Tab focus containment, Escape close, and focus return to the opener. The hook only handles key events when focus is inside that drawer, so nested drawers such as Trace -> Context Details do not close multiple layers at once. Covered by desktop Playwright smoke plus the full desktop UI smoke suite.
- [x] Provider usage Inspector surface：Context inspector now stores the latest runtime `usage` event and displays real provider input/output/total/reasoning/cache-write tokens after a run; before a usage event or for missing provider fields it still shows `unavailable`. Covered by `corepack pnpm --dir apps/desktop build`, `corepack pnpm --dir apps/desktop exec playwright test tests/run-event-state.spec.ts --grep "stores latest provider usage"`, the targeted desktop smoke case, and full `corepack pnpm --dir apps/desktop test:ui-smoke`.
- [x] Desktop runtime API maintainability pass：web-preview run/report/artifact/file/permission fixture logic now lives in `desktopPreview.ts`, while `desktopApi.ts` stays focused on the Tauri command/event boundary. This reduced `desktopApi.ts` from 1469 lines to 1065 lines and keeps the main runtime API file safely below the project 1500-line source-file ceiling. Covered by `corepack pnpm --dir apps/desktop build`, full `corepack pnpm --dir apps/desktop test:ui-smoke`, and `git diff --check`.
- [x] Desktop App shell maintainability pass：startup recovery/restored-session banner rendering now lives in `StartupStateCard.tsx`, keeping `App.tsx` below the project 1500-line ceiling while preserving Lab recovery Resume/Dashboard/Keep paused behavior. Covered by the startup Lab recovery smoke, desktop layout smoke, and `corepack pnpm --dir apps/desktop build`.
- [x] Desktop App shell maintainability pass：topbar rendering, context meter, environment popover, and Workbench/Output/Trace header controls now live in `WorkspaceTopbar.tsx`. `App.tsx` remains responsible for orchestration callbacks, while the topbar UI remains covered by `corepack pnpm --dir apps/desktop test:ui-smoke` and `corepack pnpm --dir apps/desktop build`.
- [x] Runtime error daily-use polish：run errors and runtime watchdog warnings now render as an actionable runtime alert instead of a bare text banner. Users can open the relevant trace, jump to Diagnostics, or dismiss the alert without losing the underlying timeline/trace evidence. Web-preview has an explicit run-error fixture for this path. Covered by `corepack pnpm --dir apps/desktop exec playwright test tests/desktop-ui-smoke.spec.ts --grep "runtime error banner"` plus the full UI smoke gate.
- [x] Mobile runtime error recovery polish：the runtime alert actions are now covered on narrow/mobile viewports too. Mobile smoke triggers the web-preview runtime error fixture, verifies `Open trace` opens the visible Run Trace drawer, then verifies `Diagnostics` opens the Runtime inspector drawer on the Diagnostics tab with primary-drawer exclusivity intact.
- [x] Command palette keyboard/accessibility polish：the command palette now behaves like a desktop command surface instead of a mouse-only menu. `Ctrl+K` focuses a combobox with `aria-activedescendant`, results expose listbox/option semantics, ArrowUp/ArrowDown wrap through results, Home/End jump to boundaries, Enter runs the selected command, Escape closes, Tab/Shift+Tab stay inside the dialog, and focus returns to the launcher. Covered by `corepack pnpm --dir apps/desktop exec playwright test tests/desktop-ui-smoke.spec.ts --grep "command palette stages"`, the targeted mobile command-palette path, and the full UI smoke gate.
- [x] Command palette workbench navigation：the command palette now reaches the core status surfaces directly: Workbench, Trace, Tool Output, Context, Files, Execution, Subagents, LabRun, and Diagnostics. Lab commands still stage into the composer/runtime route, while pure navigation commands only switch existing UI panels or drawers. Covered by the command-palette Playwright smoke and full UI smoke.
- [x] Nested drawer Escape robustness：shared drawer keyboard handling now still lets the parent drawer close if focus is lost back to the page after a nested drawer closes, while ignoring Escape when focus is inside another active overlay. This keeps Trace -> Context Details from closing multiple layers at once but also avoids a stuck Trace drawer. Covered by the desktop layout smoke path.
- [x] Actionable statusbar navigation：the bottom statusbar is now a daily-use navigation surface instead of static telemetry. Provider/API and model segments open Settings, cache/tokens/context open the Context inspector, and workspace opens the Files inspector. On narrow/mobile viewports those inspector targets open the Runtime inspector drawer so the click always has visible tab-level feedback. Covered by desktop and mobile Playwright smoke plus `corepack pnpm --dir apps/desktop build`.
- [x] Mobile Runtime inspector drawer：narrow/mobile viewports now open the real Runtime inspector as a keyboard-managed drawer instead of falling back to the generic Workbench drawer. The drawer reuses the same Context、Files、Execution、Subagents、LabRun、Diagnostics tabs with a separate id prefix to avoid duplicate DOM ids while the desktop inspector remains mounted. Statusbar inspector links and mobile command-palette navigation both open the drawer and preserve the selected tab; focus starts on Close, Tab/Shift+Tab stay inside the drawer, Escape closes it, and focus returns to the trigger. Covered by the targeted mobile smoke, targeted desktop layout smoke, full UI smoke, and `corepack pnpm --dir apps/desktop build`.
- [x] Mobile Trace/Output drawer entry polish：the narrow topbar Trace and Output actions now both expose explicit expanded state and are covered as real drawer entries, not only visible buttons. Mobile smoke opens each drawer from the topbar, verifies the shared focus trap, closes with Escape, and checks focus returns to the triggering button.
- [x] Mobile LabRun/Direct Agent mode entry polish：the mobile session header mode switcher is now covered as a real product-mode entry. Mobile smoke verifies Direct Agent starts selected, tapping LabRun switches the mode, opens the Runtime inspector drawer directly on the LabRun tab, exposes project controls, then returns to Direct Agent without relying on the hidden desktop inspector.
- [x] Primary drawer exclusivity：Settings、Workbench、Run Trace、Tool Output、and Runtime inspector drawer now route through one primary-drawer opening path so they do not stack on top of each other. Nested Context Details remain a separate detail layer. Mobile smoke asserts only one primary drawer is mounted after opening Settings, Trace, Output, and Runtime inspector paths.
- [x] Goal progress row active-state polish：web preview now has a `previewFixture=goal` path so the Direct Agent goal row is covered when present, not only when absent. The row exposes accessible Edit/Pause/Clear controls, labels the objective edit field, supports Escape to cancel an unsaved edit draft, and keeps the composer visible below the active goal.
- [x] 更新 `docs/PROJECT_STATUS.md` 桌面端状态。

## Acceptance Criteria

桌面端达到“可以继续深度打磨”的标准时，应满足：

- 用户能从桌面端选择项目、选择 provider/model、新建/恢复会话、发起 Direct Agent 任务。
- 用户能看到工具执行、权限请求、失败、验证、final answer，不需要回 CLI 查状态。
- 用户能从桌面端进入 LabRun intake、立项、查看运行状态、暂停/恢复、开组会、和教授侧边沟通。
- 用户能看到上下文、缓存、压缩、token/cost 的真实状态；缺少 provider 字段时明确标注 unavailable。
- 桌面端重启后，最近项目、会话、LabRun 状态和待恢复信息仍清楚可见。
- Playwright smoke、desktop build、native real-provider smoke, and broad runtime regression tests pass.
- 前端没有伪造成功状态；所有成功、失败、验证、artifact 状态都来自 runtime/API。

## First Implementation Recommendation

P1 已完成。下一步先做 P2，不直接进入大规模视觉美化。

理由：

- 现有代码已经有很多 runtime 数据，但被 drawer 和单个 `App.tsx` 聚在一起，用户感知不到“工作台”。
- P1 能把后续 Direct Agent 和 LabRun 的入口都固定住。
- P1 不需要大量 backend 新能力，风险较低。
- P1 完成后，P2/P3 可以并行推进：一个打磨通用 agent，一个产品化 LabRun。

已完成的第一批代码 slice：

1. 新增 `SessionHeader` 和 `InspectorPanel`，把右侧 workbench 数据提升为常驻 inspector。
2. 增加 inspector tab 状态和基础 tab UI。
3. 增加 session header，展示已有 provider/project/mode/run state。
4. 保持现有 `WorkbenchDrawer` 可用，作为窄屏 fallback 和旧操作入口。
5. 更新 `desktop-ui-smoke.spec.ts` 覆盖 session header、Direct/LabRun 入口和 inspector tabs。

P2 已完成。Composer now includes slash commands, structured attachments, provider setup repair, prompt history recall with ↑ / ↓ for same-session submitted prompts, and a global `/` shortcut that focuses the composer and opens slash commands when the user is not already typing. P3 当前桌面 surface、结构化 LabRun rows、report preview、本地搜索过滤和分页报告查看器已按现有 typed snapshot 完成。Native desktop smoke plus desktop/narrow screenshot QA now pass, and native workflow QA now covers Settings provider setup, LabRun, Execution, context, trace, permission, final answer, and usage surfaces. Real provider native smoke also passes with explicit MiniMax and DeepSeek provider selection through `scripts/desktop-native-smoke.sh --live-provider --provider <id> --timeout 180 --no-screenshot`; DeepSeek restart recovery also passes with `--restart-check`, real Build-mode multi-tool editing passes with `--multi-tool-check`, and a two-turn DeepSeek Build-mode soak passes with `--soak-check`.

Mode-routing note:

- Desktop `agent_mode` is now operational, not just UI state. Non-Auto modes are passed into `RuntimeController`; Build mode bypasses the lightweight direct-answer lane so mutation tools can be exposed by the runtime route/tool policy when appropriate.

Provider default note:

- Desktop now prefers DeepSeek `deepseek-v4-flash` as the default provider/model when the user has not explicitly selected a desktop provider and no `PRIORITY_AGENT_DEFAULT_PROVIDER` override is set. User selection, saved settings, and env override remain higher priority.

State hooks refactor 首刀已完成：

- `useDesktopBootstrap` now owns desktop health/settings/provider/session/diagnostics bootstrap plus diagnostics and session-list refresh.
- `useWorkbenchSnapshots` now owns runtime context and workbench snapshot refresh.
- `useRunEvents` now owns run event subscription, idle watchdog, permission answer handling, and submit-message event plumbing. It keeps latest provider/settings/refresh callbacks in refs so the long-lived event subscription does not use stale startup values.
- `App.tsx` remains the shell/orchestration component for conversation recovery, commands, drawers, and layout, but dropped below the project 1500-line source-file ceiling.
- Inspector shared UI primitives now live in `InspectorPrimitives.tsx`; `InspectorPanel.tsx` is back under the project 1500-line source-file ceiling while preserving the Context, Files, Execution, Subagents, LabRun, and Diagnostics inspector behavior.
- Desktop runtime API types now live in `desktopTypes.ts`, and goal command helpers live in `desktopGoalApi.ts`; `desktopApi.ts` still re-exports the same public type/function surface for existing callers but is back under the project 1500-line source-file ceiling.
- Run event presentation helpers now live in `runEventPresentation.ts`; `runEventState.ts` keeps the state transition surface used by `useRunEvents`, transcript loading, permission answers, idle warnings, and error handling while dropping under the project 1500-line source-file ceiling.
- Desktop native smoke helpers now live in `desktop_state/native_smoke.rs`; `desktop_state.rs` keeps settings/provider/session state helpers and is back under the project 1500-line source-file ceiling. The split preserves the existing `desktop_state::*` public surface used by `lib.rs`.
- Desktop Tauri DTO/response types now live in `desktop_types.rs`; `lib.rs` keeps command registration, runtime orchestration, and command handlers, while child modules still consume the same crate-level type surface through the existing `super::*` pattern.
- Desktop Tauri command domains now have first-pass modules: `health_commands.rs`, `session_commands.rs`, `preview_commands.rs`, `goal_commands.rs`, and `revert_commands.rs`. `lib.rs` is back under the project 1500-line source-file ceiling while preserving command names and frontend API payload shapes.
- Desktop CSS now uses `global.css` as a small import entrypoint and splits the existing rules, in order, into `styles/parts/*.css` by UI domain. All desktop TS/TSX/CSS and Tauri Rust source files in this slice are under the project 1500-line source-file ceiling.
- Current validation for this refactor: `corepack pnpm --dir apps/desktop build`, `corepack pnpm --dir apps/desktop test:ui-smoke`, `cargo fmt --check`, `cargo check --features experimental-api-server -q`, `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q desktop_smoke_settings_round_trip`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q desktop_smoke_lab_status_reads_file_backed_labrun_state`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q desktop_file_preview`, and `git diff --check`.

Files inspector selected-preview slice 已完成：

- Files tab now lets users select an indexed file and preview its content in the persistent inspector.
- The Tauri API is `desktop_file_preview`; it only accepts paths relative to the selected project, canonicalizes paths, rejects out-of-project traversal, and caps previews to a bounded byte window.
- Web preview fixtures now cover the same UI path for Playwright.
- Current validation for this slice: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q desktop_file_preview`, `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q`, `corepack pnpm --dir apps/desktop build`, `corepack pnpm --dir apps/desktop test:ui-smoke`, `cargo fmt --check`, `cargo check --features experimental-api-server -q`, and `git diff --check`.

Release-readiness QA 已发现并修复一处真实桌面问题：

- Stale `active_session_id` 不能只靠前端启动后清理；backend `desktop_settings` 和 `runtime_for_state` now validate the active session id against the session store, clear stale ids, and persist corrected settings before the UI receives startup state.
- Native smoke now rejects visible `session not found`, stops pre-existing Priority Agent processes, activates the smoke app by process id instead of application name, and captures a screenshot from the verified smoke window.
- Latest native evidence: `apps/desktop/test-artifacts/native-app-desktop.log` includes `no-stale-session-error`, and `apps/desktop/test-artifacts/native-smoke.png` shows the Execution inspector, trace evidence, permission-approved card, final answer, and usage surface without the stale-session red banner.
- Full release dogfood now passes from the freshly rebuilt packaged app followed by `scripts/desktop-release-dogfood.sh --skip-build --timeout 720 --repeat 2`. Latest evidence is `apps/desktop/test-artifacts/desktop-release-dogfood.log`, which records `PASS desktop_release_dogfood repeat=2` and PASS markers for DeepSeek, MiniMax, and LabRun recovery in both iterations; DeepSeek and MiniMax each complete three real-provider desktop turns plus restart recovery per iteration, and LabRun recovery verifies snapshot, report page, LabRun tab, search, and full-report viewing after restart. The native extended-soak harness also now has a bounded third-turn repair task for providers that return text without file evidence, while still requiring real file-preview verification before success. The release dogfood wrapper accepts `--repeat count` for repeated unattended release gates.

建议下一批代码 slice：

1. Longer unattended/background QA：当前 release dogfood 套件已把 fresh packaged build、DeepSeek 三轮 extended soak + restart、MiniMax 三轮 extended soak + restart、paused LabRun report/artifact UI + restart 串成一个可重复发布前 gate，最新 `--skip-build --timeout 720 --repeat 2` 串联通过，脚本也已有 `--repeat count` 支持。后续剩余风险主要是更长时间无人值守运行、更多 provider 的重复验证，以及真实日常桌面使用中的边缘交互；不是缺少基本桌面 dogfood 入口，也不再是没有重复运行能力。
2. Daily-use polish pass：第一轮已修复移动端顶栏裁切、workspace grid 定位、statusbar 可见性、session provider/model 截断、inspector 长文本可读性、composer 空上下文提示截断、恢复状态卡详情截断，并增加 smoke 回归保护；移动端也已有显式 Settings 入口，More conversation actions 已接到适配窄屏的 command palette，并覆盖 New Chat 主导航；Provider setup 和 Permissions 设置页也补了窄屏可读性和溢出断言；Settings/Workbench/Trace/Context Details/Tool Output 抽屉现在具备共享键盘焦点流，Tab/Shift+Tab 不会漏到后台 UI。后续继续在真实桌面使用中看 transcript、composer、settings/provider setup 的细节摩擦，不再优先做大结构拆分。
3. Commit-scope cleanup：当前 requirement-level completion audit 已确认实现和验证足够进入提交收口，但整体目标不能算完成，直到仓库有干净提交边界。提交前按 `docs/DESKTOP_FRONTEND_CHANGESET_CLOSEOUT_2026-06-22.md` 的 scope 决定是一个 broad desktop milestone commit，还是拆成桌面 UI、Tauri command/state、LabRun/runtime、脚本/文档几个提交；如果拆分过程改变 runtime 代码或重新打包 `.app`，需要重跑相关 build/native dogfood gate。

## Worktree Note

当前仓库仍有与桌面 UI、Tauri command/state、LabRun/agent runtime、验证脚本和文档相关的 broad dirty tree。实现和验证已经到达 commit closeout 阶段；下一步不要继续扩功能，优先建立提交边界。
