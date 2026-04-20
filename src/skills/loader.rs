//!  bundled Skill 加载器
//!
//! 加载编译时嵌入的系统 Skill（如 /commit, /review-pr, /explain）

use super::parser::parse_skill_md;
use super::types::Skill;
use std::path::PathBuf;

/// bundled skill 定义
const BUNDLED_SKILLS: &[(&str, &str)] = &[
    ("commit", include_str!("bundled/commit.md")),
    ("review_pr", include_str!("bundled/review_pr.md")),
    ("review", include_str!("bundled/review.md")),
    (
        "security_review",
        include_str!("bundled/security_review.md"),
    ),
    ("explain", include_str!("bundled/explain.md")),
    ("fix", include_str!("bundled/fix.md")),
    ("simplify", include_str!("bundled/simplify.md")),
    ("verify", include_str!("bundled/verify.md")),
    ("debug", include_str!("bundled/debug.md")),
    ("stuck", include_str!("bundled/stuck.md")),
    ("remember", include_str!("bundled/remember.md")),
    ("keybindings", include_str!("bundled/keybindings.md")),
];

/// 加载所有 bundled skills
pub fn load_bundled_skills() -> Vec<Skill> {
    let mut skills = Vec::new();
    for (name, raw) in BUNDLED_SKILLS {
        match parse_skill_md(raw) {
            Ok((meta, content)) => {
                skills.push(Skill {
                    meta,
                    content,
                    raw_content: raw.to_string(),
                    skill_dir: PathBuf::from("."),
                    modified: None,
                });
            }
            Err(e) => {
                tracing::warn!("Failed to parse bundled skill '{}': {}", name, e);
            }
        }
    }
    skills
}

/// 加载单个 bundled skill
pub fn load_bundled_skill(name: &str) -> Option<Skill> {
    for (n, raw) in BUNDLED_SKILLS {
        if *n == name {
            return parse_skill_md(raw).ok().map(|(meta, content)| Skill {
                meta,
                content,
                raw_content: raw.to_string(),
                skill_dir: PathBuf::from("."),
                modified: None,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_all_bundled() {
        let skills = load_bundled_skills();
        assert!(skills.len() >= 6);
        let names: Vec<_> = skills.iter().map(|s| s.meta.name.clone()).collect();
        assert!(names.contains(&"commit".to_string()));
        assert!(names.contains(&"review_pr".to_string()));
        assert!(names.contains(&"review".to_string()));
        assert!(names.contains(&"security_review".to_string()));
        assert!(names.contains(&"explain".to_string()));
        assert!(names.contains(&"fix".to_string()));
    }

    #[test]
    fn test_load_single_bundled() {
        let skill = load_bundled_skill("commit").unwrap();
        assert_eq!(skill.meta.name, "commit");
        assert!(skill.content.contains("conventional commits"));
    }
}
