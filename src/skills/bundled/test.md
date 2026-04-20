# Test - 测试助手

帮助你编写、运行和维护测试。

## 使用场景

当你需要处理以下任务时使用此 Skill：
- 编写单元测试
- 编写集成测试
- 运行测试套件
- 调试测试失败
- 提高测试覆盖率
- 优化测试性能

## 测试类型

### 1. 单元测试
- 测试单个函数或模块
- 快速执行
- 隔离依赖

### 2. 集成测试
- 测试多个模块协作
- 测试外部依赖
- 端到端测试

### 3. 性能测试
- 基准测试
- 负载测试
- 压力测试

## Rust 测试最佳实践

### 单元测试结构
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name() {
        // 准备
        let input = "test";
        
        // 执行
        let result = function_under_test(input);
        
        // 断言
        assert_eq!(result, expected);
    }
}
```

### 异步测试
```rust
#[tokio::test]
async fn test_async_function() {
    let result = async_function().await;
    assert!(result.is_ok());
}
```

### 参数化测试
```rust
#[test]
fn test_various_inputs() {
    let test_cases = vec![
        ("input1", "expected1"),
        ("input2", "expected2"),
    ];
    
    for (input, expected) in test_cases {
        assert_eq!(function(input), expected);
    }
}
```

### 错误测试
```rust
#[test]
fn test_error_case() {
    let result = function_that_might_fail();
    assert!(result.is_err());
    
    if let Err(e) = result {
        assert_eq!(e.kind(), ErrorKind::InvalidInput);
    }
}
```

## 测试工具

### assert 宏
```rust
assert!(condition);
assert_eq!(left, right);
assert_ne!(left, right);
```

### 自定义断言
```rust
macro_rules! assert_close {
    ($left:expr, $right:expr, $tolerance:expr) => {
        let diff = ($left - $right).abs();
        assert!(
            diff <= $tolerance,
            "Values not within tolerance: {} vs {}",
            $left,
            $right
        );
    };
}
```

### Mock 对象
```rust
#[cfg(test)]
mod mocks {
    use super::*;
    
    pub struct MockDatabase;
    
    impl Database for MockDatabase {
        fn get(&self, key: &str) -> Option<String> {
            Some("mock_value".to_string())
        }
    }
}
```

## 测试覆盖率

### 使用 cargo-tarpaulin
```bash
# 安装
cargo install cargo-tarpaulin

# 运行覆盖率测试
cargo tarpaulin --out Html

# 查看报告
open tarpaulin-report.html
```

### 使用 grcov
```bash
# 设置环境变量
export CARGO_INCREMENTAL=0
export RUSTFLAGS="-Zprofile -Ccodegen-units=1 -Copt-level=0 -Clink-dead-code -Coverflow-checks=off -Zpanic_abort_tests"

# 编译和测试
cargo test

# 生成报告
grcov . -s . --binary-path ./target/debug/ -t html --branch --ignore-not-existing -o ./coverage/
```

## 测试组织

### 目录结构
```
src/
├── lib.rs
├── module1/
│   ├── mod.rs
│   └── tests.rs  # 单元测试
tests/
├── integration_test.rs  # 集成测试
└── common/
    └── mod.rs  # 测试工具
```

### 测试配置
```toml
# Cargo.toml
[dev-dependencies]
tokio-test = "0.4"
mockall = "0.11"

[profile.test]
opt-level = 0
debug = true
```

## 测试模式

### Arrange-Act-Assert
```rust
#[test]
fn test_pattern() {
    // Arrange - 准备测试数据
    let input = create_test_input();
    
    // Act - 执行被测试的代码
    let result = function_under_test(input);
    
    // Assert - 验证结果
    assert_eq!(result, expected);
}
```

### Given-When-Then
```rust
#[test]
fn test_bdd_style() {
    // Given - 给定某个上下文
    let context = setup_context();
    
    // When - 当执行某个操作
    let result = perform_action(context);
    
    // Then - 那么应该得到某个结果
    assert_result(result);
}
```

## 性能测试

### 基准测试
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_function(c: &mut Criterion) {
    c.bench_function("function_name", |b| {
        b.iter(|| {
            black_box(function_under_test(black_box(input)))
        })
    });
}

criterion_group!(benches, benchmark_function);
criterion_main!(benches);
```

### 计时测试
```rust
#[test]
fn test_performance() {
    use std::time::Instant;
    
    let start = Instant::now();
    for _ in 0..1000 {
        function_under_test();
    }
    let duration = start.elapsed();
    
    assert!(
        duration.as_millis() < 100,
        "Function too slow: {:?}",
        duration
    );
}
```

## 调试测试失败

### 添加调试输出
```rust
#[test]
fn test_with_debug() {
    let result = function_under_test();
    println!("Result: {:?}", result);  // 只在测试失败时显示
    assert!(result.is_ok());
}
```

### 运行特定测试
```bash
# 运行单个测试
cargo test test_name

# 运行模块中的测试
cargo test module_name::

# 显示输出
cargo test -- --nocapture
```

## 输出格式

测试建议应该包含：
1. 测试场景描述
2. 测试代码示例
3. 预期结果
4. 边界条件
5. 错误情况
