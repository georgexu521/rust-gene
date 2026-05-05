# Priority Agent 测试指南

## 📋 目录
1. [快速测试](#快速测试)
2. [单元测试](#单元测试)
3. [CLI 模式测试](#cli-模式测试)
4. [TUI 模式测试](#tui-模式测试)
5. [AI 功能测试](#ai-功能测试)
6. [工具系统测试](#工具系统测试)
7. [完整测试脚本](#完整测试脚本)

---

## 快速测试

### 一键测试（推荐）

```bash
cd ~/Desktop/rust-agent

# 运行当前 workflow-enabled 全量测试
env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1

# 预期输出：
# test result: ok. 1059 passed; 0 failed; 0 ignored
```

### 当前推荐门禁（2026-05-05）

```bash
# 编码工作流快速/标准门禁
scripts/coding-workflow-gates.sh standard

# 文档、all-features build、workflow-enabled 全量测试
bash scripts/validate_docs.sh

# Clippy 发布门禁
cargo clippy --all-features -- -D warnings
```

### Audit API 联调 Smoke Test

先启动 API 服务（另一个终端）：

```bash
cd ~/Desktop/rust-agent
cargo run -- --api --port 8080
```

再运行审计接口联调脚本：

```bash
cd ~/Desktop/rust-agent
BASE_URL=http://127.0.0.1:8080 ./scripts/audit-api-smoke.sh
```

可选参数：

```bash
EXPORT_PATH=/tmp/my-audit.json ./scripts/audit-api-smoke.sh
```

### 编译检查

```bash
# 检查代码是否能编译
cargo check

# 编译发布版本
cargo build --release
```

---

## 单元测试

### 运行所有单元测试

```bash
cargo test
```

### 按模块测试

```bash
# 测试工具系统
cargo test tools::

# 测试 TUI 组件
cargo test tui::

# 测试权重引擎
cargo test weight_engine::

# 测试 Agent 系统
cargo test agent::

# 测试引擎
cargo test engine::
```

### 测试特定功能

```bash
# 测试危险命令检测
cargo test test_is_dangerous_command

# 测试 Unicode 输入
cargo test test_input_unicode

# 测试文件操作
cargo test test_file_write_and_read

# 测试权重计算
cargo test test_calculate_absolute_weights
```

### 查看测试输出

```bash
# 显示详细输出
cargo test -- --nocapture

# 只显示失败测试
cargo test 2>&1 | grep -A 10 "FAILED"
```

---

## CLI 模式测试

CLI 模式不需要 API Key，适合快速测试基础功能。

### 1. 基础命令测试

```bash
# 编译
cargo build

# 查看帮助
./target/debug/priority-agent --help
./target/debug/priority-agent --legacy help
```

### 2. 项目初始化测试

```bash
# 初始化项目
./target/debug/priority-agent --legacy init

# 验证数据文件创建
ls ~/Library/Application\ Support/priority-agent/  # macOS
ls ~/.local/share/priority-agent/                   # Linux
```

### 3. 任务管理测试

```bash
# 添加任务
./target/debug/priority-agent --legacy add "实现用户认证系统"
./target/debug/priority-agent --legacy add "设计数据库模型"
./target/debug/priority-agent --legacy add "编写API文档"

# 列出任务
./target/debug/priority-agent --legacy list

# 查看推荐任务
./target/debug/priority-agent --legacy next
```

### 4. 任务完成流程测试

```bash
# 获取任务 ID
./target/debug/priority-agent --legacy list

# 完成任务（使用实际的任务 ID）
./target/debug/priority-agent --legacy done task_1234567890

# 查看进度
./target/debug/priority-agent --legacy progress
```

### 5. 权重分析测试

```bash
# 基础分析
./target/debug/priority-agent --legacy analyze

# AI 分析（无需 API Key，使用启发式算法）
./target/debug/priority-agent --legacy ai-analyze

# AI 建议
./target/debug/priority-agent --legacy ai-suggest
```

### 6. 快照功能测试

```bash
# 创建快照
./target/debug/priority-agent --legacy snapshot "v1.0"

# 查看快照列表（通过状态文件）
cat ~/Library/Application\ Support/priority-agent/snapshots/*.json

# 恢复快照
./target/debug/priority-agent --legacy restore <snapshot_id>
```

---

## TUI 模式测试

TUI 模式需要配置 API Key，提供类似 Claude Code 的交互体验。

### 1. 环境准备

```bash
# 设置 API Key
export MOONSHOT_API_KEY="your-api-key"

# 验证配置
./target/debug/priority-agent --help 2>&1 | grep -i "kimi"
# 应该看到 "Kimi client initialized" 日志
```

### 2. 启动 TUI

```bash
# 方式 1：默认启动 TUI
./target/debug/priority-agent

# 方式 2：显式指定 TUI
./target/debug/priority-agent --tui

# 方式 3：强制 TUI（忽略其他参数）
./target/debug/priority-agent -t
```

### 3. TUI 操作测试

启动 TUI 后，测试以下操作：

| 按键 | 功能 | 预期结果 |
|------|------|----------|
| 输入文字 + Enter | 发送消息 | 消息显示在上方区域 |
| Ctrl+C | 退出 | 程序退出，返回 shell |
| Ctrl+Q | 退出 | 程序退出 |
| ↑ / ↓ | 滚动消息 | 浏览历史消息 |
| ← / → | 移动光标 | 在输入框中移动 |
| Backspace | 删除字符 | 删除光标前字符 |
| Delete | 删除字符 | 删除光标后字符 |

### 4. TUI 功能测试清单

- [ ] 启动后显示输入框
- [ ] 可以输入中文
- [ ] 可以输入英文
- [ ] 可以输入特殊字符
- [ ] 可以删除字符
- [ ] 可以移动光标
- [ ] 可以滚动查看长消息
- [ ] Ctrl+C 可以退出
- [ ] 窗口大小改变时自适应

### 5. 流式响应测试

TUI 支持流式响应显示：

```bash
# 启动 TUI
./target/debug/priority-agent

# 输入一个复杂问题
# 预期：响应应该逐字显示，而不是等全部完成才显示
```

### 6. 工具调用测试

在 TUI 中测试工具调用：

```
# 输入以下内容测试 Bash 工具
列出当前目录的文件

# 输入以下内容测试 File 工具
读取 Cargo.toml 文件

# 输入以下内容测试 Grep 工具
搜索代码中包含 "fn main" 的文件
```

### 7. TUI 错误处理测试

```bash
# 测试 1：无 API Key 启动
unset MOONSHOT_API_KEY
./target/debug/priority-agent
# 预期：回退到 CLI 模式或显示错误

# 测试 2：错误的 API Key
export MOONSHOT_API_KEY="invalid-key"
./target/debug/priority-agent
# 预期：显示 API 错误
```

---

## AI 功能测试

### 前提条件

```bash
export MOONSHOT_API_KEY="your-valid-api-key"
```

### 1. 基础对话测试

```bash
# TUI 模式
./target/debug/priority-agent

# 输入简单问候
你好，请介绍一下自己

# 预期：AI 应该回复并介绍 Priority Agent
```

### 2. 工具调用测试

```bash
# TUI 模式输入
请读取 README.md 文件并总结内容

# 预期：
# 1. AI 调用 file_read 工具
# 2. 显示文件内容
# 3. AI 总结内容
```

### 3. 多轮对话测试

```bash
# 第 1 轮
添加一个任务：实现用户登录功能

# 第 2 轮
查看当前有哪些任务

# 第 3 轮
哪个任务优先级最高？

# 预期：AI 应该记住上下文
```

### 4. AI 分析测试

```bash
# 先添加一些任务
./target/debug/priority-agent --legacy add "实现核心认证系统 - 这是基础功能"
./target/debug/priority-agent --legacy add "修复紧急安全漏洞"
./target/debug/priority-agent --legacy add "更新 README 文档"

# TUI 模式输入
分析当前任务的优先级

# 预期：AI 应该使用权重系统分析任务
```

---

## 工具系统测试

### 1. Bash 工具测试

```bash
# TUI 输入测试
运行命令：echo "Hello World"
运行命令：ls -la
运行命令：pwd

# 危险命令应该被拦截
运行命令：rm -rf /
# 预期：提示危险命令，拒绝执行
```

### 2. 文件工具测试

```bash
# TUI 输入测试
读取文件 Cargo.toml
写入文件 /tmp/test.txt，内容：Hello World
编辑文件 /tmp/test.txt，将 Hello 替换为 Hi

# 预期：
# 1. file_read 读取文件
# 2. file_write 创建文件
# 3. file_edit 修改文件
```

### 3. 搜索工具测试

```bash
# TUI 输入测试
搜索包含 "fn main" 的文件
查找所有 .rs 文件
在 src 目录搜索 "TODO"

# 预期：
# 1. grep 工具搜索内容
# 2. glob 工具查找文件
```

### 4. 工具权限测试

```bash
# 测试只读模式
echo "MOONSHOT_API_KEY=test" > /tmp/test-env
./target/debug/priority-agent --legacy --read-only

# 在 TUI 中尝试编辑文件
# 预期：应该被拒绝或提示确认
```

---

## 完整测试脚本

### 自动化测试脚本

创建 `run_all_tests.sh`:

```bash
#!/bin/bash
set -e

echo "=== Priority Agent 完整测试套件 ==="
echo ""

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 测试计数
TESTS_PASSED=0
TESTS_FAILED=0

run_test() {
    local test_name=$1
    local test_command=$2
    
    echo -n "测试: $test_name ... "
    if eval "$test_command" > /dev/null 2>&1; then
        echo -e "${GREEN}通过${NC}"
        ((TESTS_PASSED++))
    else
        echo -e "${RED}失败${NC}"
        ((TESTS_FAILED++))
    fi
}

echo "1. 编译检查"
echo "============"
run_test "代码检查" "cargo check"
run_test "发布版本编译" "cargo build --release"

echo ""
echo "2. 单元测试"
echo "============"
run_test "所有单元测试" "cargo test"
run_test "工具系统测试" "cargo test tools::"
run_test "TUI组件测试" "cargo test tui::"
run_test "权重引擎测试" "cargo test weight_engine::"

echo ""
echo "3. CLI 功能测试"
echo "================"
# 清理旧数据
rm -rf ~/Library/Application\ Support/priority-agent/
run_test "项目初始化" "./target/release/priority-agent --legacy init"
run_test "添加任务" "./target/release/priority-agent --legacy add '测试任务'"
run_test "查看推荐" "./target/release/priority-agent --legacy next"
run_test "查看进度" "./target/release/priority-agent --legacy progress"

echo ""
echo "4. 特定功能测试"
echo "================"
run_test "危险命令检测" "cargo test test_is_dangerous_command"
run_test "Unicode输入" "cargo test test_input_unicode"
run_test "文件操作" "cargo test test_file_write_and_read"

echo ""
echo "=== 测试结果汇总 ==="
echo -e "通过: ${GREEN}$TESTS_PASSED${NC}"
echo -e "失败: ${RED}$TESTS_FAILED${NC}"

if [ $TESTS_FAILED -eq 0 ]; then
    echo -e "${GREEN}所有测试通过！${NC}"
    exit 0
else
    echo -e "${RED}有测试失败，请检查。${NC}"
    exit 1
fi
```

运行：

```bash
chmod +x run_all_tests.sh
./run_all_tests.sh
```

### 手动测试清单

打印此清单，手动勾选：

```
□ 编译成功 (cargo build)
□ 单元测试通过 (cargo test)
□ CLI init 工作
□ CLI add 工作
□ CLI next 工作
□ CLI list 工作
□ CLI progress 工作
□ TUI 启动正常
□ TUI 可以输入
□ TUI 可以退出 (Ctrl+C)
□ TUI 中文输入正常
□ TUI 响应显示正常
□ 工具调用正常
□ AI 分析正常 (需要 API Key)
```

---

## 故障排除

### 测试失败排查

```bash
# 1. 清理并重新测试
cargo clean
cargo test

# 2. 查看详细错误
cargo test -- --nocapture 2>&1 | less

# 3. 只运行失败测试
cargo test <失败测试名> -- --nocapture

# 4. 检查环境
rustc --version  # 需要 1.70+
cargo --version
```

### 常见测试失败原因

| 失败现象 | 可能原因 | 解决方案 |
|----------|----------|----------|
| 编译错误 | 依赖问题 | `cargo update && cargo build` |
| Unicode 测试失败 | 系统编码 | `export LANG=en_US.UTF-8` |
| 文件测试失败 | 权限问题 | 检查 `/tmp` 目录权限 |
| TUI 测试失败 | 终端不支持 | 使用支持的终端（iTerm2, Terminal.app） |
| AI 测试失败 | API Key 无效 | 检查 `MOONSHOT_API_KEY` |

---

## 持续集成测试

### GitHub Actions 配置示例

```yaml
# .github/workflows/test.yml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rust-lang/setup-rust-toolchain@v1
      - run: cargo test
      - run: cargo build --release
```

### 本地预提交检查

```bash
# 创建 git hook
cat > .git/hooks/pre-commit << 'EOF'
#!/bin/bash
cargo test || exit 1
cargo clippy -- -D warnings || exit 1
EOF

chmod +x .git/hooks/pre-commit
```

---

**最后更新**: 2026-04-10
**版本**: Phase 2 MVP
**测试数量**: 68 个单元测试
