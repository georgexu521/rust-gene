//! Redaction helpers for durable LabRun audit evidence.
//!
//! Postdoc audit files are intentionally persistent. They should prove what was
//! inspected without copying raw credentials, bearer tokens, private keys, or
//! sensitive dotenv-style files into `.priority-agent/lab` artifacts.

use once_cell::sync::Lazy;
use regex::{Captures, Regex};
use sha2::{Digest, Sha256};
use std::io::{self, Read};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RedactedAuditText {
    pub(crate) text: String,
    pub(crate) redaction_applied: bool,
    pub(crate) redaction_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuditCapturedBytes {
    pub(crate) preview: Vec<u8>,
    pub(crate) byte_len: u64,
    pub(crate) content_hash: String,
    pub(crate) truncated: bool,
}

pub(crate) fn sensitive_audit_path_reason(path: &str) -> Option<&'static str> {
    let normalized = path.trim().replace('\\', "/").to_ascii_lowercase();
    let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    if file_name == ".env"
        || file_name.starts_with(".env.")
        || file_name.ends_with(".env")
        || file_name.ends_with(".env.local")
    {
        return Some("sensitive_path:dotenv");
    }
    if [".pem", ".key", ".p12", ".pfx"]
        .iter()
        .any(|suffix| file_name.ends_with(suffix))
    {
        return Some("sensitive_path:key_material");
    }
    if normalized.split('/').any(|segment| {
        segment.contains("credential")
            || segment.contains("secret")
            || segment.contains("api_key")
            || segment.contains("apikey")
    }) {
        return Some("sensitive_path:credential_name");
    }
    None
}

pub(crate) fn bulky_audit_path_reason(path: &str) -> Option<&'static str> {
    let normalized = path.trim().replace('\\', "/").to_ascii_lowercase();
    let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    if matches!(
        file_name,
        "cargo.lock" | "package-lock.json" | "pnpm-lock.yaml" | "yarn.lock" | "npm-shrinkwrap.json"
    ) {
        return Some("omitted_lockfile");
    }
    if normalized.split('/').any(|segment| {
        matches!(
            segment,
            "target"
                | "dist"
                | "build"
                | "vendor"
                | "vendors"
                | "node_modules"
                | ".next"
                | "out"
                | "coverage"
        )
    }) {
        return Some("omitted_generated_or_vendor");
    }
    if [
        ".png", ".jpg", ".jpeg", ".gif", ".webp", ".ico", ".pdf", ".zip", ".gz", ".tgz", ".tar",
        ".xz", ".7z", ".sqlite", ".sqlite3", ".db", ".wasm", ".dylib", ".so", ".dll", ".bin",
        ".mp4", ".mov",
    ]
    .iter()
    .any(|suffix| file_name.ends_with(suffix))
    {
        return Some("omitted_binary_or_media");
    }
    None
}

pub(crate) fn redact_lab_audit_text(text: &str) -> RedactedAuditText {
    let mut reasons = Vec::new();
    let mut redacted = text.to_string();

    redacted = PRIVATE_KEY_RE
        .replace_all(&redacted, |_caps: &Captures<'_>| {
            push_reason(&mut reasons, "private_key");
            "[REDACTED:private_key]".to_string()
        })
        .into_owned();
    redacted = SENSITIVE_ASSIGNMENT_RE
        .replace_all(&redacted, |caps: &Captures<'_>| {
            push_reason(&mut reasons, "secret_assignment");
            format!("{}=[REDACTED:secret_assignment]", &caps[1])
        })
        .into_owned();
    redacted = AUTH_BEARER_RE
        .replace_all(&redacted, |_caps: &Captures<'_>| {
            push_reason(&mut reasons, "authorization_bearer");
            "Authorization: Bearer [REDACTED:authorization_bearer]".to_string()
        })
        .into_owned();
    redacted = BEARER_RE
        .replace_all(&redacted, |_caps: &Captures<'_>| {
            push_reason(&mut reasons, "bearer_token");
            "Bearer [REDACTED:bearer_token]".to_string()
        })
        .into_owned();
    redacted = JWT_RE
        .replace_all(&redacted, |_caps: &Captures<'_>| {
            push_reason(&mut reasons, "jwt");
            "[REDACTED:jwt]".to_string()
        })
        .into_owned();
    redacted = OPENAI_STYLE_KEY_RE
        .replace_all(&redacted, |_caps: &Captures<'_>| {
            push_reason(&mut reasons, "api_key");
            "[REDACTED:api_key]".to_string()
        })
        .into_owned();
    redacted = HIGH_ENTROPY_RE
        .replace_all(&redacted, |caps: &Captures<'_>| {
            let value = caps.get(0).map(|m| m.as_str()).unwrap_or_default();
            if looks_high_entropy(value) {
                push_reason(&mut reasons, "high_entropy");
                "[REDACTED:high_entropy]".to_string()
            } else {
                value.to_string()
            }
        })
        .into_owned();

    reasons.sort();
    reasons.dedup();
    RedactedAuditText {
        redaction_applied: !reasons.is_empty(),
        redaction_reasons: reasons,
        text: redacted,
    }
}

#[cfg(test)]
pub(crate) fn audit_text_hash(text: &str) -> String {
    audit_bytes_hash(text.as_bytes())
}

#[cfg(test)]
pub(crate) fn audit_bytes_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

pub(crate) fn audit_reader_hash(mut reader: impl Read) -> io::Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

pub(crate) fn capture_reader_with_hash(
    mut reader: impl Read,
    max_preview_bytes: usize,
) -> io::Result<AuditCapturedBytes> {
    let mut hasher = Sha256::new();
    let mut preview = Vec::new();
    let mut byte_len = 0_u64;
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        byte_len += read as u64;
        hasher.update(&buffer[..read]);
        if preview.len() < max_preview_bytes {
            let remaining = max_preview_bytes - preview.len();
            let take = read.min(remaining);
            preview.extend_from_slice(&buffer[..take]);
        }
    }
    Ok(AuditCapturedBytes {
        truncated: byte_len as usize > preview.len(),
        preview,
        byte_len,
        content_hash: format!("sha256:{:x}", hasher.finalize()),
    })
}

fn push_reason(reasons: &mut Vec<String>, reason: &str) {
    reasons.push(reason.to_string());
}

fn looks_high_entropy(value: &str) -> bool {
    if value.len() < 32 {
        return false;
    }
    let has_lower = value.chars().any(|ch| ch.is_ascii_lowercase());
    let has_upper = value.chars().any(|ch| ch.is_ascii_uppercase());
    let has_digit = value.chars().any(|ch| ch.is_ascii_digit());
    let has_symbol = value
        .chars()
        .any(|ch| matches!(ch, '_' | '-' | '+' | '/' | '=' | '.'));
    let class_count = [has_lower, has_upper, has_digit, has_symbol]
        .into_iter()
        .filter(|present| *present)
        .count();
    if class_count < 3 {
        return false;
    }
    let mut unique = value.chars().collect::<Vec<_>>();
    unique.sort_unstable();
    unique.dedup();
    unique.len() >= 12
}

static PRIVATE_KEY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?is)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----.*?-----END [A-Z0-9 ]*PRIVATE KEY-----")
        .expect("valid private key redaction regex")
});

static SENSITIVE_ASSIGNMENT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?i)\b([A-Z0-9_]*(?:API[_-]?KEY|TOKEN|SECRET|PASSWORD|PRIVATE[_-]?KEY|ACCESS[_-]?KEY)[A-Z0-9_]*)\s*=\s*["']?[^\s"']+["']?"#,
    )
    .expect("valid sensitive assignment redaction regex")
});

static AUTH_BEARER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\bauthorization\s*:\s*bearer\s+[A-Za-z0-9._~+/=-]{8,}")
        .expect("valid authorization bearer redaction regex")
});

static BEARER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{12,}").expect("valid bearer redaction regex")
});

static JWT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b")
        .expect("valid jwt redaction regex")
});

static OPENAI_STYLE_KEY_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\bsk-[A-Za-z0-9_-]{16,}\b").expect("valid API key redaction regex"));

static HIGH_ENTROPY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b[A-Za-z0-9._+/=-]{32,}\b").expect("valid high entropy redaction regex")
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_secret_like_audit_text() {
        let raw = concat!(
            "OPENAI_API_KEY=sk-testabcdefghijklmnopqrstuvwxyz\n",
            "Authorization: Bearer abcdefghijklmnopqrstuvwxyz123456\n",
            "-----BEGIN PRIVATE KEY-----\nabc123\n-----END PRIVATE KEY-----\n",
            "token=AbCdEfGhIjKlMnOpQrStUvWxYz1234567890"
        );

        let redacted = redact_lab_audit_text(raw);

        assert!(redacted.redaction_applied);
        assert!(redacted
            .redaction_reasons
            .contains(&"secret_assignment".to_string()));
        assert!(redacted
            .redaction_reasons
            .contains(&"authorization_bearer".to_string()));
        assert!(redacted
            .redaction_reasons
            .contains(&"private_key".to_string()));
        assert!(!redacted.text.contains("sk-testabcdefghijklmnopqrstuvwxyz"));
        assert!(!redacted.text.contains("abcdefghijklmnopqrstuvwxyz123456"));
        assert!(!redacted.text.contains("BEGIN PRIVATE KEY"));
        assert!(!redacted
            .text
            .contains("AbCdEfGhIjKlMnOpQrStUvWxYz1234567890"));
    }

    #[test]
    fn classifies_sensitive_audit_paths() {
        for path in [
            ".env",
            ".env.example",
            "config/app.env",
            "certs/client.pem",
            "keys/provider.key",
            "config/credentials.toml",
            "fixtures/secrets/provider.json",
        ] {
            assert!(
                sensitive_audit_path_reason(path).is_some(),
                "{path} should be sensitive"
            );
        }
        assert!(sensitive_audit_path_reason("src/lab/model.rs").is_none());
    }

    #[test]
    fn classifies_bulky_audit_paths() {
        for path in [
            "Cargo.lock",
            "apps/desktop/pnpm-lock.yaml",
            "target/debug/build.log",
            "vendor/bundle.js",
            "dist/app.js",
            "screenshots/ui.png",
            "docs/report.pdf",
        ] {
            assert!(
                bulky_audit_path_reason(path).is_some(),
                "{path} should be omitted as bulky or low-value"
            );
        }
        assert!(bulky_audit_path_reason("src/lab/model.rs").is_none());
    }

    #[test]
    fn hashes_are_stable_and_prefixed() {
        assert_eq!(audit_text_hash("abc"), audit_text_hash("abc"));
        assert_ne!(audit_text_hash("abc"), audit_text_hash("abcd"));
        assert!(audit_text_hash("abc").starts_with("sha256:"));
    }

    #[test]
    fn captures_reader_with_bounded_preview_and_hash() {
        let data = b"abcdefghijklmnopqrstuvwxyz";
        let captured = capture_reader_with_hash(&data[..], 8).unwrap();
        assert_eq!(captured.preview, b"abcdefgh");
        assert_eq!(captured.byte_len, 26);
        assert!(captured.truncated);
        assert_eq!(captured.content_hash, audit_bytes_hash(data));
    }
}
