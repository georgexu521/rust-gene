//! Skills 系统 - 文件驱动的声明式知识
//!
//! 类似 Claude Code 和 Hermes Agent 的 SKILL.md 架构：
//! - 每个 Skill 是一个目录，包含 SKILL.md 文件
//! - SKILL.md 支持 YAML frontmatter + Markdown 内容
//! - 自动从 skills/ 目录发现和加载
//! - 运行时通过 skill_manage 工具管理

pub mod loader;
mod parser;
mod registry;
mod tools;
mod types;

pub use tools::{SkillListTool, SkillManageTool, SkillViewTool};
pub use types::Skill;

#[cfg(test)]
mod tests {
    use super::parser::parse_skill_md;
    use super::registry::SkillRegistry;
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_parse_frontmatter() {
        let md = r#"---
name: test-skill
description: "A test skill"
version: "1.0.0"
triggers:
  - test
  - example
---

# Test Skill

This is the body content."#;

        let (meta, content) = parse_skill_md(md).unwrap();
        assert_eq!(meta.name, "test-skill");
        assert_eq!(meta.description, "A test skill");
        assert_eq!(meta.triggers, vec!["test", "example"]);
        assert!(content.contains("This is the body content."));
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let md = "# My Skill\n\nInstructions here.";
        let (meta, content) = parse_skill_md(md).unwrap();
        assert_eq!(meta.name, "my-skill");
        assert!(content.contains("Instructions here."));
    }

    #[test]
    fn test_skill_matches() {
        let md = r#"---
name: git-commit
description: "Git commit helper"
triggers:
  - commit
  - git
---

Always write meaningful commit messages."#;

        let (meta, content) = parse_skill_md(md).unwrap();
        let skill = Skill {
            meta,
            content,
            raw_content: String::new(),
            skill_dir: PathBuf::from("."),
            modified: None,
        };

        assert!(skill.matches(&["commit".to_string()]));
        assert!(skill.matches(&["git".to_string()]));
        assert!(!skill.matches(&["docker".to_string()]));
    }

    #[test]
    fn test_discover_and_load() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join("skills");
        let skill_dir = skills_dir.join("test-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: test-skill\n---\n\nBody",
        )
        .unwrap();

        let mut registry = SkillRegistry::new();
        registry.add_search_path(skills_dir);
        let loaded = registry.discover_and_load();
        assert_eq!(loaded, 1);
        assert!(registry.get("test-skill").is_some());
    }

    #[test]
    fn test_skill_injection() {
        let md = "---\nname: helper\n---\n\nDo X then Y.";
        let (meta, content) = parse_skill_md(md).unwrap();
        let skill = Skill {
            meta,
            content,
            raw_content: String::new(),
            skill_dir: PathBuf::from("."),
            modified: None,
        };

        let injection = skill.to_injection();
        assert!(injection.contains("# Skill: helper"));
        assert!(injection.contains("Do X then Y."));
    }
}
