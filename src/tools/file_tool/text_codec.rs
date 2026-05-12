use serde_json::json;
use std::hash::{Hash, Hasher};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TextFileEncoding {
    Utf8,
    Utf16Le,
}

impl TextFileEncoding {
    pub(super) fn label(self) -> &'static str {
        match self {
            TextFileEncoding::Utf8 => "utf-8",
            TextFileEncoding::Utf16Le => "utf-16le",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LineEndingStyle {
    Lf,
    Crlf,
}

impl LineEndingStyle {
    pub(super) fn label(self) -> &'static str {
        match self {
            LineEndingStyle::Lf => "LF",
            LineEndingStyle::Crlf => "CRLF",
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct TextFileSnapshot {
    pub(super) content: String,
    pub(super) encoding: TextFileEncoding,
    pub(super) has_bom: bool,
    pub(super) line_ending: LineEndingStyle,
    pub(super) byte_len: usize,
    pub(super) raw_content_hash: String,
}

pub(super) async fn read_text_file(
    path: &Path,
    operation: &str,
) -> Result<TextFileSnapshot, String> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| format!("Failed to {} file '{}': {}", operation, path.display(), e))?;
    decode_text_file(path, operation, bytes)
}

pub(super) fn decode_text_file(
    path: &Path,
    operation: &str,
    bytes: Vec<u8>,
) -> Result<TextFileSnapshot, String> {
    let raw_content_hash = bytes_hash_hex(&bytes);
    let byte_len = bytes.len();
    let (encoding, has_bom, text) = if bytes.starts_with(&[0xff, 0xfe]) {
        (
            TextFileEncoding::Utf16Le,
            true,
            decode_utf16le(path, operation, &bytes[2..])?,
        )
    } else {
        let (has_bom, body) = if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
            (true, &bytes[3..])
        } else {
            (false, bytes.as_slice())
        };
        if body.contains(&0) {
            return Err(format!(
                "Refusing to {} file '{}': binary or unknown text encoding detected",
                operation,
                path.display()
            ));
        }
        let text = String::from_utf8(body.to_vec()).map_err(|_| {
            format!(
                "Refusing to {} file '{}': unsupported text encoding (expected UTF-8 or UTF-16LE with BOM)",
                operation,
                path.display()
            )
        })?;
        (TextFileEncoding::Utf8, has_bom, text)
    };
    let line_ending = detect_line_ending(&text);
    Ok(TextFileSnapshot {
        content: normalize_text_line_endings(&text),
        encoding,
        has_bom,
        line_ending,
        byte_len,
        raw_content_hash,
    })
}

fn bytes_hash_hex(bytes: &[u8]) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn decode_utf16le(path: &Path, operation: &str, body: &[u8]) -> Result<String, String> {
    if !body.len().is_multiple_of(2) {
        return Err(format!(
            "Refusing to {} file '{}': invalid UTF-16LE byte length",
            operation,
            path.display()
        ));
    }
    let units = body
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&units).map_err(|_| {
        format!(
            "Refusing to {} file '{}': invalid UTF-16LE content",
            operation,
            path.display()
        )
    })
}

pub(super) fn detect_line_ending(content: &str) -> LineEndingStyle {
    let mut crlf_count = 0usize;
    let mut lf_count = 0usize;
    let mut previous = '\0';
    for ch in content.chars() {
        if ch == '\n' {
            if previous == '\r' {
                crlf_count += 1;
            } else {
                lf_count += 1;
            }
        }
        previous = ch;
    }
    if crlf_count > lf_count {
        LineEndingStyle::Crlf
    } else {
        LineEndingStyle::Lf
    }
}

pub(super) fn normalize_text_line_endings(content: &str) -> String {
    content.replace("\r\n", "\n")
}

fn convert_to_line_ending(content: &str, line_ending: LineEndingStyle) -> String {
    let normalized = normalize_text_line_endings(content);
    match line_ending {
        LineEndingStyle::Lf => normalized,
        LineEndingStyle::Crlf => normalized.replace('\n', "\r\n"),
    }
}

pub(super) fn split_leading_text_bom(content: &str) -> (bool, &str) {
    match content.strip_prefix('\u{feff}') {
        Some(stripped) => (true, stripped),
        None => (false, content),
    }
}

pub(super) fn encode_text_content(
    content: &str,
    encoding: TextFileEncoding,
    has_bom: bool,
    line_ending: LineEndingStyle,
) -> Vec<u8> {
    let content = convert_to_line_ending(content, line_ending);
    match encoding {
        TextFileEncoding::Utf8 => {
            let mut bytes = Vec::new();
            if has_bom {
                bytes.extend_from_slice(&[0xef, 0xbb, 0xbf]);
            }
            bytes.extend_from_slice(content.as_bytes());
            bytes
        }
        TextFileEncoding::Utf16Le => {
            let mut bytes = Vec::new();
            if has_bom {
                bytes.extend_from_slice(&[0xff, 0xfe]);
            }
            for unit in content.encode_utf16() {
                bytes.extend_from_slice(&unit.to_le_bytes());
            }
            bytes
        }
    }
}

pub(super) async fn write_text_file(
    path: &Path,
    content: &str,
    encoding: TextFileEncoding,
    has_bom: bool,
    line_ending: LineEndingStyle,
    max_size_bytes: u64,
) -> Result<usize, String> {
    let bytes = encode_text_content(content, encoding, has_bom, line_ending);
    let bytes_written = bytes.len();
    if bytes_written as u64 > max_size_bytes {
        return Err(format!(
            "Refusing to write encoded content larger than {} bytes",
            max_size_bytes
        ));
    }
    tokio::fs::write(path, bytes)
        .await
        .map_err(|e| format!("Failed to write file: {}", e))?;
    Ok(bytes_written)
}

pub(super) fn text_format_json(snapshot: &TextFileSnapshot) -> serde_json::Value {
    json!({
        "encoding": snapshot.encoding.label(),
        "bom": snapshot.has_bom,
        "line_ending": snapshot.line_ending.label(),
        "raw_content_hash": snapshot.raw_content_hash,
    })
}

pub(super) fn text_write_format_json(
    encoding: TextFileEncoding,
    has_bom: bool,
    line_ending: LineEndingStyle,
) -> serde_json::Value {
    json!({
        "encoding": encoding.label(),
        "bom": has_bom,
        "line_ending": line_ending.label(),
    })
}
