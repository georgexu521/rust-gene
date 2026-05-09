use crate::services::api::ToolCall;
use crate::tools::ToolResult;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const MAX_SUBJECT_PATHS: usize = 4;
const MAX_DIR_ENTRIES: usize = 200;
const MAX_CANDIDATES: usize = 3;
const MAX_READ_BYTES: u64 = 192 * 1024;
const MIN_SCORE: i32 = 5;

#[derive(Debug, Clone)]
pub(super) struct CompanionContextNote {
    pub key: String,
    pub text: String,
}

#[derive(Debug, Clone)]
struct CompanionCandidate {
    path: PathBuf,
    score: i32,
    reasons: Vec<String>,
}

pub(super) fn companion_context_note(
    cwd: &Path,
    task_preview: &str,
    tool_call: &ToolCall,
    result: &ToolResult,
) -> Option<CompanionContextNote> {
    if !result.success {
        return None;
    }

    let task_tokens = word_tokens(task_preview);
    let subjects = subject_paths(cwd, tool_call, result);
    if subjects.is_empty() {
        return None;
    }

    let mut candidates_by_path: HashMap<PathBuf, CompanionCandidate> = HashMap::new();
    let mut subject_labels = Vec::new();
    for subject in subjects.into_iter().take(MAX_SUBJECT_PATHS) {
        let subject = canonical_or_self(subject);
        if !subject.is_file() {
            continue;
        }
        subject_labels.push(relative_display(cwd, &subject));
        for candidate in candidates_for_subject(cwd, &subject, &task_tokens) {
            candidates_by_path
                .entry(candidate.path.clone())
                .and_modify(|existing| {
                    if candidate.score > existing.score {
                        *existing = candidate.clone();
                    }
                })
                .or_insert(candidate);
        }
    }

    if subject_labels.is_empty() || candidates_by_path.is_empty() {
        return None;
    }

    let mut candidates = candidates_by_path.into_values().collect::<Vec<_>>();
    candidates.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| relative_display(cwd, &a.path).cmp(&relative_display(cwd, &b.path)))
    });
    candidates.truncate(MAX_CANDIDATES);
    if candidates.is_empty() {
        return None;
    }

    let mut lines = Vec::new();
    for candidate in &candidates {
        let mut reasons = unique_reasons(candidate.reasons.clone());
        reasons.truncate(3);
        lines.push(format!(
            "- `{}`: {}",
            relative_display(cwd, &candidate.path),
            reasons.join("; ")
        ));
    }

    let key = candidates
        .iter()
        .map(|candidate| relative_display(cwd, &candidate.path))
        .collect::<Vec<_>>()
        .join("|");
    let subject = subject_labels
        .into_iter()
        .take(2)
        .collect::<Vec<_>>()
        .join(", ");
    let text = format!(
        "<companion-context>\nRelated helper files near `{}`:\n{}\nUse this as background context only. If a helper matches the requested behavior, inspect or reuse it before reimplementing similar parsing, reporting, or shared logic.\n</companion-context>",
        subject,
        lines.join("\n")
    );

    Some(CompanionContextNote { key, text })
}

fn subject_paths(cwd: &Path, tool_call: &ToolCall, result: &ToolResult) -> Vec<PathBuf> {
    match tool_call.name.as_str() {
        "file_read" => tool_call.arguments["path"]
            .as_str()
            .map(|path| vec![resolve_tool_path(cwd, path)])
            .unwrap_or_default(),
        "grep" => grep_subject_paths(cwd, tool_call, result),
        _ => Vec::new(),
    }
}

fn grep_subject_paths(cwd: &Path, tool_call: &ToolCall, result: &ToolResult) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(matches) = result
        .data
        .as_ref()
        .and_then(|data| data.get("matches"))
        .and_then(|matches| matches.as_array())
    {
        for item in matches {
            if let Some(file) = item.get("file").and_then(|file| file.as_str()) {
                push_unique_path(&mut paths, resolve_tool_path(cwd, file));
            }
            if paths.len() >= MAX_SUBJECT_PATHS {
                return paths;
            }
        }
    }

    if paths.is_empty() {
        if let Some(path) = tool_call.arguments["path"].as_str() {
            let path = resolve_tool_path(cwd, path);
            if path.is_file() {
                paths.push(path);
            }
        }
    }
    paths
}

fn candidates_for_subject(
    cwd: &Path,
    subject: &Path,
    task_tokens: &HashSet<String>,
) -> Vec<CompanionCandidate> {
    let Some(dir) = subject.parent() else {
        return Vec::new();
    };
    let subject_ext = extension(subject);
    let subject_tokens = word_tokens(&format!(
        "{} {}",
        subject
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(""),
        read_limited(subject).unwrap_or_default()
    ));
    let subject_content = read_limited(subject).unwrap_or_default();
    let subject_imports = imported_names(&subject_content);

    let mut entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries.filter_map(Result::ok).collect::<Vec<_>>(),
        Err(_) => return Vec::new(),
    };
    entries.sort_by_key(|entry| entry.file_name());

    let subject_canonical = canonical_or_self(subject.to_path_buf());
    let mut candidates = Vec::new();
    for entry in entries.into_iter().take(MAX_DIR_ENTRIES) {
        let path = canonical_or_self(entry.path());
        if path == subject_canonical || !path.is_file() || !is_candidate_file(&path) {
            continue;
        }
        let candidate = score_candidate(
            cwd,
            &path,
            &subject_ext,
            &subject_tokens,
            task_tokens,
            &subject_imports,
        );
        if candidate.score >= MIN_SCORE {
            candidates.push(candidate);
        }
    }
    candidates
}

fn score_candidate(
    cwd: &Path,
    path: &Path,
    subject_ext: &str,
    subject_tokens: &HashSet<String>,
    task_tokens: &HashSet<String>,
    subject_imports: &HashSet<String>,
) -> CompanionCandidate {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    let candidate_tokens = word_tokens(name);
    let content = read_limited(path).unwrap_or_default();
    let content_tokens = word_tokens(&content);

    let mut score = 0;
    let mut reasons = Vec::new();

    let shared_subject = shared_tokens(&candidate_tokens, subject_tokens);
    if !shared_subject.is_empty() {
        score += (shared_subject.len() as i32 * 2).min(5);
        reasons.push(format!("shares target tokens {}", shared_subject.join("/")));
    }

    let shared_task = shared_tokens(&candidate_tokens, task_tokens);
    if !shared_task.is_empty() {
        score += (shared_task.len() as i32 * 2).min(4);
        reasons.push(format!("shares task tokens {}", shared_task.join("/")));
    }

    if subject_imports.contains(&stem.to_ascii_lowercase()) {
        score += 6;
        reasons.push("referenced by the inspected file".to_string());
    }

    let helper_tokens = helper_tokens(&candidate_tokens);
    if !helper_tokens.is_empty() {
        score += 2;
        reasons.push(format!("helper-style name {}", helper_tokens.join("/")));
    }

    if cross_language_helper(subject_ext, &extension(path)) {
        score += 2;
        reasons.push("nearby cross-language helper".to_string());
    }

    let shared_content = shared_tokens(&content_tokens, task_tokens);
    if !shared_content.is_empty() {
        score += shared_content.len().min(3) as i32;
        reasons.push(format!("content matches task {}", shared_content.join("/")));
    }

    if content.contains("def report_rows") || content.contains("fn report_rows") {
        score += 3;
        reasons.insert(0, "contains report_rows parser".to_string());
    }

    CompanionCandidate {
        path: canonical_or_self(path.to_path_buf()),
        score,
        reasons: if reasons.is_empty() {
            vec![format!("near {}", relative_display(cwd, path))]
        } else {
            reasons
        },
    }
}

fn resolve_tool_path(cwd: &Path, raw: &str) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    let path = Path::new(raw);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn canonical_or_self(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

fn relative_display(cwd: &Path, path: &Path) -> String {
    let cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    path.strip_prefix(&cwd)
        .unwrap_or(&path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn read_limited(path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() > MAX_READ_BYTES {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

fn is_candidate_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if name.starts_with('.') || name.ends_with('~') {
        return false;
    }
    matches!(
        extension(path).as_str(),
        "py" | "sh" | "bash" | "rs" | "js" | "mjs" | "ts" | "tsx" | "go" | "rb"
    )
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn cross_language_helper(subject_ext: &str, candidate_ext: &str) -> bool {
    matches!(
        (subject_ext, candidate_ext),
        ("sh", "py") | ("bash", "py") | ("sh", "rb") | ("bash", "rb")
    )
}

fn word_tokens(text: &str) -> HashSet<String> {
    let mut normalized = String::new();
    let mut prev_lower_or_digit = false;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && prev_lower_or_digit {
                normalized.push(' ');
            }
            normalized.push(ch.to_ascii_lowercase());
            prev_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else {
            normalized.push(' ');
            prev_lower_or_digit = false;
        }
    }
    normalized
        .split_whitespace()
        .filter(|token| token.len() > 1)
        .filter(|token| !STOP_TOKENS.contains(token))
        .map(ToString::to_string)
        .collect()
}

fn shared_tokens(left: &HashSet<String>, right: &HashSet<String>) -> Vec<String> {
    let mut shared = left.intersection(right).cloned().collect::<Vec<String>>();
    shared.sort();
    shared
}

fn helper_tokens(tokens: &HashSet<String>) -> Vec<String> {
    let helpers = [
        "parser",
        "helper",
        "helpers",
        "util",
        "utils",
        "common",
        "shared",
        "report",
        "summary",
        "aggregate",
    ];
    let helper_set = helpers.into_iter().collect::<HashSet<_>>();
    let mut found = tokens
        .iter()
        .filter(|token| helper_set.contains(token.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    found.sort();
    found
}

fn imported_names(content: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    for token in content.split(|ch: char| {
        !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '-' || ch == '/')
    }) {
        let token = token.trim_matches('.');
        if token.is_empty() {
            continue;
        }
        let token = token
            .rsplit(['/', '.'])
            .next()
            .unwrap_or(token)
            .trim_end_matches(".py")
            .trim_end_matches(".rs")
            .trim_end_matches(".sh")
            .to_ascii_lowercase();
        if token.len() > 2 {
            names.insert(token);
        }
    }
    names
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    let path = canonical_or_self(path);
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn unique_reasons(reasons: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    reasons
        .into_iter()
        .filter(|reason| seen.insert(reason.clone()))
        .collect()
}

const STOP_TOKENS: &[&str] = &[
    "the", "and", "for", "with", "from", "into", "this", "that", "task", "file", "files", "src",
    "lib", "mod", "main", "index", "test", "tests", "script", "scripts", "run", "one", "two",
    "new", "old",
];

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn surfaces_nearby_live_eval_parser_for_summary_script() {
        let tmp = tempdir().expect("tmp dir");
        let scripts = tmp.path().join("scripts");
        std::fs::create_dir_all(&scripts).expect("scripts dir");
        std::fs::write(
            scripts.join("run_live_eval.sh"),
            "summary_task() {\n  echo \"summary mode is not implemented yet\" >&2\n  return 2\n}\n",
        )
        .expect("write runner");
        std::fs::write(
            scripts.join("live_eval_report_parser.py"),
            "def report_rows(run_dir):\n    return []\n",
        )
        .expect("write parser");
        std::fs::write(scripts.join("unrelated.py"), "print('hello')\n").expect("write other");

        let call = ToolCall {
            id: "call_1".to_string(),
            name: "file_read".to_string(),
            arguments: json!({"path": "scripts/run_live_eval.sh", "offset": 1, "limit": 5}),
        };
        let result = ToolResult::success("1 | summary_task() {");

        let note = companion_context_note(
            tmp.path(),
            "live-eval-dashboard-summary should implement summary_task and plan_quality reporting",
            &call,
            &result,
        )
        .expect("companion context");

        assert!(note.text.contains("scripts/live_eval_report_parser.py"));
        assert!(note.text.contains("contains report_rows parser"));
        assert!(!note.text.contains("scripts/unrelated.py"));
    }

    #[test]
    fn grep_matches_can_surface_companion_files() {
        let tmp = tempdir().expect("tmp dir");
        let scripts = tmp.path().join("scripts");
        std::fs::create_dir_all(&scripts).expect("scripts dir");
        std::fs::write(
            scripts.join("run_live_eval.sh"),
            "summary_task() {\n  echo stub\n}\n",
        )
        .expect("write runner");
        std::fs::write(
            scripts.join("live_eval_report_parser.py"),
            "def report_rows(run_dir):\n    return []\n",
        )
        .expect("write parser");

        let call = ToolCall {
            id: "call_1".to_string(),
            name: "grep".to_string(),
            arguments: json!({"pattern": "summary_task", "path": "scripts"}),
        };
        let result = ToolResult::success_with_data(
            "scripts/run_live_eval.sh\n   1: summary_task() {",
            json!({
                "matches": [
                    {"file": "scripts/run_live_eval.sh", "line": 1, "content": "summary_task() {"}
                ]
            }),
        );

        let note = companion_context_note(
            tmp.path(),
            "live-eval-dashboard-summary should generate summary rows",
            &call,
            &result,
        )
        .expect("companion context");

        assert!(note.key.contains("scripts/live_eval_report_parser.py"));
    }
}
