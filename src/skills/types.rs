//! Skill 类型定义

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Skill 元数据（从 frontmatter 解析）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta {
    /// Skill 名称
    pub name: String,
    /// 描述（用于 agent 决定何时加载）
    #[serde(default)]
    pub description: String,
    /// 版本
    #[serde(default = "default_version")]
    pub version: String,
    /// 作者
    #[serde(default)]
    pub author: String,
    /// 触发条件（关键词列表，agent 可据此决定是否加载）
    #[serde(default)]
    pub triggers: Vec<String>,
    /// 需要的环境变量
    #[serde(default)]
    pub required_env: Vec<String>,
    /// Tool allow-list scoped to this skill, Claude-style `allowed-tools`.
    #[serde(default, alias = "allowed-tools")]
    pub allowed_tools: Vec<String>,
    /// Preferred model for this skill, if any.
    #[serde(default)]
    pub model: Option<String>,
    /// Preferred reasoning effort for this skill, if any.
    #[serde(default)]
    pub effort: Option<String>,
    /// Context behavior hint, e.g. inherit or fork.
    #[serde(default)]
    pub context: Option<String>,
    /// Whether users can invoke this skill directly as `/skill-name`.
    #[serde(default = "default_user_invocable", alias = "user-invocable")]
    pub user_invocable: bool,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

fn default_user_invocable() -> bool {
    true
}

impl Default for SkillMeta {
    fn default() -> Self {
        Self {
            name: "unnamed".to_string(),
            description: String::new(),
            version: default_version(),
            author: String::new(),
            triggers: Vec::new(),
            required_env: Vec::new(),
            allowed_tools: Vec::new(),
            model: None,
            effort: None,
            context: None,
            user_invocable: true,
        }
    }
}

/// 加载后的 Skill
#[derive(Debug, Clone, Serialize)]
pub struct Skill {
    /// 元数据
    pub meta: SkillMeta,
    /// 指令内容（frontmatter 之后的 Markdown）
    pub content: String,
    /// 原始完整内容
    pub raw_content: String,
    /// SKILL.md 所在目录
    pub skill_dir: PathBuf,
    /// 最后修改时间
    pub modified: Option<std::time::SystemTime>,
}

impl Skill {
    /// 检查 skill 是否匹配给定关键词
    pub fn matches(&self, keywords: &[String]) -> bool {
        if keywords.is_empty() {
            return false;
        }
        let lower_name = self.meta.name.to_lowercase();
        let lower_desc = self.meta.description.to_lowercase();
        let lower_content = self.content.to_lowercase();

        keywords.iter().any(|kw| {
            let kw_lower = kw.to_lowercase();
            lower_name.contains(&kw_lower)
                || lower_desc.contains(&kw_lower)
                || self
                    .meta
                    .triggers
                    .iter()
                    .any(|t| t.to_lowercase().contains(&kw_lower))
                || lower_content.contains(&kw_lower)
        })
    }

    /// 获取注入到 system prompt 的格式
    pub fn to_injection(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# Skill: {}\n\n", self.meta.name));
        if !self.meta.description.is_empty() {
            out.push_str(&format!("{}\n\n", self.meta.description));
        }
        out.push_str(&self.content);
        out.push('\n');
        out
    }
}
