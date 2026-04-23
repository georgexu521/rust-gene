# Priority Agent - 快速开始指南

## 📋 目录
1. [环境准备](#环境准备)
2. [编译项目](#编译项目)
3. [配置 API](#配置-api)
4. [运行项目](#运行项目)
5. [测试项目](#测试项目)
6. [故障排除](#故障排除)

---

## 环境准备

### 1. 安装 Rust

```bash
# 使用 rustup 安装（推荐）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 验证安装
rustc --version  # 应该显示 1.70 或更高版本
cargo --version
```

### 2. 克隆项目

```bash
cd ~/Desktop
# 项目已在本地，无需克隆
```

---

## 编译项目

### 开发版本（推荐用于开发测试）

```bash
cd ~/Desktop/rust-agent

# 编译（约 2-5 分钟）
cargo build

# 验证编译成功
ls -la target/debug/priority-agent
```

### 发布版本（推荐用于日常使用）

```bash
# 编译优化版本（约 5-10 分钟，但运行更快）
cargo build --release

# 验证编译成功
ls -la target/release/priority-agent
```

### 编译遇到问题？

```bash
# 清理缓存重新编译
cargo clean
cargo build

# 更新依赖
cargo update
cargo build
```

---

## 配置 API

### 方式一：环境变量（推荐）

```bash
# 编辑你的 shell 配置文件
nano ~/.zshrc  # 或 ~/.bashrc

# 添加以下行
export MINIMAX_API_KEY="你的_MiniMax_Token_Plan_Key_这里"
export MINIMAX_BASE_URL="https://api.minimaxi.com/v1"
export MINIMAX_MODEL="MiniMax-M2.7"

# 或使用 Kimi
export MOONSHOT_API_KEY="你的_API_Key_这里"
export MOONSHOT_BASE_URL="https://api.moonshot.cn/v1"
export MOONSHOT_MODEL="kimi-k2.5"

# 使配置生效
source ~/.zshrc
```

**获取 API Key**:
- MiniMax Token Plan: [MiniMax 平台](https://platform.minimaxi.com/)
- Kimi: [Kimi 开放平台](https://platform.moonshot.cn/console/api-keys)

### 方式二：临时设置（仅当前终端）

```bash
export MINIMAX_API_KEY="你的_MiniMax_Token_Plan_Key_这里"
./target/debug/priority-agent
```

### 验证配置

```bash
# 运行帮助命令，查看是否有 provider 初始化日志（MiniMax/OpenAI/Kimi）
./target/debug/priority-agent --help
```

---

## 运行项目

### 方式一：TUI 模式（推荐，类似 Claude Code）

```bash
# 启动 TUI 界面
./target/debug/priority-agent

# 或显式指定
./target/debug/priority-agent --tui
```

**TUI 操作说明**:
- 输入消息，按 `Enter` 发送
- `Ctrl+C` 或 `Ctrl+Q` 退出
- 使用方向键浏览历史消息

### 方式二：Chat CLI 模式

```bash
# 显式使用 Chat CLI
./target/debug/priority-agent --cli
# 或
./target/debug/priority-agent chat
```

**Chat CLI 常用命令**:

```bash
# 1. 进入 CLI 聊天
./target/debug/priority-agent --cli

# 2. 在聊天中用斜杠命令
/help
/tools
/cost
/exit
```

### 方式三：安装到系统 PATH

```bash
# 安装到 /usr/local/bin（需要 sudo）
sudo cp target/release/priority-agent /usr/local/bin/

# 然后可以直接使用
priority-agent --help
priority-agent chat
```

---

## 测试项目

### 1. 运行单元测试

```bash
cd ~/Desktop/rust-agent

# 运行所有测试（约 10-20 秒）
cargo test

# 预期输出：
# test result: ok. 68 passed; 0 failed; 0 ignored
```

### 2. 运行特定测试

```bash
# 测试特定模块
cargo test tools::bash_tool
cargo test weight_engine
cargo test tui::components

# 测试特定功能
cargo test test_is_dangerous_command
cargo test test_input_unicode
```

### 3. 功能测试流程

```bash
# Step 1: 编译
cargo build

# Step 2: 测试 CLI 模式（无需 API Key）
./target/debug/priority-agent --cli
# 进入后执行 /help 然后 /exit

# Step 3: 测试 TUI 模式（需要 API Key）
./target/debug/priority-agent --tui
# 按 Ctrl+C 退出

# Step 4: 测试 API 模式（可选）
./target/debug/priority-agent --api --port 8787
```

### 4. 快速测试脚本

创建 `quick_test.sh`:

```bash
#!/bin/bash
set -e

echo "=== 编译项目 ==="
cargo build

echo ""
echo "=== 运行单元测试 ==="
cargo test

echo ""
echo "=== 测试 CLI 命令 ==="
printf '/help\n/exit\n' | ./target/debug/priority-agent --cli

echo ""
echo "✅ 所有测试通过！"
```

运行：
```bash
chmod +x quick_test.sh
./quick_test.sh
```

---

## 故障排除

### 问题 1: 编译错误 "package not found"

**解决**:
```bash
# 确保在项目根目录
cd ~/Desktop/rust-agent

# 检查 Cargo.toml 是否存在
ls Cargo.toml

# 重新生成 lock 文件
rm Cargo.lock
cargo build
```

### 问题 2: API Key 未设置

**错误信息**: `MINIMAX_API_KEY not set` 或 `MOONSHOT_API_KEY not set`

**解决**:
```bash
# 检查环境变量是否设置（示例以 MiniMax 为准）
echo $MINIMAX_API_KEY

# 如果没有输出，重新设置
export MINIMAX_API_KEY="your_key_here"

# 或添加到 shell 配置文件永久生效
echo 'export MINIMAX_API_KEY="your_key_here"' >> ~/.zshrc
source ~/.zshrc
```

### 问题 3: TUI 界面显示异常

**解决**:
```bash
# 确保终端支持 Unicode
export LANG=en_US.UTF-8

# 使用兼容模式运行
./target/debug/priority-agent --cli
```

### 问题 4: 测试失败

**解决**:
```bash
# 清理并重新测试
cargo clean
cargo test

# 查看详细错误信息
cargo test -- --nocapture
```

### 问题 5: 找不到命令

**错误信息**: `command not found: priority-agent`

**解决**:
```bash
# 方法 1: 使用完整路径
~/Desktop/rust-agent/target/debug/priority-agent

# 方法 2: 创建 alias
alias pa='~/Desktop/rust-agent/target/debug/priority-agent'
pn init

# 方法 3: 安装到 PATH
sudo cp ~/Desktop/rust-agent/target/release/priority-agent /usr/local/bin/
```

### 问题 6: 重置数据

```bash
# macOS
rm -rf ~/Library/Application\ Support/priority-agent/

# Linux
rm -rf ~/.local/share/priority-agent/

# 然后重新初始化
./target/debug/priority-agent --cli
```

---

## 📚 相关文档

- [API_SETUP.md](API_SETUP.md) - 详细的 API 配置说明
- [TESTING.md](TESTING.md) - 详细的测试指南
- [CLAUDE.md](CLAUDE.md) - 开发指南（给 Claude Code）
- [CODE_REVIEW.md](CODE_REVIEW.md) - 代码审查报告
- [IMPROVEMENTS.md](IMPROVEMENTS.md) - 最近的改进记录

---

## 🎯 快速参考

```bash
# 开发流程
cd ~/Desktop/rust-agent
cargo build
./target/debug/priority-agent --help

# 测试流程
cargo test
./target/debug/priority-agent --cli
# 进入后可执行: /help /tools /cost

# 日常使用（安装后）
pa
priority-agent
priority-agent --api --port 8787
```

---

**最后更新**: 2026-04-10
**版本**: Phase 2 MVP
