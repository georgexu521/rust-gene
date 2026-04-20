# Debug - 调试助手

帮助你系统地调试代码问题。

## 使用场景

当你遇到以下情况时使用此 Skill：
- 程序崩溃或 panic
- 测试失败
- 逻辑错误导致结果不正确
- 性能问题
- 内存泄漏
- 并发问题

## 调试步骤

### 1. 理解问题
- 错误信息是什么？
- 什么时候发生的？
- 能稳定复现吗？
- 最近改了什么？

### 2. 收集信息
- 检查日志和错误输出
- 查看堆栈跟踪
- 添加调试打印
- 使用断点调试

### 3. 缩小范围
- 二分法定位问题
- 隔离可疑代码
- 最小化复现案例

### 4. 分析根因
- 检查变量值
- 验证假设
- 查看代码路径

### 5. 修复验证
- 实施修复
- 编写测试
- 验证修复有效

## Rust 调试技巧

### 使用 dbg! 宏
```rust
let result = dbg!(some_function(x));
```

### 使用 tracing
```rust
use tracing::{debug, info, warn, error};

debug!(value = ?x, "Processing value");
info!("Operation completed");
warn!("Unexpected condition");
error!(error = %e, "Operation failed");
```

### 条件编译调试代码
```rust
#[cfg(debug_assertions)]
fn debug_only_function() {
    // 只在 debug 模式编译
}
```

### 使用 RUST_BACKTRACE
```bash
RUST_BACKTRACE=1 cargo run
RUST_BACKTRACE=full cargo run  # 完整堆栈
```

### 内存调试
```bash
# Valgrind (Linux)
valgrind --leak-check=full ./target/debug/your_app

# AddressSanitizer
RUSTFLAGS="-Z sanitizer=address" cargo run
```

## 常见问题模式

### 所有权问题
- 检查 move/borrow
- 使用 `.clone()` 临时解决
- 考虑使用 `Rc`/`Arc`

### 生命周期问题
- 检查引用有效期
- 使用 `'static` 或 owned types
- 考虑使用 `Cow`

### 并发问题
- 检查锁的顺序
- 使用 `Arc<Mutex<T>>`
- 考虑使用 channel

### 类型问题
- 检查类型推断
- 显式标注类型
- 使用 `as` 转换或 `From`/`Into`

## 调试工具

### 基础工具
- `println!` / `dbg!` - 快速调试
- `tracing` - 结构化日志
- `RUST_LOG` - 控制日志级别

### 高级工具
- `gdb` / `lldb` - 断点调试
- `cargo flamegraph` - 性能分析
- `miri` - 未定义行为检测

## 输出格式

调试结果应该包含：
1. 问题描述
2. 复现步骤
3. 根因分析
4. 修复建议
5. 验证方法
