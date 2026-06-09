//! Shell AST parser using tree-sitter-bash.
//!
//! This module provides a **shadow** parser that extracts structured
//! observations from bash commands.  It is **not** used to gate or
//! modify command execution — it only produces diagnostic metadata.
//!
//! When parsing fails (unsupported syntax, WASM unavailability, etc.)
//! the parser returns `ParserStatus::Failed` and the existing tokenizer
//! remains authoritative.

use std::path::PathBuf;

/// Outcome of the tree-sitter parse attempt.
#[derive(Debug, Clone, Default)]
pub enum ParserStatus {
    /// AST was built successfully and observations were collected.
    Ok,
    /// The grammar or runtime is unavailable (e.g. no WASM backend).
    Unavailable,
    /// tree-sitter could not parse the command string.
    #[default]
    Failed,
}

/// Structured facts extracted from a shell command AST.
#[derive(Debug, Clone, Default)]
pub struct ShellAstObservation {
    /// Whether the AST parse succeeded.
    pub parser_status: ParserStatus,

    /// Command name (first token).
    pub executable: Option<String>,

    /// All subcommands found.
    pub subcommands: Vec<ParsedCommand>,

    /// File-system paths referenced in arguments.
    pub path_args: Vec<PathObservation>,

    /// Whether any path argument resolves outside the project workspace.
    pub has_external_paths: bool,

    /// Whether the command contains dynamic substitutions ($(...), backticks).
    pub has_dynamic_expressions: bool,

    /// Whether glob wildcards were detected in path arguments.
    pub has_globs: bool,

    /// Whether redirections (> , >>, <, 2>, &>) were found.
    pub has_redirections: bool,
}

/// A single subcommand extracted from a compound shell command.
#[derive(Debug, Clone)]
pub struct ParsedCommand {
    /// Command name.
    pub name: String,
    /// Raw arguments (after stripping flags).
    pub args: Vec<String>,
}

/// A resolved file path reference from a command argument.
#[derive(Debug, Clone)]
pub struct PathObservation {
    /// Raw argument text.
    pub raw: String,
    /// Resolved absolute path (when resolvable without shell expansion).
    pub resolved: Option<PathBuf>,
    /// Whether this path is outside the project workspace root.
    pub is_external: bool,
    /// Whether the argument contains glob wildcards.
    pub has_glob: bool,
    /// Whether the argument contains dynamic shell expressions.
    pub has_dynamic: bool,
}

impl ShellAstObservation {
    /// Try to parse `command` and populate observations.
    ///
    /// On parse failure, `parser_status` is set to `Failed` and all
    /// other fields remain at their defaults.  The caller should fall
    /// back to the existing tokenizer-based classifier.
    pub fn parse(command: &str, _workspace_root: &std::path::Path) -> Self {
        let mut obs = Self::default();

        let mut parser = tree_sitter::Parser::new();
        if parser
            .set_language(&tree_sitter_bash::LANGUAGE.into())
            .is_err()
        {
            obs.parser_status = ParserStatus::Unavailable;
            return obs;
        }
        let Some(tree) = parser.parse(command, None) else {
            obs.parser_status = ParserStatus::Failed;
            return obs;
        };

        // Empty input produces a tree with no children — treat as failed.
        if tree.root_node().child_count() == 0 {
            obs.parser_status = ParserStatus::Failed;
            return obs;
        }

        obs.parser_status = ParserStatus::Ok;
        let root = tree.root_node();
        let source = command.as_bytes();

        // Extract all `command` nodes.
        let cmd_nodes = collect_command_nodes(root);
        for cmd_node in &cmd_nodes {
            let parts = extract_command_parts(cmd_node, source);
            if let Some(first) = parts.first() {
                if obs.executable.is_none() {
                    obs.executable = Some(first.clone());
                }
                let mut cmd = ParsedCommand {
                    name: first.clone(),
                    args: parts[1..].to_vec(),
                };
                cmd.args.retain(|a| !a.is_empty());
                obs.subcommands.push(cmd);

                for arg in &parts[1..] {
                    if let Some(path_obs) = resolve_path_arg(arg, _workspace_root) {
                        if path_obs.has_dynamic {
                            obs.has_dynamic_expressions = true;
                        }
                        if path_obs.has_glob {
                            obs.has_globs = true;
                        }
                        if path_obs.is_external {
                            obs.has_external_paths = true;
                        }
                        obs.path_args.push(path_obs);
                    }
                }
            }
        }

        obs.has_redirections = any_node_matches(root, "redirected_statement");

        // Dynamic expressions can appear anywhere in the command, not just
        // in path arguments (e.g. `echo $(date)`).
        obs.has_dynamic_expressions = obs.has_dynamic_expressions || has_dynamic_in_source(command);

        obs
    }
}

// ── AST traversal helpers ──────────────────────────────────────

fn collect_command_nodes(node: tree_sitter::Node) -> Vec<tree_sitter::Node> {
    let mut result = Vec::new();
    collect_recursive(node, &mut result);
    result
}

fn collect_recursive<'a>(node: tree_sitter::Node<'a>, out: &mut Vec<tree_sitter::Node<'a>>) {
    if node.kind() == "command" {
        out.push(node);
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_recursive(child, out);
        }
    }
}

fn extract_command_parts(cmd_node: &tree_sitter::Node, source: &[u8]) -> Vec<String> {
    let mut parts = Vec::new();
    for i in 0..cmd_node.child_count() {
        let Some(child) = cmd_node.child(i) else {
            continue;
        };
        match child.kind() {
            "command_name" | "word" | "string" | "raw_string" | "concatenation"
            | "simple_expansion" => {
                let text = match child.utf8_text(source) {
                    Ok(t) => t.to_string(),
                    Err(_) => continue,
                };
                if !text.starts_with('-') || text == "-" {
                    let cleaned = strip_quotes(&text);
                    if !cleaned.is_empty() {
                        parts.push(cleaned);
                    }
                }
            }
            _ => {}
        }
    }
    parts
}

fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 {
        let first = s.chars().next().unwrap();
        let last = s.chars().last().unwrap();
        if (first == '\'' || first == '"') && first == last {
            return s[1..s.len() - 1].to_string();
        }
    }
    s.to_string()
}

fn resolve_path_arg(raw: &str, workspace_root: &std::path::Path) -> Option<PathObservation> {
    let trimmed = raw.trim();
    if trimmed.starts_with('-') {
        return None;
    }
    let has_dynamic = trimmed.contains("$(")
        || trimmed.contains("${")
        || trimmed.contains('`')
        || trimmed.starts_with('$');
    let has_glob = trimmed.contains('*') || trimmed.contains('?') || trimmed.contains('[');

    let resolved = if has_dynamic || has_glob {
        None
    } else {
        let expanded = expand_tilde(trimmed);
        let path = std::path::Path::new(&expanded);
        if path.is_absolute() {
            Some(path.to_path_buf())
        } else {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let resolved = cwd.join(path);
            Some(std::fs::canonicalize(&resolved).unwrap_or(resolved))
        }
    };

    let is_external = resolved.as_ref().is_some_and(|r| {
        let workspace =
            std::fs::canonicalize(workspace_root).unwrap_or_else(|_| workspace_root.to_path_buf());
        !r.starts_with(&workspace)
    });

    Some(PathObservation {
        raw: trimmed.to_string(),
        resolved,
        is_external,
        has_glob,
        has_dynamic,
    })
}

fn expand_tilde(s: &str) -> String {
    if s == "~" {
        return dirs::home_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "~".to_string());
    }
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}/{}", home.to_string_lossy(), rest);
        }
    }
    s.to_string()
}

fn any_node_matches(node: tree_sitter::Node, kind: &str) -> bool {
    if node.kind() == kind {
        return true;
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if any_node_matches(child, kind) {
                return true;
            }
        }
    }
    false
}

fn has_dynamic_in_source(cmd: &str) -> bool {
    cmd.contains("$(") || cmd.contains("${") || cmd.contains('`')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace() -> PathBuf {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }

    #[test]
    fn parse_simple_command() {
        let obs = ShellAstObservation::parse("cargo build --release", &workspace());
        assert!(matches!(obs.parser_status, ParserStatus::Ok));
        assert_eq!(obs.executable.as_deref(), Some("cargo"));
        assert_eq!(obs.subcommands.len(), 1);
        assert_eq!(obs.subcommands[0].name, "cargo");
    }

    #[test]
    fn parse_compound_command() {
        let obs = ShellAstObservation::parse("git add . && git commit -m 'fix'", &workspace());
        assert!(matches!(obs.parser_status, ParserStatus::Ok));
        assert_eq!(obs.subcommands.len(), 2);
    }

    #[test]
    fn parse_piped_command() {
        let obs = ShellAstObservation::parse("cargo test 2>&1 | grep FAILED", &workspace());
        assert!(matches!(obs.parser_status, ParserStatus::Ok));
        assert!(obs.has_redirections);
    }

    #[test]
    fn detect_dynamic_expressions() {
        let obs = ShellAstObservation::parse("echo $(date)", &workspace());
        assert!(matches!(obs.parser_status, ParserStatus::Ok));
        assert!(obs.has_dynamic_expressions);
    }

    #[test]
    fn detect_glob_in_path() {
        let obs = ShellAstObservation::parse("ls *.rs", &workspace());
        assert!(matches!(obs.parser_status, ParserStatus::Ok));
        assert!(obs.has_globs);
    }

    #[test]
    fn parse_empty_command_is_failed() {
        let obs = ShellAstObservation::parse("", &workspace());
        assert!(matches!(obs.parser_status, ParserStatus::Failed));
    }

    #[test]
    fn tilde_expansion() {
        let expanded = expand_tilde("~/projects/foo");
        assert!(!expanded.starts_with('~'));
        assert!(expanded.contains("projects"));
    }
}
