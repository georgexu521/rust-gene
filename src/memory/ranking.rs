use crate::engine::project_progress::{ProjectProgressKind, ProjectProgressRecord};
use crate::memory::manager::{kind_label, record_needs_revalidation};
use crate::memory::reports::{MemoryFileSnapshot, MemoryMatch};
use crate::memory::types::{MemoryKind, MemoryRecord, MemoryScope, MemoryScopeKind, MemoryStatus};
use std::collections::HashSet;

pub(super) fn rank_memory_records(
    records: &[MemoryRecord],
    keywords: &[String],
    active_scope: &MemoryScope,
) -> Vec<MemoryMatch> {
    if keywords.is_empty() || records.is_empty() {
        return Vec::new();
    }
    records
        .iter()
        .filter(|record| matches!(record.status, MemoryStatus::Accepted))
        .filter(|record| !record.is_expired())
        .filter(|record| memory_record_scope_matches(record, active_scope))
        .filter_map(|record| {
            let score = semantic_memory_score(&record.content, keywords, &record_source(record));
            if score == 0 {
                return None;
            }
            let importance_boost = usize::from(record.importance.min(5));
            let verified_boost = if record.last_verified_at.is_some() {
                3
            } else {
                0
            };
            let stale = record_needs_revalidation(record);
            let score = score + importance_boost + verified_boost;
            let score = if stale {
                score.saturating_div(2).max(1)
            } else {
                score
            };
            Some(MemoryMatch {
                source: record_source(record),
                score,
                rerank_score: None,
                snippet: record.content.trim().chars().take(800).collect(),
            })
        })
        .collect()
}

pub(super) fn memory_record_scope_matches(
    record: &MemoryRecord,
    active_scope: &MemoryScope,
) -> bool {
    if matches!(record.kind, MemoryKind::UserPreference) {
        return true;
    }

    let record_identity = record.scope.identity();
    let active_identity = active_scope.identity();
    match record_identity.kind {
        MemoryScopeKind::User | MemoryScopeKind::Agent => true,
        MemoryScopeKind::Project => {
            active_identity.kind == MemoryScopeKind::Project
                && record_identity.id == active_identity.id
        }
        MemoryScopeKind::Topic if active_identity.kind == MemoryScopeKind::Project => {
            record_identity.parent.as_deref() == Some(active_identity.id.as_str())
        }
        MemoryScopeKind::Topic => {
            record_identity.parent.as_deref() == Some(active_identity.id.as_str())
                || record_identity.parent == active_identity.parent
        }
        MemoryScopeKind::Session => record_identity.id == active_identity.id,
    }
}

pub(super) fn rank_project_progress_records(
    records: &[ProjectProgressRecord],
    keywords: &[String],
) -> Vec<MemoryMatch> {
    if keywords.is_empty() || records.is_empty() {
        return Vec::new();
    }
    records
        .iter()
        .filter_map(|record| {
            let content = format!(
                "{} {} {} {}",
                record.kind.label(),
                record.objective,
                record.content,
                record.evidence.join(" ")
            );
            let mut score = semantic_memory_score(
                &content,
                keywords,
                &format!("project_progress/{}", record.kind.label()),
            );
            if score == 0 {
                return None;
            }
            score += match record.kind {
                ProjectProgressKind::ProjectStatus => 5,
                ProjectProgressKind::NextStep => 4,
                ProjectProgressKind::ValidationBaseline => 4,
                ProjectProgressKind::OpenRisk => 3,
            };
            if record.is_stale() {
                score = score.saturating_div(2).max(1);
            }
            let stale = if record.is_stale() { ":stale" } else { "" };
            Some(MemoryMatch {
                source: format!(
                    "project_progress/{}{}:{}",
                    record.id,
                    stale,
                    record.kind.label()
                ),
                score,
                rerank_score: None,
                snippet: record.content.trim().chars().take(800).collect(),
            })
        })
        .collect()
}

pub(super) fn dedupe_memory_matches(matches: &mut Vec<MemoryMatch>) {
    let mut seen = HashSet::new();
    matches.retain(|entry| {
        let key = format!("{}:{}", entry.source, entry.snippet);
        seen.insert(key)
    });
}

pub(super) fn record_source(record: &MemoryRecord) -> String {
    let projection = record
        .projection
        .as_ref()
        .map(|projection| projection.path.as_str())
        .unwrap_or(kind_label(record.kind));
    let stale = if record_needs_revalidation(record) {
        ":stale"
    } else {
        ""
    };
    let pinned = if record.tags.iter().any(|tag| {
        matches!(
            tag.as_str(),
            "pinned" | "user_pinned" | "user-pinned" | "always_include"
        )
    }) {
        ":pinned"
    } else {
        ""
    };
    format!(
        "memory_record/{}{}{}:{}",
        record.id, stale, pinned, projection
    )
}

pub(super) fn memory_record_id_from_source(source: &str) -> Option<String> {
    let rest = source.strip_prefix("memory_record/")?;
    let id = rest.split(':').next()?.trim();
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

pub(super) fn rank_memory_paragraphs(
    source: &str,
    content: &str,
    keywords: &[String],
) -> Vec<MemoryMatch> {
    if keywords.is_empty() || content.trim().is_empty() {
        return Vec::new();
    }

    split_memory_paragraphs(content)
        .into_iter()
        .filter_map(|paragraph| {
            let score = semantic_memory_score(&paragraph, keywords, source);
            if score == 0 {
                None
            } else {
                Some(MemoryMatch {
                    source: source.to_string(),
                    score,
                    rerank_score: None,
                    snippet: paragraph.trim().chars().take(800).collect(),
                })
            }
        })
        .collect()
}

pub(super) fn rank_memory_files(
    files: &[MemoryFileSnapshot],
    keywords: &[String],
) -> Vec<MemoryMatch> {
    files
        .iter()
        .filter_map(|file| {
            let source = format!("memory/{}", file.relative_path);
            let snippet = best_memory_file_snippet(&file.content, keywords);
            let score = semantic_memory_score(&file.content, keywords, &source);
            if score == 0 {
                None
            } else {
                Some(MemoryMatch {
                    source,
                    score,
                    rerank_score: None,
                    snippet,
                })
            }
        })
        .collect()
}

fn semantic_memory_score(content: &str, keywords: &[String], source: &str) -> usize {
    let lower = content.to_lowercase();
    let source_lower = source.to_lowercase();
    let mut score = 0usize;

    for keyword in keywords {
        if lower.contains(keyword.as_str()) {
            score += 8;
        }
        if source_lower.contains(keyword.as_str()) {
            score += 6;
        }
        for alias in semantic_aliases(keyword) {
            if lower.contains(alias) {
                score += 4;
            }
            if source_lower.contains(alias) {
                score += 3;
            }
        }
    }

    if lower.contains("user preference:") || lower.contains("偏好") {
        score += 2;
    }
    if lower.contains("decision") || lower.contains("决策") {
        score += 2;
    }
    if lower.contains("solution:") || lower.contains("fix") || lower.contains("修复") {
        score += 2;
    }

    score
}

fn semantic_aliases(keyword: &str) -> &'static [&'static str] {
    match keyword {
        "tui" | "terminal" | "ui" | "界面" | "设计" => &[
            "tui", "terminal", "ui", "界面", "布局", "claude", "scroll", "滚动",
        ],
        "context" | "prompt" | "token" | "上下文" | "提示词" => &[
            "context",
            "prompt",
            "token",
            "上下文",
            "提示词",
            "compression",
            "memory",
        ],
        "memory" | "remember" | "记忆" => &[
            "memory",
            "remember",
            "记忆",
            "preference",
            "偏好",
            "learned",
        ],
        "permission" | "permissions" | "权限" => &[
            "permission",
            "permissions",
            "权限",
            "approval",
            "allow",
            "deny",
        ],
        "tool" | "tools" | "工具" => &["tool", "tools", "工具", "bash", "mcp"],
        "rust" | "cargo" => &["rust", "cargo", ".rs", "crate"],
        "test" | "tests" | "测试" => &["test", "tests", "测试", "cargo test"],
        "concise" | "brief" | "terse" | "short" | "简洁" | "简短" => {
            &["concise", "brief", "terse", "short", "简洁", "简短"]
        }
        "verify" | "validate" | "validation" | "proof" | "验证" => &[
            "verify",
            "validate",
            "validation",
            "proof",
            "evidence",
            "验证",
        ],
        _ => &[],
    }
}

pub(super) fn best_memory_file_snippet(content: &str, keywords: &[String]) -> String {
    let candidates: Vec<&str> = content
        .split("\n\n")
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();

    let best = candidates
        .iter()
        .max_by_key(|candidate| {
            let lower = candidate.to_lowercase();
            keywords
                .iter()
                .filter(|keyword| lower.contains(keyword.as_str()))
                .count()
        })
        .copied()
        .unwrap_or_else(|| content.trim());

    best.chars().take(800).collect()
}

/// 从文本中提取关键词
pub(super) fn extract_keywords(text: &str) -> Vec<String> {
    let stop_words: HashSet<&str> = [
        "的", "了", "在", "是", "我", "有", "和", "就", "不", "人", "都", "一", "一个", "上", "也",
        "很", "到", "说", "要", "去", "你", "会", "着", "the", "a", "an", "is", "are", "was",
        "were", "be", "been", "have", "has", "had", "do", "does", "did", "will", "would", "could",
        "should", "i", "you", "he", "she", "it", "we", "they", "this", "that", "what",
    ]
    .iter()
    .cloned()
    .collect();

    text.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .filter(|w| w.len() >= 2 && !stop_words.contains(w.to_lowercase().as_str()))
        .map(|w| w.to_lowercase())
        .collect()
}

/// 从记忆文件中搜索相关段落
pub(super) fn search_memory(content: &str, keywords: &[String], max_results: usize) -> Vec<String> {
    if keywords.is_empty() || content.trim().is_empty() {
        return Vec::new();
    }

    let paragraphs = split_memory_paragraphs(content);

    let mut scored: Vec<(usize, String)> = paragraphs
        .into_iter()
        .map(|p| {
            let p_lower = p.to_lowercase();
            let score = keywords
                .iter()
                .filter(|k| p_lower.contains(k.as_str()))
                .count();
            (score, p)
        })
        .filter(|(score, _)| *score > 0)
        .collect();

    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored
        .into_iter()
        .take(max_results)
        .map(|(_, content)| content)
        .collect()
}

fn split_memory_paragraphs(content: &str) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let mut current = String::new();
    for line in content.lines() {
        if line.starts_with("## ") || (line.trim().is_empty() && !current.trim().is_empty()) {
            if !current.trim().is_empty() {
                paragraphs.push(current.clone());
            }
            current = if line.starts_with("## ") {
                line.to_string()
            } else {
                String::new()
            };
        } else {
            current.push_str(line);
            current.push('\n');
        }
    }
    if !current.trim().is_empty() {
        paragraphs.push(current);
    }
    paragraphs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_keywords() {
        let keywords = extract_keywords("How do I implement authentication in Rust?");
        assert!(keywords.contains(&"implement".to_string()));
        assert!(keywords.contains(&"authentication".to_string()));
        assert!(keywords.contains(&"rust".to_string()));
        assert!(!keywords.contains(&"do".to_string())); // stop word
    }

    #[test]
    fn test_search_memory() {
        let content = r#"# Memory

## Project Conventions
Use snake_case for Rust functions.

## API Notes
The auth endpoint requires JWT tokens.

## Debugging Tips
Always check logs first.
"#;
        let keywords = vec!["auth".to_string(), "jwt".to_string()];
        let results = search_memory(content, &keywords, 3);
        assert!(!results.is_empty());
        assert!(results[0].contains("auth"));
    }
}
