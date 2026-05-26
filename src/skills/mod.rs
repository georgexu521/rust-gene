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
mod runtime;
mod tools;
mod types;

pub use runtime::{SkillInvocation, SkillRuntime};
pub use tools::{SkillListTool, SkillManageTool, SkillViewTool};
pub use types::{Skill, SkillLoadMetadata, SkillSource, SkillTrustLevel};

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
            load_metadata: SkillLoadMetadata::programmatic("test skill"),
        };

        assert!(skill.matches(&["commit".to_string()]));
        assert!(skill.matches(&["git".to_string()]));
        assert!(!skill.matches(&["docker".to_string()]));

        let evidence = skill.match_evidence(&["commit".to_string()]).unwrap();
        assert_eq!(evidence.skill, "git-commit");
        assert!(evidence.matched_fields.contains(&"name".to_string()));
        assert!(evidence.matched_fields.contains(&"trigger".to_string()));
        assert_eq!(evidence.matched_keywords, vec!["commit".to_string()]);
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
            load_metadata: SkillLoadMetadata::programmatic("test skill"),
        };

        let injection = skill.to_injection();
        assert!(injection.contains("<skill-context>"));
        assert!(injection.contains("background guidance"));
        assert!(injection.contains("not user instruction text"));
        assert!(injection.contains("# Skill: helper"));
        assert!(injection.contains("Do X then Y."));
    }

    #[test]
    fn test_skill_discovery_summary_is_compact() {
        let md = r#"---
name: helper
description: "Helps with focused local edits"
triggers:
  - edit
  - patch
---

Do X then Y. This body should only appear after skill_view or direct skill invocation."#;
        let (meta, content) = parse_skill_md(md).unwrap();
        let skill = Skill {
            meta,
            content,
            raw_content: String::new(),
            skill_dir: PathBuf::from("."),
            modified: None,
            load_metadata: SkillLoadMetadata::programmatic("test skill"),
        };

        let summary = skill.discovery_summary();
        assert!(summary.contains("helper"));
        assert!(summary.contains("Helps with focused local edits"));
        assert!(summary.contains("when task mentions edit, patch"));
        assert!(!summary.contains("Do X then Y"));
    }
}
