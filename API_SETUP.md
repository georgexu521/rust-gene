# Kimi API 配置指南

## 获取 API Key

1. 访问 [Kimi 开放平台](https://platform.moonshot.ai/console/api-keys)
2. 登录并创建新的 API Key
3. **重要**：API Key 只显示一次，请妥善保存

## 配置方式

### 方式一：环境变量（推荐）

```bash
# 添加到 ~/.zshrc 或 ~/.bashrc
export MOONSHOT_API_KEY="your_api_key_here"
export MOONSHOT_BASE_URL="https://api.moonshot.ai/v1"
export MOONSHOT_MODEL="kimi-k2.5"
```

### 方式二：.env 文件

```bash
# 复制模板文件
cp .env.example .env

# 编辑 .env 文件，填入你的 API Key
MOONSHOT_API_KEY=your_api_key_here
```

### 方式三：Claude Code 兼容模式

如果你想在 Claude Code 中使用 Kimi API：

```bash
export ANTHROPIC_BASE_URL=https://api.moonshot.ai/anthropic
export ANTHROPIC_AUTH_TOKEN=your_moonshot_api_key_here
export ANTHROPIC_MODEL=kimi-k2.5
```

## 支持的模型

| 模型 | 上下文长度 | 特点 |
|------|-----------|------|
| `kimi-k2.5` | 256K | 旗舰模型，推荐 |
| `kimi-k2-thinking` | 256K | 推理模式，更彻底 |
| `kimi-k2-turbo-preview` | - | 最快 (60-100 tokens/s) |

## 验证配置

```bash
# 编译并运行
cargo run -- --help

# 如果看到日志输出 "Kimi client initialized"，说明配置成功
```

## 国内用户

如果访问国际节点较慢，可以使用国内节点：

```bash
export MOONSHOT_BASE_URL="https://api.moonshot.cn/v1"
```

## 故障排除

### 错误：MOONSHOT_API_KEY not set
- 确保环境变量已正确设置
- 运行 `source ~/.zshrc` 或重启终端

### 错误：Failed to get response from Kimi API
- 检查 API Key 是否正确
- 检查网络连接
- 确认 base URL 是否正确

## 价格参考

请参考 [Kimi 官方定价](https://platform.moonshot.ai/docs/pricing)

---

配置完成后，你就可以使用 Priority Agent 的 AI 功能了！
