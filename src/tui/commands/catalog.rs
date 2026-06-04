use super::CommandDef;

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

pub(crate) const USABLE_COMMANDS: &[&str] = &[
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

pub(crate) const PLACEHOLDER_COMMANDS: &[&str] = &["/desktop", "/reset", "/slack", "/chrome"];

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
    "/memory [control|search|why|snapshot|records [--scope project]|eval|doctor|review|migrate|repair-proposals|conflicts]",
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
