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

// Phase 9 Task 3: New high-value commands
pub const CMD_BTW: CommandDef = CommandDef::new(
    "/btw",
    &[],
    "General",
    "/btw <message>",
    "Add a side note without disrupting conversation",
);

pub const CMD_CONTEXT: CommandDef = CommandDef::new(
    "/context",
    &[],
    "Info",
    "/context",
    "Show current context status (session, model, tokens)",
);

pub const CMD_GIT: CommandDef = CommandDef::new(
    "/git",
    &[],
    "Git",
    "/git [status|diff|log|...]",
    "Run git commands inline",
);

pub const CMD_HISTORY: CommandDef = CommandDef::new(
    "/history",
    &[],
    "Session",
    "/history [n]",
    "Show recent message history",
);

pub const CMD_MODE: CommandDef = CommandDef::new(
    "/mode",
    &[],
    "General",
    "/mode [chat|settings|vim]",
    "Switch interaction mode",
);

pub const CMD_PACKAGE: CommandDef = CommandDef::new(
    "/package",
    &[],
    "Info",
    "/package [list|deps|outdated]",
    "Show package info and dependencies",
);

// Phase 9 Task 1: Advanced Agent Types
pub const CMD_TEAMMATE: CommandDef = CommandDef::new(
    "/teammate",
    &[],
    "Agents",
    "/teammate [domain]",
    "Start a collaborative teammate agent",
);

pub const CMD_CRITIC: CommandDef = CommandDef::new(
    "/critic",
    &[],
    "Agents",
    "/critic [scope]",
    "Start a critic agent to review code",
);

pub const CMD_ASSISTANT: CommandDef = CommandDef::new(
    "/assistant",
    &[],
    "Agents",
    "/assistant [domain:task]",
    "Start a domain-specific assistant (code_review/security/data/infrastructure/testing)",
);

pub const CMD_REMOTE: CommandDef = CommandDef::new(
    "/remote",
    &[],
    "Agents",
    "/remote [task]",
    "Start a remote agent via bridge",
);

// Phase 10 Batch 1: Session & Control Commands
pub const CMD_SESSION: CommandDef = CommandDef::new(
    "/session",
    &[],
    "Session",
    "/session [list|new|delete <id>]",
    "Manage sessions (list/create/delete)",
);

pub const CMD_UNDO: CommandDef = CommandDef::new(
    "/undo",
    &[],
    "Session",
    "/undo [n]",
    "Undo the last n edits",
);

pub const CMD_REDO: CommandDef = CommandDef::new(
    "/redo",
    &[],
    "Session",
    "/redo [n]",
    "Redo the last n undone edits",
);

pub const CMD_RETRY: CommandDef = CommandDef::new(
    "/retry",
    &[],
    "Session",
    "/retry",
    "Retry the last LLM call with the same input",
);

pub const CMD_STOP: CommandDef = CommandDef::new(
    "/stop",
    &[],
    "Session",
    "/stop",
    "Stop the current operation",
);

pub const CMD_RELOAD: CommandDef = CommandDef::new(
    "/reload",
    &[],
    "Session",
    "/reload [config|skills|tools|all]",
    "Reload configuration, skills, or tools",
);

pub const CMD_SHARE: CommandDef = CommandDef::new(
    "/share",
    &[],
    "Session",
    "/share",
    "Share current session as a transcript",
);

pub const CMD_TOKEN: CommandDef = CommandDef::new(
    "/token",
    &[],
    "Info",
    "/token",
    "Show token usage and cost breakdown",
);

pub const CMD_LSP: CommandDef = CommandDef::new(
    "/lsp",
    &[],
    "Info",
    "/lsp [check|restart|symbol <name>]",
    "LSP server status and operations",
);

pub const CMD_NPM: CommandDef = CommandDef::new(
    "/npm",
    &[],
    "Info",
    "/npm [install|update|outdated] [package]",
    "Run npm package operations",
);

// Phase 10 Batch 2: hooks, profiling, prompt, migrate, focus, pause, install, skeleton, branch, color
pub const CMD_HOOKS: CommandDef = CommandDef::new(
    "/hooks",
    &[],
    "Info",
    "/hooks",
    "Show hook configuration status",
);

pub const CMD_PROFILING: CommandDef = CommandDef::new(
    "/profiling",
    &[],
    "Info",
    "/profiling",
    "Show runtime profiling info",
);

pub const CMD_PROMPT: CommandDef = CommandDef::new(
    "/prompt",
    &[],
    "Config",
    "/prompt [show|edit <text>]",
    "Show or edit system prompt",
);

pub const CMD_MIGRATE: CommandDef = CommandDef::new(
    "/migrate",
    &[],
    "Config",
    "/migrate [up|down|status]",
    "Run database migrations",
);

pub const CMD_FOCUS: CommandDef = CommandDef::new(
    "/focus",
    &[],
    "General",
    "/focus [on|off]",
    "Toggle focus mode",
);

pub const CMD_PAUSE: CommandDef = CommandDef::new(
    "/pause",
    &[],
    "General",
    "/pause [pause|resume]",
    "Pause or resume agent",
);

pub const CMD_INSTALL: CommandDef = CommandDef::new(
    "/install",
    &[],
    "Info",
    "/install [cargo|npm|pip] [package]",
    "Install dependencies",
);

pub const CMD_SKELETON: CommandDef = CommandDef::new(
    "/skeleton",
    &[],
    "Info",
    "/skeleton <language> [filename]",
    "Generate code skeleton",
);

pub const CMD_BRANCH: CommandDef = CommandDef::new(
    "/branch",
    &[],
    "Git",
    "/branch [create <name>|current]",
    "Git branch management",
);

pub const CMD_COLOR: CommandDef = CommandDef::new(
    "/color",
    &[],
    "Config",
    "/color [dark|light|high-contrast]",
    "Change theme color",
);

// Phase 10 Batch 3: webhook, wizard, workspace, slack, stealth, shadow, reject, subscribe, slots, ticker
pub const CMD_WEBHOOK: CommandDef = CommandDef::new(
    "/webhook",
    &[],
    "Info",
    "/webhook [list|create|delete] <url>",
    "Manage webhooks",
);

pub const CMD_WIZARD: CommandDef = CommandDef::new(
    "/wizard",
    &[],
    "Config",
    "/wizard",
    "Start setup wizard",
);

pub const CMD_WORKSPACE: CommandDef = CommandDef::new(
    "/workspace",
    &[],
    "Info",
    "/workspace [list|info]",
    "Show workspace info",
);

pub const CMD_SLACK: CommandDef = CommandDef::new(
    "/slack",
    &[],
    "Info",
    "/slack [connect|disconnect|send]",
    "Slack integration",
);

pub const CMD_STEALTH: CommandDef = CommandDef::new(
    "/stealth",
    &[],
    "Config",
    "/stealth [on|off]",
    "Toggle stealth mode",
);

pub const CMD_SHADOW: CommandDef = CommandDef::new(
    "/shadow",
    &[],
    "Config",
    "/shadow [on|off]",
    "Toggle shadow mode",
);

pub const CMD_REJECT: CommandDef = CommandDef::new(
    "/reject",
    &[],
    "General",
    "/reject",
    "Reject pending approval",
);

pub const CMD_SUBSCRIBE: CommandDef = CommandDef::new(
    "/subscribe",
    &[],
    "Info",
    "/subscribe <event_type>",
    "Subscribe to events",
);

pub const CMD_SLOTS: CommandDef = CommandDef::new(
    "/slots",
    &[],
    "Config",
    "/slots [list|set <name> <value>|clear]",
    "View/edit slot variables",
);

pub const CMD_TICKER: CommandDef = CommandDef::new(
    "/ticker",
    &[],
    "General",
    "/ticker <message>",
    "Display scrolling ticker",
);

// Phase 10 Batch 4: config, copy, desktop, chrome, effort, preamble, untrap, verbose, write
pub const CMD_CONFIG: CommandDef = CommandDef::new(
    "/config",
    &[],
    "Config",
    "/config [edit|get <key>]",
    "View/edit configuration",
);

pub const CMD_COPY: CommandDef = CommandDef::new(
    "/copy",
    &[],
    "General",
    "/copy <text>",
    "Copy text to clipboard",
);

pub const CMD_DESKTOP: CommandDef = CommandDef::new(
    "/desktop",
    &[],
    "Info",
    "/desktop [open|close|notify] <target>",
    "Desktop integration",
);

pub const CMD_CHROME: CommandDef = CommandDef::new(
    "/chrome",
    &[],
    "Info",
    "/chrome [open|tabs|bookmarks]",
    "Chrome integration",
);

pub const CMD_EFFORT: CommandDef = CommandDef::new(
    "/effort",
    &[],
    "Config",
    "/effort [minimal|normal|maximum]",
    "Set effort level",
);

pub const CMD_PREAMBLE: CommandDef = CommandDef::new(
    "/preamble",
    &[],
    "Config",
    "/preamble [show|set <text>|reset]",
    "Customize agent preamble",
);

pub const CMD_UNTRAP: CommandDef = CommandDef::new(
    "/untrap",
    &[],
    "General",
    "/untrap",
    "Reset trapped state",
);

pub const CMD_VERBOSE: CommandDef = CommandDef::new(
    "/verbose",
    &[],
    "Config",
    "/verbose [on|off]",
    "Toggle verbose output",
);

pub const CMD_WRITE: CommandDef = CommandDef::new(
    "/write",
    &[],
    "General",
    "/write <filepath> <content>",
    "Write content to a file",
);

// Phase 10 Extended: More commands
pub const CMD_ROLLBACK: CommandDef = CommandDef::new(
    "/rollback",
    &[],
    "Git",
    "/rollback [target]",
    "Rollback changes via git",
);

pub const CMD_PROJECT: CommandDef = CommandDef::new(
    "/project",
    &[],
    "Info",
    "/project [info|list|init]",
    "Project management",
);

pub const CMD_BACKEND: CommandDef = CommandDef::new(
    "/backend",
    &[],
    "Config",
    "/backend [local|restricted|external]",
    "Switch execution backend",
);

pub const CMD_SANDBOX: CommandDef = CommandDef::new(
    "/sandbox",
    &[],
    "Config",
    "/sandbox [on|off]",
    "Toggle sandbox mode",
);

pub const CMD_ENV: CommandDef = CommandDef::new(
    "/env",
    &[],
    "Info",
    "/env [list|get <key>]",
    "Show environment variables",
);

pub const CMD_CACHE: CommandDef = CommandDef::new(
    "/cache",
    &[],
    "Config",
    "/cache [clear|stats]",
    "Cache management",
);

pub const CMD_BENCHMARK: CommandDef = CommandDef::new(
    "/benchmark",
    &[],
    "Info",
    "/benchmark [n]",
    "Run performance benchmark",
);

pub const CMD_TEST: CommandDef = CommandDef::new(
    "/test",
    &[],
    "Info",
    "/test [filter]",
    "Run tests",
);

// Note: CMD_DEBUG not added - bundled skill handles /debug

pub const CMD_TRACE: CommandDef = CommandDef::new(
    "/trace",
    &[],
    "Config",
    "/trace [on|off|status]",
    "Tracing controls",
);

pub const CMD_SKILLS: CommandDef = CommandDef::new(
    "/skills",
    &[],
    "Info",
    "/skills",
    "List available skills",
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
    // Phase 9 Task 3: Register new commands
    registry.register(&CMD_BTW);
    registry.register(&CMD_CONTEXT);
    registry.register(&CMD_GIT);
    registry.register(&CMD_HISTORY);
    registry.register(&CMD_MODE);
    registry.register(&CMD_PACKAGE);
// Phase 9 Task 1: Advanced Agent Types
    registry.register(&CMD_TEAMMATE);
    registry.register(&CMD_CRITIC);
    registry.register(&CMD_ASSISTANT);
    registry.register(&CMD_REMOTE);
    // Phase 10 Batch 1: Session & Control Commands
    registry.register(&CMD_SESSION);
    registry.register(&CMD_UNDO);
    registry.register(&CMD_REDO);
    registry.register(&CMD_RETRY);
    registry.register(&CMD_STOP);
    registry.register(&CMD_RELOAD);
    registry.register(&CMD_SHARE);
    registry.register(&CMD_TOKEN);
    registry.register(&CMD_LSP);
    registry.register(&CMD_NPM);
    // Phase 10 Batch 2: hooks, profiling, prompt, migrate, focus, pause, install, skeleton, branch, color
    registry.register(&CMD_HOOKS);
    registry.register(&CMD_PROFILING);
    registry.register(&CMD_PROMPT);
    registry.register(&CMD_MIGRATE);
    registry.register(&CMD_FOCUS);
    registry.register(&CMD_PAUSE);
    registry.register(&CMD_INSTALL);
    registry.register(&CMD_SKELETON);
    registry.register(&CMD_BRANCH);
    registry.register(&CMD_COLOR);
    // Phase 10 Batch 3: webhook, wizard, workspace, slack, stealth, shadow, reject, subscribe, slots, ticker
    registry.register(&CMD_WEBHOOK);
    registry.register(&CMD_WIZARD);
    registry.register(&CMD_WORKSPACE);
    registry.register(&CMD_SLACK);
    registry.register(&CMD_STEALTH);
    registry.register(&CMD_SHADOW);
    registry.register(&CMD_REJECT);
    registry.register(&CMD_SUBSCRIBE);
    registry.register(&CMD_SLOTS);
    registry.register(&CMD_TICKER);
    // Phase 10 Batch 4: config, copy, desktop, chrome, effort, preamble, untrap, verbose, write
    registry.register(&CMD_CONFIG);
    registry.register(&CMD_COPY);
    registry.register(&CMD_DESKTOP);
    registry.register(&CMD_CHROME);
    registry.register(&CMD_EFFORT);
    registry.register(&CMD_PREAMBLE);
    registry.register(&CMD_UNTRAP);
    registry.register(&CMD_VERBOSE);
    registry.register(&CMD_WRITE);
    // Phase 10 Extended: More commands
    registry.register(&CMD_ROLLBACK);
    registry.register(&CMD_PROJECT);
    registry.register(&CMD_BACKEND);
    registry.register(&CMD_SANDBOX);
    registry.register(&CMD_ENV);
    registry.register(&CMD_CACHE);
    registry.register(&CMD_BENCHMARK);
    registry.register(&CMD_TEST);
    // Note: CMD_DEBUG not registered - bundled skill handles /debug
    registry.register(&CMD_TRACE);
    registry.register(&CMD_MEMORY);
    registry.register(&CMD_SKILLS);
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
