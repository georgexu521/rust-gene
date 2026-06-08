//! 记忆矛盾检测：发现同一主题下有冲突的记忆记录。
//!
//! 借鉴自 Hermes Holographic 插件的 `contradict()` 方法
//! (`plugins/memory/holographic/retrieval.py:338-442`)。
//!
//! 原理：两条记录如果共享关键词（同一主题）但内容相似度低（说法不同），
//! 很可能存在矛盾。用 keyword overlap + word-overlap similarity 做近似，
//! 替代 Hermes 的 HRR 向量比较。

use crate::memory::types::{MemoryKind, MemoryRecord, MemoryStatus};
use std::collections::HashSet;

/// 一对可能存在矛盾的记忆记录。
#[derive(Debug, Clone)]
pub struct ContradictionPair {
    /// 记录 A 的 ID
    pub record_a: String,
    /// 记录 B 的 ID
    pub record_b: String,
    /// 记录 A 的关键词
    pub keywords_a: Vec<String>,
    /// 记录 B 的关键词
    pub keywords_b: Vec<String>,
    /// 共享的关键词（共同主题）
    pub shared_keywords: Vec<String>,
    /// 关键词重叠度 (Jaccard)
    pub keyword_overlap: f32,
    /// 内容相似度 (基于词频)
    pub content_similarity: f32,
    /// 矛盾分数: keyword_overlap × (1.0 - content_similarity)
    pub contradiction_score: f32,
}

/// 检测 Accepted 类型记录中可能存在的矛盾。
///
/// 只扫描 `ProjectFact`、`Decision`、`WorkflowConvention` 类型的记录。
/// 上限 500 条以防止 O(n²) 爆炸。
pub fn detect_contradictions(
    records: &[MemoryRecord],
    threshold: f32,
    limit: usize,
) -> Vec<ContradictionPair> {
    let candidates: Vec<&MemoryRecord> = records
        .iter()
        .filter(|r| r.status == MemoryStatus::Accepted)
        .filter(|r| {
            matches!(
                r.kind,
                MemoryKind::ProjectFact | MemoryKind::Decision | MemoryKind::WorkflowConvention
            )
        })
        .take(500)
        .collect();

    if candidates.len() < 2 {
        return Vec::new();
    }

    // 预计算每条记录的关键词
    let indexed: Vec<(Vec<String>, &MemoryRecord)> = candidates
        .iter()
        .map(|r| (extract_keywords(&r.content), *r))
        .collect();

    let mut pairs = Vec::new();

    for i in 0..indexed.len() {
        for j in (i + 1)..indexed.len() {
            let (kw_a, rec_a) = &indexed[i];
            let (kw_b, rec_b) = &indexed[j];

            let set_a: HashSet<&String> = kw_a.iter().collect();
            let set_b: HashSet<&String> = kw_b.iter().collect();
            let shared: Vec<String> = set_a.intersection(&set_b).map(|s| s.to_string()).collect();

            let union_size = set_a.union(&set_b).count();
            let keyword_overlap = if union_size == 0 {
                0.0
            } else {
                shared.len() as f32 / union_size as f32
            };

            if keyword_overlap < 0.3 {
                continue;
            }

            let content_sim = word_overlap_similarity(&rec_a.content, &rec_b.content);

            // 矛盾分数：关键词重叠越高 + 内容越不相似 = 越矛盾
            let contradiction_score = keyword_overlap * (1.0 - content_sim);

            if contradiction_score >= threshold {
                pairs.push(ContradictionPair {
                    record_a: rec_a.id.clone(),
                    record_b: rec_b.id.clone(),
                    keywords_a: kw_a.clone(),
                    keywords_b: kw_b.clone(),
                    shared_keywords: shared,
                    keyword_overlap,
                    content_similarity: content_sim,
                    contradiction_score,
                });
            }
        }
    }

    pairs.sort_by(|a, b| {
        b.contradiction_score
            .partial_cmp(&a.contradiction_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    pairs.truncate(limit);
    pairs
}

fn extract_keywords(content: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut keywords = Vec::new();

    for word in content
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
    {
        let word = word.trim().trim_matches('-').trim_matches('_');
        if word.len() < 3 {
            continue;
        }
        if is_stop_word(word) {
            continue;
        }
        if seen.insert(word.to_string()) {
            keywords.push(word.to_string());
        }
    }
    keywords
}

fn is_stop_word(word: &str) -> bool {
    matches!(
        word,
        "the"
            | "and"
            | "for"
            | "with"
            | "that"
            | "this"
            | "from"
            | "have"
            | "are"
            | "was"
            | "not"
            | "but"
            | "can"
            | "has"
            | "all"
            | "will"
            | "when"
            | "been"
            | "its"
            | "what"
            | "use"
            | "using"
            | "should"
            | "would"
            | "could"
            | "which"
            | "each"
            | "also"
            | "into"
            | "just"
            | "preference"
            | "project"
            | "memory"
            | "user"
            | "agent"
    )
}

fn word_overlap_similarity(a: &str, b: &str) -> f32 {
    let words_a: HashSet<&str> = a
        .split(|c: char| !c.is_alphanumeric())
        .map(str::trim)
        .filter(|w| !w.is_empty() && w.len() >= 3)
        .collect();
    let words_b: HashSet<&str> = b
        .split(|c: char| !c.is_alphanumeric())
        .map(str::trim)
        .filter(|w| !w.is_empty() && w.len() >= 3)
        .collect();

    let total = words_a.len().min(words_b.len()).max(1);
    let shared = words_a.intersection(&words_b).count() as f32;
    shared / total as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::types::{
        MemoryEvidenceRef, MemoryProvenance, MemoryRecord, MemoryScope, MemoryStatus,
    };

    fn make_record(id: &str, content: &str, kind: MemoryKind) -> MemoryRecord {
        MemoryRecord {
            id: id.to_string(),
            kind,
            content: content.to_string(),
            status: MemoryStatus::Accepted,
            scope: MemoryScope::local("test"),
            summary: String::new(),
            provenance: MemoryProvenance::local("test"),
            confidence: 0.8,
            utility: 0.5,
            sensitivity: crate::memory::types::SensitivityLevel::Public,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            expires_at: None,
            stale_after: None,
            importance: 3,
            evidence: vec![MemoryEvidenceRef::inferred("test", "test")],
            last_verified_at: None,
            last_used_at: None,
            use_count: 0,
            success_count: 0,
            failure_count: 0,
            supersedes: Vec::new(),
            superseded_by: None,
            projection: None,
            strategy: None,
            tags: Vec::new(),
        }
    }

    #[test]
    fn detects_contradiction_on_same_topic() {
        let records = vec![
            make_record(
                "a",
                "server listens on port 8080 using rust hyper framework",
                MemoryKind::ProjectFact,
            ),
            make_record(
                "b",
                "server listens on port 3000 using node express framework",
                MemoryKind::ProjectFact,
            ),
        ];

        let pairs = detect_contradictions(&records, 0.05, 5);
        // "server", "listens", "port", "framework" are shared topic keywords
        // But "8080/rust/hyper" vs "3000/node/express" are different content
        assert!(
            !pairs.is_empty(),
            "expected contradiction, got {} pairs",
            pairs.len()
        );
    }

    #[test]
    fn no_contradiction_on_consistent_content() {
        let records = vec![
            make_record("a", "run cargo test before commit", MemoryKind::ProjectFact),
            make_record(
                "b",
                "always run cargo test before committing",
                MemoryKind::ProjectFact,
            ),
        ];

        let pairs = detect_contradictions(&records, 0.2, 5);
        assert!(
            pairs.is_empty(),
            "similar content should not trigger contradiction"
        );
    }

    #[test]
    fn skips_unrelated_topics() {
        let records = vec![
            make_record("a", "use tabs for Go projects", MemoryKind::ProjectFact),
            make_record(
                "b",
                "strain database should support CSV export",
                MemoryKind::ProjectFact,
            ),
        ];

        let pairs = detect_contradictions(&records, 0.2, 5);
        assert!(pairs.is_empty(), "unrelated topics should not match");
    }

    #[test]
    fn ignores_non_accepted_records() {
        let mut rec = make_record("a", "use english", MemoryKind::ProjectFact);
        rec.status = MemoryStatus::Proposed;
        let records = vec![
            rec,
            make_record("b", "use chinese", MemoryKind::ProjectFact),
        ];

        let pairs = detect_contradictions(&records, 0.1, 5);
        assert!(pairs.is_empty(), "non-accepted records should be ignored");
    }
}
