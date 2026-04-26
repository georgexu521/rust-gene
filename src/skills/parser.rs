//! SKILL.md 解析器
//!
//! 解析 YAML frontmatter + Markdown 内容

use super::types::SkillMeta;

/// 解析 SKILL.md 的 YAML frontmatter + 内容
pub fn parse_skill_md(raw: &str) -> anyhow::Result<(SkillMeta, String)> {
    // 检查是否有 frontmatter (--- 开头)
    let trimmed = raw.trim();
    if !trimmed.starts_with("---") {
        // 没有 frontmatter，整个内容作为指令，使用默认元数据
        let first_line = trimmed.lines().next().unwrap_or("unnamed");
        let name = first_line
            .trim_start_matches('#')
            .trim()
            .to_lowercase()
            .replace(' ', "-");
        return Ok((
            SkillMeta {
                name,
                ..SkillMeta::default()
            },
            trimmed.to_string(),
        ));
    }

    // 找到结束的 ---
    let after_first = &trimmed[3..];
    let end_idx = match after_first.find("\n---") {
        Some(idx) => idx,
        None => {
            // 没有 closing ---，当作无 frontmatter 处理
            return Ok((SkillMeta::default(), trimmed.to_string()));
        }
    };

    let frontmatter_str = &after_first[..end_idx];
    let content = after_first[end_idx + 4..].trim().to_string();

    // YAML 解析失败时降级为默认元数据
    let meta: SkillMeta = match serde_yaml::from_str(frontmatter_str) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("SKILL.md frontmatter parse error: {}, using defaults", e);
            SkillMeta::default()
        }
    };

    Ok((meta, content))
}
