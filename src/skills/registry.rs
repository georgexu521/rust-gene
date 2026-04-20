//! Skill 注册表
//!
//! 文件驱动的 Skill 发现和管理

use super::parser::parse_skill_md;
use super::types::Skill;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Skill 注册表 - 文件驱动
pub struct SkillRegistry {
    /// 已加载的 skills（name -> Skill）
    skills: HashMap<String, Skill>,
    /// skills 搜索路径
    search_paths: Vec<PathBuf>,
}

impl SkillRegistry {
    /// 创建新的 Skill 注册表
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
            search_paths: Vec::new(),
        }
    }

    /// 添加搜索路径
    pub fn add_search_path(&mut self, path: PathBuf) {
        if !self.search_paths.contains(&path) {
            self.search_paths.push(path);
        }
    }

    /// 设置默认搜索路径（项目 skills/ + 用户 ~/.priority-agent/skills/）
    pub fn with_default_paths(mut self, project_root: &Path) -> Self {
        // 项目内 skills 目录
        self.add_search_path(project_root.join("skills"));

        // 用户级 skills 目录
        if let Some(home) = dirs::home_dir() {
            self.add_search_path(home.join(".priority-agent").join("skills"));
        }

        self
    }

    /// 扫描所有搜索路径，加载 SKILL.md 文件
    pub fn discover_and_load(&mut self) -> usize {
        let paths = self.search_paths.clone();
        let mut loaded = 0;

        for search_path in &paths {
            if !search_path.is_dir() {
                continue;
            }

            match std::fs::read_dir(search_path) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        let skill_dir = entry.path();
                        if skill_dir.is_dir() {
                            let skill_md = skill_dir.join("SKILL.md");
                            if skill_md.is_file() {
                                match Self::load_skill_file(&skill_md) {
                                    Ok(skill) => {
                                        info!("Loaded skill: {}", skill.meta.name);
                                        self.skills.insert(skill.meta.name.clone(), skill);
                                        loaded += 1;
                                    }
                                    Err(e) => {
                                        warn!(
                                            "Failed to load skill from {}: {}",
                                            skill_md.display(),
                                            e
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Cannot read skills dir {}: {}", search_path.display(), e);
                }
            }
        }

        loaded
    }

    /// 从 SKILL.md 文件加载单个 skill
    fn load_skill_file(path: &Path) -> anyhow::Result<Skill> {
        let raw_content = std::fs::read_to_string(path)?;
        let (meta, content) = parse_skill_md(&raw_content)?;

        let skill_dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();

        let modified = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());

        Ok(Skill {
            meta,
            content,
            raw_content,
            skill_dir,
            modified,
        })
    }

    /// 注册一个 skill（程序化，不从文件加载）
    pub fn register(&mut self, skill: Skill) {
        info!("Registering skill: {}", skill.meta.name);
        self.skills.insert(skill.meta.name.clone(), skill);
    }

    /// 获取 skill
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    /// 列出所有 skill
    pub fn list(&self) -> Vec<&Skill> {
        self.skills.values().collect()
    }

    /// 搜索匹配的 skill
    pub fn search(&self, keywords: &[String]) -> Vec<&Skill> {
        self.skills
            .values()
            .filter(|s| s.matches(keywords))
            .collect()
    }

    /// 列出所有 skill 名称
    pub fn list_names(&self) -> Vec<String> {
        self.skills.keys().cloned().collect()
    }

    /// 移除 skill
    pub fn remove(&mut self, name: &str) -> bool {
        self.skills.remove(name).is_some()
    }

    /// 获取 skill 数量
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    /// 热重载：重新扫描所有路径
    pub fn reload(&mut self) -> usize {
        self.skills.clear();
        self.discover_and_load()
    }

    /// 加载 bundled skills
    pub fn load_bundled(&mut self) -> usize {
        let skills = super::loader::load_bundled_skills();
        let count = skills.len();
        for skill in skills {
            self.register(skill);
        }
        count
    }

    /// 预发现：根据用户消息关键词预取可能相关的 skills
    ///
    /// 在工具执行期间调用，将预取结果缓存到下一轮 context
    pub fn prefetch(&self, user_message: &str) -> Vec<String> {
        let keywords: Vec<String> = user_message
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .map(|w| w.to_lowercase())
            .collect();

        if keywords.is_empty() {
            return Vec::new();
        }

        // 提取名词短语（2-3词的组合）
        let phrases: Vec<String> = user_message
            .split_whitespace()
            .collect::<Vec<_>>()
            .windows(2)
            .filter_map(|w| {
                if w.iter().all(|s| s.len() > 2) {
                    Some(w.join(" ").to_lowercase())
                } else {
                    None
                }
            })
            .collect();

        let mut matched = Vec::new();
        for skill in self.skills.values() {
            let name_lower = skill.meta.name.to_lowercase();
            let desc_lower = skill.meta.description.to_lowercase();

            // 精确匹配 skill 名
            if keywords.iter().any(|k| name_lower.contains(k)) {
                matched.push(skill.meta.name.clone());
                continue;
            }

            // 描述中匹配
            if keywords.iter().any(|k| desc_lower.contains(k)) {
                matched.push(skill.meta.name.clone());
                continue;
            }

            // 短语匹配
            if phrases.iter().any(|p| name_lower.contains(p) || desc_lower.contains(p)) {
                matched.push(skill.meta.name.clone());
            }
        }

        // 去重并限制数量
        matched.sort();
        matched.dedup();
        matched.truncate(5);
        debug!("Prefetched {} relevant skills for next turn", matched.len());
        matched
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}
