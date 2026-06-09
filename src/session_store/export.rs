//! Session export with privacy tiers.
//!
//! Exports session data to JSON or Markdown files with configurable
//! privacy levels: Full, Redacted, or Summary.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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

/// The complete export payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionExport {
    pub meta: SessionExportMeta,
    pub messages: Vec<ExportMessage>,
    pub tool_stats: serde_json::Value,
}

/// Minimal input for building an export.
pub struct SessionExportInput {
    pub session_id: String,
    pub title: Option<String>,
    pub model: Option<String>,
    pub messages: Vec<ExportMessage>,
    pub changed_files: Vec<String>,
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

    SessionExport {
        meta: SessionExportMeta {
            session_id: input.session_id,
            title: input.title,
            model: input.model,
            message_count: messages.len(),
            changed_files: input.changed_files,
            privacy: privacy.label().to_string(),
            export_format: format!("{:?}", format).to_lowercase(),
            exported_at: chrono::Local::now().to_rfc3339(),
            agent_version: env!("CARGO_PKG_VERSION").to_string(),
        },
        messages,
        tool_stats: serde_json::json!({}),
    }
}

/// Redact sensitive content from a message.
pub fn redact_content(content: &str) -> String {
    content
        .lines()
        .filter(|line| {
            let lower = line.to_lowercase();
            !(lower.contains("api_key")
                || lower.contains("secret")
                || lower.contains("token")
                || lower.contains("password")
                || lower.contains("credential"))
        })
        .collect::<Vec<_>>()
        .join("\n")
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
        export.meta.title.as_deref().unwrap_or(&export.meta.session_id)
    ));
    out.push_str(&format!(
        "- **Session ID**: {}\n\
         - **Model**: {}\n\
         - **Messages**: {}\n\
         - **Changed files**: {}\n\
         - **Privacy**: {}\n\
         - **Exported at**: {}\n\n",
        export.meta.session_id,
        export.meta.model.as_deref().unwrap_or("unknown"),
        export.meta.message_count,
        export.meta.changed_files.join(", "),
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
            changed_files: vec!["src/main.rs".into()],
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
        let export = build_export(input, SessionExportPrivacy::Redacted, SessionExportFormat::Json);
        assert_eq!(export.messages.len(), 2);
        let assistant = &export.messages[1];
        assert!(!assistant.content.contains("API_KEY"));
        assert!(assistant.content.contains("normal reply"));
    }

    #[test]
    fn summary_export_has_no_messages() {
        let input = sample_input();
        let export = build_export(input, SessionExportPrivacy::Summary, SessionExportFormat::Json);
        assert_eq!(export.messages.len(), 0);
    }

    #[test]
    fn markdown_export_includes_metadata() {
        let input = sample_input();
        let export = build_export(input, SessionExportPrivacy::Full, SessionExportFormat::Markdown);
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
