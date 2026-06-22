//! TUI support module.
//!
//! Keeps terminal rendering and interaction helpers separate from runtime execution.

use serde_json::Value;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRunStatus {
    Queued,
    Running,
    Backgrounded,
    WaitingPermission,
    TimedOut,
    Cancelled,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct ToolRunView {
    pub id: String,
    pub name: String,
    pub args_buffer: String,
    pub arguments: Option<Value>,
    pub status: ToolRunStatus,
    pub progress: Vec<String>,
    pub result_body: Option<String>,
    pub result_preview: Option<String>,
    pub metadata: Option<Value>,
    pub started_at: Instant,
    pub completed_at: Option<Instant>,
    /// Structured JSON data from the tool result (e.g. mutation_result).
    pub result_data: Option<Value>,
}

impl ToolRunView {
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            args_buffer: String::new(),
            arguments: None,
            status: ToolRunStatus::Queued,
            progress: Vec::new(),
            result_body: None,
            result_preview: None,
            metadata: None,
            result_data: None,
            started_at: Instant::now(),
            completed_at: None,
        }
    }

    pub fn push_args_delta(&mut self, delta: &str) {
        self.args_buffer.push_str(delta);
        self.arguments = serde_json::from_str(&self.args_buffer).ok();
    }

    pub fn mark_running(&mut self, name: String) {
        if !name.is_empty() {
            self.name = name;
        }
        self.status = ToolRunStatus::Running;
    }

    pub fn mark_waiting_permission(&mut self, name: String, arguments: Value) {
        if !name.is_empty() {
            self.name = name;
        }
        self.arguments = Some(arguments);
        self.status = ToolRunStatus::WaitingPermission;
    }

    pub fn push_progress(&mut self, progress: String) {
        self.status = ToolRunStatus::Running;
        if self.progress.last() != Some(&progress) {
            self.progress.push(progress);
        }
        const MAX_PROGRESS_LINES: usize = 6;
        if self.progress.len() > MAX_PROGRESS_LINES {
            let extra = self.progress.len() - MAX_PROGRESS_LINES;
            self.progress.drain(0..extra);
        }
    }

    pub fn mark_complete(&mut self, result: String) {
        self.mark_complete_with_metadata(result, None);
    }

    pub fn mark_complete_with_metadata(&mut self, result: String, metadata: Option<Value>) {
        let body = tool_result_body(&result).to_string();
        self.status = if let Some(status) = terminal_status_from_metadata(metadata.as_ref()) {
            status
        } else if body.contains("Started background shell command") {
            ToolRunStatus::Backgrounded
        } else if body.contains("Command timed out after")
            || body.contains(" is timed_out.")
            || body.contains(" is timed out.")
        {
            ToolRunStatus::TimedOut
        } else if body.contains(" is cancelled.") || body.contains(" is canceled.") {
            ToolRunStatus::Cancelled
        } else if result.contains("Result: ERROR") || result.contains("[Error:") {
            ToolRunStatus::Failed
        } else {
            ToolRunStatus::Completed
        };
        self.metadata = metadata;
        self.result_preview = Some(compact_line(&body, 180));
        self.result_body = Some(body);
        self.completed_at = Some(Instant::now());
    }

    pub fn elapsed(&self) -> Duration {
        self.completed_at.unwrap_or_else(Instant::now) - self.started_at
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            ToolRunStatus::Queued | ToolRunStatus::Running | ToolRunStatus::WaitingPermission
        )
    }

    pub fn tool_kind_label(&self) -> &'static str {
        match self.name.as_str() {
            "file_write" | "file_edit" | "file_patch" | "format" | "rewind" | "memory_save"
            | "memory_clear" | "copy" | "clear" | "refactor" => "Edit",
            "bash" | "powershell" | "repl" => "Shell",
            "file_read"
            | "git_status"
            | "git_diff"
            | "memory_load"
            | "skill_view"
            | "context"
            | "context_visualization"
            | "datetime"
            | "cost"
            | "telemetry"
            | "diff"
            | "lsp"
            | "symbol_query"
            | "brief"
            | "notebook"
            | "project_list"
            | "bash_output" => "Read",
            "glob" | "grep" | "web_search" | "json_query" | "tool_search" => "Search",
            "agent" | "todo_write" | "enter_plan_mode" | "exit_plan_mode" | "plan" | "ask_user"
            | "socratic_analyze" | "cron" | "swarm" | "task_create" | "task_get" | "task_list"
            | "task_update" | "task_stop" | "task_output" | "resume" | "run_tests"
            | "start_dev_server" | "send_message" | "share" | "team" | "workbench"
            | "skills_list" | "skill_manage" | "bash_cancel" | "bash_tasks" | "encode"
            | "desktop" | "browser" | "sleep" => "Task",
            "web_fetch" | "install_dependencies" | "github" => "Network",
            "mcp" | "mcp_tool" | "mcp_auth" | "list_mcp_resources" | "read_mcp_resource" => "Mcp",
            "plugin_list" | "plugin_manage" => "Plugin",
            _ => "",
        }
    }

    pub fn summary(&self) -> String {
        let args = self.arguments.as_ref();
        let kind_label = self.tool_kind_label();
        let text = match self.name.as_str() {
            "bash" => summarize_bash(args, self),
            "bash_output" => {
                terminal_summary(self, "Reading background shell", "Read background shell")
            }
            "bash_cancel" => terminal_summary(
                self,
                "Stopping background shell",
                "Stopped background shell",
            ),
            "bash_tasks" => terminal_summary(
                self,
                "Listing background shells",
                "Listed background shells",
            ),
            "powershell" => {
                summarize_command_tool("Running PowerShell", "Ran PowerShell", args, self)
            }
            "repl" => summarize_repl(args, self),
            "git" => summarize_git(args, self),
            "file_read" => summarize_file("Reading", "Read", args, "path", self),
            "file_write" => summarize_file("Writing", "Wrote", args, "path", self),
            "file_edit" => summarize_file("Editing", "Edited", args, "path", self),
            "file_patch" => summarize_file("Patching", "Patched", args, "path", self),
            "format" => summarize_file("Formatting", "Formatted", args, "file_path", self),
            "diff" => summarize_diff(args, self),
            "grep" => summarize_grep(args, self),
            "glob" => summarize_glob(args, self),
            "project_list" => summarize_project(args, self),
            "web_search" => summarize_query_tool("Searching web", "Searched web", args, self),
            "web_fetch" => summarize_url_tool("Fetching URL", "Fetched URL", args, self),
            "browser" | "desktop" => summarize_action_tool(args, self),
            "calculate" => summarize_query_tool("Calculating", "Calculated", args, self),
            "datetime" => terminal_summary(self, "Checking time", "Checked time"),
            "json_query" => summarize_json_query(args, self),
            "encode" => summarize_action_tool(args, self),
            "memory_save" => terminal_summary(self, "Saving memory", "Saved memory"),
            "memory_load" => terminal_summary(self, "Loading memory", "Loaded memory"),
            "agent" => terminal_summary(self, "Running sub-agent", "Completed sub-agent"),
            "swarm" => terminal_summary(self, "Coordinating agents", "Coordinated agents"),
            "mcp" | "mcp_tool" => summarize_mcp(args, self),
            _ => terminal_summary(
                self,
                &format!("Running {}", self.name),
                &format!("Ran {}", self.name),
            ),
        };
        if kind_label.is_empty() {
            text
        } else {
            format!("[{}] {}", kind_label, text)
        }
    }

    pub fn detail_line(&self) -> Option<String> {
        let args = self.arguments.as_ref()?;
        match self.name.as_str() {
            "bash" => {
                string_arg(args, "command").map(|cmd| format!("└ $ {}", compact_line(cmd, 120)))
            }
            "bash_output" | "bash_cancel" => string_arg(args, "handle")
                .map(|handle| format!("└ handle: {}", compact_line(handle, 120))),
            "powershell" => first_string_arg(args, &["command", "script_path"])
                .map(|cmd| format!("└ {}", compact_line(cmd, 120))),
            "repl" => first_string_arg(args, &["code", "command"])
                .map(|code| format!("└ {}", compact_line(code, 120))),
            "file_read" | "file_write" | "file_edit" | "format" | "diff" => {
                first_string_arg(args, &["path", "file_path"])
                    .map(|path| format!("└ {}", compact_line(path, 120)))
            }
            "grep" => string_arg(args, "pattern")
                .map(|pattern| format!("└ pattern: {}", compact_line(pattern, 100))),
            "glob" => string_arg(args, "pattern")
                .map(|pattern| format!("└ pattern: {}", compact_line(pattern, 100))),
            "project_list" => string_arg(args, "query")
                .map(|query| format!("└ query: {}", compact_line(query, 100))),
            "mcp" | "mcp_tool" => {
                let server = string_arg(args, "server_name").unwrap_or("server");
                let tool = string_arg(args, "tool_name").unwrap_or("tool");
                Some(format!("└ {}:{}", server, tool))
            }
            "git" | "browser" | "desktop" | "encode" | "datetime" => string_arg(args, "action")
                .map(|action| format!("└ action: {}", compact_line(action, 100))),
            "web_search" | "calculate" => first_string_arg(args, &["query", "expression"])
                .map(|query| format!("└ query: {}", compact_line(query, 100))),
            "web_fetch" => first_string_arg(args, &["url", "target"])
                .map(|url| format!("└ {}", compact_line(url, 120))),
            "json_query" => {
                string_arg(args, "path").map(|path| format!("└ path: {}", compact_line(path, 100)))
            }
            _ => None,
        }
    }

    pub fn render_lines(&self, expanded: bool) -> Vec<String> {
        let mut lines = Vec::new();
        let status_hint = match self.status {
            ToolRunStatus::Queued => "waiting",
            ToolRunStatus::Running => "running",
            ToolRunStatus::Backgrounded => "backgrounded",
            ToolRunStatus::WaitingPermission => "waiting for permission",
            ToolRunStatus::TimedOut => "timed out",
            ToolRunStatus::Cancelled => "cancelled",
            ToolRunStatus::Completed => "done",
            ToolRunStatus::Failed => "failed",
        };

        let details_hint = if expanded {
            "ctrl+o collapse"
        } else {
            "ctrl+o details"
        };
        let elapsed = if self.elapsed().as_secs() > 0 {
            format!(" · {}s", self.elapsed().as_secs())
        } else {
            String::new()
        };
        let mut title = format!("{} · {}{}", self.summary(), status_hint, elapsed);
        if !expanded {
            title = if self.status == ToolRunStatus::Running {
                format!("{} · running{}", self.summary(), elapsed)
            } else if self.status == ToolRunStatus::WaitingPermission {
                format!("{} · permission needed", self.summary())
            } else if self.status == ToolRunStatus::Queued {
                format!("{} · waiting", self.summary())
            } else if self.result_body.is_some() {
                let output_stats = self
                    .result_stats()
                    .unwrap_or_else(|| "output: unavailable".to_string());
                format!(
                    "{} · {}{} · {} · {} · ctrl+t output",
                    self.summary(),
                    status_hint,
                    elapsed,
                    output_stats,
                    details_hint
                )
            } else {
                title
            };
        }
        lines.push(title);

        if let Some(detail) = self.detail_line() {
            lines.push(format!("  {}", detail));
        }

        if expanded {
            let kind = self.tool_kind_label();
            let kind_info = if kind.is_empty() {
                String::new()
            } else {
                format!(" ({})", kind)
            };
            lines.push(format!("  ├ tool: {}{}", self.name, kind_info));
            lines.push(format!("  ├ status: {}", status_hint));
            lines.push(format!("  ├ elapsed: {}s", self.elapsed().as_secs()));
            if let Some(arguments) = self.arguments.as_ref() {
                let arg_lines = compact_json_lines(arguments, 4, 112);
                if !arg_lines.is_empty() {
                    lines.push("  ├ arguments:".to_string());
                    for (idx, arg_line) in arg_lines.iter().enumerate() {
                        let branch = if idx + 1 == arg_lines.len() {
                            "  │ └"
                        } else {
                            "  │ ├"
                        };
                        lines.push(format!("{} {}", branch, arg_line));
                    }
                }
            }
            if let Some(stats) = self.result_stats() {
                lines.push(format!("  ├ {}", stats));
            }
            for progress in &self.progress {
                lines.push(format!("  ├ {}", compact_line(progress, 140)));
            }
            if let Some(body) = &self.result_body {
                const RESULT_PREVIEW_LINES: usize = 5;
                let result_lines = compact_text_lines(body, RESULT_PREVIEW_LINES, 132);
                if result_lines.is_empty() {
                    lines.push("  └ result: empty".to_string());
                } else {
                    let result_title = if non_empty_line_count(body) > RESULT_PREVIEW_LINES {
                        "  └ result: (ctrl+t full output)"
                    } else {
                        "  └ result:"
                    };
                    lines.push(result_title.to_string());
                    for result_line in result_lines {
                        lines.push(format!("    {}", result_line));
                    }
                }
            } else if self.progress.is_empty() {
                lines.push("  └ no output yet".to_string());
            }
        }

        lines
    }

    pub fn result_stats(&self) -> Option<String> {
        let body = self.result_body.as_ref()?;
        let line_count = non_empty_line_count(body);
        let char_count = body.chars().count();
        if line_count == 0 && char_count == 0 {
            Some("output: empty".to_string())
        } else {
            let line_unit = if line_count == 1 { "line" } else { "lines" };
            let char_unit = if char_count == 1 { "char" } else { "chars" };
            Some(format!(
                "output: {} {}, {} {}",
                line_count, line_unit, char_count, char_unit
            ))
        }
    }

    pub fn full_details(&self) -> String {
        let mut sections = vec![
            format!("Tool: {}", self.name),
            format!("Status: {:?}", self.status),
            format!("Elapsed: {}s", self.elapsed().as_secs()),
        ];

        if let Some(arguments) = self.arguments.as_ref() {
            sections.push("".to_string());
            sections.push("Arguments:".to_string());
            sections.push(
                serde_json::to_string_pretty(arguments).unwrap_or_else(|_| arguments.to_string()),
            );
        }

        if !self.progress.is_empty() {
            sections.push("".to_string());
            sections.push("Progress:".to_string());
            sections.extend(self.progress.iter().map(|line| format!("- {}", line)));
        }

        sections.push("".to_string());
        sections.push("Result:".to_string());
        sections.push(
            self.result_body
                .clone()
                .unwrap_or_else(|| "(no output yet)".to_string()),
        );

        sections.join("\n")
    }
}

fn terminal_status_from_metadata(metadata: Option<&Value>) -> Option<ToolRunStatus> {
    let task = metadata?.get("terminal_task")?;
    let status = task.get("status").and_then(Value::as_str)?;
    match status {
        "running" => {
            let terminal_kind = task.get("terminal_kind").and_then(Value::as_str);
            if terminal_kind == Some("background_shell") {
                Some(ToolRunStatus::Backgrounded)
            } else {
                Some(ToolRunStatus::Running)
            }
        }
        "completed" => Some(ToolRunStatus::Completed),
        "failed" => Some(ToolRunStatus::Failed),
        "timed_out" => Some(ToolRunStatus::TimedOut),
        "cancelled" | "canceled" => Some(ToolRunStatus::Cancelled),
        _ => None,
    }
}

pub fn render_tool_runs(runs: &[ToolRunView], expanded: bool) -> String {
    if runs.is_empty() {
        return String::new();
    }
    runs.iter()
        .flat_map(|run| run.render_lines(expanded))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn upsert_tool_run(runs: &mut Vec<ToolRunView>, id: String, name: String) {
    if let Some(run) = runs.iter_mut().find(|run| run.id == id) {
        if !name.is_empty() {
            run.name = name;
        }
    } else {
        runs.push(ToolRunView::new(id, name));
    }
}

pub fn with_tool_run<F>(runs: &mut [ToolRunView], id: &str, f: F)
where
    F: FnOnce(&mut ToolRunView),
{
    if let Some(run) = runs.iter_mut().find(|run| run.id == id) {
        f(run);
    }
}

fn summarize_bash(args: Option<&Value>, run: &ToolRunView) -> String {
    let Some(command) = args.and_then(|args| string_arg(args, "command")) else {
        return terminal_summary(run, "Running shell command", "Ran shell command");
    };
    let command = command.trim();
    if args
        .and_then(|args| string_arg(args, "mode"))
        .is_some_and(|mode| mode == "pty")
    {
        return terminal_summary(run, "Running PTY command", "Ran PTY command");
    }
    let classification = crate::tools::bash_tool::command_classifier::classify_command(command);
    let listed_count = (classification.category
        == crate::tools::bash_tool::command_classifier::ShellCommandCategory::List
        && run.status == ToolRunStatus::Completed)
        .then(|| run.result_body.as_deref().map(count_ls_entries))
        .flatten();
    if let Some(count) = listed_count {
        return format!("Listed {} items", count);
    }
    match classification.validation_family {
        Some(crate::tools::bash_tool::command_classifier::ValidationFamily::CargoTest)
        | Some(crate::tools::bash_tool::command_classifier::ValidationFamily::NpmTest)
        | Some(crate::tools::bash_tool::command_classifier::ValidationFamily::PnpmTest)
        | Some(crate::tools::bash_tool::command_classifier::ValidationFamily::YarnTest)
        | Some(crate::tools::bash_tool::command_classifier::ValidationFamily::Pytest)
        | Some(crate::tools::bash_tool::command_classifier::ValidationFamily::PythonUnittest)
        | Some(crate::tools::bash_tool::command_classifier::ValidationFamily::GoTest) => {
            terminal_summary(run, "Running tests", "Ran tests")
        }
        Some(crate::tools::bash_tool::command_classifier::ValidationFamily::CargoCheck)
        | Some(crate::tools::bash_tool::command_classifier::ValidationFamily::CargoClippy)
        | Some(crate::tools::bash_tool::command_classifier::ValidationFamily::CargoFmtCheck) => {
            terminal_summary(run, "Running cargo", "Ran cargo")
        }
        Some(crate::tools::bash_tool::command_classifier::ValidationFamily::RgAssertion) => {
            summarize_search_terminal(run, "Running search assertion", "Ran search assertion")
        }
        Some(crate::tools::bash_tool::command_classifier::ValidationFamily::ShellAssertion) => {
            terminal_summary(run, "Running shell assertion", "Ran shell assertion")
        }
        Some(crate::tools::bash_tool::command_classifier::ValidationFamily::BashSyntax)
        | Some(crate::tools::bash_tool::command_classifier::ValidationFamily::PythonCompile)
        | Some(crate::tools::bash_tool::command_classifier::ValidationFamily::ProjectScript)
        | Some(crate::tools::bash_tool::command_classifier::ValidationFamily::NodeScript) => {
            terminal_summary(run, "Running validation", "Ran validation")
        }
        None => match classification.category {
            crate::tools::bash_tool::command_classifier::ShellCommandCategory::List => {
                terminal_summary(run, "Listing directory", "Listed directory")
            }
            crate::tools::bash_tool::command_classifier::ShellCommandCategory::Search => {
                summarize_search_terminal(run, "Searching files", "Searched files")
            }
            crate::tools::bash_tool::command_classifier::ShellCommandCategory::Read => {
                summarize_search_terminal(run, "Reading file", "Read file")
            }
            crate::tools::bash_tool::command_classifier::ShellCommandCategory::PackageInstall => {
                terminal_summary(run, "Installing package", "Installed package")
            }
            crate::tools::bash_tool::command_classifier::ShellCommandCategory::DevServer => {
                terminal_summary(run, "Starting dev server", "Started dev server")
            }
            crate::tools::bash_tool::command_classifier::ShellCommandCategory::Interactive => {
                terminal_summary(
                    run,
                    "Checking terminal requirement",
                    "Checked terminal requirement",
                )
            }
            crate::tools::bash_tool::command_classifier::ShellCommandCategory::GitMutation => {
                terminal_summary(run, "Running git command", "Ran git command")
            }
            crate::tools::bash_tool::command_classifier::ShellCommandCategory::FileMutation => {
                terminal_summary(run, "Running shell mutation", "Ran shell mutation")
            }
            crate::tools::bash_tool::command_classifier::ShellCommandCategory::Destructive => {
                terminal_summary(run, "Reviewing shell command", "Ran shell command")
            }
            _ => terminal_summary(run, "Running shell command", "Ran shell command"),
        },
    }
}

fn summarize_file(
    action: &str,
    completed_action: &str,
    args: Option<&Value>,
    key: &str,
    run: &ToolRunView,
) -> String {
    // Phase 1 (opencode alignment): prefer mutation_result.ui_summary when available.
    if run.status == ToolRunStatus::Completed {
        if let Some(ref data) = run.result_data {
            if let Some(mr) = crate::tools::file_tool::mutation_result::from_tool_data(data) {
                return mr.ui_summary;
            }
        }
    }
    let Some(path) = args.and_then(|args| string_arg(args, key)) else {
        let fallback = args.and_then(|args| first_string_arg(args, &["path", "file_path"]));
        if let Some(path) = fallback {
            let verb = if run.status == ToolRunStatus::Completed {
                completed_action
            } else {
                action
            };
            return format!("{} {}", verb, display_name(path));
        }
        return terminal_summary(
            run,
            &format!("{} file", action),
            &format!("{} file", completed_action),
        );
    };
    let verb = if run.status == ToolRunStatus::Completed {
        completed_action
    } else {
        action
    };
    format!("{} {}", verb, display_name(path))
}

fn summarize_command_tool(
    active: &str,
    completed: &str,
    args: Option<&Value>,
    run: &ToolRunView,
) -> String {
    let base = terminal_summary(run, active, completed);
    let Some(cmd) = args.and_then(|args| first_string_arg(args, &["command", "script_path"]))
    else {
        return base;
    };
    if run.is_active() {
        format!("{} {}", base, compact_line(cmd, 32))
    } else {
        base
    }
}

fn summarize_repl(args: Option<&Value>, run: &ToolRunView) -> String {
    let language = args
        .and_then(|args| string_arg(args, "language"))
        .unwrap_or("code");
    terminal_summary(
        run,
        &format!("Running {} REPL", language),
        &format!("Ran {} REPL", language),
    )
}

fn summarize_git(args: Option<&Value>, run: &ToolRunView) -> String {
    let action = args
        .and_then(|args| string_arg(args, "action"))
        .unwrap_or("command");
    let active = match action {
        "status" => "Checking git status".to_string(),
        "diff" => "Reading git diff".to_string(),
        "log" => "Reading git log".to_string(),
        "add" => "Staging files".to_string(),
        "commit" => "Creating git commit".to_string(),
        _ => format!("Running git {}", action),
    };
    let completed = match action {
        "status" => "Checked git status".to_string(),
        "diff" => "Read git diff".to_string(),
        "log" => "Read git log".to_string(),
        "add" => "Staged files".to_string(),
        "commit" => "Created git commit".to_string(),
        _ => format!("Ran git {}", action),
    };
    terminal_summary(run, &active, &completed)
}

fn summarize_diff(args: Option<&Value>, run: &ToolRunView) -> String {
    let action = args
        .and_then(|args| string_arg(args, "action"))
        .unwrap_or("diff");
    terminal_summary(
        run,
        &format!("Preparing {} diff", action),
        &format!("Prepared {} diff", action),
    )
}

fn summarize_query_tool(
    active: &str,
    completed: &str,
    args: Option<&Value>,
    run: &ToolRunView,
) -> String {
    let base = terminal_summary(run, active, completed);
    let Some(query) = args.and_then(|args| first_string_arg(args, &["query", "expression"])) else {
        return base;
    };
    if run.is_active() {
        format!("{} {}", base, compact_line(query, 36))
    } else {
        base
    }
}

fn summarize_url_tool(
    active: &str,
    completed: &str,
    args: Option<&Value>,
    run: &ToolRunView,
) -> String {
    let base = terminal_summary(run, active, completed);
    let Some(url) = args.and_then(|args| first_string_arg(args, &["url", "target"])) else {
        return base;
    };
    if run.is_active() {
        format!("{} {}", base, compact_line(url, 42))
    } else {
        base
    }
}

fn summarize_action_tool(args: Option<&Value>, run: &ToolRunView) -> String {
    let action = args
        .and_then(|args| string_arg(args, "action"))
        .unwrap_or("action");
    terminal_summary(
        run,
        &format!("Running {}", action),
        &format!("Ran {}", action),
    )
}

fn summarize_json_query(args: Option<&Value>, run: &ToolRunView) -> String {
    let action = args
        .and_then(|args| string_arg(args, "action"))
        .unwrap_or("query");
    terminal_summary(
        run,
        &format!("Querying JSON {}", action),
        &format!("Queried JSON {}", action),
    )
}

fn summarize_grep(args: Option<&Value>, run: &ToolRunView) -> String {
    let pattern = args
        .and_then(|args| string_arg(args, "pattern"))
        .unwrap_or("");
    if run.status == ToolRunStatus::Completed {
        let matches = run
            .result_body
            .as_deref()
            .map(non_empty_line_count)
            .unwrap_or_default();
        return format!("Found {} matches", matches);
    }
    if pattern.is_empty() {
        "Searching text".to_string()
    } else {
        format!("Searching {}", compact_line(pattern, 40))
    }
}

fn summarize_glob(args: Option<&Value>, run: &ToolRunView) -> String {
    let pattern = args
        .and_then(|args| string_arg(args, "pattern"))
        .unwrap_or("");
    if run.status == ToolRunStatus::Completed {
        let files = run
            .result_body
            .as_deref()
            .map(non_empty_line_count)
            .unwrap_or_default();
        return format!("Found {} files", files);
    }
    if pattern.is_empty() {
        "Finding files".to_string()
    } else {
        format!("Finding {}", compact_line(pattern, 40))
    }
}

fn summarize_project(args: Option<&Value>, run: &ToolRunView) -> String {
    let action = args
        .and_then(|args| string_arg(args, "action"))
        .unwrap_or("list");
    if run.status == ToolRunStatus::Completed {
        return match action {
            "summary" => "Summarized project".to_string(),
            "search" => summarize_search_terminal(run, "Searching project", "Searched project"),
            "dir" => "Listed project directory".to_string(),
            "refresh" => "Refreshed project index".to_string(),
            _ => "Listed project files".to_string(),
        };
    }
    match action {
        "summary" => "Summarizing project".to_string(),
        "search" => "Searching project".to_string(),
        "dir" => "Listing project directory".to_string(),
        "refresh" => "Refreshing project index".to_string(),
        _ => "Listing project files".to_string(),
    }
}

fn summarize_mcp(args: Option<&Value>, run: &ToolRunView) -> String {
    let tool = args
        .and_then(|args| string_arg(args, "tool_name"))
        .unwrap_or("");
    if run.status == ToolRunStatus::Completed {
        if tool.is_empty() {
            return "Used MCP tool".to_string();
        }
        return format!("Used MCP {}", compact_line(tool, 40));
    }
    if tool.is_empty() {
        "Using MCP tool".to_string()
    } else {
        format!("Using MCP {}", compact_line(tool, 40))
    }
}

fn string_arg<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn first_string_arg<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| string_arg(value, key))
}

fn display_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

fn terminal_summary(run: &ToolRunView, active: &str, completed: &str) -> String {
    match run.status {
        ToolRunStatus::Completed => completed.to_string(),
        ToolRunStatus::Backgrounded => format!("{} in background", completed),
        ToolRunStatus::TimedOut => format!("{} timed out", completed),
        ToolRunStatus::Cancelled => format!("{} cancelled", completed),
        ToolRunStatus::Failed => format!("{} failed", completed),
        ToolRunStatus::WaitingPermission => format!("Waiting to {}", active.to_ascii_lowercase()),
        ToolRunStatus::Queued | ToolRunStatus::Running => active.to_string(),
    }
}

fn summarize_search_terminal(run: &ToolRunView, active: &str, completed: &str) -> String {
    match run.status {
        ToolRunStatus::Completed => {
            let lines = run
                .result_body
                .as_deref()
                .map(non_empty_line_count)
                .unwrap_or_default();
            format!("{} ({} lines)", completed, lines)
        }
        ToolRunStatus::Backgrounded => format!("{} in background", completed),
        ToolRunStatus::TimedOut => format!("{} timed out", completed),
        ToolRunStatus::Cancelled => format!("{} cancelled", completed),
        ToolRunStatus::Failed => format!("{} failed", completed),
        _ => active.to_string(),
    }
}

fn tool_result_body(result: &str) -> &str {
    result
        .strip_prefix("Result: OK\n")
        .or_else(|| result.strip_prefix("Result: ERROR\n"))
        .unwrap_or(result)
        .trim()
}

fn non_empty_line_count(text: &str) -> usize {
    text.lines().filter(|line| !line.trim().is_empty()).count()
}

fn count_ls_entries(text: &str) -> usize {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty()
                && !trimmed.starts_with("total ")
                && !trimmed.ends_with(" .")
                && !trimmed.ends_with(" ..")
        })
        .count()
}

fn compact_line(text: &str, max_chars: usize) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    if line.chars().count() <= max_chars {
        line.to_string()
    } else {
        format!(
            "{}…",
            line.chars()
                .take(max_chars.saturating_sub(1))
                .collect::<String>()
        )
    }
}

fn compact_json_lines(value: &Value, max_lines: usize, max_chars: usize) -> Vec<String> {
    let rendered = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
    compact_text_lines(&rendered, max_lines, max_chars)
}

fn compact_text_lines(text: &str, max_lines: usize, max_chars: usize) -> Vec<String> {
    let mut lines = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(max_lines)
        .map(|line| compact_line(line, max_chars))
        .collect::<Vec<_>>();
    let total = non_empty_line_count(text);
    if total > max_lines {
        lines.push(format!("… {} more lines", total - max_lines));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bash_ls_summary_counts_entries_after_completion() {
        let mut run = ToolRunView::new("tool_1".to_string(), "bash".to_string());
        run.arguments = Some(json!({ "command": "ls -la ~/Desktop" }));
        assert_eq!(run.summary(), "[Shell] Listing directory");

        run.mark_complete(
            "Result: OK\ntotal 8\ndrwxr-xr-x  .\ndrwxr-xr-x  ..\n-rw-r--r--  a.txt\ndrwxr-xr-x  project\n"
                .to_string(),
        );

        assert_eq!(run.summary(), "[Shell] Listed 2 items");
        assert_eq!(
            run.result_stats().as_deref(),
            Some("output: 5 lines, 74 chars")
        );
    }

    #[test]
    fn bash_summary_uses_shared_shell_categories() {
        let mut install = ToolRunView::new("tool_install".to_string(), "bash".to_string());
        install.arguments = Some(json!({ "command": "pip3 install pygame" }));
        assert_eq!(install.summary(), "[Shell] Installing package");

        let mut dev = ToolRunView::new("tool_dev".to_string(), "bash".to_string());
        dev.arguments = Some(json!({ "command": "npm run dev" }));
        assert_eq!(dev.summary(), "[Shell] Starting dev server");

        let mut search = ToolRunView::new("tool_search".to_string(), "bash".to_string());
        search.arguments = Some(json!({ "command": "rg TODO src" }));
        assert_eq!(search.summary(), "[Shell] Searching files");
    }

    #[test]
    fn shell_lifecycle_statuses_render_background_timeout_and_cancel() {
        let mut background = ToolRunView::new("tool_bg".to_string(), "bash".to_string());
        background.arguments = Some(json!({ "command": "npm run dev", "mode": "background" }));
        background.mark_complete(
            "Result: OK\nStarted background shell command.\nHandle: shell_123\nStatus: running."
                .to_string(),
        );
        assert_eq!(background.status, ToolRunStatus::Backgrounded);
        assert!(background.render_lines(false)[0].contains("backgrounded"));

        let mut timed_out = ToolRunView::new("tool_timeout".to_string(), "bash".to_string());
        timed_out.arguments = Some(json!({ "command": "sleep 60" }));
        timed_out.mark_complete("Result: ERROR\nCommand timed out after 1 seconds".to_string());
        assert_eq!(timed_out.status, ToolRunStatus::TimedOut);
        assert!(timed_out.render_lines(false)[0].contains("timed out"));

        let mut cancelled = ToolRunView::new("tool_cancel".to_string(), "bash_cancel".to_string());
        cancelled.arguments = Some(json!({ "handle": "shell_123" }));
        cancelled.mark_complete("Result: OK\nBackground shell shell_123 is cancelled.".to_string());
        assert_eq!(cancelled.status, ToolRunStatus::Cancelled);
        assert!(cancelled.render_lines(false)[0].contains("cancelled"));
    }

    #[test]
    fn shell_lifecycle_prefers_terminal_task_metadata() {
        let mut background = ToolRunView::new("tool_bg".to_string(), "bash".to_string());
        background.mark_complete_with_metadata(
            "Result: OK\nStarted shell\n".to_string(),
            Some(json!({
                "terminal_task": {
                    "task_id": "shell_bg_1",
                    "status": "running",
                    "terminal_kind": "background_shell"
                }
            })),
        );

        assert_eq!(background.status, ToolRunStatus::Backgrounded);
        assert_eq!(
            background.metadata.as_ref().unwrap()["terminal_task"]["task_id"],
            "shell_bg_1"
        );

        let mut pty = ToolRunView::new("tool_pty".to_string(), "bash".to_string());
        pty.mark_complete_with_metadata(
            "Result: OK\n".to_string(),
            Some(json!({
                "terminal_task": {
                    "task_id": "shell_pty_1",
                    "status": "completed",
                    "terminal_kind": "pty_shell"
                }
            })),
        );

        assert_eq!(pty.status, ToolRunStatus::Completed);
    }

    #[test]
    fn grep_summary_reports_matches_after_completion() {
        let mut run = ToolRunView::new("tool_2".to_string(), "grep".to_string());
        run.arguments = Some(json!({ "pattern": "ToolRunView" }));

        run.mark_complete("Result: OK\nsrc/tui/tool_view.rs:struct ToolRunView\n".to_string());

        assert_eq!(run.summary(), "[Search] Found 1 matches");
        assert!(run.render_lines(false)[0].contains("done"));
        assert!(run.render_lines(false)[0].contains("output: 1 line"));
        assert!(run.render_lines(false)[0].contains("ctrl+t output"));
    }

    #[test]
    fn expanded_lines_include_progress_and_result_stats() {
        let mut run = ToolRunView::new("tool_3".to_string(), "file_read".to_string());
        run.arguments = Some(json!({ "path": "/tmp/example.txt" }));
        run.push_progress("Reading file...".to_string());
        run.mark_complete("Result: OK\nhello\nworld\n".to_string());

        let lines = run.render_lines(true);

        assert!(lines.iter().any(|line| line.contains("output: 2 lines")));
        assert!(lines.iter().any(|line| line.contains("arguments:")));
        assert!(lines.iter().any(|line| line.contains("Reading file")));
        assert!(lines.iter().any(|line| line.contains("result:")));
        assert!(lines.iter().any(|line| line.contains("hello")));
    }

    #[test]
    fn file_path_tools_use_file_path_argument() {
        let mut run = ToolRunView::new("tool_4".to_string(), "format".to_string());
        run.arguments = Some(json!({ "file_path": "src/main.rs" }));

        assert_eq!(run.summary(), "[Edit] Formatting main.rs");
        assert_eq!(run.detail_line().as_deref(), Some("└ src/main.rs"));

        run.mark_complete("Result: OK\nFormatted src/main.rs using rustfmt\n".to_string());
        assert_eq!(run.summary(), "[Edit] Formatted main.rs");
    }

    #[test]
    fn git_action_summary_tracks_completion() {
        let mut run = ToolRunView::new("tool_5".to_string(), "git".to_string());
        run.arguments = Some(json!({ "action": "status" }));

        assert_eq!(run.summary(), "Checking git status");

        run.mark_complete("Result: OK\nOn branch main\nnothing to commit\n".to_string());
        assert_eq!(run.summary(), "Checked git status");
    }

    #[test]
    fn expanded_long_result_points_to_full_output_viewer() {
        let mut run = ToolRunView::new("tool_6".to_string(), "bash".to_string());
        run.arguments = Some(json!({ "command": "printf lines" }));
        run.mark_complete("Result: OK\none\ntwo\nthree\nfour\nfive\nsix\n".to_string());

        let lines = run.render_lines(true);

        assert!(lines.iter().any(|line| line.contains("ctrl+t full output")));
    }
}
