# 高频命令闭环清单（Execution Checklist）

> 版本：2026-04-21  
> 目标：从“命令入口存在”转为“命令行为闭环可用”

## 1. 使用规则（必须遵守）

1. 每个命令必须通过 4 类验收：`Success / Failure / Boundary / Recovery`。
2. 命令未通过 4 类验收，不得从 `Scaffold` 升级到 `Usable`。
3. 命令仍返回 `not implemented`，统一保持 `Scaffold`。
4. 每次迭代只提升一批命令，不新增新入口。

---

## 2. P0 批次（先做，直接影响可靠性）

| Command | 当前状态 | 闭环目标 | 验收要点 |
|---|---|---|---|
| `/rollback` | Scaffold | 可回滚且有保护 | 必须确认目标；错误提示明确；失败不破坏工作区 |
| `/session` | Usable | 会话切换稳定 | `0/越界/非法ID` 全部正确报错；切换后一致性正确 |
| `/retry` | Usable | 重试不破坏上下文 | 只重试目标轮次；不误删非目标消息；失败可恢复 |
| `/undo` + `/redo` | Usable | 语义正确 | redo 不得再次 undo；无 redo 栈时明确拒绝并解释 |
| `/import` | Scaffold | 输入安全+行为可预期 | 路径检查本地化；空格/特殊字符路径正确；错误可读 |
| `/test` | Usable | 结果可信 | 保留原始退出码；失败不误报成功；输出截断但状态正确 |

---

## 3. P1 批次（高频操作链）

| Command | 当前状态 | 闭环目标 | 验收要点 |
|---|---|---|---|
| `/config` | Scaffold | 可查可改可回滚 | get/set 行为一致；非法 key/value 报错 |
| `/reload` | Scaffold | reload 真正生效 | config/plugins/skills 各分支可验证 |
| `/desktop` | Scaffold | 最小可用实现 | open/notify 至少一条链路可用；失败提示明确 |
| `/chrome` | Scaffold | 最小可用实现 | open 可用；tabs/bookmarks 未实现时明确标注实验态 |
| `/prompt` | Scaffold | 可读可改 | show/edit 行为一致；输入边界不崩溃 |
| `/migrate` | Scaffold | 状态可查 | status 真实反映；up/down 未实现前禁发布 |
| `/webhook` | Scaffold | 基本生命周期 | list/create/delete 行为一致；URL 校验 |
| `/slack` | Scaffold | 连接状态可判定 | connect/disconnect/send 返回真实状态 |

---

## 4. P2 批次（体验与治理）

| Command | 当前状态 | 闭环目标 | 验收要点 |
|---|---|---|---|
| `/lsp` | Scaffold | list/restart/stop 可用 | server 不存在时错误清晰 |
| `/npm` | Scaffold | 多语言包管理结果可信 | 无包管理器时提示可行动 |
| `/hooks` | Scaffold | hook 配置可观测 | 当前 hook 状态可读；异常可诊断 |
| `/workspace` | Scaffold | 信息真实 | list/info 与实际路径一致 |
| `/write` | Scaffold | 写入行为安全 | 覆盖前确认；路径校验 |
| `/pause` | Scaffold | 状态可切换 | pause/resume 与实际执行状态一致 |
| `/focus` | Scaffold | 模式切换可见 | on/off 可验证 UI 或状态变化 |

---

## 5. 单命令验收模板（复制后执行）

```md
## /<command>
- [ ] Success: 正常输入返回正确结果
- [ ] Failure: 非法输入返回明确错误（无 panic）
- [ ] Boundary: 空输入/极长输入/特殊字符输入稳定
- [ ] Recovery: 失败后可重试，状态不污染
- [ ] Docs: CAPABILITY_MATRIX 状态与实现一致
- [ ] Tests: 添加契约测试（至少 4 条）
```

---

## 6. 发布门禁（命令维度）

1. `P0` 命令闭环通过率必须 `100%`。  
2. `Scaffold` 命令不得进入默认帮助主列表。  
3. 每次发布前必须跑：
   - `cargo test -q`
   - `cargo clippy --all-targets --all-features`
   - 命令契约测试集（建议新增 `tests/command_contracts/`）

---

## 7. 本周建议执行顺序（可直接照做）

1. `/rollback`  
2. `/undo` + `/redo`  
3. `/session` + `/retry`  
4. `/import` + `/test`  
5. `/config` + `/reload`

