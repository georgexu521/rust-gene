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

mod catalog;
pub use catalog::*;

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

    /// 迭代所有已注册的命令（去重后的主名称）。
    pub fn commands(&self) -> impl Iterator<Item = &CommandDef> {
        let mut seen = HashSet::new();
        self.commands
            .values()
            .filter(move |cmd| seen.insert(cmd.name))
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
    registry.register(&CMD_LAB);
    registry.register(&CMD_TASKS);
    registry.register(&CMD_AGENT);
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
    registry.register(&CMD_PROMPT_HISTORY);
    registry.register(&CMD_PROMPT_STASH);
    registry.register(&CMD_PASTE);
    registry.register(&CMD_ATTACH);
    registry.register(&CMD_JUMP);
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
    registry.register(&CMD_CONNECT);
    registry.register(&CMD_CREDENTIALS);
    registry.register(&CMD_PRODUCT_READY);
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

#[cfg(test)]
mod tests;
