//! 工具系统测试

use crate::tools::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_default_registry() {
        let registry = ToolRegistry::with_profile(ToolRegistryProfile::Core);

        assert!(registry.has("file_read"));
        assert!(registry.has("file_write"));
        assert!(registry.has("file_edit"));
        assert!(registry.has("bash"));
        assert!(registry.has("glob"));
        assert!(registry.has("grep"));
        assert!(registry.has("todo_write"));
        assert!(registry.has("agent"));
        assert!(registry.has("send_message"));

        let full = ToolRegistry::full_registry();
        assert!(full.has("agent"));
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
