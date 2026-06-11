//! Session export with privacy tiers.
//!
//! Exports session data to JSON or Markdown files with configurable
//! privacy levels: Full, Redacted, or Summary.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use regex::Regex;

/// Export format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionExportFormat {
    Json,
    Markdown,
}

/// Privacy tier for exported content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionExportPrivacy {
    Full,
    Redacted,
    Summary,
}

impl SessionExportPrivacy {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Redacted => "redacted",
            Self::Summary => "summary",
        }
    }
}

/// Metadata included in every export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionExportMeta {
    pub session_id: String,
    pub title: Option<String>,
    pub model: Option<String>,
    pub message_count: usize,
    pub changed_files: Vec<String>,
    pub warnings: Vec<String>,
    pub privacy: String,
    pub export_format: String,
    pub exported_at: String,
    pub agent_version: String,
}

/// A single message in the export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportMessage {
    pub role: String,
    pub content: String,
    pub timestamp: Option<String>,
}

/// A session part in the export (lightweight projection).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportPart {
    pub part_id: String,
    pub kind: String,
    pub tool_name: Option<String>,
    pub status: Option<String>,
    pub message_id: Option<String>,
}

/// The complete export payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionExport {
    pub meta: SessionExportMeta,
    pub messages: Vec<ExportMessage>,
    pub parts: Vec<ExportPart>,
    pub reverts: Vec<ExportRevert>,
    pub diagnostics: Vec<ExportDiagnosticRecord>,
    pub tool_stats: serde_json::Value,
    pub closeout_status: Option<String>,
    pub compaction_count: usize,
    pub unresolved_settlement: Vec<String>,
    pub tool_outputs: Vec<ExportToolOutput>,
}

/// Tool output metadata in the export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportToolOutput {
    pub id: String,
    pub tool_name: String,
    pub original_bytes: u64,
}

/// Revert metadata included in richer exports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportRevert {
    pub operation: String,
    pub status: String,
    pub paths: Vec<String>,
    pub diff_summary: Option<String>,
    pub unreverted: bool,
    pub created_at: String,
}

/// Optional diagnostic summary included in exports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportDiagnosticRecord {
    pub source: String,
    pub status: String,
    pub path: Option<String>,
    pub error_count: usize,
    pub warning_count: usize,
    pub detail: Option<String>,
}

/// Minimal input for building an export.
pub struct SessionExportInput {
    pub session_id: String,
    pub title: Option<String>,
    pub model: Option<String>,
    pub messages: Vec<ExportMessage>,
    pub parts: Vec<ExportPart>,
    pub changed_files: Vec<String>,
    pub reverts: Vec<ExportRevert>,
    pub diagnostics: Vec<ExportDiagnosticRecord>,
    pub tool_stats: serde_json::Value,
    pub warnings: Vec<String>,
    pub closeout_status: Option<String>,
    pub compaction_count: usize,
    pub unresolved_settlement: Vec<String>,
    pub tool_outputs: Vec<ExportToolOutput>,
}

/// Build a session export.
pub fn build_export(
    input: SessionExportInput,
    privacy: SessionExportPrivacy,
    format: SessionExportFormat,
) -> SessionExport {
    let messages = match privacy {
        SessionExportPrivacy::Full => input.messages,
        SessionExportPrivacy::Redacted => input
            .messages
            .into_iter()
            .map(|m| ExportMessage {
                content: redact_content(&m.content),
                ..m
            })
            .collect(),
        SessionExportPrivacy::Summary => vec![],
    };
    let changed_files = match privacy {
        SessionExportPrivacy::Full => input.changed_files,
        SessionExportPrivacy::Redacted | SessionExportPrivacy::Summary => vec![],
    };
    let reverts = match privacy {
        SessionExportPrivacy::Full => input.reverts,
        SessionExportPrivacy::Redacted => input
            .reverts
            .into_iter()
            .map(|revert| ExportRevert {
                paths: vec![],
                diff_summary: None,
                ..revert
            })
            .collect(),
        SessionExportPrivacy::Summary => vec![],
    };
    let diagnostics = match privacy {
        SessionExportPrivacy::Full => input.diagnostics,
        SessionExportPrivacy::Redacted => input
            .diagnostics
            .into_iter()
            .map(|diagnostic| ExportDiagnosticRecord {
                path: None,
                detail: None,
                ..diagnostic
            })
            .collect(),
        SessionExportPrivacy::Summary => vec![],
    };
    let tool_stats = match privacy {
        SessionExportPrivacy::Full | SessionExportPrivacy::Redacted => input.tool_stats,
        SessionExportPrivacy::Summary => serde_json::json!({}),
    };
    let parts = match privacy {
        SessionExportPrivacy::Full | SessionExportPrivacy::Redacted => input.parts,
        SessionExportPrivacy::Summary => vec![],
    };
    let tool_outputs = match privacy {
        SessionExportPrivacy::Full | SessionExportPrivacy::Redacted => input.tool_outputs,
        SessionExportPrivacy::Summary => vec![],
    };
    let unresolved_settlement = match privacy {
        SessionExportPrivacy::Full => input.unresolved_settlement,
        SessionExportPrivacy::Redacted | SessionExportPrivacy::Summary => vec![],
    };

    SessionExport {
        meta: SessionExportMeta {
            session_id: input.session_id,
            title: input.title,
            model: input.model,
            message_count: messages.len(),
            changed_files,
            warnings: input.warnings,
            privacy: privacy.label().to_string(),
            export_format: format!("{:?}", format).to_lowercase(),
            exported_at: chrono::Local::now().to_rfc3339(),
            agent_version: env!("CARGO_PKG_VERSION").to_string(),
        },
        messages,
        parts,
        reverts,
        diagnostics,
        tool_stats,
        closeout_status: input.closeout_status,
        compaction_count: input.compaction_count,
        unresolved_settlement,
        tool_outputs,
    }
}

/// Redact sensitive content from a message.
pub fn redact_content(content: &str) -> String {
    content
        .lines()
        .map(redact_sensitive_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn redact_sensitive_line(line: &str) -> String {
    static SENSITIVE_ASSIGNMENT_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(
            r#"(?ix)
            \b(?:api[_-]?key|secret|token|password|credential|authorization)\b
            \s*[:=]\s*
            (?:"[^"]*"|'[^']*'|[^\s,;]+)
            "#,
        )
        .expect("valid sensitive assignment redaction regex")
    });
    static BEARER_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{12,}").expect("valid bearer redaction regex")
    });
    static STANDALONE_SECRET_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(
            r#"(?ix)
            \b(?:
                sk|pk|rk|ghp|gho|ghu|ghs|ghr|github_pat|glpat|hf|xox[baprs]
            )[-_][A-Za-z0-9][A-Za-z0-9._-]{10,}\b
            "#,
        )
        .expect("valid standalone secret redaction regex")
    });
    static JWT_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b")
            .expect("valid jwt redaction regex")
    });

    let line = BEARER_RE.replace_all(line, "Bearer [REDACTED]");
    let line = STANDALONE_SECRET_RE.replace_all(&line, "[REDACTED]");
    let line = JWT_RE.replace_all(&line, "[REDACTED]");
    SENSITIVE_ASSIGNMENT_RE
        .replace_all(&line, "[REDACTED]")
        .to_string()
}

/// Serialize the export to a string.
pub fn serialize(export: &SessionExport, format: SessionExportFormat) -> anyhow::Result<String> {
    match format {
        SessionExportFormat::Json => Ok(serde_json::to_string_pretty(export)?),
        SessionExportFormat::Markdown => serialize_markdown(export),
    }
}

fn serialize_markdown(export: &SessionExport) -> anyhow::Result<String> {
    let mut out = String::new();
    out.push_str(&format!(
        "# Session Export: {}\n\n",
        export
            .meta
            .title
            .as_deref()
            .unwrap_or(&export.meta.session_id)
    ));
    out.push_str(&format!(
        "- **Session ID**: {}\n\
         - **Model**: {}\n\
         - **Messages**: {}\n\
         - **Changed files**: {}\n\
         - **Reverts**: {}\n\
         - **Diagnostics records**: {}\n\
         - **Warnings**: {}\n\
         - **Privacy**: {}\n\
         - **Exported at**: {}\n\n",
        export.meta.session_id,
        export.meta.model.as_deref().unwrap_or("unknown"),
        export.meta.message_count,
        export.meta.changed_files.join(", "),
        export.reverts.len(),
        export.diagnostics.len(),
        export.meta.warnings.len(),
        export.meta.privacy,
        export.meta.exported_at,
    ));

    out.push_str("## Messages\n\n");
    for msg in &export.messages {
        out.push_str(&format!("### {}\n\n", msg.role.to_uppercase()));
        if !msg.content.is_empty() {
            out.push_str(&format!("{}\n\n", msg.content));
        }
    }

    if !export.reverts.is_empty() {
        out.push_str("## Reverts\n\n");
        for revert in &export.reverts {
            out.push_str(&format!(
                "- {} [{}] paths={} unreverted={} {}\n",
                revert.operation,
                revert.status,
                revert.paths.join(", "),
                revert.unreverted,
                revert.diff_summary.as_deref().unwrap_or("")
            ));
        }
        out.push('\n');
    }

    if !export.diagnostics.is_empty() {
        out.push_str("## Diagnostics\n\n");
        for diagnostic in &export.diagnostics {
            out.push_str(&format!(
                "- {} [{}] path={} errors={} warnings={} {}\n",
                diagnostic.source,
                diagnostic.status,
                diagnostic.path.as_deref().unwrap_or("none"),
                diagnostic.error_count,
                diagnostic.warning_count,
                diagnostic.detail.as_deref().unwrap_or("")
            ));
        }
        out.push('\n');
    }

    if !export.meta.warnings.is_empty() {
        out.push_str("## Export Warnings\n\n");
        for warning in &export.meta.warnings {
            out.push_str(&format!("- {}\n", warning));
        }
        out.push('\n');
    }

    Ok(out)
}

/// Default export directory.
pub fn default_export_dir() -> PathBuf {
    dirs::data_dir()
        .map(|d| d.join("priority-agent").join("exports"))
        .unwrap_or_else(|| PathBuf::from(".priority-agent/exports"))
}

/// Write the export to a file and return the path.
pub fn write_export(
    export: &SessionExport,
    dir: &Path,
    format: SessionExportFormat,
) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let ext = match format {
        SessionExportFormat::Json => "json",
        SessionExportFormat::Markdown => "md",
    };
    let filename = format!(
        "session-{}-{}.{ext}",
        chrono::Local::now().format("%Y%m%d-%H%M%S"),
        &export.meta.session_id.chars().take(8).collect::<String>(),
    );
    let path = dir.join(filename);
    let content = serialize(export, format)?;
    std::fs::write(&path, &content)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_input() -> SessionExportInput {
        SessionExportInput {
            session_id: "test-session-1".into(),
            title: Some("Test Session".into()),
            model: Some("gpt-4o".into()),
            messages: vec![
                ExportMessage {
                    role: "user".into(),
                    content: "hello world".into(),
                    timestamp: None,
                },
                ExportMessage {
                    role: "assistant".into(),
                    content: "API_KEY=sk-secret\nnormal reply".into(),
                    timestamp: None,
                },
            ],
            parts: vec![ExportPart {
                part_id: "tool_c1".into(),
                kind: "tool".into(),
                tool_name: Some("bash".into()),
                status: Some("completed".into()),
                message_id: None,
            }],
            changed_files: vec!["src/main.rs".into()],
            reverts: vec![ExportRevert {
                operation: "checkpoint_revert".into(),
                status: "completed".into(),
                paths: vec!["src/private.rs".into()],
                diff_summary: Some("secret diff summary".into()),
                unreverted: false,
                created_at: "2026-06-09T00:00:00Z".into(),
            }],
            diagnostics: vec![ExportDiagnosticRecord {
                source: "lsp".into(),
                status: "recorded".into(),
                path: Some("src/private.rs".into()),
                error_count: 1,
                warning_count: 2,
                detail: Some("private diagnostic detail".into()),
            }],
            tool_stats: serde_json::json!({"calls": {"file_write": 1}}),
            warnings: vec![],
            closeout_status: Some("passed".into()),
            compaction_count: 0,
            unresolved_settlement: vec![],
            tool_outputs: vec![ExportToolOutput {
                id: "tool-output-1".into(),
                tool_name: "bash".into(),
                original_bytes: 1024,
            }],
        }
    }

    #[test]
    fn privacy_labels_are_distinct() {
        assert_eq!(SessionExportPrivacy::Full.label(), "full");
        assert_eq!(SessionExportPrivacy::Redacted.label(), "redacted");
        assert_eq!(SessionExportPrivacy::Summary.label(), "summary");
    }

    #[test]
    fn full_export_includes_all_messages() {
        let input = sample_input();
        let export = build_export(input, SessionExportPrivacy::Full, SessionExportFormat::Json);
        assert_eq!(export.messages.len(), 2);
    }

    #[test]
    fn redacted_export_strips_secrets() {
        let input = sample_input();
        let export = build_export(
            input,
            SessionExportPrivacy::Redacted,
            SessionExportFormat::Json,
        );
        assert_eq!(export.messages.len(), 2);
        let assistant = &export.messages[1];
        assert!(!assistant.content.contains("API_KEY"));
        assert!(assistant.content.contains("normal reply"));
        assert!(export.meta.changed_files.is_empty());
        assert!(export.reverts[0].paths.is_empty());
        assert!(export.reverts[0].diff_summary.is_none());
        assert!(export.diagnostics[0].path.is_none());
        assert!(export.diagnostics[0].detail.is_none());
        assert_eq!(export.tool_stats["calls"]["file_write"], 1);
    }

    #[test]
    fn redaction_replaces_secret_shapes_without_keyword_labels() {
        let content = "\
normal line
use sk-live-abcdefghijklmnopqrstuvwxyz here
Authorization: Bearer abcdefghijklmnopqrstuvwxyz012345
jwt eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJnZXgifQ.signaturevalue";

        let redacted = redact_content(content);

        assert!(redacted.contains("normal line"));
        assert!(!redacted.contains("sk-live-"));
        assert!(!redacted.contains("abcdefghijklmnopqrstuvwxyz012345"));
        assert!(!redacted.contains("eyJhbGci"));
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn redacted_markdown_hides_metadata_paths_and_details() {
        let input = sample_input();
        let export = build_export(
            input,
            SessionExportPrivacy::Redacted,
            SessionExportFormat::Markdown,
        );
        let md = serialize_markdown(&export).expect("md serialize");
        assert!(!md.contains("src/main.rs"));
        assert!(!md.contains("src/private.rs"));
        assert!(!md.contains("secret diff summary"));
        assert!(!md.contains("private diagnostic detail"));
    }

    #[test]
    fn summary_export_has_no_messages() {
        let input = sample_input();
        let export = build_export(
            input,
            SessionExportPrivacy::Summary,
            SessionExportFormat::Json,
        );
        assert_eq!(export.messages.len(), 0);
        assert!(export.meta.changed_files.is_empty());
        assert!(export.reverts.is_empty());
        assert!(export.diagnostics.is_empty());
        assert_eq!(export.tool_stats, serde_json::json!({}));
    }

    #[test]
    fn markdown_export_includes_metadata() {
        let input = sample_input();
        let export = build_export(
            input,
            SessionExportPrivacy::Full,
            SessionExportFormat::Markdown,
        );
        let md = serialize_markdown(&export).expect("md serialize");
        assert!(md.contains("# Session Export"));
        assert!(md.contains("Test Session"));
        assert!(md.contains("### USER"));
    }

    #[test]
    fn json_export_is_valid_json() {
        let input = sample_input();
        let export = build_export(input, SessionExportPrivacy::Full, SessionExportFormat::Json);
        let json = serialize(&export, SessionExportFormat::Json).expect("serialize");
        let _parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    }
}
