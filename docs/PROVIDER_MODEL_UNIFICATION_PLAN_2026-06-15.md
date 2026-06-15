# Provider / Model 统一接入与交互配置补完计划

**Date:** 2026-06-15  
**Scope:** `priority-agent` TUI + `src/services/api` provider/model layer  
**Reference:** OpenCode (`https://github.com/anomalyco/opencode`) provider architecture

---

## 1. 现状与差距

### 1.1 priority-agent 现状

| 能力 | 当前实现 | 关键文件 |
|------|---------|---------|
| Provider 注册 | `ProviderType` enum + `DEFAULT_PROVIDER_ENV_SPECS` + `PRIORITY_AGENT_PROVIDER_*` 环境变量 | `src/services/api/provider.rs` |
| 模型列表 | 静态 `builtin_catalog()`，按 provider 硬编码 `Vec<String>` | `src/services/api/provider_catalog.rs` |
| Adapter | 大部分是 `OpenAiClient`；特殊 provider 单独实现（`KimiClient`, `MiniMaxClient`） | `src/services/api/openai.rs`, `src/services/api/minimax.rs` |
| TUI 切换 | `ctrl+m` / `ctrl+l` 打开 picker，↑/↓ + Enter 选择 | `src/tui/app/palette.rs` |
| 凭证 | `/connect <provider> <key>` 写入 `~/.priority-agent/.env` | `src/services/api/credentials.rs` |
| 配置持久化 | `AppConfig` 仅存 `provider_name`, `model`, `base_url` | `src/services/config.rs` |

### 1.2 与 OpenCode 的核心差距

1. **接入层未统一**：每加一个 provider 要改 `ProviderType`、`create_provider`、能力检测、模型 catalog，成本 30–500+ 行。
2. **无交互式配置**：`/connect` 只能命令行一次性传入 key，没有 OAuth/API key 向导，也不验证 key。
3. **模型静态**：新模型必须改代码发布，不会调用 provider `/models` 接口。
4. **配置分散**：key 在 `.env`，provider 元数据在 Rust 代码，用户无法写自定义 provider。

---

## 2. 目标架构（OpenCode 等价实现）

priority-agent 是 Rust 二进制，无法直接复用 OpenCode 的 npm/AI-JS-SDK 插件体系，但可以在 Rust 侧实现**同构的架构目标**：

```
┌─────────────────────────────────────────────────────────────┐
│                         TUI Layer                            │
│  /connect  →  ProviderConnectDialog                          │
│  /models   →  ModelSelectDialog (dynamic + static)           │
└───────────────────────┬─────────────────────────────────────┘
                        │
┌───────────────────────▼─────────────────────────────────────┐
│                  Provider Manager                            │
│  • 读取 providers.toml / providers.json + env + auth store   │
│  • 维护 ProviderManifest 注册表                              │
│  • 调用 /models 动态发现并缓存                               │
└───────────────────────┬─────────────────────────────────────┘
                        │
┌───────────────────────▼─────────────────────────────────────┐
│                 Adapter Runtime                              │
│  • OpenAI-compatible adapter (default)                       │
│  • Provider-specific override adapters (rare)                │
│  • Capability detection by manifest + runtime probe          │
└───────────────────────┬─────────────────────────────────────┘
                        │
┌───────────────────────▼─────────────────────────────────────┐
│                 Auth Store                                   │
│  • ~/.priority-agent/auth.toml / keyring (optional)          │
│  • env fallback                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## 3. 实施路线图

建议分 **4 个阶段**，每个阶段可独立合并并通过 CI。

### Phase 1: 配置即代码 — 外置 Provider Manifest

**目标**：把 provider 定义从 Rust 代码移到配置文件，消除双 catalog 重复。

#### 3.1.1 新增 `ProviderManifest` 结构

```rust
// src/services/api/provider_manifest.rs
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderManifest {
    pub id: String,                 // "deepseek"
    pub name: String,               // "DeepSeek"
    pub provider_family: ProviderProtocolFamily,
    pub npm_adapter: Option<String>, // reserved for future wasm/node plugin bridge
    pub env: Vec<String>,           // ["DEEPSEEK_API_KEY"]
    pub base_url: String,
    pub default_model: String,
    pub models_source: ModelsSource,
    pub capabilities: ProviderCapabilitiesSpec,
    pub auth: AuthSpec,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModelsSource {
    Static { models: Vec<String> },
    Dynamic { list_url: String },
    OpenAiCompatible,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthSpec {
    pub methods: Vec<AuthMethod>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthMethod {
    ApiKey { prompt: String, env: String },
    OAuth { authorize_url: String },
    EnvOnly { env: String },
}
```

#### 3.1.2 配置文件位置

优先级从高到低：

1. `PRIORITY_AGENT_PROVIDERS_CONFIG=/path/to/providers.toml`
2. `./.priority-agent/providers.toml`（项目级）
3. `~/.config/priority-agent/providers.toml`（用户级）
4. 内置 fallback（把当前 `DEFAULT_PROVIDER_ENV_SPECS` + `builtin_catalog` 转成 toml 内嵌）

#### 3.1.3 替换 `DEFAULT_PROVIDER_ENV_SPECS`

- 新建 `resources/providers.toml`，把当前 6 个内置 provider 完整描述迁移进去。
- `ProviderRegistry::from_env()` 改为先读 manifest，再覆盖 env/auth。
- `ProviderType` 保留为运行时协议 family 与默认值推断器（从 provider id 解析到 `ProviderProtocolFamily`、默认 base URL / model）。不再新增 enum variant；自定义 provider 统一映射到 `ProviderType::Custom` 并回退到 OpenAI-compatible adapter。

#### 3.1.4 文件改动

| 文件 | 改动 |
|------|------|
| `src/services/api/provider_manifest.rs` | 新增 manifest schema + load/merge |
| `resources/providers.toml` | 内置 provider 清单 |
| `src/services/api/provider.rs` | `ProviderRegistry` 读 manifest；`ProviderType` 保留为 family/默认值推断器 |
| `src/services/api/provider_catalog.rs` | 删除 `builtin_catalog()`；从 manifest 取模型列表 |
| `src/services/config.rs` | 增加 `providers_config_path` |

#### 3.1.5 验收

```bash
cargo test -q provider_manifest
cargo test -q provider
cargo test -q route_scoped_tools
bash scripts/tui_tool_turn_spine_fixture_matrix.sh
```

---

### Phase 2: 统一接入层 — Adapter Registry

**目标**：用 OpenAI-compatible adapter 作为默认路径，把 `create_provider` 的硬编码 match 改成注册表。

#### 3.2.1 抽象 Adapter Factory

```rust
// src/services/api/adapter.rs
#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    async fn chat(&self, req: ChatRequest) -> anyhow::Result<ChatResponse>;
    async fn chat_stream(&self, req: ChatRequest) -> anyhow::Result<ChatCompletionResponseStream>;
    fn base_url(&self) -> &str;
    fn default_model(&self) -> &str;
}

pub type AdapterFactory = Arc<dyn Fn(ProviderConfig, AuthResolver) -> Arc<dyn ProviderAdapter> + Send + Sync>;

pub struct AdapterRegistry {
    factories: HashMap<ProviderProtocolFamily, AdapterFactory>,
}
```

#### 3.2.2 注册表初始化

```rust
// src/services/api/mod.rs
pub fn default_adapter_registry() -> AdapterRegistry {
    let mut r = AdapterRegistry::new();
    r.register(ProviderProtocolFamily::OpenAiCompatible, Arc::new(|cfg, auth| {
        Arc::new(OpenAiClient::new(cfg, auth))
    }));
    r.register(ProviderProtocolFamily::MiniMax, Arc::new(|cfg, auth| {
        Arc::new(MiniMaxClient::new(cfg, auth))
    }));
    r.register(ProviderProtocolFamily::Kimi, Arc::new(|cfg, auth| {
        Arc::new(KimiClient::new(cfg, auth))
    }));
    r
}
```

#### 3.2.3 运行时构造

```rust
// 原 create_provider 中的 match 改为
let adapter = registry.get(&manifest.provider_family)
    .unwrap_or_else(|| registry.get(&ProviderProtocolFamily::OpenAiCompatible).unwrap())
    .call(config.clone(), auth_resolver);
```

#### 3.2.4 能力检测去字符串化

把 `ProviderCapabilities::detect` 从字符串匹配改为读取 manifest 的 `capabilities` 字段：

```toml
[[provider]]
id = "deepseek"
capabilities = { streaming = true, streaming_tools = true, merge_system = false }
```

保留运行时 probe 作为 override（Phase 4）。

#### 3.2.5 文件改动

| 文件 | 改动 |
|------|------|
| `src/services/api/adapter.rs` | 新增 adapter trait + registry |
| `src/services/api/openai.rs` | `OpenAiClient` 实现 `ProviderAdapter` |
| `src/services/api/minimax.rs` | `MiniMaxClient` 实现 `ProviderAdapter` |
| `src/services/api/kimi.rs`（如存在） | 实现 `ProviderAdapter` |
| `src/services/api/provider.rs` | `create_provider` 使用 registry |
| `src/services/api/provider_protocol.rs` | 能力检测读 manifest |

#### 3.2.6 验收

```bash
cargo test -q --lib
cargo clippy --all-targets --all-features -- -D warnings
bash scripts/tui_tool_turn_spine_fixture_matrix.sh
```

---

### Phase 3: 交互式配置 — `/connect` 向导

**目标**：TUI 内直接 `/connect` 调出 provider 列表，按提示输入 key 或 OAuth，自动保存并验证。

#### 3.3.1 TUI 新增 `ConnectWizard` 模式

```rust
// src/tui/app/connect_wizard.rs
pub struct ConnectWizardState {
    pub step: ConnectStep,
    pub provider_id: Option<String>,
    pub auth_method_idx: usize,
    pub input_buffer: String,
    pub status: WizardStatus,
}

pub enum ConnectStep {
    SelectProvider,
    SelectMethod,
    InputCredential,
    Validating,
    Done,
}
```

#### 3.3.2 Slash command 注册

在 `src/tui/app/slash_commands.rs` 中把 `/connect` 从参数解析改为无参打开 wizard：

```rust
SlashCommand {
    name: "connect",
    description: "Configure a model provider interactively",
    handler: Box::new(|app, _args| {
        app.open_connect_wizard();
        Ok(())
    }),
}
```

#### 3.3.3 交互渲染

```rust
// src/tui/screens/main_screen/render_connect_wizard.rs
pub fn render_connect_wizard(f: &mut Frame, app: &TuiApp, area: Rect) {
    // 弹窗：provider 列表 / method 列表 / 输入框 / 验证中 / 结果
}
```

#### 3.3.4 凭证保存与验证流程

1. 用户选择 provider 和 method
2. 输入 key → `AuthStore::set(provider_id, key)`
3. 触发 `ProviderManager::validate(provider_id)`
4. 校验方法：调用一次 cheap 请求（例如 `GET /models` 或 `POST /chat/completions` 带 `"max_tokens":1`）
5. 成功后写入配置并显示 notice；失败保留输入框并显示错误

#### 3.3.5 AuthStore 升级

当前 `~/.priority-agent/.env` 可以继续作为底层存储，但增加结构化层：

```rust
// src/services/api/auth_store.rs
pub struct AuthStore {
    path: PathBuf,
}
impl AuthStore {
    pub fn set(&self, provider_id: &str, key: &str) -> anyhow::Result<()>;
    pub fn get(&self, provider_id: &str) -> Option<String>;
    pub fn remove(&self, provider_id: &str) -> anyhow::Result<()>;
    pub fn list(&self) -> Vec<String>;
}
```

#### 3.3.6 OAuth 支持（可选 MVP 后实现）

MVP 先支持 `ApiKey` 和 `EnvOnly`。OAuth 在 manifest 中声明，wizard 显示“在浏览器打开以下 URL，授权后粘贴 code”，底层用 `tokio::net` 启动本地 callback server（参考 OpenCode `provider/auth.ts`）。

#### 3.3.7 文件改动

| 文件 | 改动 |
|------|------|
| `src/tui/app/connect_wizard.rs` | 新增 wizard 状态机 |
| `src/tui/screens/main_screen/render_connect_wizard.rs` | 新增渲染 |
| `src/tui/mod.rs` | 添加 `AppMode::ConnectWizard` 路由 |
| `src/tui/app/slash_commands.rs` | `/connect` 改为打开 wizard |
| `src/services/api/auth_store.rs` | 结构化凭证存储 |
| `src/services/api/provider_manager.rs` | `validate()` 方法 |
| `src/tui/app.rs` | 添加 `open_connect_wizard()` |

#### 3.3.8 验收

```bash
# 手动测试（需要真实 key）
PRIORITY_AGENT_DEFAULT_PROVIDER=deepseek ./target/debug/priority-agent --tui
# 输入 /connect，选择 deepseek，输入 key，验证成功
```

自动化：

```bash
bash scripts/tui_tool_turn_spine_fixture_matrix.sh
```

---

### Phase 4: 动态模型发现 — `/models`

**目标**：TUI `/models` 能拉取 provider 的实时模型列表并缓存。

#### 3.4.1 模型发现服务

```rust
// src/services/api/model_discovery.rs
pub struct ModelDiscovery {
    cache: Arc<Mutex<ModelCache>>,
}

impl ModelDiscovery {
    pub async fn list(&self, provider_id: &str, manifest: &ProviderManifest) -> Vec<DiscoveredModel>;
    pub async fn refresh(&self, provider_id: &str);
}

pub struct DiscoveredModel {
    pub id: String,
    pub name: String,
    pub context_limit: Option<usize>,
    pub supports_tools: Option<bool>,
    pub supports_vision: Option<bool>,
}
```

#### 3.4.2 发现策略

按 `models_source` 执行：

- `Static`：直接返回 manifest 列表。
- `OpenAiCompatible`：调用 `GET {base_url}/models`（OpenAI-compatible），解析 `data[].id`。
- `Dynamic { list_url }`：调用自定义 URL，通过 `jq`-like JSON path 映射。

缓存到 `~/.cache/priority-agent/models/{provider_id}.json`， freshness 5 分钟，background refresh 60 分钟（同 OpenCode）。

#### 3.4.3 TUI `/models` 命令

```rust
SlashCommand {
    name: "models",
    handler: Box::new(|app, _args| {
        app.open_model_select();  // 已存在，但改为从 ModelDiscovery 取列表
        Ok(())
    }),
}
```

`model_choices()` 改为优先使用 discovery 结果，fallback 到 manifest static 列表。

#### 3.4.4 文件改动

| 文件 | 改动 |
|------|------|
| `src/services/api/model_discovery.rs` | 新增发现服务 |
| `src/services/api/provider_manifest.rs` | 增加 `ModelsSource` |
| `src/tui/app/palette.rs` | `model_choices()` 调用 discovery |
| `src/tui/app/slash_commands.rs` | 注册 `/models` |
| `Cargo.toml` | 可能需要 `dirs`, `reqwest` 已在 |

#### 3.4.5 验收

```bash
cargo test -q model_discovery
bash scripts/tui_tool_turn_spine_fixture_matrix.sh
```

---

## 4. 文件改动总表

| 新增文件 | 说明 |
|---------|------|
| `resources/providers.toml` | 内置 provider manifest |
| `src/services/api/provider_manifest.rs` | manifest schema + load |
| `src/services/api/adapter.rs` | adapter trait + registry |
| `src/services/api/auth_store.rs` | 结构化凭证存储 |
| `src/services/api/provider_manager.rs` | provider 生命周期管理 |
| `src/services/api/model_discovery.rs` | 动态模型发现 |
| `src/tui/app/connect_wizard.rs` | `/connect` wizard 状态 |
| `src/tui/screens/main_screen/render_connect_wizard.rs` | wizard UI |

| 修改文件 | 说明 |
|---------|------|
| `src/services/api/provider.rs` | 改用 manifest + registry |
| `src/services/api/provider_catalog.rs` | 删除静态 catalog，从 manifest 读取 |
| `src/services/api/provider_protocol.rs` | 能力检测读 manifest |
| `src/services/api/openai.rs` | 实现 `ProviderAdapter` |
| `src/services/api/minimax.rs` | 实现 `ProviderAdapter` |
| `src/services/config.rs` | 持久化 provider/model/base_url |
| `src/services/api/credentials.rs` | 迁移到 `AuthStore` |
| `src/tui/app/palette.rs` | picker 对接 discovery |
| `src/tui/app/slash_commands.rs` | `/connect` / `/models` 命令 |
| `src/tui/mod.rs` | `AppMode::ConnectWizard` 路由 |
| `src/tui/app.rs` | 添加 wizard 状态字段 |
| `src/tui/screens/main_screen/mod.rs` | 渲染 dispatch |
| `src/tui/keybindings.rs` | 可保留 ctrl+m/l，增加 `/connect` 帮助文案 |
| `Cargo.toml` | 视需要加依赖 |

---

## 5. 关键设计决策

### 5.1 为什么不用 wasm/node bridge 复用 OpenCode 的 npm adapter？

priority-agent 是单一 Rust 二进制，引入 Node/npm 运行时会显著增加包体积和启动复杂度。本计划选择**用 OpenAI-compatible HTTP adapter 覆盖 80% 供应商**，仅在协议真的不兼容时才写专用 adapter（如 MiniMax）。这与 OpenCode 默认 AI SDK adapter 的思路等价。

### 5.2 为什么保留 `~/.priority-agent/.env` 作为底层存储？

兼容现有用户配置，不破坏已有 key。`AuthStore` 是结构化的访问层，未来可透明迁移到 keyring 或加密文件。

### 5.3 manifest 为什么用 toml 而不是 json？

priority-agent 是终端工具，toml 更适合人工编辑的 provider 清单，且 Rust 生态对 toml 支持成熟（项目已依赖 `toml`）。

### 5.4 能力检测为什么不继续用字符串匹配？

字符串匹配在 provider 自定义 base URL 或新模型名时容易误判。manifest 显式声明能力是更可靠的方式，同时保留运行时 probe 做 override。

---

## 6. 验收标准

### 6.1 自动化验收

```bash
# 1. 全量测试
cargo check -q
cargo fmt --check
cargo test -q

# 2. 新增模块测试
cargo test -q provider_manifest
cargo test -q adapter_registry
cargo test -q auth_store
cargo test -q model_discovery

# 3. TUI fixture 矩阵
bash scripts/tui_tool_turn_spine_fixture_matrix.sh

# 4. 生产 gates
bash scripts/workflow-production-gates.sh
```

### 6.2 手动验收

1. 删除 `~/.config/priority-agent/providers.toml`，启动 TUI，确认内置 provider 仍可用。
2. 创建自定义 `~/.config/priority-agent/providers.toml`，声明一个 OpenAI-compatible 本地 proxy，TUI 能切换。
3. TUI 输入 `/connect`，选择 deepseek/openai，输入 key，看到 "Provider connected" notice。
4. TUI 输入 `/models`，看到 provider 实时模型列表（可离线 fallback 到 static）。
5. 切换模型后发一条消息，确认请求用上新模型。

---

## 7. 风险与回退

| 风险 | 缓解 |
|------|------|
| manifest 加载失败导致启动崩溃 | 内置 fallback + schema 校验 + 错误时回退到旧 env 注册表 |
| 动态 /models 请求慢或失败 | 5 秒超时 + 回退 static manifest |
| OAuth 本地 callback 端口冲突 | 尝试 8080–8090 端口范围 |
| 旧用户 `.env` 格式不兼容 | `AuthStore` 迁移层：读取旧格式并自动重写 |
| 添加 adapter registry 引入性能回退 | registry 在启动时构建一次，运行时 Arc 共享 |

---

## 8. 下一步行动

1. 由我或执行者先创建 `resources/providers.toml` 和 `src/services/api/provider_manifest.rs`（Phase 1）。
2. 合并 Phase 1 后再进入 Phase 2，避免一次改动过大。
3. Phase 3 和 Phase 4 可以并行开发，但建议 Phase 3 先合并，因为 `/connect` 不依赖 discovery。
4. 每个 Phase 完成后更新本计划文档，勾选完成项。
