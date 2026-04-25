use serde_json::Value;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRunStatus {
    Queued,
    Running,
    WaitingPermission,
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
    pub started_at: Instant,
    pub completed_at: Option<Instant>,
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
        self.status = if result.contains("Result: ERROR") || result.contains("[Error:") {
            ToolRunStatus::Failed
        } else {
            ToolRunStatus::Completed
        };
        let body = tool_result_body(&result).to_string();
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

    pub fn summary(&self) -> String {
        let args = self.arguments.as_ref();
        match self.name.as_str() {
            "bash" => summarize_bash(args, self),
            "powershell" => {
                summarize_command_tool("Running PowerShell", "Ran PowerShell", args, self)
            }
            "repl" => summarize_repl(args, self),
            "git" => summarize_git(args, self),
            "file_read" => summarize_file("Reading", "Read", args, "path", self),
            "file_write" => summarize_file("Writing", "Wrote", args, "path", self),
            "file_edit" => summarize_file("Editing", "Edited", args, "path", self),
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
        }
    }

    pub fn detail_line(&self) -> Option<String> {
        let args = self.arguments.as_ref()?;
        match self.name.as_str() {
            "bash" => {
                string_arg(args, "command").map(|cmd| format!("└ $ {}", compact_line(cmd, 120)))
            }
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
            ToolRunStatus::WaitingPermission => "waiting for permission",
            ToolRunStatus::Completed => "done",
            ToolRunStatus::Failed => "failed",
        };

        let expand_hint = if expanded {
            "ctrl+o to collapse"
        } else {
            "ctrl+o to expand"
        };
        let elapsed = if self.elapsed().as_secs() > 0 {
            format!(" · {}s", self.elapsed().as_secs())
        } else {
            String::new()
        };
        let mut title = format!("{} ({}{})", self.summary(), status_hint, elapsed);
        if !expanded {
            title = if self.is_active() {
                format!("{} ({})", self.summary(), expand_hint)
            } else if self.result_body.is_some() {
                format!("{} ({} · {})", self.summary(), status_hint, expand_hint)
            } else {
                title
            };
        }
        lines.push(title);

        if let Some(detail) = self.detail_line() {
            lines.push(format!("  {}", detail));
        }

        if expanded {
            lines.push(format!("  ├ tool: {}", self.name));
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
                let result_lines = compact_text_lines(body, 5, 132);
                if result_lines.is_empty() {
                    lines.push("  └ result: empty".to_string());
                } else {
                    lines.push("  └ result:".to_string());
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
            Some(format!(
                "output: {} lines, {} chars",
                line_count, char_count
            ))
        }
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

pub fn with_tool_run<F>(runs: &mut Vec<ToolRunView>, id: &str, f: F)
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
    if command.starts_with("ls ") || command == "ls" {
        if run.status == ToolRunStatus::Completed {
            if let Some(count) = run.result_body.as_deref().map(count_ls_entries) {
                return format!("Listed {} items", count);
            }
        }
        terminal_summary(run, "Listing directory", "Listed directory")
    } else if command.starts_with("rg ") || command.starts_with("grep ") {
        summarize_search_terminal(run, "Searching files", "Searched files")
    } else if command.starts_with("git ") {
        terminal_summary(run, "Running git command", "Ran git command")
    } else if command.starts_with("cat ") || command.starts_with("sed ") {
        summarize_search_terminal(run, "Reading file", "Read file")
    } else if command.starts_with("cargo test") {
        terminal_summary(run, "Running tests", "Ran tests")
    } else if command.starts_with("cargo ") {
        terminal_summary(run, "Running cargo", "Ran cargo")
    } else {
        terminal_summary(run, "Running shell command", "Ran shell command")
    }
}

fn summarize_file(
    action: &str,
    completed_action: &str,
    args: Option<&Value>,
    key: &str,
    run: &ToolRunView,
) -> String {
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
        assert_eq!(run.summary(), "Listing directory");

        run.mark_complete(
            "Result: OK\ntotal 8\ndrwxr-xr-x  .\ndrwxr-xr-x  ..\n-rw-r--r--  a.txt\ndrwxr-xr-x  project\n"
                .to_string(),
        );

        assert_eq!(run.summary(), "Listed 2 items");
        assert_eq!(
            run.result_stats().as_deref(),
            Some("output: 5 lines, 74 chars")
        );
    }

    #[test]
    fn grep_summary_reports_matches_after_completion() {
        let mut run = ToolRunView::new("tool_2".to_string(), "grep".to_string());
        run.arguments = Some(json!({ "pattern": "ToolRunView" }));

        run.mark_complete("Result: OK\nsrc/tui/tool_view.rs:struct ToolRunView\n".to_string());

        assert_eq!(run.summary(), "Found 1 matches");
        assert!(run.render_lines(false)[0].contains("done"));
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

        assert_eq!(run.summary(), "Formatting main.rs");
        assert_eq!(run.detail_line().as_deref(), Some("└ src/main.rs"));

        run.mark_complete("Result: OK\nFormatted src/main.rs using rustfmt\n".to_string());
        assert_eq!(run.summary(), "Formatted main.rs");
    }

    #[test]
    fn git_action_summary_tracks_completion() {
        let mut run = ToolRunView::new("tool_5".to_string(), "git".to_string());
        run.arguments = Some(json!({ "action": "status" }));

        assert_eq!(run.summary(), "Checking git status");

        run.mark_complete("Result: OK\nOn branch main\nnothing to commit\n".to_string());
        assert_eq!(run.summary(), "Checked git status");
    }
}
