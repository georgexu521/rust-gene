//! Skill 注册表
//!
//! 文件驱动的 Skill 发现和管理

use super::parser::parse_skill_md;
use super::types::{Skill, SkillLoadMetadata};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
struct SkillSearchPath {
    path: PathBuf,
    load_metadata: SkillLoadMetadata,
}

/// Skill 注册表 - 文件驱动
pub struct SkillRegistry {
    /// 已加载的 skills（name -> Skill）
    skills: HashMap<String, Skill>,
    /// skills 搜索路径
    search_paths: Vec<SkillSearchPath>,
    /// 远程 skill URL 列表
    remote_urls: Vec<String>,
}

#[allow(dead_code)]
impl SkillRegistry {
    /// 创建新的 Skill 注册表
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
            search_paths: Vec::new(),
            remote_urls: Vec::new(),
        }
    }

    /// 添加搜索路径
    pub fn add_search_path(&mut self, path: PathBuf) {
        self.add_search_path_with_metadata(
            path,
            SkillLoadMetadata::user_configured("explicit skill search path"),
        );
    }

    pub fn add_search_path_with_metadata(
        &mut self,
        path: PathBuf,
        load_metadata: SkillLoadMetadata,
    ) {
        if self.search_paths.iter().any(|entry| entry.path == path) {
            return;
        }
        self.search_paths.push(SkillSearchPath {
            path,
            load_metadata,
        });
    }

    /// 设置默认搜索路径（workspace roots + 用户 ~/.priority-agent/skills/ + env paths）
    pub fn with_default_paths(mut self, project_root: &Path) -> Self {
        for path in super::loader::discover_workspace_skill_roots(project_root) {
            let metadata = if path.ends_with(Path::new(".agents/skills")) {
                SkillLoadMetadata::workspace_agents(format!(
                    "workspace .agents/skills root {}",
                    path.display()
                ))
            } else {
                SkillLoadMetadata::workspace(format!("workspace skills root {}", path.display()))
            };
            self.add_search_path_with_metadata(path, metadata);
        }

        for path in super::loader::get_user_skill_paths() {
            if path.is_dir() {
                info!(
                    "Adding user-configured skill search path: {}",
                    path.display()
                );
            }
            self.add_search_path_with_metadata(
                path.clone(),
                SkillLoadMetadata::user_configured(format!(
                    "user-configured skill root {}",
                    path.display()
                )),
            );
        }

        // 支持 PRIORITY_AGENT_SKILLS_URL 环境变量（逗号、分号或空白分隔多 URL）
        for url in super::loader::get_remote_skill_urls() {
            if !url.is_empty() {
                info!(
                    "Adding remote skill URL from PRIORITY_AGENT_SKILLS_URL: {}",
                    url
                );
                self.remote_urls.push(url);
            }
        }

        self
    }

    /// 扫描所有搜索路径，加载 SKILL.md 文件
    pub fn discover_and_load(&mut self) -> usize {
        let paths = self.search_paths.clone();
        let mut loaded = 0;

        for search_path in &paths {
            if !search_path.path.is_dir() {
                tracing::debug!(
                    target: "skills.load",
                    event = "skill_root_skipped",
                    path = %search_path.path.display(),
                    reason = "not a directory",
                    "Skill root skipped"
                );
                continue;
            }

            match std::fs::read_dir(&search_path.path) {
                Ok(entries) => {
                    let mut entries = entries.flatten().collect::<Vec<_>>();
                    entries.sort_by_key(|entry| entry.path());
                    let allowlist = super::loader::get_skill_allowlist();
                    for entry in entries {
                        let skill_dir = entry.path();
                        if skill_dir.is_dir() {
                            let skill_md = skill_dir.join("SKILL.md");
                            if skill_md.is_file() {
                                tracing::debug!(
                                    target: "skills.load",
                                    event = "skill_considered",
                                    source = %search_path.load_metadata.source.label(),
                                    trust = %search_path.load_metadata.trust.label(),
                                    path = %skill_md.display(),
                                    "Considering skill"
                                );
                                match Self::load_skill_file(
                                    &skill_md,
                                    search_path.load_metadata.clone(),
                                    allowlist.as_ref(),
                                ) {
                                    Ok(skill) => {
                                        let skill_name = skill.meta.name.clone();
                                        if self.skills.contains_key(&skill_name) {
                                            tracing::info!(
                                                target: "skills.load",
                                                event = "skill_skipped",
                                                skill = %skill_name,
                                                path = %skill_md.display(),
                                                reason = "lower-precedence duplicate",
                                                "Skill skipped"
                                            );
                                            continue;
                                        }
                                        info!(
                                            target: "skills.load",
                                            event = "skill_loaded",
                                            skill = %skill_name,
                                            source = %skill.load_metadata.source.label(),
                                            trust = %skill.load_metadata.trust.label(),
                                            path = %skill_md.display(),
                                            "Loaded skill"
                                        );
                                        self.skills.insert(skill_name, skill);
                                        loaded += 1;
                                    }
                                    Err(e) => {
                                        warn!(
                                            target: "skills.load",
                                            event = "skill_rejected",
                                            path = %skill_md.display(),
                                            reason = %e,
                                            "Failed to load skill"
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        "Cannot read skills dir {}: {}",
                        search_path.path.display(),
                        e
                    );
                }
            }
        }

        loaded
    }

    /// 从 SKILL.md 文件加载单个 skill
    fn load_skill_file(
        path: &Path,
        load_metadata: SkillLoadMetadata,
        allowlist: Option<&std::collections::HashSet<String>>,
    ) -> anyhow::Result<Skill> {
        let raw_content = std::fs::read_to_string(path)?;
        let scan = super::loader::scan_third_party_skill(&raw_content);
        if !scan.allowed {
            anyhow::bail!("{}", scan.reason);
        }
        let (meta, content) = parse_skill_md(&raw_content)?;

        let skill_dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let dir_name = skill_dir.file_name().and_then(|name| name.to_str());
        if !super::loader::skill_allowed_by_allowlist(&meta.name, dir_name, allowlist) {
            anyhow::bail!(
                "skill '{}' skipped because it is not in PRIORITY_AGENT_SKILL_ALLOWLIST",
                meta.name
            );
        }

        let modified = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());

        Ok(Skill {
            meta,
            content,
            raw_content,
            skill_dir,
            modified,
            load_metadata,
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
        let mut count = 0;
        for skill in skills {
            let skill_name = skill.meta.name.clone();
            if self.skills.contains_key(&skill_name) {
                tracing::info!(
                    target: "skills.load",
                    event = "skill_skipped",
                    skill = %skill_name,
                    reason = "higher-precedence duplicate already loaded",
                    "Bundled skill skipped"
                );
                continue;
            }
            self.register(skill);
            count += 1;
        }
        count
    }

    /// 获取配置的远程 skill URLs
    pub fn get_remote_urls(&self) -> &[String] {
        &self.remote_urls
    }

    /// 添加远程 skill URL
    pub fn add_remote_url(&mut self, url: String) {
        if !self.remote_urls.contains(&url) {
            self.remote_urls.push(url);
        }
    }

    /// 异步加载远程 skills（从配置的 URL）
    pub async fn load_remote_skills(&mut self) -> usize {
        use super::loader::load_skill_from_url;

        let mut loaded = 0;
        let urls = self.remote_urls.clone();
        for url in &urls {
            match load_skill_from_url(url).await {
                Ok(skill) => {
                    info!("Loaded remote skill from URL: {}", url);
                    self.register(skill);
                    loaded += 1;
                }
                Err(e) => {
                    warn!("Failed to load remote skill from {}: {}", url, e);
                }
            }
        }
        loaded
    }

    /// 异步加载所有外部 skills（文件路径 + 远程 URL）
    pub async fn load_external_skills(&mut self) -> usize {
        use super::loader::{get_user_skill_paths, load_skill_from_url};

        let mut loaded = 0;

        // 从文件路径加载
        for path in get_user_skill_paths() {
            if path.is_dir() {
                let metadata = SkillLoadMetadata::user_configured(format!(
                    "user-configured skill root {}",
                    path.display()
                ));
                match Self::load_skills_from_dir_sync(&path, metadata).await {
                    Ok(skills) => {
                        for skill in skills {
                            self.register(skill);
                            loaded += 1;
                        }
                    }
                    Err(e) => {
                        warn!("Failed to load skills from {}: {}", path.display(), e);
                    }
                }
            }
        }

        // 从 URL 加载
        let urls = self.remote_urls.clone();
        for url in &urls {
            match load_skill_from_url(url).await {
                Ok(skill) => {
                    info!("Loaded remote skill from URL: {}", url);
                    self.register(skill);
                    loaded += 1;
                }
                Err(e) => {
                    warn!("Failed to load remote skill from {}: {}", url, e);
                }
            }
        }

        loaded
    }

    /// 同步从目录加载 skills（内部使用）
    async fn load_skills_from_dir_sync(
        dir: &PathBuf,
        load_metadata: SkillLoadMetadata,
    ) -> anyhow::Result<Vec<Skill>> {
        let mut skills = Vec::new();
        let mut entries = tokio::fs::read_dir(dir).await?;
        let allowlist = super::loader::get_skill_allowlist();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                let skill_md = path.join("SKILL.md");
                if skill_md.is_file() {
                    match Self::load_skill_file_sync(
                        &skill_md,
                        load_metadata.clone(),
                        allowlist.as_ref(),
                    )
                    .await
                    {
                        Ok(skill) => skills.push(skill),
                        Err(e) => warn!("Failed to load skill from {}: {}", skill_md.display(), e),
                    }
                }
            }
        }

        Ok(skills)
    }

    /// 同步加载单个 skill 文件
    async fn load_skill_file_sync(
        path: &Path,
        load_metadata: SkillLoadMetadata,
        allowlist: Option<&std::collections::HashSet<String>>,
    ) -> anyhow::Result<Skill> {
        use super::parser::parse_skill_md;

        let raw_content = tokio::fs::read_to_string(path).await?;
        let scan = super::loader::scan_third_party_skill(&raw_content);
        if !scan.allowed {
            anyhow::bail!("{}", scan.reason);
        }
        let (meta, content) = parse_skill_md(&raw_content)?;
        let skill_dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let dir_name = skill_dir.file_name().and_then(|name| name.to_str());
        if !super::loader::skill_allowed_by_allowlist(&meta.name, dir_name, allowlist) {
            anyhow::bail!(
                "skill '{}' skipped because it is not in PRIORITY_AGENT_SKILL_ALLOWLIST",
                meta.name
            );
        }
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
            load_metadata,
        })
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
            if phrases
                .iter()
                .any(|p| name_lower.contains(p) || desc_lower.contains(p))
            {
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
