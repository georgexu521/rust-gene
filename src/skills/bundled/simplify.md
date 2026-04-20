# Simplify - 代码简化

帮助你简化和优化代码，提高可读性和可维护性。

## 使用场景

当你遇到以下情况时使用此 Skill：
- 代码过于复杂
- 重复代码过多
- 函数太长
- 嵌套太深
- 性能需要优化

## 简化原则

### 1. DRY (Don't Repeat Yourself)
- 提取公共代码
- 使用函数/宏
- 避免复制粘贴

### 2. KISS (Keep It Simple, Stupid)
- 简单直接的解决方案
- 避免过度工程
- 清晰的命名

### 3. YAGNI (You Aren't Gonna Need It)
- 只实现需要的功能
- 避免过度设计
- 延迟决策

## Rust 简化技巧

### 使用迭代器
```rust
// 复杂的方式
let mut result = Vec::new();
for i in 0..10 {
    if i % 2 == 0 {
        result.push(i * 2);
    }
}

// 简化后
let result: Vec<i32> = (0..10)
    .filter(|i| i % 2 == 0)
    .map(|i| i * 2)
    .collect();
```

### 使用 Option 和 Result
```rust
// 复杂的方式
fn get_value(key: &str) -> Option<String> {
    match map.get(key) {
        Some(value) => Some(value.clone()),
        None => None,
    }
}

// 简化后
fn get_value(key: &str) -> Option<String> {
    map.get(key).cloned()
}
```

### 使用 if let 和 match
```rust
// 复杂的方式
if option.is_some() {
    let value = option.unwrap();
    // 使用 value
}

// 简化后
if let Some(value) = option {
    // 使用 value
}
```

### 使用 #[derive]
```rust
// 复杂的方式
impl Default for Config {
    fn default() -> Self {
        Config {
            host: "localhost".to_string(),
            port: 8080,
        }
    }
}

// 简化后
#[derive(Default)]
struct Config {
    #[default = "localhost"]
    host: String,
    #[default = 8080]
    port: u32,
}
```

## 重构模式

### 提取函数
```rust
// 重构前
fn process_data(data: &[i32]) -> i32 {
    let sum: i32 = data.iter().sum();
    let count = data.len();
    let average = sum as f64 / count as f64;
    average.round() as i32
}

// 重构后
fn calculate_average(data: &[i32]) -> f64 {
    let sum: i32 = data.iter().sum();
    sum as f64 / data.len() as f64
}

fn process_data(data: &[i32]) -> i32 {
    calculate_average(data).round() as i32
}
```

### 使用类型别名
```rust
// 复杂的方式
fn process(data: Vec<HashMap<String, Vec<(String, i32)>>>) -> Result<(), Box<dyn Error>> {
    // ...
}

// 简化后
type DataMap = HashMap<String, Vec<(String, i32)>>;
type ProcessResult = Result<(), Box<dyn Error>>;

fn process(data: Vec<DataMap>) -> ProcessResult {
    // ...
}
```

### 使用 Builder 模式
```rust
// 复杂的方式
let config = Config {
    host: "localhost".to_string(),
    port: 8080,
    timeout: 30,
    retries: 3,
    // ... 很多字段
};

// 简化后
let config = Config::builder()
    .host("localhost")
    .port(8080)
    .timeout(30)
    .retries(3)
    .build();
```

## 性能优化

### 避免不必要的分配
```rust
// 低效
fn get_name() -> String {
    "John".to_string()
}

// 高效
fn get_name() -> &'static str {
    "John"
}
```

### 使用 Cow (Clone on Write)
```rust
use std::borrow::Cow;

fn process(input: Cow<str>) -> Cow<str> {
    if input.contains("error") {
        Cow::Owned(input.to_uppercase())
    } else {
        input
    }
}
```

### 预分配容量
```rust
// 低效
let mut vec = Vec::new();
for i in 0..1000 {
    vec.push(i);
}

// 高效
let mut vec = Vec::with_capacity(1000);
for i in 0..1000 {
    vec.push(i);
}
```

## 代码审查清单

- [ ] 函数是否太长 (>50 行)
- [ ] 是否有重复代码
- [ ] 命名是否清晰
- [ ] 是否有过度嵌套
- [ ] 错误处理是否一致
- [ ] 是否有魔法数字
- [ ] 注释是否必要
- [ ] 测试是否充分

## 输出格式

简化建议应该包含：
1. 原始代码
2. 简化后的代码
3. 改进说明
4. 性能影响
5. 可读性改进
