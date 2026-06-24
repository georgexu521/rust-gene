use super::{extract_path_patterns, shell_mutation_paths, shell_redirection_facts, shell_tokens};

/// Product-facing structured view of a shell command for permission explain
/// and TUI display.
#[derive(Debug, Clone)]
pub struct ShellCommandView {
    pub primary_command: String,
    pub normalized: String,
    pub detected_paths: Vec<String>,
    pub cwd_changing: bool,
    pub cwd_targets: Vec<String>,
    pub mutation_family: String,
    pub has_write_redirection: bool,
    pub write_targets: Vec<String>,
    pub dynamic_segments: Vec<String>,
    pub recommended_tool: Option<String>,
    pub warnings: Vec<String>,
}

impl ShellCommandView {
    pub fn from_command(raw: &str) -> Self {
        let shell_tokens_vec = shell_tokens(raw);
        let primary = shell_tokens_vec.first().cloned().unwrap_or_default();
        let paths = extract_path_patterns(raw);
        let redir = shell_redirection_facts(raw);
        let has_write_redir = redir
            .iter()
            .any(|r| !matches!(r.operator.as_str(), "<" | "2>"));
        let write_targets: Vec<String> = redir
            .iter()
            .filter(|r| !matches!(r.operator.as_str(), "<" | "2>"))
            .filter_map(|r| r.target.clone())
            .collect();
        let mutation_paths = shell_mutation_paths(raw, &redir);

        let mutation_family = if raw.contains("sed -i") || raw.contains("perl -pi") {
            "inline_edit"
        } else if raw.contains("rm ") || raw.contains("rmdir") {
            "delete"
        } else if mutation_paths.is_empty() {
            "read_only"
        } else {
            "file_write"
        }
        .to_string();

        let recommended_tool = match mutation_family.as_str() {
            "inline_edit" => Some("file_edit".to_string()),
            "file_write" => Some("file_write or file_edit".to_string()),
            _ => None,
        };

        let warnings = if has_write_redir {
            vec!["This command writes to files via redirection.".to_string()]
        } else {
            Vec::new()
        };

        Self {
            primary_command: primary,
            normalized: raw.trim().to_string(),
            detected_paths: paths,
            cwd_changing: raw.contains("cd "),
            cwd_targets: shell_tokens_vec
                .iter()
                .skip_while(|t| t.as_str() != "cd")
                .skip(1)
                .filter(|t| !t.is_empty() && !t.contains('-'))
                .filter_map(|t| if t.is_empty() { None } else { Some(t.clone()) })
                .collect(),
            mutation_family,
            has_write_redirection: has_write_redir,
            write_targets,
            dynamic_segments: Vec::new(),
            recommended_tool,
            warnings,
        }
    }

    pub fn format_summary(&self) -> String {
        let mut lines = vec![
            format!("primary_cmd: {}", self.primary_command),
            format!("mutation_family: {}", self.mutation_family),
        ];
        if !self.detected_paths.is_empty() {
            lines.push(format!("paths: {}", self.detected_paths.join(", ")));
        }
        if self.cwd_changing {
            lines.push(format!("cwd_change: {}", self.cwd_targets.join(", ")));
        }
        if self.has_write_redirection {
            lines.push(format!("write_targets: {}", self.write_targets.join(", ")));
        }
        if let Some(tool) = &self.recommended_tool {
            lines.push(format!("suggested_tool: {tool}"));
        }
        for w in &self.warnings {
            lines.push(format!("warning: {w}"));
        }
        lines.join("\n")
    }
}
