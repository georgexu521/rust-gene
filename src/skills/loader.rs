//! bundled Skill 加载器
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
    // Phase 9 Task 1: Advanced Agent Types
    ("teammate", include_str!("bundled/teammate.md")),
    ("critic", include_str!("bundled/critic.md")),
    ("assistant", include_str!("bundled/assistant.md")),
    ("remote", include_str!("bundled/remote.md")),
    ("dream", include_str!("bundled/dream.md")),
    ("custom", include_str!("bundled/custom.md")),
    ("orchestrate", include_str!("bundled/orchestrate.md")),
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

/// 从 URL 加载 skill
pub async fn load_skill_from_url(url: &str) -> anyhow::Result<Skill> {
    let response = reqwest::get(url).await?;
    let raw_content = response.text().await?;

    let (meta, content) = parse_skill_md(&raw_content)?;

    // skill 名称从 frontmatter 的 meta.name 获取

    Ok(Skill {
        meta,
        content,
        raw_content,
        skill_dir: PathBuf::from(format!("url:{}", url)),
        modified: Some(std::time::SystemTime::now()),
    })
}

/// 批量从 URL 列表加载 skills
pub async fn load_skills_from_urls(urls: &[String]) -> Vec<anyhow::Result<Skill>> {
    use futures::stream::FuturesUnordered;
    use futures::StreamExt;

    let futures: FuturesUnordered<_> = urls.iter().map(|url| load_skill_from_url(url)).collect();

    futures.collect().await
}

/// 获取配置的额外搜索路径
pub fn get_extra_skill_paths() -> Vec<PathBuf> {
    std::env::var("PRIORITY_AGENT_SKILLS_PATH")
        .ok()
        .map(|v| {
            v.split(':')
                .filter(|p| !p.is_empty())
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default()
}

/// 获取配置的远程 URL 列表
pub fn get_remote_skill_urls() -> Vec<String> {
    std::env::var("PRIORITY_AGENT_SKILLS_URL")
        .ok()
        .map(|v| {
            v.split(':')
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

/// 异步加载所有外部 skills（文件 + URL）
pub async fn load_external_skills() -> Vec<anyhow::Result<Skill>> {
    let mut results = Vec::new();

    // 从文件路径加载
    for path in get_extra_skill_paths() {
        if path.is_dir() {
            match load_skills_from_dir(&path).await {
                Ok(skills) => results.extend(skills.into_iter().map(Ok)),
                Err(e) => tracing::warn!("Failed to load skills from {}: {}", path.display(), e),
            }
        }
    }

    // 从 URL 加载
    let urls = get_remote_skill_urls();
    if !urls.is_empty() {
        let remote_skills = load_skills_from_urls(&urls).await;
        results.extend(remote_skills);
    }

    results
}

/// 从目录加载所有 skills
async fn load_skills_from_dir(dir: &PathBuf) -> anyhow::Result<Vec<Skill>> {
    let mut skills = Vec::new();

    if !dir.is_dir() {
        return Ok(skills);
    }

    let mut entries = tokio::fs::read_dir(dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_dir() {
            let skill_md = path.join("SKILL.md");
            if skill_md.is_file() {
                match load_skill_file(&skill_md).await {
                    Ok(skill) => {
                        tracing::info!("Loaded external skill: {}", skill.meta.name);
                        skills.push(skill);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load skill from {}: {}", skill_md.display(), e);
                    }
                }
            }
        }
    }

    Ok(skills)
}

/// 从文件异步加载单个 skill
async fn load_skill_file(path: &std::path::Path) -> anyhow::Result<Skill> {
    let raw_content = tokio::fs::read_to_string(path).await?;
    let (meta, content) = parse_skill_md(&raw_content)?;

    let skill_dir = path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();
    let modified = tokio::fs::metadata(path)
        .await
        .ok()
        .and_then(|m| m.modified().ok());

    Ok(Skill {
        meta,
        content,
        raw_content,
        skill_dir,
        modified,
    })
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
