use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

fn memory_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".priority-agent")
}

/// MEMORY.md 文件路径
fn memory_path() -> PathBuf {
    memory_root().join("MEMORY.md")
}

fn user_path() -> PathBuf {
    memory_root().join("USER.md")
}

fn memory_dir() -> PathBuf {
    memory_root().join("memory")
}

fn legacy_agent_memory_dir() -> PathBuf {
    memory_root().join("agent_memories")
}

fn memory_decision_log_path() -> PathBuf {
    memory_dir().join("decisions.jsonl")
}

fn memory_flush_log_path() -> PathBuf {
    memory_dir().join("flush_queue.jsonl")
}

#[derive(Debug, Clone)]
struct MemoryDocument {
    namespace: String,
    path: String,
    content: String,
}

#[derive(Debug, Clone, Deserialize)]
struct AgentMemoryJsonEntry {
    key: String,
    value: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Clone)]
struct MemoryKeyValue {
    namespace: String,
    key: String,
    value: String,
}

fn load_memory_dir_files() -> Vec<(String, String)> {
    let root = memory_dir();
    let mut files = Vec::new();
    collect_memory_dir_files(&root, &root, &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

fn load_memory_documents() -> Vec<MemoryDocument> {
    let mut docs = Vec::new();
    push_text_document(&mut docs, "project", "MEMORY.md", &memory_path());
    push_text_document(&mut docs, "user", "USER.md", &user_path());

    for (path, content) in load_memory_dir_files() {
        docs.push(MemoryDocument {
            namespace: "topic".to_string(),
            path: format!("memory/{}", path),
            content,
        });
    }

    collect_agent_memory_documents(&memory_dir().join("agents"), "agent", &mut docs);
    collect_agent_memory_documents(&legacy_agent_memory_dir(), "agent_legacy", &mut docs);
    docs.sort_by(|a, b| {
        a.namespace
            .cmp(&b.namespace)
            .then_with(|| a.path.cmp(&b.path))
    });
    docs
}

fn push_text_document(docs: &mut Vec<MemoryDocument>, namespace: &str, label: &str, path: &Path) {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return,
    };
    if content.trim().is_empty() {
        return;
    }
    docs.push(MemoryDocument {
        namespace: namespace.to_string(),
        path: label.to_string(),
        content,
    });
}

fn collect_agent_memory_documents(dir: &Path, namespace: &str, docs: &mut Vec<MemoryDocument>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        if content.trim().is_empty() {
            continue;
        }
        let display_content = format_agent_memory_content(&content);
        if display_content.trim().is_empty() {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown.json");
        docs.push(MemoryDocument {
            namespace: namespace.to_string(),
            path: format!("memory/agents/{}", file_name),
            content: display_content,
        });
    }
}

fn format_agent_memory_content(content: &str) -> String {
    match serde_json::from_str::<Vec<AgentMemoryJsonEntry>>(content) {
        Ok(entries) => entries
            .into_iter()
            .map(|entry| {
                let tags = if entry.tags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", entry.tags.join(","))
                };
                format!("{}: {}{}", entry.key, entry.value, tags)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Err(_) => content.to_string(),
    }
}

fn collect_memory_dir_files(root: &Path, dir: &Path, files: &mut Vec<(String, String)>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }

        if path.is_dir() {
            collect_memory_dir_files(root, path.as_path(), files);
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        if content.trim().is_empty() {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        files.push((relative, content));
    }
}

#[cfg(test)]
fn infer_topic(content: &str, category: &str) -> Option<&'static str> {
    let lower = content.to_lowercase();
    let category = category.to_lowercase();

    if category == "preference" || lower.contains("user preference") || lower.contains("偏好") {
        return None;
    }
    if contains_any(
        &lower,
        &[
            "tui", "terminal", "ui", "claude", "scroll", "界面", "设计", "滚动",
        ],
    ) {
        return Some("tui-design");
    }
    if contains_any(
        &lower,
        &[
            "context",
            "prompt",
            "token",
            "memory",
            "compression",
            "上下文",
            "提示词",
            "记忆",
        ],
    ) {
        return Some("context-management");
    }
    if contains_any(
        &lower,
        &["permission", "approval", "allow", "deny", "权限", "授权"],
    ) {
        return Some("permissions");
    }
    if contains_any(&lower, &["tool", "bash", "mcp", "工具"]) {
        return Some("tools");
    }
    if contains_any(&lower, &["rust", "cargo", ".rs", "crate"]) {
        return Some("rust-workflow");
    }
    if category == "decision" {
        return Some("decisions");
    }
    if category == "convention" {
        return Some("conventions");
    }
    None
}

#[cfg(test)]
fn contains_any(content: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| content.contains(needle))
}

fn search_memory_documents(docs: &[MemoryDocument], query: &str) -> Vec<String> {
    let query_lower = query.to_lowercase();
    let mut matching = Vec::new();

    for doc in docs {
        for line in doc.content.lines() {
            if line.to_lowercase().contains(&query_lower) {
                matching.push(format!("[{}:{}] {}", doc.namespace, doc.path, line.trim()));
            }
        }
    }

    matching
}

fn memory_conflicts(docs: &[MemoryDocument], max_conflicts: usize) -> Vec<String> {
    let mut by_key: HashMap<String, Vec<MemoryKeyValue>> = HashMap::new();
    for doc in docs {
        for entry in extract_key_values(doc) {
            by_key
                .entry(entry.key.to_lowercase())
                .or_default()
                .push(entry);
        }
    }

    let mut conflicts = by_key
        .into_iter()
        .filter_map(|(key, entries)| {
            if entries.len() < 2 {
                return None;
            }
            let mut values = entries
                .iter()
                .map(|entry| normalize_value(&entry.value))
                .collect::<Vec<_>>();
            values.sort();
            values.dedup();
            if values.len() < 2 {
                return None;
            }
            let locations = entries
                .iter()
                .take(4)
                .map(|entry| {
                    format!(
                        "{}={} ({})",
                        entry.namespace,
                        compact_line(&entry.value, 70),
                        entry.key
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
            Some(format!(
                "- key '{}' has conflicting values: {}",
                key, locations
            ))
        })
        .collect::<Vec<_>>();

    conflicts.sort();
    conflicts.truncate(max_conflicts);
    conflicts
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct MemoryDecisionCounts {
    accepted: usize,
    proposed: usize,
    rejected: usize,
    blocked: usize,
}

#[derive(Debug, serde::Serialize)]
struct MemoryDoctorJson {
    root: String,
    documents: MemoryDoctorDocumentsJson,
    records: MemoryRecordSummaryJson,
    decisions: MemoryDecisionCountsJson,
    flushes: MemoryFlushCountsJson,
    quality_gates: MemoryQualityGatesJson,
    calibration: MemoryCalibrationReportJson,
    conflicts: Vec<String>,
    maintenance: Vec<MemoryMaintenanceJson>,
}

#[derive(Debug, serde::Serialize)]
struct MemoryDoctorDocumentsJson {
    total: usize,
    topic: usize,
    agent: usize,
    chars: usize,
}

#[derive(Debug, serde::Serialize)]
struct MemoryRecordSummaryJson {
    total: usize,
    accepted: usize,
    proposed: usize,
    rejected: usize,
    archived: usize,
    superseded: usize,
    missing_evidence: usize,
    stale: usize,
    used: usize,
    projection_drift: usize,
}

#[derive(Debug, serde::Serialize)]
struct MemoryDecisionCountsJson {
    accepted: usize,
    proposed: usize,
    rejected: usize,
    blocked: usize,
}

#[derive(Debug, serde::Serialize)]
struct MemoryFlushCountsJson {
    completed: usize,
    pending: usize,
    running: usize,
    failed: usize,
    skipped_duplicate: usize,
    total: usize,
}

#[derive(Debug, serde::Serialize)]
struct MemoryQualityGatesJson {
    accept_threshold: f32,
    propose_threshold: f32,
    explicit_override_threshold: f32,
    hard_stops: Vec<&'static str>,
}

#[derive(Debug, serde::Serialize)]
struct MemoryCalibrationReportJson {
    passed: usize,
    total: usize,
    results: Vec<crate::memory::MemoryCalibrationResult>,
}

#[derive(Debug, serde::Serialize)]
struct MemoryMaintenanceJson {
    path: String,
    score: f32,
    action: String,
    reason: String,
}

fn load_memory_decision_counts() -> MemoryDecisionCounts {
    let content = std::fs::read_to_string(memory_decision_log_path()).unwrap_or_default();
    memory_decision_counts_from_jsonl(&content)
}

fn load_memory_flush_summary() -> crate::memory::MemoryFlushSummary {
    let content = std::fs::read_to_string(memory_flush_log_path()).unwrap_or_default();
    let mut latest = std::collections::HashMap::new();
    for line in content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(record) = serde_json::from_str::<crate::memory::MemoryFlushRecord>(line) else {
            continue;
        };
        latest.insert(record.id.clone(), record);
    }

    let mut summary = crate::memory::MemoryFlushSummary {
        total: latest.len(),
        ..Default::default()
    };
    for record in latest.values() {
        match record.status {
            crate::memory::MemoryFlushStatus::Pending => summary.pending += 1,
            crate::memory::MemoryFlushStatus::Running => summary.running += 1,
            crate::memory::MemoryFlushStatus::Completed => summary.completed += 1,
            crate::memory::MemoryFlushStatus::Failed => summary.failed += 1,
            crate::memory::MemoryFlushStatus::SkippedDuplicate => summary.skipped_duplicate += 1,
        }
    }
    summary
}

fn memory_decision_counts_from_jsonl(content: &str) -> MemoryDecisionCounts {
    let mut counts = MemoryDecisionCounts::default();
    for line in content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        match value["status"].as_str().unwrap_or("") {
            "accepted" => counts.accepted += 1,
            "proposed" => counts.proposed += 1,
            "rejected" => counts.rejected += 1,
            "blocked" => counts.blocked += 1,
            _ => {}
        }
    }
    counts
}

fn format_memory_doctor(docs: &[MemoryDocument], conflicts: &[String]) -> String {
    let counts = load_memory_decision_counts();
    let flushes = load_memory_flush_summary();
    let calibration = crate::memory::run_memory_calibration_samples();
    let calibration_passed = calibration.iter().filter(|result| result.passed).count();
    let record_summary = crate::memory::MemoryManager::new().memory_record_summary();
    let total_chars: usize = docs.iter().map(|doc| doc.content.chars().count()).sum();
    let topic_count = docs.iter().filter(|doc| doc.namespace == "topic").count();
    let agent_count = docs
        .iter()
        .filter(|doc| doc.namespace.starts_with("agent"))
        .count();

    let mut out = String::new();
    out.push_str("Memory Doctor\n");
    out.push_str(&format!("  Root: {}\n", memory_root().display()));
    out.push_str(&format!(
        "  Documents: {} total · {} topic · {} agent · {} chars\n",
        docs.len(),
        topic_count,
        agent_count,
        total_chars
    ));
    out.push_str(&format!(
        "  Decisions: {} accepted · {} proposed · {} rejected · {} blocked\n",
        counts.accepted, counts.proposed, counts.rejected, counts.blocked
    ));
    out.push_str(&format!(
        "  Records: {} total · {} accepted · {} proposed · {} missing evidence · {} stale · {} used · {} projection drift\n",
        record_summary.total,
        record_summary.accepted,
        record_summary.proposed,
        record_summary.missing_evidence,
        record_summary.stale,
        record_summary.used,
        record_summary.projection_drift
    ));
    out.push_str(&format!(
        "  Flushes: {} completed · {} pending · {} running · {} failed · {} skipped\n",
        flushes.completed,
        flushes.pending,
        flushes.running,
        flushes.failed,
        flushes.skipped_duplicate
    ));
    out.push_str("  Quality gates: accept>=0.65 · propose>=0.45 · explicit>=0.60 with safety/duplicate hard stops\n");
    out.push_str(&format!(
        "  Calibration: {}/{} passed\n",
        calibration_passed,
        calibration.len()
    ));
    for result in calibration.iter().filter(|result| !result.passed).take(5) {
        let score = result
            .score
            .map(|score| format!("{score:.2}"))
            .unwrap_or_else(|| "n/a".to_string());
        out.push_str(&format!(
            "    FAIL {} expected={} actual={} score={} reason={}\n",
            result.id,
            result.expected.label(),
            result.actual.label(),
            score,
            compact_line(&result.reason, 120)
        ));
    }
    if conflicts.is_empty() {
        out.push_str("  Conflicts: none\n");
    } else {
        out.push_str(&format!("  Conflicts: {}\n", conflicts.len()));
        for conflict in conflicts.iter().take(5) {
            out.push_str("    ");
            out.push_str(conflict.trim_start_matches("- "));
            out.push('\n');
        }
    }
    let maintenance = memory_maintenance_decisions(docs, conflicts);
    if !maintenance.is_empty() {
        out.push_str("  Maintenance scores:\n");
        for (path, decision) in maintenance.iter().take(5) {
            out.push_str(&format!(
                "    {}: {:.2} {:?}\n",
                path, decision.score, decision.action
            ));
        }
    }
    out
}

fn memory_doctor_json(docs: &[MemoryDocument], conflicts: &[String]) -> serde_json::Value {
    let counts = load_memory_decision_counts();
    let flushes = load_memory_flush_summary();
    let calibration = crate::memory::run_memory_calibration_samples();
    let calibration_passed = calibration.iter().filter(|result| result.passed).count();
    let total_chars: usize = docs.iter().map(|doc| doc.content.chars().count()).sum();
    let topic_count = docs.iter().filter(|doc| doc.namespace == "topic").count();
    let agent_count = docs
        .iter()
        .filter(|doc| doc.namespace.starts_with("agent"))
        .count();
    let maintenance = memory_maintenance_decisions(docs, conflicts)
        .into_iter()
        .map(|(path, decision)| MemoryMaintenanceJson {
            path,
            score: decision.score,
            action: format!("{:?}", decision.action),
            reason: decision.reason,
        })
        .collect();
    let record_summary = crate::memory::MemoryManager::new().memory_record_summary();
    let report = MemoryDoctorJson {
        root: memory_root().display().to_string(),
        documents: MemoryDoctorDocumentsJson {
            total: docs.len(),
            topic: topic_count,
            agent: agent_count,
            chars: total_chars,
        },
        records: MemoryRecordSummaryJson {
            total: record_summary.total,
            accepted: record_summary.accepted,
            proposed: record_summary.proposed,
            rejected: record_summary.rejected,
            archived: record_summary.archived,
            superseded: record_summary.superseded,
            missing_evidence: record_summary.missing_evidence,
            stale: record_summary.stale,
            used: record_summary.used,
            projection_drift: record_summary.projection_drift,
        },
        decisions: MemoryDecisionCountsJson {
            accepted: counts.accepted,
            proposed: counts.proposed,
            rejected: counts.rejected,
            blocked: counts.blocked,
        },
        flushes: MemoryFlushCountsJson {
            completed: flushes.completed,
            pending: flushes.pending,
            running: flushes.running,
            failed: flushes.failed,
            skipped_duplicate: flushes.skipped_duplicate,
            total: flushes.total,
        },
        quality_gates: MemoryQualityGatesJson {
            accept_threshold: 0.65,
            propose_threshold: 0.45,
            explicit_override_threshold: 0.60,
            hard_stops: vec!["unsafe_content", "secret_like_content", "duplicate_memory"],
        },
        calibration: MemoryCalibrationReportJson {
            passed: calibration_passed,
            total: calibration.len(),
            results: calibration,
        },
        conflicts: conflicts.to_vec(),
        maintenance,
    };
    serde_json::to_value(report).unwrap_or_else(|_| serde_json::json!({}))
}

fn memory_maintenance_decisions(
    docs: &[MemoryDocument],
    conflicts: &[String],
) -> Vec<(String, crate::memory::MemoryKeepDecision)> {
    let mut decisions = docs
        .iter()
        .map(|doc| {
            let redundancy = repeated_line_ratio(&doc.content);
            let has_conflict = document_has_conflict(doc, conflicts);
            let factors = crate::memory::memory_keep_factors_from_document(
                &doc.namespace,
                &doc.content,
                has_conflict,
                redundancy,
            );
            (doc.path.clone(), crate::memory::score_memory_keep(factors))
        })
        .collect::<Vec<_>>();
    decisions.sort_by(|a, b| a.1.score.total_cmp(&b.1.score));
    decisions
}

fn document_has_conflict(doc: &MemoryDocument, conflicts: &[String]) -> bool {
    if conflicts.is_empty() {
        return false;
    }
    let lower_path = doc.path.to_lowercase();
    let lower_namespace = doc.namespace.to_lowercase();
    conflicts.iter().any(|conflict| {
        let lower = conflict.to_lowercase();
        lower.contains(&lower_path) || lower.contains(&lower_namespace)
    })
}

fn repeated_line_ratio(content: &str) -> f32 {
    let mut total = 0usize;
    let mut unique = HashSet::new();
    for line in content.lines().map(str::trim) {
        if line.len() < 12 {
            continue;
        }
        total += 1;
        unique.insert(line.to_lowercase());
    }
    if total == 0 {
        return 0.0;
    }
    ((total - unique.len()) as f32 / total as f32).clamp(0.0, 1.0)
}

fn extract_key_values(doc: &MemoryDocument) -> Vec<MemoryKeyValue> {
    doc.content
        .lines()
        .filter_map(|line| {
            let trimmed = line
                .trim()
                .trim_start_matches("- ")
                .trim_start_matches("* ");
            let (key, value) = trimmed.split_once(':')?;
            let key = key.trim().trim_matches('`');
            let value = value.trim();
            if key.is_empty()
                || value.is_empty()
                || key.starts_with('#')
                || key.chars().count() > 80
                || key.contains("://")
            {
                return None;
            }
            Some(MemoryKeyValue {
                namespace: format!("{}:{}", doc.namespace, doc.path),
                key: key.to_string(),
                value: value.to_string(),
            })
        })
        .collect()
}

fn normalize_value(value: &str) -> String {
    value
        .trim()
        .trim_end_matches('.')
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn compact_line(text: &str, max_chars: usize) -> String {
    let mut value = text.replace('\n', " ");
    if value.chars().count() > max_chars {
        value = value.chars().take(max_chars).collect::<String>();
        value.push_str("...");
    }
    value
}

fn sanitize_topic(topic: &str) -> Option<String> {
    let mut output = String::new();
    let mut last_dash = false;

    for ch in topic.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() || ch == '_' {
            output.push(ch);
            last_dash = false;
        } else if !last_dash {
            output.push('-');
            last_dash = true;
        }
    }

    let output = output
        .trim_matches('-')
        .chars()
        .take(80)
        .collect::<String>();
    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

/// Memory Save 工具 - 保存信息到持久记忆
pub struct MemorySaveTool;

#[async_trait]
impl Tool for MemorySaveTool {
    fn name(&self) -> &str {
        "memory_save"
    }

    fn description(&self) -> &str {
        "Save durable facts, preferences, decisions, and stable quirks to persistent memory. Do not save task progress, command history, or repeatable procedures; procedures belong in skills. By default it auto-routes to USER.md or memory/<topic>.md; use target=index to force MEMORY.md."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "Durable information to save. Exclude task progress, command history, and step-by-step procedures; procedures belong in skills."
                },
                "category": {
                    "type": "string",
                    "description": "Category: preference, convention, decision, note",
                    "enum": ["preference", "convention", "decision", "note"],
                    "default": "note"
                },
                "target": {
                    "type": "string",
                    "description": "Optional target: auto infers destination, index writes MEMORY.md, user writes USER.md, topic writes memory/<topic>.md",
                    "enum": ["auto", "index", "user", "topic"],
                    "default": "auto"
                },
                "topic": {
                    "type": "string",
                    "description": "Optional topic filename for memory/<topic>.md. Example: tui-design, context-management, rust-workflow"
                }
            },
            "required": ["content"]
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let content = params["content"].as_str().unwrap_or("");
        if content.is_empty() {
            return ToolResult::error("Content cannot be empty");
        }

        let category = params["category"].as_str().unwrap_or("note");
        let target = params["target"].as_str().unwrap_or("auto");
        let topic = params["topic"].as_str().unwrap_or("").trim();

        let mut candidate = crate::memory::MemoryCandidate::new(
            content,
            category,
            crate::memory::MemoryScope {
                project_root: Some(context.working_dir.clone()),
                session_id: context.session_id.clone(),
                platform: "tool".to_string(),
                ..Default::default()
            },
            crate::memory::MemoryProvenance {
                source: "memory_save_tool".to_string(),
                session_id: Some(context.session_id.clone()),
                turn_index: None,
                tool_name: Some("memory_save".to_string()),
            },
        )
        .explicit(true);
        candidate
            .evidence
            .push(crate::memory::MemoryEvidenceRef::new(
                crate::memory::MemoryEvidenceKind::ToolOutput,
                "memory_save_tool",
                "explicit memory_save tool call",
                0.85,
            ));

        let write_target = if target == "user" || category == "preference" {
            crate::memory::MemoryWriteTarget::User
        } else if target == "topic" || !topic.is_empty() {
            let topic = if topic.is_empty() { category } else { topic };
            if sanitize_topic(topic).is_none() {
                return ToolResult::error("Topic must contain at least one valid character");
            }
            crate::memory::MemoryWriteTarget::Topic(topic.to_string())
        } else if target == "index" {
            crate::memory::MemoryWriteTarget::Index
        } else {
            crate::memory::MemoryWriteTarget::Auto
        };

        let outcome = if let Some(memory_manager) = context.memory_manager.as_ref() {
            let manager = memory_manager.lock().await;
            manager
                .submit_candidate_with_provider_notifications(candidate, write_target)
                .await
        } else {
            let manager = crate::memory::MemoryManager::new();
            manager
                .submit_candidate_with_provider_notifications(candidate, write_target)
                .await
        };
        let path = outcome
            .path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| memory_root().display().to_string());
        let score = outcome
            .quality_score
            .map(|score| format!("{score:.2}"))
            .unwrap_or_else(|| "n/a".to_string());

        match outcome.status {
            crate::memory::manager::MemoryWriteOutcomeStatus::Saved => ToolResult::success(
                format!("Saved to {} (quality {}): [{}] {}", path, score, category, content),
            ),
            crate::memory::manager::MemoryWriteOutcomeStatus::Duplicate => ToolResult::success(
                format!(
                    "Memory already exists in {} (quality {}): [{}] {}",
                    path, score, category, content
                ),
            ),
            crate::memory::manager::MemoryWriteOutcomeStatus::Proposed => ToolResult::success(
                format!(
                    "Memory proposed for review, not injected as accepted memory yet (quality {}). Reason: {}",
                    score, outcome.reason
                ),
            ),
            crate::memory::manager::MemoryWriteOutcomeStatus::Rejected => ToolResult::success(
                format!(
                    "Memory not saved: quality gate rejected it (quality {}). Reason: {}",
                    score, outcome.reason
                ),
            ),
            crate::memory::manager::MemoryWriteOutcomeStatus::Blocked => ToolResult::error(
                format!("Blocked unsafe memory: {}", outcome.reason),
            ),
            crate::memory::manager::MemoryWriteOutcomeStatus::Failed => {
                ToolResult::error(format!("Failed to save memory: {}", outcome.reason))
            }
            crate::memory::manager::MemoryWriteOutcomeStatus::InvalidTarget => {
                ToolResult::error(format!("Invalid memory target: {}", outcome.reason))
            }
        }
    }
}

/// Memory Load 工具 - 读取持久记忆
pub struct MemoryLoadTool;

#[async_trait]
impl Tool for MemoryLoadTool {
    fn name(&self) -> &str {
        "memory_load"
    }

    fn description(&self) -> &str {
        "Load, search, or diagnose persistent memory from MEMORY.md, USER.md, memory/*.md, and agent memory namespaces."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "load returns memory content, search filters by query, doctor summarizes health, doctor_json returns machine-readable health, conflicts lists conflicts, review summarizes decisions/flushes/conflicts, explain shows why a matching memory was retrieved.",
                    "enum": ["load", "search", "doctor", "doctor_json", "conflicts", "review", "explain"],
                    "default": "load"
                },
                "query": {
                    "type": "string",
                    "description": "Optional: search query to filter memories. If empty, returns all memories."
                },
                "include_conflicts": {
                    "type": "boolean",
                    "description": "Whether to include duplicate/conflicting key hints across memory namespaces.",
                    "default": true
                }
            }
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let docs = load_memory_documents();
        let include_conflicts = params["include_conflicts"].as_bool().unwrap_or(true);
        let action = params["action"].as_str().unwrap_or("load");

        if docs.is_empty() {
            if action == "doctor_json" {
                return ToolResult::success(memory_doctor_json(&docs, &[]).to_string());
            }
            if matches!(action, "doctor" | "review") {
                return ToolResult::success(format_memory_doctor(&docs, &[]));
            }
            return ToolResult::success("Memory is empty.");
        }

        let query = params["query"].as_str().unwrap_or("");
        let conflicts = if include_conflicts {
            memory_conflicts(&docs, 8)
        } else {
            Vec::new()
        };

        if action == "doctor" {
            return ToolResult::success(format_memory_doctor(&docs, &conflicts));
        }

        if action == "doctor_json" {
            return ToolResult::success(memory_doctor_json(&docs, &conflicts).to_string());
        }

        if action == "conflicts" {
            return if conflicts.is_empty() {
                ToolResult::success("Memory conflicts: none")
            } else {
                ToolResult::success(format!("Memory Conflicts\n{}", conflicts.join("\n")))
            };
        }

        if action == "review" {
            return ToolResult::success(format_memory_doctor(&docs, &conflicts));
        }

        if action == "explain" {
            if query.trim().is_empty() {
                return ToolResult::error("query is required for memory explain");
            }
            let matching = search_memory_documents(&docs, query);
            return if matching.is_empty() {
                ToolResult::success(format!("No memories matching '{}'", query))
            } else {
                ToolResult::success(format!(
                    "Memory Explain\nselector: {}\nreason: matched memory namespace/path/content text. Use /memory search for retrieval ids in the interactive CLI.\n\n{}",
                    query,
                    matching.join("\n")
                ))
            };
        }

        if action == "search" || !query.is_empty() {
            let mut matching = search_memory_documents(&docs, query);

            if matching.is_empty() {
                ToolResult::success(format!("No memories matching '{}'", query))
            } else {
                if !conflicts.is_empty() {
                    matching.push(String::new());
                    matching.push("Conflicts:".to_string());
                    matching.extend(conflicts);
                }
                let result = matching.join("\n");
                let truncated: String = result.chars().take(3000).collect();
                ToolResult::success(truncated)
            }
        } else {
            // 返回全部（限制大小）
            let mut output = String::new();
            for doc in &docs {
                output.push_str(&format!("# [{}] {}\n", doc.namespace, doc.path));
                output.push_str(doc.content.trim());
                output.push_str("\n\n");
            }
            if !conflicts.is_empty() {
                output.push_str("# Conflicts\n");
                output.push_str(&conflicts.join("\n"));
                output.push('\n');
            }
            let truncated: String = output.chars().take(5000).collect();
            ToolResult::success(truncated)
        }
    }
}

/// Memory Clear 工具 - 清空记忆
pub struct MemoryClearTool;

#[async_trait]
impl Tool for MemoryClearTool {
    fn name(&self) -> &str {
        "memory_clear"
    }

    fn description(&self) -> &str {
        "Clear all persistent memory. Use with caution - this will delete all saved preferences and notes."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "confirm": {
                    "type": "boolean",
                    "description": "Must be true to confirm deletion"
                }
            },
            "required": ["confirm"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        if !params["confirm"].as_bool().unwrap_or(false) {
            return ToolResult::error("Set confirm=true to clear memory");
        }

        let path = memory_path();
        let memory_dir = memory_dir();
        let write_result = std::fs::write(&path, "# Priority Agent Memory\n");
        if memory_dir.exists() {
            let _ = std::fs::remove_dir_all(&memory_dir);
        }
        let _ = std::fs::create_dir_all(&memory_dir);

        match write_result {
            Ok(_) => ToolResult::success("Memory cleared"),
            Err(e) => ToolResult::error(format!("Failed to clear memory: {}", e)),
        }
    }

    fn requires_confirmation(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn confirmation_prompt(&self, _params: &serde_json::Value) -> Option<String> {
        Some("This will delete all saved memory. Continue?".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_path() {
        let path = memory_path();
        assert!(path.to_string_lossy().contains("MEMORY.md"));
    }

    #[test]
    fn test_sanitize_topic() {
        assert_eq!(sanitize_topic("TUI Design").as_deref(), Some("tui-design"));
        assert_eq!(
            sanitize_topic("../Context 管理.md").as_deref(),
            Some("context-管理-md")
        );
        assert_eq!(sanitize_topic("!!!"), None);
    }

    #[test]
    fn test_infer_topic() {
        assert_eq!(
            infer_topic("The TUI should keep Claude-style scroll anchoring.", "note"),
            Some("tui-design")
        );
        assert_eq!(
            infer_topic(
                "Prompt token budget should include memory snapshots.",
                "note"
            ),
            Some("context-management")
        );
        assert_eq!(
            infer_topic("User preference: respond in Chinese", "preference"),
            None
        );
    }

    #[test]
    fn test_memory_document_search_includes_namespaces() {
        let docs = vec![
            MemoryDocument {
                namespace: "user".to_string(),
                path: "USER.md".to_string(),
                content: "language: Chinese".to_string(),
            },
            MemoryDocument {
                namespace: "agent".to_string(),
                path: "memory/agents/reviewer.json".to_string(),
                content: "review_style: strict".to_string(),
            },
        ];

        let results = search_memory_documents(&docs, "strict");
        assert_eq!(results.len(), 1);
        assert!(results[0].starts_with("[agent:memory/agents/reviewer.json]"));
    }

    #[test]
    fn test_memory_conflicts_detect_duplicate_keys() {
        let docs = vec![
            MemoryDocument {
                namespace: "user".to_string(),
                path: "USER.md".to_string(),
                content: "language: Chinese".to_string(),
            },
            MemoryDocument {
                namespace: "topic".to_string(),
                path: "memory/preferences.md".to_string(),
                content: "language: English".to_string(),
            },
        ];

        let conflicts = memory_conflicts(&docs, 8);
        assert_eq!(conflicts.len(), 1);
        assert!(conflicts[0].contains("key 'language'"));
    }

    #[test]
    fn test_memory_decision_counts_from_jsonl() {
        let content = r#"{"status":"accepted"}
{"status":"blocked"}
{"status":"rejected"}
{"status":"accepted"}"#;
        let counts = memory_decision_counts_from_jsonl(content);
        assert_eq!(counts.accepted, 2);
        assert_eq!(counts.blocked, 1);
        assert_eq!(counts.rejected, 1);
    }

    #[test]
    fn test_format_memory_doctor_includes_conflicts_and_counts() {
        let docs = vec![MemoryDocument {
            namespace: "project".to_string(),
            path: "MEMORY.md".to_string(),
            content: "language: Chinese".to_string(),
        }];
        let doctor = format_memory_doctor(&docs, &["- key 'language' conflicts".to_string()]);
        assert!(doctor.contains("Memory Doctor"));
        assert!(doctor.contains("Documents: 1 total"));
        assert!(doctor.contains("Conflicts: 1"));
        assert!(doctor.contains("Quality gates:"));
        assert!(doctor.contains("Calibration:"));
    }

    #[test]
    fn test_memory_doctor_json_includes_calibration_and_gates() {
        let docs = vec![MemoryDocument {
            namespace: "project".to_string(),
            path: "MEMORY.md".to_string(),
            content: "language: Chinese".to_string(),
        }];
        let report = memory_doctor_json(&docs, &[]);
        assert_eq!(report["documents"]["total"].as_u64(), Some(1));
        assert!(report["calibration"]["total"].as_u64().unwrap_or(0) >= 1);
        let accept_threshold = report["quality_gates"]["accept_threshold"]
            .as_f64()
            .unwrap_or_default();
        assert!((accept_threshold - 0.65).abs() < 0.001);
    }

    #[test]
    fn test_agent_memory_json_formats_as_key_values() {
        let content = r#"[{"key":"review_style","value":"strict","created_at":1,"updated_at":1,"tags":["review"]}]"#;
        let formatted = format_agent_memory_content(content);
        assert!(formatted.contains("review_style: strict [review]"));
    }
}
