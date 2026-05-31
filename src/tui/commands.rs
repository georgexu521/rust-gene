//! TUI 命令注册表
//!
//! 统一管理所有 slash 命令，支持别名、分类、帮助信息。
//! 借鉴 Hermes 的 CommandDef 设计。

use std::collections::{BTreeMap, HashMap, HashSet};

/// Slash command maturity shown in help and command-palette surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandMaturity {
    Production,
    Usable,
    Placeholder,
}

impl CommandMaturity {
    pub const fn label(self) -> &'static str {
        match self {
            CommandMaturity::Production => "production",
            CommandMaturity::Usable => "usable",
            CommandMaturity::Placeholder => "placeholder",
        }
    }

    pub const fn badge(self) -> &'static str {
        match self {
            CommandMaturity::Production => "[production]",
            CommandMaturity::Usable => "[usable]",
            CommandMaturity::Placeholder => "[placeholder]",
        }
    }
}

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
    /// 是否为实验性命令
    pub experimental: bool,
    /// 是否为占位命令（功能尚未完全实现）
    pub placeholder: bool,
    /// 用户可见成熟度分类
    pub maturity: CommandMaturity,
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
            experimental: false,
            placeholder: false,
            maturity: CommandMaturity::Production,
        }
    }

    /// Create a new command with experimental and placeholder flags
    pub const fn new_with_flags(
        name: &'static str,
        aliases: &'static [&'static str],
        category: &'static str,
        usage: &'static str,
        description: &'static str,
        experimental: bool,
        placeholder: bool,
    ) -> Self {
        Self {
            name,
            aliases,
            category,
            usage,
            description,
            experimental,
            placeholder,
            maturity: if placeholder {
                CommandMaturity::Placeholder
            } else if experimental {
                CommandMaturity::Usable
            } else {
                CommandMaturity::Production
            },
        }
    }
}

/// 命令注册表
pub struct CommandRegistry {
    /// 按名称索引
    commands: HashMap<String, CommandDef>,
    /// 按分类分组
    categories: HashMap<&'static str, Vec<String>>,
}

pub const SUGGESTED_COMMANDS: &[&str] = &[
    "/quick",
    "/doctor",
    "/permissions",
    "/session",
    "/model",
    "/provider",
    "/init",
];

pub fn is_suggested_command(name: &str) -> bool {
    SUGGESTED_COMMANDS.contains(&name)
}

const USABLE_COMMANDS: &[&str] = &[
    "/tool-output",
    "/panel",
    "/agents",
    "/tasks",
    "/teammate",
    "/critic",
    "/assistant",
    "/dream",
    "/custom",
    "/orchestrate",
    "/remote",
    "/lsp",
    "/npm",
    "/profiling",
    "/migrate",
    "/install",
    "/skeleton",
    "/branch",
    "/webhook",
    "/wizard",
    "/workspace",
    "/stealth",
    "/shadow",
    "/subscribe",
    "/ticker",
    "/eval",
    "/resource",
    "/evolution",
    "/skill-proposals",
];

const PLACEHOLDER_COMMANDS: &[&str] = &["/desktop", "/reset", "/slack", "/chrome"];

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            categories: HashMap::new(),
        }
    }

    /// 注册一个命令
    pub fn register(&mut self, def: &CommandDef) {
        let name = def.name.to_string();
        // 注册主名称
        self.commands.insert(name.clone(), def.clone());
        // 注册别名
        for alias in def.aliases {
            self.commands.insert(alias.to_string(), def.clone());
        }
        // 按分类分组
        self.categories.entry(def.category).or_default().push(name);
    }

    /// 查找命令
    pub fn get(&self, name: &str) -> Option<&CommandDef> {
        self.commands.get(name)
    }

    /// 生成帮助文本
    pub fn help_text(&self) -> String {
        let mut result = String::from("Commands:\n");

        let mut cats: Vec<_> = self.categories.keys().copied().collect();
        cats.sort();

        for cat in cats {
            result.push_str(&format!("\n  {}:\n", cat));
            if let Some(cmd_names) = self.categories.get(cat) {
                for cmd_name in cmd_names {
                    if let Some(cmd) = self.commands.get(cmd_name) {
                        if cmd.placeholder {
                            continue;
                        }
                        let alias_str = if cmd.aliases.is_empty() {
                            String::new()
                        } else {
                            format!(" ({})", cmd.aliases.join(", "))
                        };
                        result.push_str(&format!(
                            "    {:<24} {}{} {}\n",
                            cmd.usage,
                            cmd.description,
                            alias_str,
                            cmd.maturity.badge()
                        ));
                    }
                }
            }
        }
        result
    }

    /// 生成带标记的帮助文本（显示所有命令，包括 experimental 和 placeholder）
    pub fn help_text_all(&self) -> String {
        let mut result = String::from("Commands (all):\n");

        let mut cats: Vec<_> = self.categories.keys().copied().collect();
        cats.sort();

        for cat in cats {
            result.push_str(&format!("\n  {}:\n", cat));
            if let Some(cmd_names) = self.categories.get(cat) {
                for cmd_name in cmd_names {
                    if let Some(cmd) = self.commands.get(cmd_name) {
                        let alias_str = if cmd.aliases.is_empty() {
                            String::new()
                        } else {
                            format!(" ({})", cmd.aliases.join(", "))
                        };
                        result.push_str(&format!(
                            "    {:<24} {}{} {}\n",
                            cmd.usage,
                            cmd.description,
                            alias_str,
                            cmd.maturity.badge()
                        ));
                    }
                }
            }
        }
        result
    }

    /// 标记命令为占位符
    pub fn mark_placeholder(&mut self, name: &str) {
        let Some(canonical_name) = self.commands.get(name).map(|cmd| cmd.name) else {
            return;
        };
        for cmd in self
            .commands
            .values_mut()
            .filter(|cmd| cmd.name == canonical_name)
        {
            cmd.placeholder = true;
            cmd.maturity = CommandMaturity::Placeholder;
        }
    }

    /// 标记命令为实验性
    pub fn mark_experimental(&mut self, name: &str) {
        let Some(canonical_name) = self.commands.get(name).map(|cmd| cmd.name) else {
            return;
        };
        for cmd in self
            .commands
            .values_mut()
            .filter(|cmd| cmd.name == canonical_name)
        {
            cmd.experimental = true;
            if cmd.maturity != CommandMaturity::Placeholder {
                cmd.maturity = CommandMaturity::Usable;
            }
        }
    }

    /// 标记命令为可用但尚未达到日常生产成熟度
    pub fn mark_usable(&mut self, name: &str) {
        let Some(canonical_name) = self.commands.get(name).map(|cmd| cmd.name) else {
            return;
        };
        for cmd in self
            .commands
            .values_mut()
            .filter(|cmd| cmd.name == canonical_name)
        {
            if cmd.maturity != CommandMaturity::Placeholder {
                cmd.maturity = CommandMaturity::Usable;
            }
        }
    }

    /// 按分类列出命令
    pub fn by_category(&self) -> std::collections::HashMap<&'static str, Vec<&CommandDef>> {
        let mut result: std::collections::HashMap<&'static str, Vec<&CommandDef>> =
            Default::default();
        for (cat, cmd_names) in &self.categories {
            let cmds: Vec<&CommandDef> = cmd_names
                .iter()
                .filter_map(|n| self.commands.get(n))
                .collect();
            result.insert(cat, cmds);
        }
        result
    }

    /// 获取实验性命令
    pub fn experimental_commands(&self) -> Vec<&CommandDef> {
        self.commands
            .values()
            .filter(|cmd| cmd.experimental)
            .collect()
    }

    /// 获取占位命令
    pub fn placeholder_commands(&self) -> Vec<&CommandDef> {
        self.commands
            .values()
            .filter(|cmd| cmd.placeholder)
            .collect()
    }

    /// 获取指定成熟度命令
    pub fn maturity_commands(&self, maturity: CommandMaturity) -> Vec<&CommandDef> {
        let mut seen = HashSet::new();
        self.commands
            .values()
            .filter(|cmd| seen.insert(cmd.name))
            .filter(|cmd| cmd.maturity == maturity)
            .collect()
    }

    pub fn maturity_summary(&self) -> BTreeMap<&'static str, usize> {
        let mut summary = BTreeMap::new();
        for maturity in [
            CommandMaturity::Production,
            CommandMaturity::Usable,
            CommandMaturity::Placeholder,
        ] {
            summary.insert(maturity.label(), self.maturity_commands(maturity).len());
        }
        summary
    }

    pub fn maturity_report(&self) -> String {
        let mut lines = vec!["Command maturity:".to_string()];
        for maturity in [
            CommandMaturity::Production,
            CommandMaturity::Usable,
            CommandMaturity::Placeholder,
        ] {
            let mut names = self
                .maturity_commands(maturity)
                .into_iter()
                .map(|cmd| cmd.name)
                .collect::<Vec<_>>();
            names.sort_unstable();
            lines.push(format!(
                "- {} ({}): {}",
                maturity.label(),
                names.len(),
                if names.is_empty() {
                    "none".to_string()
                } else {
                    names.join(", ")
                }
            ));
        }
        lines.join("\n")
    }

    /// 命令面板候选项，使用轻量 fuzzy 排序，并过滤别名重复项。
    pub fn palette_items(
        &self,
        query: &str,
        limit: usize,
        recent_commands: &[String],
    ) -> Vec<&CommandDef> {
        let query = query.trim().to_ascii_lowercase();
        let mut seen = HashSet::new();
        let mut scored = Vec::new();
        let recent_rank = recent_commands
            .iter()
            .rev()
            .enumerate()
            .map(|(idx, name)| (name.as_str(), 2_000_i32.saturating_sub(idx as i32 * 100)))
            .collect::<HashMap<_, _>>();

        for cmd in self.commands.values() {
            if !seen.insert(cmd.name) {
                continue;
            }
            if query.is_empty() && cmd.placeholder {
                continue;
            }
            let haystack = format!(
                "{} {} {} {}",
                cmd.name,
                cmd.aliases.join(" "),
                cmd.category,
                cmd.description
            )
            .to_ascii_lowercase();
            let Some(score) = command_match_score(&query, &haystack, cmd) else {
                continue;
            };
            let mut score = score + recent_rank.get(cmd.name).copied().unwrap_or_default();
            if query.is_empty() {
                if let Some(idx) = SUGGESTED_COMMANDS
                    .iter()
                    .position(|suggested| *suggested == cmd.name)
                {
                    score += 1_500_i32.saturating_sub(idx as i32 * 100);
                }
            }
            scored.push((score, cmd.category, cmd.name, cmd));
        }

        scored.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.cmp(b.1))
                .then_with(|| a.2.cmp(b.2))
        });

        if query.is_empty() && recent_commands.is_empty() {
            let mut items = Vec::new();
            for name in SUGGESTED_COMMANDS {
                if let Some(cmd) = self.commands.get(*name) {
                    items.push(cmd);
                    if items.len() >= limit {
                        return items;
                    }
                }
            }

            let mut grouped: BTreeMap<&str, Vec<&CommandDef>> = BTreeMap::new();
            for (_, category, _, cmd) in scored {
                if is_suggested_command(cmd.name) {
                    continue;
                }
                grouped.entry(category).or_default().push(cmd);
            }
            for commands in grouped.values_mut() {
                commands.sort_by_key(|cmd| cmd.name);
                for cmd in commands.iter().take(limit.saturating_sub(items.len())) {
                    items.push(*cmd);
                    if items.len() >= limit {
                        return items;
                    }
                }
            }
            return items;
        }

        scored
            .into_iter()
            .take(limit)
            .map(|(_, _, _, cmd)| cmd)
            .collect()
    }
}

pub fn command_accept_behavior(cmd: &CommandDef) -> CommandAcceptBehavior {
    if cmd.placeholder {
        return CommandAcceptBehavior::Insert;
    }
    if command_requires_arguments(cmd) {
        CommandAcceptBehavior::Insert
    } else {
        CommandAcceptBehavior::Execute
    }
}

fn command_requires_arguments(cmd: &CommandDef) -> bool {
    let usage = cmd.usage;
    usage.contains('<')
        || matches!(
            cmd.name,
            "/save"
                | "/btw"
                | "/copy"
                | "/write"
                | "/import"
                | "/load-session"
                | "/merge"
                | "/search"
                | "/filter"
                | "/feedback"
        )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandAcceptBehavior {
    Execute,
    Insert,
}

fn command_match_score(query: &str, haystack: &str, cmd: &CommandDef) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }

    let slashless = cmd.name.trim_start_matches('/').to_ascii_lowercase();
    let mut score = fuzzy_score(query, haystack)?;
    if slashless == query {
        score += 10_000;
    } else if slashless.starts_with(query) {
        score += 5_000;
    } else if cmd.name.to_ascii_lowercase().contains(query) {
        score += 2_500;
    }
    if cmd
        .aliases
        .iter()
        .any(|alias| alias.trim_start_matches('/').eq_ignore_ascii_case(query))
    {
        score += 4_000;
    }
    Some(score)
}

fn fuzzy_score(query: &str, haystack: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }

    let mut score = 0_i32;
    let mut search_from = 0_usize;
    let mut prev_match: Option<usize> = None;

    for q in query.chars() {
        let slice = haystack.get(search_from..)?;
        let rel = slice.find(q)?;
        let pos = search_from + rel;
        score += 100;
        if let Some(prev) = prev_match {
            if pos == prev + 1 {
                score += 80;
            } else {
                score -= ((pos - prev).min(20) as i32) * 2;
            }
        }
        if pos == 0 || haystack.as_bytes().get(pos.saturating_sub(1)) == Some(&b'/') {
            score += 60;
        } else if matches!(
            haystack.as_bytes().get(pos.saturating_sub(1)),
            Some(b' ' | b'-' | b'_')
        ) {
            score += 35;
        }
        prev_match = Some(pos);
        search_from = pos + q.len_utf8();
    }

    Some(score)
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════
// 命令定义（编译期常量）
// ═══════════════════════════════════════

pub const CMD_HELP: CommandDef = CommandDef::new(
    "/help",
    &["/h"],
    "General",
    "/help [maturity]",
    "Show help or command maturity report",
);

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

pub const CMD_MEMORY: CommandDef = CommandDef::new(
    "/memory",
    &[],
    "Memory",
    "/memory [search|snapshot|records [--scope project]|eval|doctor|review|migrate|repair-proposals|conflicts]",
    "Show saved memory",
);

pub const CMD_SAVE: CommandDef =
    CommandDef::new("/save", &[], "Memory", "/save <text>", "Save to memory");

pub const CMD_COST: CommandDef =
    CommandDef::new("/cost", &[], "Info", "/cost", "Show token usage and cost");

pub const CMD_MODEL: CommandDef =
    CommandDef::new("/model", &[], "Info", "/model", "Show current model");

pub const CMD_PROVIDER: CommandDef = CommandDef::new(
    "/provider",
    &[],
    "Info",
    "/provider",
    "Show or switch LLM provider",
);

pub const CMD_STATUS: CommandDef =
    CommandDef::new("/status", &[], "Info", "/status", "Show session status");

pub const CMD_STATUSBAR: CommandDef = CommandDef::new(
    "/statusbar",
    &[],
    "Info",
    "/statusbar [compact|normal|debug]",
    "Show or set status bar density",
);

pub const CMD_TOOLS: CommandDef =
    CommandDef::new("/tools", &[], "Info", "/tools", "List available tools");

pub const CMD_TOOL_OUTPUT: CommandDef = CommandDef::new(
    "/tool-output",
    &["/tool"],
    "Info",
    "/tool-output [list|latest|<tool_id>]",
    "Open or list captured tool outputs",
);

pub const CMD_PANEL: CommandDef = CommandDef::new(
    "/panel",
    &["/panels", "/runtime"],
    "Info",
    "/panel [all|diff|approval|hooks|context|tasks|agents|mcp|bridge|trace]",
    "Show runtime panels for diffs, approvals, hooks, context, tasks, agents, MCP, bridge, and traces",
);

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
    "/agents [worktree review|merge|cleanup <agent_id>]",
    "List active/known agents and manage isolated worktrees",
);

pub const CMD_CHECKPOINTS: CommandDef = CommandDef::new(
    "/checkpoints",
    &[],
    "Info",
    "/checkpoints",
    "List file checkpoints (snapshots) for this session",
);

pub const CMD_RESTORE: CommandDef = CommandDef::new(
    "/restore",
    &["/r"],
    "Action",
    "/restore <checkpoint_id>",
    "Restore files to a checkpoint state",
);

pub const CMD_BATCH: CommandDef = CommandDef::new(
    "/batch",
    &[],
    "Action",
    "/batch <description> --files <patterns...>",
    "Run batch refactoring across multiple files in parallel",
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
    "/audit [summary|recent|tools|export] ...",
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

pub const CMD_COMMIT_PUSH_PR: CommandDef = CommandDef::new(
    "/commit-push-pr",
    &[],
    "Skills",
    "/commit-push-pr [description]",
    "Stage, commit, push, and create a PR in one workflow",
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

pub const CMD_KARPATHY: CommandDef = CommandDef::new(
    "/karpathy",
    &[],
    "Skills",
    "/karpathy [task]",
    "Apply careful coding guidelines to a task",
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
    "/mode [auto|build|plan|explore|review]",
    "Switch coding agent mode",
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
    "/remote [status|task]",
    "Show bridge status or start a remote agent",
);

pub const CMD_DREAM: CommandDef = CommandDef::new(
    "/dream",
    &[],
    "Agents",
    "/dream [task]",
    "Start a background exploratory analysis",
);

pub const CMD_CUSTOM: CommandDef = CommandDef::new(
    "/custom",
    &[],
    "Agents",
    "/custom [role] [domain]",
    "Create a custom agent",
);

pub const CMD_ORCHESTRATE: CommandDef = CommandDef::new(
    "/orchestrate",
    &[],
    "Agents",
    "/orchestrate [task]",
    "Coordinate multiple agents",
);

// Phase 10 Batch 1: Session & Control Commands
pub const CMD_SESSION: CommandDef = CommandDef::new(
    "/session",
    &["/sessions"],
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
    "/prompt [show|templates|render|edit|append|apply|reset]",
    "Show, render, or edit prompts",
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
    "/focus [on|off|toggle|status]",
    "Toggle focus mode",
);

pub const CMD_PAUSE: CommandDef = CommandDef::new(
    "/pause",
    &[],
    "General",
    "/pause [pause|resume|toggle|status]",
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
    "/color [dark|light|high-contrast|hc]",
    "Theme color alias (same as /theme)",
);

// Phase 10 Batch 3: webhook, wizard, workspace, slack, stealth, shadow, reject, subscribe, slots, ticker
pub const CMD_WEBHOOK: CommandDef = CommandDef::new(
    "/webhook",
    &[],
    "Info",
    "/webhook [list|create <url> [name]|delete <name>|test <name|url> [payload]]",
    "Manage webhooks",
);

pub const CMD_WIZARD: CommandDef = CommandDef::new(
    "/wizard",
    &[],
    "Config",
    "/wizard",
    "Open guided setup in settings mode",
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
    "/slack [status|connect <webhook_url> [channel]|disconnect|send [#channel] <message>]",
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
    "/subscribe [list|add <event>|remove <event>|clear]",
    "Subscribe to events",
);

pub const CMD_SLOTS: CommandDef = CommandDef::new(
    "/slots",
    &[],
    "Config",
    "/slots [list|get <name>|set <name> <value>|unset <name>|clear]",
    "View/edit slot variables",
);

pub const CMD_TICKER: CommandDef = CommandDef::new(
    "/ticker",
    &[],
    "General",
    "/ticker [show|clear|<message>]",
    "Manage ticker message",
);

// Phase 10 Batch 4: config, copy, desktop, chrome, effort, preamble, untrap, verbose, write
pub const CMD_CONFIG: CommandDef = CommandDef::new(
    "/config",
    &[],
    "Config",
    "/config [list|schema|paths|doctor|export|get <key>|set <key> <value>]",
    "View, edit, validate, or export redacted configuration",
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
    "/chrome [open <url>|tabs|bookmarks]",
    "Chrome integration",
);

pub const CMD_EFFORT: CommandDef = CommandDef::new(
    "/effort",
    &[],
    "Config",
    "/effort [minimal|normal|maximum|status]",
    "Set effort level",
);

pub const CMD_PREAMBLE: CommandDef = CommandDef::new(
    "/preamble",
    &[],
    "Config",
    "/preamble [show|set <text>|reset]",
    "Customize agent preamble",
);

pub const CMD_UNTRAP: CommandDef =
    CommandDef::new("/untrap", &[], "General", "/untrap", "Reset trapped state");

pub const CMD_VERBOSE: CommandDef = CommandDef::new(
    "/verbose",
    &[],
    "Config",
    "/verbose [on|off|toggle|status]",
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
    "/rollback [target|last-file|file_change_id] --yes",
    "Rollback git history or restore a recorded file change",
);

pub const CMD_PROJECT: CommandDef = CommandDef::new(
    "/project",
    &[],
    "Info",
    "/project [info|soul|pulse|progress|heartbeat|list|tree [depth]|init <name>]",
    "Project management",
);

pub const CMD_BACKEND: CommandDef = CommandDef::new(
    "/backend",
    &[],
    "Config",
    "/backend [local|restricted|external|status]",
    "Switch execution backend",
);

pub const CMD_SANDBOX: CommandDef = CommandDef::new(
    "/sandbox",
    &[],
    "Config",
    "/sandbox [on|off|toggle|status]",
    "Toggle sandbox mode",
);

pub const CMD_ENV: CommandDef = CommandDef::new(
    "/env",
    &[],
    "Info",
    "/env [list|get <key>|set <key> <value>|unset <key>]",
    "Show environment variables",
);

pub const CMD_CACHE: CommandDef = CommandDef::new(
    "/cache",
    &[],
    "Config",
    "/cache [clear|stats|prompt|miss-report]",
    "Cache and prompt-cache diagnostics",
);

pub const CMD_BENCHMARK: CommandDef = CommandDef::new(
    "/benchmark",
    &[],
    "Info",
    "/benchmark [n] (script or synthetic)",
    "Run performance benchmark",
);

pub const CMD_TEST: CommandDef =
    CommandDef::new("/test", &[], "Info", "/test [filter]", "Run tests");

// Note: CMD_DEBUG not added - bundled skill handles /debug

pub const CMD_TRACE: CommandDef = CommandDef::new(
    "/trace",
    &[],
    "Config",
    "/trace [last|recent|on|off|toggle|status]",
    "Show runtime trace or configure log tracing",
);

pub const CMD_EVAL: CommandDef = CommandDef::new(
    "/eval",
    &[],
    "Info",
    "/eval [list|matrix|parity|parity-record|baseline|baseline-validate|baseline-template|baseline-write|baseline-import|run]",
    "Run evalsets, report/record parity, or manage baseline files",
);

pub const CMD_RESOURCE: CommandDef = CommandDef::new(
    "/resource",
    &[],
    "Info",
    "/resource",
    "Show latest resource policy",
);

pub const CMD_SKILLS: CommandDef =
    CommandDef::new("/skills", &[], "Info", "/skills", "List available skills");

// Phase 10 Extended 2: More commands
pub const CMD_INIT: CommandDef = CommandDef::new(
    "/init",
    &[],
    "General",
    "/init <project_name>",
    "Initialize a new project",
);

pub const CMD_LOGIN: CommandDef = CommandDef::new(
    "/login",
    &[],
    "General",
    "/login [provider|status]",
    "Authenticate with provider",
);

pub const CMD_LOGOUT: CommandDef =
    CommandDef::new("/logout", &[], "General", "/logout", "Logout from provider");

pub const CMD_KEY: CommandDef = CommandDef::new(
    "/key",
    &[],
    "Config",
    "/key [show|clear]",
    "API key management",
);

pub const CMD_HEALTH: CommandDef =
    CommandDef::new("/health", &[], "Info", "/health", "Health check");

pub const CMD_PING: CommandDef = CommandDef::new("/ping", &[], "Info", "/ping", "Latency check");

pub const CMD_UPTIME: CommandDef =
    CommandDef::new("/uptime", &[], "Info", "/uptime", "Show uptime");

pub const CMD_VERSION: CommandDef =
    CommandDef::new("/version", &[], "Info", "/version", "Show version");

pub const CMD_ABOUT: CommandDef =
    CommandDef::new("/about", &[], "Info", "/about", "About this agent");

// Phase 10 Extended 3: Session management and utility commands
pub const CMD_RESET: CommandDef = CommandDef::new(
    "/reset",
    &[],
    "Session",
    "/reset [session|all]",
    "Reset session state",
);

pub const CMD_EXPORT: CommandDef = CommandDef::new(
    "/export",
    &[],
    "Session",
    "/export [json|md]",
    "Export session data",
);

pub const CMD_IMPORT: CommandDef = CommandDef::new(
    "/import",
    &[],
    "Session",
    "/import <filepath>",
    "Import session data",
);

pub const CMD_SAVE_SESSION: CommandDef = CommandDef::new(
    "/save-session",
    &[],
    "Session",
    "/save-session",
    "Save current session",
);

pub const CMD_LOAD_SESSION: CommandDef = CommandDef::new(
    "/load-session",
    &[],
    "Session",
    "/load-session <session_id>",
    "Load a session",
);

pub const CMD_MERGE: CommandDef = CommandDef::new(
    "/merge",
    &[],
    "Session",
    "/merge <session_id>",
    "Merge sessions",
);

pub const CMD_CLEANUP: CommandDef = CommandDef::new(
    "/cleanup",
    &[],
    "Config",
    "/cleanup [sessions|cache|logs|all]",
    "Cleanup old data",
);

pub const CMD_COMPACT: CommandDef = CommandDef::new(
    "/compact",
    &[],
    "General",
    "/compact",
    "Compact conversation context",
);

pub const CMD_SNIPPET: CommandDef = CommandDef::new(
    "/snippet",
    &[],
    "General",
    "/snippet [save|load|list] <name>",
    "Manage code snippets",
);

pub const CMD_BOOKMARK: CommandDef = CommandDef::new(
    "/bookmark",
    &[],
    "General",
    "/bookmark [add|go|list]",
    "Bookmark locations",
);

pub const CMD_TAG: CommandDef =
    CommandDef::new("/tag", &[], "General", "/tag [add|list|find]", "Tag items");

pub const CMD_SEARCH_CMD: CommandDef = CommandDef::new(
    "/search",
    &[],
    "Session",
    "/search <query>",
    "Search within session",
);

pub const CMD_FILTER: CommandDef = CommandDef::new(
    "/filter",
    &[],
    "Session",
    "/filter <user|assistant|tool|system|all> [query]",
    "Filter messages",
);

// Phase 10 Final: Final commands to reach 101
pub const CMD_PROFILE: CommandDef = CommandDef::new(
    "/profile",
    &[],
    "Config",
    "/profile [show [key]|set <key> <value>|unset <key>]",
    "Edit user profile",
);

pub const CMD_THEME: CommandDef = CommandDef::new(
    "/theme",
    &[],
    "Config",
    "/theme [list]",
    "Theme customization",
);

pub const CMD_SHORTCUTS: CommandDef = CommandDef::new(
    "/shortcuts",
    &[],
    "Info",
    "/shortcuts",
    "Show keyboard shortcuts",
);

pub const CMD_QUICK: CommandDef =
    CommandDef::new("/quick", &[], "General", "/quick", "Quick actions menu");

pub const CMD_ACTIVE_TASK: CommandDef = CommandDef::new(
    "/active-task",
    &["/progress"],
    "General",
    "/active-task",
    "Inspect unified task plan, progress, verification, closeout, and memory proposal state",
);

pub const CMD_GOAL: CommandDef = CommandDef::new(
    "/goal",
    &[],
    "General",
    "/goal [set <text>|clear]",
    "Show or pin the current session goal",
);

pub const CMD_LEARN: CommandDef = CommandDef::new(
    "/learn",
    &[],
    "General",
    "/learn [limit|show <id>]",
    "Show recent runtime learning events",
);

pub const CMD_EXPERIENCE: CommandDef = CommandDef::new(
    "/experience",
    &[],
    "General",
    "/experience [last|list|show <id>]",
    "Inspect structured experience ledger records",
);

pub const CMD_MEMORY_PROPOSALS: CommandDef = CommandDef::new(
    "/memory-proposals",
    &["/memory-proposal"],
    "General",
    "/memory-proposals [list [--source background|repair]|show|accept|reject|edit|apply|repair-drift]",
    "Review closeout-generated memory candidates before persistence",
);

pub const CMD_EVOLUTION: CommandDef = CommandDef::new(
    "/evolution",
    &[],
    "General",
    "/evolution [status|audit|json|show <id>]",
    "Inspect controlled self-evolution state and audit events",
);

pub const CMD_IMPROVEMENTS: CommandDef = CommandDef::new(
    "/improvements",
    &[],
    "General",
    "/improvements [list|scan|show|bind-eval|eval|accept|reject|apply|rollback]",
    "Review controlled self-evolution proposals",
);

pub const CMD_SKILL_PROPOSALS: CommandDef = CommandDef::new(
    "/skill-proposals",
    &["/skill-proposal"],
    "General",
    "/skill-proposals [list|scan|show|eval|fitness|gate|versions|rollback-list|rollback|restore|bind-eval|record|accept|reject|apply]",
    "Review generated skill candidates before activation",
);

pub const CMD_RECOVER: CommandDef = CommandDef::new(
    "/recover",
    &[],
    "General",
    "/recover [limit]",
    "Show recent recovery plans",
);

pub const CMD_FEEDBACK: CommandDef = CommandDef::new(
    "/feedback",
    &[],
    "General",
    "/feedback <message>",
    "Send feedback",
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
    registry.register(&CMD_PROVIDER);
    registry.register(&CMD_STATUS);
    registry.register(&CMD_STATUSBAR);
    registry.register(&CMD_TOOLS);
    registry.register(&CMD_TOOL_OUTPUT);
    registry.register(&CMD_PANEL);
    registry.register(&CMD_TASKS);
    registry.register(&CMD_AGENTS);
    registry.register(&CMD_CHECKPOINTS);
    registry.register(&CMD_RESTORE);
    registry.register(&CMD_BATCH);
    registry.register(&CMD_DOCTOR);
    registry.register(&CMD_AUDIT);
    registry.register(&CMD_PERMISSIONS);
    registry.register(&CMD_DIFF);
    registry.register(&CMD_RESUME);
    registry.register(&CMD_REWIND);
    registry.register(&CMD_COMMIT);
    registry.register(&CMD_COMMIT_PUSH_PR);
    registry.register(&CMD_REVIEW_PR);
    registry.register(&CMD_REVIEW);
    registry.register(&CMD_SECURITY_REVIEW);
    registry.register(&CMD_EXPLAIN);
    registry.register(&CMD_FIX);
    registry.register(&CMD_KARPATHY);
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
    registry.register(&CMD_DREAM);
    registry.register(&CMD_CUSTOM);
    registry.register(&CMD_ORCHESTRATE);
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
    registry.register(&CMD_EVAL);
    registry.register(&CMD_RESOURCE);
    registry.register(&CMD_MEMORY);
    registry.register(&CMD_SKILLS);
    // Phase 10 Extended 2: More commands
    registry.register(&CMD_INIT);
    registry.register(&CMD_LOGIN);
    registry.register(&CMD_LOGOUT);
    registry.register(&CMD_KEY);
    registry.register(&CMD_HEALTH);
    registry.register(&CMD_PING);
    registry.register(&CMD_UPTIME);
    registry.register(&CMD_VERSION);
    registry.register(&CMD_ABOUT);
    // Phase 10 Extended 3: Session management and utility commands
    registry.register(&CMD_RESET);
    registry.register(&CMD_EXPORT);
    registry.register(&CMD_IMPORT);
    registry.register(&CMD_SAVE_SESSION);
    registry.register(&CMD_LOAD_SESSION);
    registry.register(&CMD_MERGE);
    registry.register(&CMD_CLEANUP);
    registry.register(&CMD_COMPACT);
    registry.register(&CMD_SNIPPET);
    registry.register(&CMD_BOOKMARK);
    registry.register(&CMD_TAG);
    registry.register(&CMD_SEARCH_CMD);
    registry.register(&CMD_FILTER);
    // Phase 10 Final: Final commands
    registry.register(&CMD_PROFILE);
    registry.register(&CMD_THEME);
    registry.register(&CMD_SHORTCUTS);
    registry.register(&CMD_QUICK);
    registry.register(&CMD_ACTIVE_TASK);
    registry.register(&CMD_GOAL);
    registry.register(&CMD_LEARN);
    registry.register(&CMD_EXPERIENCE);
    registry.register(&CMD_MEMORY_PROPOSALS);
    registry.register(&CMD_EVOLUTION);
    registry.register(&CMD_IMPROVEMENTS);
    registry.register(&CMD_SKILL_PROPOSALS);
    registry.register(&CMD_RECOVER);
    registry.register(&CMD_FEEDBACK);

    apply_command_maturity(&mut registry);

    registry
}

fn apply_command_maturity(registry: &mut CommandRegistry) {
    // Keep partially implemented commands visible but honest. Mature CLIs should
    // not make unavailable integrations look production-ready in help/palette UI.
    for name in USABLE_COMMANDS {
        registry.mark_usable(name);
    }
    for name in PLACEHOLDER_COMMANDS {
        registry.mark_placeholder(name);
    }
}

/// All registered command definitions (for gap analysis and introspection)
pub const ALL_COMMANDS: &[&CommandDef] = &[
    &CMD_HELP,
    &CMD_CLEAR,
    &CMD_QUIT,
    &CMD_MEMORY,
    &CMD_SAVE,
    &CMD_COST,
    &CMD_MODEL,
    &CMD_PROVIDER,
    &CMD_STATUS,
    &CMD_STATUSBAR,
    &CMD_TOOLS,
    &CMD_TOOL_OUTPUT,
    &CMD_PANEL,
    &CMD_TASKS,
    &CMD_AGENTS,
    &CMD_CHECKPOINTS,
    &CMD_RESTORE,
    &CMD_BATCH,
    &CMD_DOCTOR,
    &CMD_AUDIT,
    &CMD_PERMISSIONS,
    &CMD_DIFF,
    &CMD_RESUME,
    &CMD_REWIND,
    &CMD_COMMIT,
    &CMD_REVIEW_PR,
    &CMD_REVIEW,
    &CMD_SECURITY_REVIEW,
    &CMD_EXPLAIN,
    &CMD_FIX,
    &CMD_KARPATHY,
    &CMD_MCP,
    &CMD_VIM,
    &CMD_BTW,
    &CMD_CONTEXT,
    &CMD_GIT,
    &CMD_HISTORY,
    &CMD_MODE,
    &CMD_PACKAGE,
    &CMD_TEAMMATE,
    &CMD_CRITIC,
    &CMD_ASSISTANT,
    &CMD_REMOTE,
    &CMD_DREAM,
    &CMD_CUSTOM,
    &CMD_ORCHESTRATE,
    &CMD_SESSION,
    &CMD_UNDO,
    &CMD_REDO,
    &CMD_RETRY,
    &CMD_STOP,
    &CMD_RELOAD,
    &CMD_SHARE,
    &CMD_TOKEN,
    &CMD_LSP,
    &CMD_NPM,
    &CMD_HOOKS,
    &CMD_PROFILING,
    &CMD_PROMPT,
    &CMD_MIGRATE,
    &CMD_FOCUS,
    &CMD_PAUSE,
    &CMD_INSTALL,
    &CMD_SKELETON,
    &CMD_BRANCH,
    &CMD_COLOR,
    &CMD_WEBHOOK,
    &CMD_WIZARD,
    &CMD_WORKSPACE,
    &CMD_SLACK,
    &CMD_STEALTH,
    &CMD_SHADOW,
    &CMD_REJECT,
    &CMD_SUBSCRIBE,
    &CMD_SLOTS,
    &CMD_TICKER,
    &CMD_CONFIG,
    &CMD_COPY,
    &CMD_DESKTOP,
    &CMD_CHROME,
    &CMD_EFFORT,
    &CMD_PREAMBLE,
    &CMD_UNTRAP,
    &CMD_VERBOSE,
    &CMD_WRITE,
    &CMD_ROLLBACK,
    &CMD_PROJECT,
    &CMD_BACKEND,
    &CMD_SANDBOX,
    &CMD_ENV,
    &CMD_CACHE,
    &CMD_BENCHMARK,
    &CMD_TEST,
    &CMD_TRACE,
    &CMD_EVAL,
    &CMD_RESOURCE,
    &CMD_SKILLS,
    &CMD_INIT,
    &CMD_LOGIN,
    &CMD_LOGOUT,
    &CMD_KEY,
    &CMD_HEALTH,
    &CMD_PING,
    &CMD_UPTIME,
    &CMD_VERSION,
    &CMD_ABOUT,
    &CMD_RESET,
    &CMD_EXPORT,
    &CMD_IMPORT,
    &CMD_SAVE_SESSION,
    &CMD_LOAD_SESSION,
    &CMD_MERGE,
    &CMD_CLEANUP,
    &CMD_COMPACT,
    &CMD_SNIPPET,
    &CMD_BOOKMARK,
    &CMD_TAG,
    &CMD_SEARCH_CMD,
    &CMD_FILTER,
    &CMD_PROFILE,
    &CMD_THEME,
    &CMD_SHORTCUTS,
    &CMD_QUICK,
    &CMD_ACTIVE_TASK,
    &CMD_GOAL,
    &CMD_LEARN,
    &CMD_EXPERIENCE,
    &CMD_MEMORY_PROPOSALS,
    &CMD_EVOLUTION,
    &CMD_IMPROVEMENTS,
    &CMD_SKILL_PROPOSALS,
    &CMD_RECOVER,
    &CMD_FEEDBACK,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_lookup() {
        let registry = default_command_registry();
        assert!(registry.get("/help").is_some());
        assert!(registry.get("/h").is_some()); // alias
        assert!(registry.get("/tool-output").is_some());
        assert!(registry.get("/tool").is_some()); // alias
        assert!(registry.get("/panel").is_some());
        assert!(registry.get("/runtime").is_some()); // alias
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
        assert!(help.contains("[production]"));
        assert!(help.contains("[usable]"));
        assert!(!help.contains("[placeholder]"));
        assert!(!help.contains("/desktop"));

        let all_help = registry.help_text_all();
        assert!(all_help.contains("[placeholder]"));
        assert!(all_help.contains("/desktop"));
    }

    #[test]
    fn test_command_maturity_labels_are_explicit() {
        let registry = default_command_registry();
        assert_eq!(
            registry.get("/help").map(|cmd| cmd.maturity),
            Some(CommandMaturity::Production)
        );
        assert_eq!(
            registry.get("/agents").map(|cmd| cmd.maturity),
            Some(CommandMaturity::Usable)
        );
        assert_eq!(
            registry.get("/panel").map(|cmd| cmd.maturity),
            Some(CommandMaturity::Usable)
        );
        assert_eq!(
            registry.get("/runtime").map(|cmd| cmd.maturity),
            Some(CommandMaturity::Usable)
        );
        assert_eq!(
            registry.get("/tool-output").map(|cmd| cmd.maturity),
            Some(CommandMaturity::Usable)
        );
        assert_eq!(
            registry.get("/tool").map(|cmd| cmd.maturity),
            Some(CommandMaturity::Usable)
        );
        assert_eq!(
            registry.get("/desktop").map(|cmd| cmd.maturity),
            Some(CommandMaturity::Placeholder)
        );
        assert_eq!(
            registry.get("/desktop").map(|cmd| cmd.placeholder),
            Some(true)
        );
        assert!(!registry
            .maturity_commands(CommandMaturity::Placeholder)
            .is_empty());
    }

    #[test]
    fn test_command_maturity_lists_are_registered_and_disjoint() {
        let registry = default_command_registry();
        let mut listed = HashSet::new();

        for name in USABLE_COMMANDS {
            assert!(
                registry.get(name).is_some(),
                "usable command {name} is registered"
            );
            assert!(
                listed.insert(*name),
                "duplicate command maturity entry {name}"
            );
        }
        for name in PLACEHOLDER_COMMANDS {
            assert!(
                registry.get(name).is_some(),
                "placeholder command {name} is registered"
            );
            assert!(
                listed.insert(*name),
                "duplicate command maturity entry {name}"
            );
        }

        let summary = registry.maturity_summary();
        assert_eq!(
            summary.get(CommandMaturity::Usable.label()).copied(),
            Some(USABLE_COMMANDS.len())
        );
        assert_eq!(
            summary.get(CommandMaturity::Placeholder.label()).copied(),
            Some(PLACEHOLDER_COMMANDS.len())
        );
        assert!(
            summary
                .get(CommandMaturity::Production.label())
                .copied()
                .unwrap_or_default()
                > 0
        );
    }

    #[test]
    fn test_maturity_report_lists_runtime_surfaces() {
        let registry = default_command_registry();
        let report = registry.maturity_report();

        assert!(report.contains("Command maturity:"));
        assert!(report.contains("- usable"));
        assert!(report.contains("/panel"));
        assert!(report.contains("/tool-output"));
        assert!(report.contains("- placeholder"));
        assert!(report.contains("/desktop"));
    }

    #[test]
    fn test_palette_items_filters_and_deduplicates_aliases() {
        let registry = default_command_registry();
        let items = registry.palette_items("help", 20, &[]);
        assert!(items.iter().any(|cmd| cmd.name == "/help"));
        let help_count = items.iter().filter(|cmd| cmd.name == "/help").count();
        assert_eq!(help_count, 1);
    }

    #[test]
    fn test_palette_items_rank_exact_command_above_description_match() {
        let registry = default_command_registry();
        let items = registry.palette_items("model", 20, &[]);
        assert_eq!(items.first().map(|cmd| cmd.name), Some("/model"));
    }

    #[test]
    fn test_palette_items_support_subsequence_query() {
        let registry = default_command_registry();
        let items = registry.palette_items("prv", 20, &[]);
        assert!(items.iter().any(|cmd| cmd.name == "/provider"));
    }

    #[test]
    fn test_palette_items_rank_recent_commands_when_query_empty() {
        let registry = default_command_registry();
        let recent = vec!["/provider".to_string()];
        let items = registry.palette_items("", 20, &recent);
        assert_eq!(items.first().map(|cmd| cmd.name), Some("/provider"));
    }

    #[test]
    fn test_palette_items_show_suggested_commands_first_when_empty() {
        let registry = default_command_registry();
        let items = registry.palette_items("", 20, &[]);
        let names = items.iter().take(4).map(|cmd| cmd.name).collect::<Vec<_>>();
        assert_eq!(names, vec!["/quick", "/doctor", "/permissions", "/session"]);
    }

    #[test]
    fn test_palette_items_hide_placeholder_until_explicit_query() {
        let registry = default_command_registry();
        let default_items = registry.palette_items("", 200, &[]);
        assert!(!default_items.iter().any(|cmd| cmd.name == "/desktop"));

        let explicit_items = registry.palette_items("desktop", 20, &[]);
        assert!(explicit_items.iter().any(|cmd| cmd.name == "/desktop"));
    }

    #[test]
    fn test_command_accept_behavior_inserts_required_args() {
        assert_eq!(
            command_accept_behavior(&CMD_SAVE),
            CommandAcceptBehavior::Insert
        );
        assert_eq!(
            command_accept_behavior(&CMD_STATUS),
            CommandAcceptBehavior::Execute
        );
        let registry = default_command_registry();
        assert_eq!(
            registry.get("/desktop").map(command_accept_behavior),
            Some(CommandAcceptBehavior::Insert)
        );
    }
}
