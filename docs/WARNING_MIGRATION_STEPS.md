# Warning 清理迁移步骤（逐步去掉 `allow`）

## 目标

在不破坏现有可运行性的前提下，逐步移除 `#[allow(dead_code)]`，把代码从“能跑”推进到“可维护”。

## Step 1（已完成）: 用 feature gate 隔离未接线模块

已将以下模块改为按 feature 编译，默认关闭：

- `task_analyzer` -> `experimental-task-analyzer`
- `priority` -> `experimental-priority`
- `api` -> `experimental-api-server`

对应文件：

- `src/main.rs`
- `Cargo.toml`

验证方式：

```bash
cargo check -q
cargo test -q
```

按需启用示例：

```bash
cargo check --features experimental-api-server
cargo check --features experimental-priority
cargo check --features experimental-task-analyzer
```

## Step 2（已完成）: 移除核心路径上的局部 `allow(dead_code)`

优先顺序：

1. `engine` ✅ 已完成
2. `tools` ✅ 已完成
3. `tui` ✅ 已完成
4. `services` ✅ 已完成

策略：

- 先删 `mod` 级别 allow
- 若出现 warning：
  - 真正会用到但未接线 -> 补接线
  - 短期不会用 -> 下沉到具体 item 的最小范围 `#[allow(dead_code)]`

本轮完成内容：

- `src/main.rs`：移除 `mod engine;`、`mod tools;` 的模块级 `#[allow(dead_code)]`
- `src/engine/mod.rs`：把 `allow(dead_code)` 下沉到具体子模块
- `src/engine/socratic.rs`：`has_pending` 改为 `#[cfg(test)]`
- `src/tools/mod.rs`：仅对少量测试/预留 API 做 item 级 `allow`
- `src/tools/ask_tool/mod.rs`：仅对 `AskPending` / `take_pending` 做 item 级 `allow`
- `src/tools/examples.rs`：示例文件声明 `#![allow(dead_code)]`
- `src/tui/*`：移除 `mod tui` 的模块级 allow，未接线 UI 能力下沉到 item 级 allow
- `src/services/*`：移除 `mod services` 的模块级 allow，配置层暂用文件级 allow 保留接口

验证结果：

- `cargo check -q`：通过，0 warning
- `cargo test -q`：通过，151/151

## Step 3（已完成）: 建立长期约束（防回退）

建议加入 CI 检查：

- 默认构建：`cargo check -q`
- 关键 feature 构建：
  - `cargo check --features experimental-api-server`
  - `cargo check --features experimental-priority`
  - `cargo check --features experimental-task-analyzer`

已落地：

- `scripts/lint-check.sh`：统一执行默认构建 + 3 个 feature 构建
- `.github/workflows/ci.yml`：在 push/PR 上运行 `lint-check.sh` + `cargo test -q`

当前基线（2026-04-12，已清零）：

- default check: 0 warnings
- experimental-api-server: 0 warnings
- experimental-priority: 0 warnings
- experimental-task-analyzer: 0 warnings
- total: 0 warnings
