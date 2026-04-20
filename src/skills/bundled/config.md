# Config - 配置管理

帮助你管理项目和应用的配置。

## 使用场景

当你需要处理以下任务时使用此 Skill：
- 添加新的配置选项
- 读取配置文件
- 修改配置结构
- 环境变量管理
- 配置验证
- 默认值处理

## 配置类型

### 1. 配置文件
- TOML (推荐用于 Rust)
- YAML
- JSON
- INI

### 2. 环境变量
- `.env` 文件
- 系统环境变量
- 命令行参数

### 3. 运行时配置
- 动态配置
- 热重载
- 配置中心

## Rust 配置最佳实践

### 使用 serde 序列化
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct Config {
    database: DatabaseConfig,
    server: ServerConfig,
    logging: LoggingConfig,
}

#[derive(Debug, Deserialize, Serialize)]
struct DatabaseConfig {
    url: String,
    max_connections: u32,
    timeout_seconds: u64,
}
```

### 使用 config crate
```rust
use config::{Config, ConfigError, File};

fn load_config() -> Result<Config, ConfigError> {
    Config::builder()
        .add_source(File::with_name("config/default"))
        .add_source(File::with_name("config/local").required(false))
        .build()
}
```

### 环境变量覆盖
```rust
use std::env;

fn get_database_url() -> String {
    env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/mydb".to_string())
}
```

### 配置验证
```rust
impl Config {
    fn validate(&self) -> Result<(), String> {
        if self.server.port == 0 {
            return Err("Server port cannot be 0".to_string());
        }
        if self.database.max_connections == 0 {
            return Err("Max connections must be > 0".to_string());
        }
        Ok(())
    }
}
```

## 配置结构示例

```toml
# config/default.toml
[server]
host = "0.0.0.0"
port = 8080
workers = 4

[database]
url = "postgres://localhost/mydb"
max_connections = 10
timeout_seconds = 30

[logging]
level = "info"
file = "app.log"
```

## 环境变量模式

### 前缀模式
```rust
const PREFIX: &str = "MY_APP_";

fn load_from_env() -> Config {
    Config {
        database_url: env::var(format!("{}DATABASE_URL", PREFIX))
            .expect("DATABASE_URL required"),
        port: env::var(format!("{}PORT", PREFIX))
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .expect("PORT must be a number"),
    }
}
```

### 嵌套变量
```bash
MY_APP_SERVER_HOST=0.0.0.0
MY_APP_SERVER_PORT=8080
MY_APP_DATABASE_URL=postgres://...
```

## 配置热重载

```rust
use notify::{Watcher, RecursiveMode, watcher};
use std::sync::mpsc::channel;
use std::time::Duration;

fn watch_config(path: &str, reload: impl Fn()) {
    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_secs(1)).unwrap();
    watcher.watch(path, RecursiveMode::NonRecursive).unwrap();

    loop {
        match rx.recv() {
            Ok(_) => reload(),
            Err(e) => eprintln!("Watch error: {:?}", e),
        }
    }
}
```

## 配置优先级

从高到低：
1. 命令行参数
2. 环境变量
3. 本地配置文件 (config/local.toml)
4. 环境配置文件 (config/{env}.toml)
5. 默认配置文件 (config/default.toml)
6. 代码中的默认值

## 安全考虑

- 不要在配置中存储敏感信息
- 使用环境变量存储密码/密钥
- 配置文件不要提交到版本控制
- 使用 `.gitignore` 忽略敏感配置
- 考虑使用 vault 或 secrets manager

## 输出格式

配置管理结果应该包含：
1. 配置结构定义
2. 加载逻辑
3. 验证规则
4. 使用示例
5. 环境变量映射
