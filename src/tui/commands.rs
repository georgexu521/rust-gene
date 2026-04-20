//! TUI 命令注册表
//!
//! 统一管理所有 slash 命令，支持别名、分类、帮助信息。
//! 借鉴 Hermes 的 CommandDef 设计。

use std::collections::HashMap;

/// 命令定义
#[derive(Clone)]
pub struct CommandDef {
    /// 命令名称 (如 "/help")
    pub name: &'static str,
    /// 别名 (如 ["/h"])
    pub aliases: &'static [&'static str],
    /// 分类
    pub category: &'static str,
    /// 用法说明
    pub usage: &'static str,
    /// 详细描述
    pub description: &'static str,
}

impl CommandDef {
    pub const fn new(
        name: &'static str,
        aliases: &'static [&'static str],
        category: &'static str,
        usage: &'static str,
        description: &'static str,
    ) -> Self {
        Self {
            name,
            aliases,
            category,
            usage,
            description,
        }
    }
}

/// 命令注册表
pub struct CommandRegistry {
    /// 按名称索引
    commands: HashMap<String, &'static CommandDef>,
    /// 按分类分组
    categories: HashMap<&'static str, Vec<&'static CommandDef>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            categories: HashMap::new(),
        }
    }

    /// 注册一个命令
    pub fn register(&mut self, def: &'static CommandDef) {
        // 注册主名称
        self.commands.insert(def.name.to_string(), def);
        // 注册别名
        for alias in def.aliases {
            self.commands.insert(alias.to_string(), def);
        }
        // 按分类分组
        self.categories.entry(def.category).or_default().push(def);
    }

    /// 查找命令
    pub fn get(&self, name: &str) -> Option<&&CommandDef> {
        self.commands.get(name)
    }

    /// 生成帮助文本
    pub fn help_text(&self) -> String {
        let mut result = String::from("Commands:\n");

        let mut cats: Vec<_> = self.categories.keys().copied().collect();
        cats.sort();

        for cat in cats {
            result.push_str(&format!("\n  {}:\n", cat));
            if let Some(cmds) = self.categories.get(cat) {
                for cmd in cmds {
                    let alias_str = if cmd.aliases.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", cmd.aliases.join(", "))
                    };
                    result.push_str(&format!(
                        "    {:<24} {}{}\n",
                        cmd.usage, cmd.description, alias_str
                    ));
                }
            }
        }
        result
    }
}

// ═══════════════════════════════════════
// 命令定义（编译期常量）
// ═══════════════════════════════════════

pub const CMD_HELP: CommandDef =
    CommandDef::new("/help", &["/h"], "General", "/help", "Show this help");

pub const CMD_CLEAR: CommandDef = CommandDef::new(
    "/clear",
    &[],
    "General",
    "/clear",
    "Clear conversation history",
);

pub const CMD_QUIT: CommandDef = CommandDef::new(
    "/quit",
    &["/exit", "/q"],
    "General",
    "/quit",
    "Exit the application",
);

pub const CMD_MEMORY: CommandDef =
    CommandDef::new("/memory", &[], "Memory", "/memory", "Show saved memory");

pub const CMD_SAVE: CommandDef =
    CommandDef::new("/save", &[], "Memory", "/save <text>", "Save to memory");

pub const CMD_COST: CommandDef =
    CommandDef::new("/cost", &[], "Info", "/cost", "Show token usage and cost");

pub const CMD_MODEL: CommandDef =
    CommandDef::new("/model", &[], "Info", "/model", "Show current model");

pub const CMD_STATUS: CommandDef =
    CommandDef::new("/status", &[], "Info", "/status", "Show session status");

pub const CMD_TOOLS: CommandDef =
    CommandDef::new("/tools", &[], "Info", "/tools", "List available tools");

pub const CMD_TASKS: CommandDef = CommandDef::new(
    "/tasks",
    &[],
    "Info",
    "/tasks",
    "List tracked tasks and status summary",
);

pub const CMD_AGENTS: CommandDef = CommandDef::new(
    "/agents",
    &[],
    "Info",
    "/agents",
    "List active/known agents and status",
);

pub const CMD_DOCTOR: CommandDef = CommandDef::new(
    "/doctor",
    &[],
    "Info",
    "/doctor [json]",
    "Run quick environment diagnostics. Use 'json' to export report.",
);

pub const CMD_AUDIT: CommandDef = CommandDef::new(
    "/audit",
    &[],
    "Info",
    "/audit [summary|recent|export] ...",
    "Show/export tool audit snapshot",
);

pub const CMD_PERMISSIONS: CommandDef = CommandDef::new(
    "/permissions",
    &["/perm"],
    "Info",
    "/permissions [mode|rules|allow|deny|ask] ...",
    "View/update permission mode and policy rules",
);

pub const CMD_DIFF: CommandDef =
    CommandDef::new("/diff", &[], "Info", "/diff", "Show recent git changes");

pub const CMD_RESUME: CommandDef = CommandDef::new(
    "/resume",
    &[],
    "Session",
    "/resume",
    "List and resume past sessions",
);

pub const CMD_REWIND: CommandDef = CommandDef::new(
    "/rewind",
    &[],
    "Session",
    "/rewind [n|file]",
    "Rewind the last n edits or a specific file to its previous version",
);

pub const CMD_COMMIT: CommandDef = CommandDef::new(
    "/commit",
    &[],
    "Skills",
    "/commit",
    "Generate a commit message for staged changes",
);

pub const CMD_REVIEW_PR: CommandDef = CommandDef::new(
    "/review-pr",
    &[],
    "Skills",
    "/review-pr <number>",
    "Review a pull request",
);

pub const CMD_REVIEW: CommandDef = CommandDef::new(
    "/review",
    &[],
    "Skills",
    "/review",
    "Review local uncommitted changes",
);

pub const CMD_SECURITY_REVIEW: CommandDef = CommandDef::new(
    "/security-review",
    &[],
    "Skills",
    "/security-review",
    "Run a security-focused review on local changes",
);

pub const CMD_EXPLAIN: CommandDef = CommandDef::new(
    "/explain",
    &[],
    "Skills",
    "/explain [<file_or_symbol>]",
    "Explain code or concepts",
);

pub const CMD_FIX: CommandDef = CommandDef::new(
    "/fix",
    &[],
    "Skills",
    "/fix",
    "Suggest fixes for current changes",
);

pub const CMD_MCP: CommandDef = CommandDef::new(
    "/mcp",
    &[],
    "MCP",
    "/mcp [approve|revoke|list] [server_name]",
    "Manage MCP server approvals and list configured servers",
);

pub const CMD_VIM: CommandDef = CommandDef::new(
    "/vim",
    &[],
    "General",
    "/vim",
    "Toggle Vim keybindings mode",
);

/// 创建默认命令注册表
pub fn default_command_registry() -> CommandRegistry {
    let mut registry = CommandRegistry::new();
    registry.register(&CMD_HELP);
    registry.register(&CMD_CLEAR);
    registry.register(&CMD_QUIT);
    registry.register(&CMD_MEMORY);
    registry.register(&CMD_SAVE);
    registry.register(&CMD_COST);
    registry.register(&CMD_MODEL);
    registry.register(&CMD_STATUS);
    registry.register(&CMD_TOOLS);
    registry.register(&CMD_TASKS);
    registry.register(&CMD_AGENTS);
    registry.register(&CMD_DOCTOR);
    registry.register(&CMD_AUDIT);
    registry.register(&CMD_PERMISSIONS);
    registry.register(&CMD_DIFF);
    registry.register(&CMD_RESUME);
    registry.register(&CMD_REWIND);
    registry.register(&CMD_COMMIT);
    registry.register(&CMD_REVIEW_PR);
    registry.register(&CMD_REVIEW);
    registry.register(&CMD_SECURITY_REVIEW);
    registry.register(&CMD_EXPLAIN);
    registry.register(&CMD_FIX);
    registry.register(&CMD_MCP);
    registry.register(&CMD_VIM);
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_lookup() {
        let registry = default_command_registry();
        assert!(registry.get("/help").is_some());
        assert!(registry.get("/h").is_some()); // alias
        assert!(registry.get("/quit").is_some());
        assert!(registry.get("/exit").is_some()); // alias
        assert!(registry.get("/nonexistent").is_none());
    }

    #[test]
    fn test_help_text() {
        let registry = default_command_registry();
        let help = registry.help_text();
        assert!(help.contains("/help"));
        assert!(help.contains("/cost"));
        assert!(help.contains("General:"));
        assert!(help.contains("Memory:"));
    }
}
