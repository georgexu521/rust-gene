//! Skill 工具实现
//!
//! SkillManageTool, SkillListTool, SkillViewTool

use std::path::{Path, PathBuf};

/// Skill 管理工具 - 让 agent 管理 skills
pub struct SkillManageTool {
    skills_dir: PathBuf,
}

impl SkillManageTool {
    pub fn new(skills_dir: PathBuf) -> Self {
        Self { skills_dir }
    }
}

#[async_trait::async_trait]
impl crate::tools::Tool for SkillManageTool {
    fn name(&self) -> &str {
        "skill_manage"
    }

    fn description(&self) -> &str {
        "Manage skills: list, view, create, patch, or delete SKILL.md files. \
         Skills are declarative knowledge that the agent can load on demand."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "view", "create", "patch", "delete", "reload"],
                    "description": "list: show all skills. view: read a skill. \
                                   create: write new SKILL.md. patch: update skill content. \
                                   delete: remove a skill. reload: rescan skills dir."
                },
                "name": {
                    "type": "string",
                    "description": "Skill name (for view/create/patch/delete)"
                },
                "content": {
                    "type": "string",
                    "description": "Full SKILL.md content (for create) or patch content"
                },
                "description": {
                    "type": "string",
                    "description": "Skill description (for create, in frontmatter)"
                },
                "triggers": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Trigger keywords (for create, in frontmatter)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        _context: crate::tools::ToolContext,
    ) -> crate::tools::ToolResult {
        let action = params["action"].as_str().unwrap_or("list");
        let name = params["name"].as_str().unwrap_or("");

        match action {
            "list" => {
                let entries = match std::fs::read_dir(&self.skills_dir) {
                    Ok(e) => e,
                    Err(_) => {
                        return crate::tools::ToolResult::success(format!(
                            "Skills directory does not exist: {}. Create it to add skills.",
                            self.skills_dir.display()
                        ));
                    }
                };

                let mut names = Vec::new();
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        let skill_md = entry.path().join("SKILL.md");
                        if skill_md.is_file() {
                            names.push(entry.file_name().to_string_lossy().to_string());
                        }
                    }
                }

                if names.is_empty() {
                    crate::tools::ToolResult::success(
                        "No skills found. Use action='create' to add skills.".to_string(),
                    )
                } else {
                    names.sort();
                    crate::tools::ToolResult::success(format!(
                        "Skills ({}):\n{}",
                        names.len(),
                        names
                            .iter()
                            .map(|n| format!("  - {}", n))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ))
                }
            }

            "view" => {
                if name.is_empty() {
                    return crate::tools::ToolResult::error("Skill name required for 'view'");
                }
                let skill_md = self.skills_dir.join(name).join("SKILL.md");
                match std::fs::read_to_string(&skill_md) {
                    Ok(content) => crate::tools::ToolResult::success(content),
                    Err(e) => crate::tools::ToolResult::error(format!(
                        "Cannot read skill '{}': {}",
                        name, e
                    )),
                }
            }

            "create" => {
                if name.is_empty() {
                    return crate::tools::ToolResult::error("Skill name required for 'create'");
                }
                let skill_dir = self.skills_dir.join(name);
                let skill_md = skill_dir.join("SKILL.md");

                if skill_md.exists() {
                    return crate::tools::ToolResult::error(format!(
                        "Skill '{}' already exists. Use 'patch' to update.",
                        name
                    ));
                }

                // 构建 SKILL.md 内容
                let desc = params["description"].as_str().unwrap_or("");
                let triggers: Vec<String> = params["triggers"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let body = params["content"]
                    .as_str()
                    .unwrap_or("# TODO\n\nAdd instructions here.");

                let mut md = String::new();
                md.push_str("---\n");
                md.push_str(&format!("name: {}\n", name));
                if !desc.is_empty() {
                    md.push_str(&format!("description: \"{}\"\n", desc));
                }
                md.push_str("version: \"1.0.0\"\n");
                if !triggers.is_empty() {
                    md.push_str("triggers:\n");
                    for t in &triggers {
                        md.push_str(&format!("  - {}\n", t));
                    }
                }
                md.push_str("---\n\n");
                md.push_str(body);

                if let Err(e) = std::fs::create_dir_all(&skill_dir) {
                    return crate::tools::ToolResult::error(format!(
                        "Cannot create skill dir: {}",
                        e
                    ));
                }
                match std::fs::write(&skill_md, &md) {
                    Ok(_) => crate::tools::ToolResult::success(format!(
                        "Created skill '{}' at {}",
                        name,
                        skill_md.display()
                    )),
                    Err(e) => crate::tools::ToolResult::error(format!("Cannot write skill: {}", e)),
                }
            }

            "patch" => {
                if name.is_empty() {
                    return crate::tools::ToolResult::error("Skill name required for 'patch'");
                }
                let new_content = params["content"].as_str().unwrap_or("");
                if new_content.is_empty() {
                    return crate::tools::ToolResult::error("Content required for 'patch'");
                }
                let skill_md = self.skills_dir.join(name).join("SKILL.md");
                if !skill_md.exists() {
                    return crate::tools::ToolResult::error(format!(
                        "Skill '{}' not found. Use 'create' first.",
                        name
                    ));
                }

                match std::fs::write(&skill_md, new_content) {
                    Ok(_) => crate::tools::ToolResult::success(format!("Patched skill '{}'", name)),
                    Err(e) => crate::tools::ToolResult::error(format!("Cannot patch skill: {}", e)),
                }
            }

            "delete" => {
                if name.is_empty() {
                    return crate::tools::ToolResult::error("Skill name required for 'delete'");
                }
                let skill_dir = self.skills_dir.join(name);
                match std::fs::remove_dir_all(&skill_dir) {
                    Ok(_) => crate::tools::ToolResult::success(format!("Deleted skill '{}'", name)),
                    Err(e) => crate::tools::ToolResult::error(format!(
                        "Cannot delete skill '{}': {}",
                        name, e
                    )),
                }
            }

            "reload" => crate::tools::ToolResult::success(
                "Use the main agent reload mechanism to rescan skills.".to_string(),
            ),

            _ => crate::tools::ToolResult::error(format!(
                "Unknown action: {}. Use list, view, create, patch, delete, reload",
                action
            )),
        }
    }
}

/// Skill 列表工具 - 让 agent 浏览可用的 skills
pub struct SkillListTool;

#[async_trait::async_trait]
impl crate::tools::Tool for SkillListTool {
    fn name(&self) -> &str {
        "skills_list"
    }

    fn description(&self) -> &str {
        "List all available skills and their descriptions. Use this to find skills that match your current task."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Optional search query to filter skills"
                }
            }
        })
    }

    async fn execute(
        &self,
        _params: serde_json::Value,
        _context: crate::tools::ToolContext,
    ) -> crate::tools::ToolResult {
        crate::tools::ToolResult::success(
            "Skills list requires a loaded SkillRegistry. Use skill_manage(action='list') to see skills on disk.".to_string(),
        )
    }
}

/// Skill 查看工具 - 让 agent 读取 skill 内容
pub struct SkillViewTool;

#[async_trait::async_trait]
impl crate::tools::Tool for SkillViewTool {
    fn name(&self) -> &str {
        "skill_view"
    }

    fn description(&self) -> &str {
        "View a specific skill's full content. Load skill instructions into your context."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The skill name to view"
                }
            },
            "required": ["name"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        context: crate::tools::ToolContext,
    ) -> crate::tools::ToolResult {
        let name = params["name"].as_str().unwrap_or("");
        if name.is_empty() {
            return crate::tools::ToolResult::error("Skill name required");
        }

        // 从 metadata 获取 skills_dir
        let skills_dir = context
            .metadata
            .get("skills_dir")
            .map(|s| s.as_str())
            .unwrap_or("skills");

        let skill_md = Path::new(skills_dir).join(name).join("SKILL.md");
        match std::fs::read_to_string(&skill_md) {
            Ok(content) => crate::tools::ToolResult::success(content),
            Err(e) => {
                crate::tools::ToolResult::error(format!("Cannot read skill '{}': {}", name, e))
            }
        }
    }
}
