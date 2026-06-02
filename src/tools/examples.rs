//! 工具系统使用示例
//!
//! 展示如何使用工具系统

use crate::tools::*;

/// 示例：使用工具注册表执行工具
#[allow(dead_code)]
pub async fn example_tool_usage() {
    // 创建工具注册表
    let registry = ToolRegistry::default_registry();

    // 创建工具上下文
    let context = ToolContext::new(".", "example-session");

    // 示例 1: 读取文件
    println!("=== Example 1: File Read ===");
    let file_params = serde_json::json!({
        "path": "Cargo.toml"
    });

    if let Some(tool) = registry.get("file_read") {
        let result = tool.execute(file_params, context.clone()).await;
        println!("Success: {}", result.success);
        println!(
            "Content preview: {}...",
            &result.content[..100.min(result.content.len())]
        );
    }

    // 示例 2: 执行 bash 命令
    println!("\n=== Example 2: Bash ===");
    let bash_params = serde_json::json!({
        "command": "ls -la",
        "description": "List files"
    });

    if let Some(tool) = registry.get("bash") {
        let result = tool.execute(bash_params, context.clone()).await;
        println!("Success: {}", result.success);
        println!("Output: {}", result.content);
    }

    // 示例 3: Glob 搜索
    println!("\n=== Example 3: Glob ===");
    let glob_params = serde_json::json!({
        "pattern": "src/**/*.rs"
    });

    if let Some(tool) = registry.get("glob") {
        let result = tool.execute(glob_params, context.clone()).await;
        println!("Success: {}", result.success);
        println!("Files found: {}", result.content.lines().count());
    }

    // 示例 4: Grep 搜索
    println!("\n=== Example 4: Grep ===");
    let grep_params = serde_json::json!({
        "pattern": "fn main",
        "path": "src",
        "include": "*.rs"
    });

    if let Some(tool) = registry.get("grep") {
        let result = tool.execute(grep_params, context.clone()).await;
        println!("Success: {}", result.success);
        println!("Matches: {}", result.content);
    }
}

/// 示例：转换为 OpenAI 工具格式
#[allow(dead_code)]
pub fn example_openai_tools() {
    let registry = ToolRegistry::default_registry();
    let openai_tools = registry.to_openai_tools();

    println!("Available tools for LLM:");
    for tool in &openai_tools {
        println!("  - {}", tool.function.name);
    }
}

/// 示例：带 LLM 的工具调用循环
#[allow(dead_code)]
pub async fn example_tool_loop() {
    // TODO: Phase 3 实现完整的 LLM 工具调用循环
    // 这将展示如何：
    // 1. 发送用户消息给 LLM
    // 2. 接收 LLM 的工具调用请求
    // 3. 执行工具
    // 4. 返回结果给 LLM
    // 5. 获取最终响应
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_default_registry() {
        let registry = ToolRegistry::default_registry();

        assert!(registry.has("file_read"));
        assert!(registry.has("file_write"));
        assert!(registry.has("file_edit"));
        assert!(registry.has("bash"));
        assert!(registry.has("glob"));
        assert!(registry.has("grep"));
        assert!(registry.has("todo_write"));
        assert!(registry.has("agent"));
    }

    #[tokio::test]
    async fn test_openai_tools_conversion() {
        let registry = ToolRegistry::default_registry();
        let tools = registry.to_openai_tools();

        assert!(!tools.is_empty());
        assert!(tools.iter().any(|t| t.function.name == "bash"));
        assert!(tools.iter().any(|t| t.function.name == "file_read"));
    }
}
