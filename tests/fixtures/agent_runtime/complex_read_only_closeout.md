复杂只读收口确认：请在当前 rust-agent 项目里检查工具循环、只读 Direct 任务收口、桌面事件桥接是否走同一套后端 runtime。

约束：
- 不要修改任何文件。
- 不要写入文件。
- 使用可用的只读工具读取或检索这些文件：
  - src/engine/streaming.rs
  - src/engine/conversation_loop/turn_iteration_loop_controller.rs
  - src/engine/code_change_workflow.rs
  - src/desktop_runtime/mod.rs
  - apps/desktop/src-tauri/src/lib.rs

最后严格按四项回复：
A 路由和实际工具；
B 是否进入重复工具循环；
C 只读 Direct 任务是否被错误拉进 code-change closeout；
D 桌面端是否只是薄入口，是否还像前端未连接。
