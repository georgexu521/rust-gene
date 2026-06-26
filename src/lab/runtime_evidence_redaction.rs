//! Shared redaction for durable runtime evidence.
//!
//! LabRun evidence can include command output, tool previews, provider errors,
//! and audit snippets. All persistent evidence should pass through this layer
//! before being written to events or artifacts.

use crate::lab::audit_redaction::{redact_lab_audit_text, RedactedAuditText};

pub(crate) fn redact_runtime_evidence_text(text: &str) -> RedactedAuditText {
    redact_lab_audit_text(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_runtime_validation_evidence() {
        let raw = concat!(
            "OPENAI_API_KEY=sk-testabcdefghijklmnopqrstuvwxyz\n",
            "Authorization: Bearer abcdefghijklmnopqrstuvwxyz123456\n",
            "-----BEGIN PRIVATE KEY-----\nabc123\n-----END PRIVATE KEY-----"
        );

        let redacted = redact_runtime_evidence_text(raw);

        assert!(redacted.redaction_applied);
        assert!(!redacted.text.contains("sk-testabcdefghijklmnopqrstuvwxyz"));
        assert!(!redacted.text.contains("abcdefghijklmnopqrstuvwxyz123456"));
        assert!(!redacted.text.contains("BEGIN PRIVATE KEY"));
    }
}
