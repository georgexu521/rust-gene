//! 编码/解码工具
//!
//! Base64、URL 编码、HTML 实体编码等

use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// 编码/解码工具
pub struct EncodeTool;

fn base64_encode(data: &str) -> String {
    let mut buf = Vec::new();
    // Simple base64 encoding without external crate
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = data.as_bytes();
    for chunk in bytes.chunks(3) {
        let mut n = 0u32;
        for (i, &b) in chunk.iter().enumerate() {
            n |= (b as u32) << (16 - i * 8);
        }
        for i in 0..4 {
            if i <= chunk.len() {
                buf.push(CHARSET[((n >> (18 - i * 6)) & 0x3F) as usize]);
            } else {
                buf.push(b'=');
            }
        }
    }
    String::from_utf8(buf).unwrap_or_default()
}

fn base64_decode(data: &str) -> Result<Vec<u8>, String> {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = Vec::new();
    let mut buf = 0u32;
    let mut buf_len = 0;

    for c in data.chars().filter(|&c| c != '=' && !c.is_whitespace()) {
        let val = CHARSET
            .iter()
            .position(|&b| b as char == c)
            .ok_or_else(|| format!("Invalid base64 character: {}", c))?;
        buf = (buf << 6) | val as u32;
        buf_len += 1;
        if buf_len == 4 {
            result.push((buf >> 16) as u8);
            result.push((buf >> 8) as u8);
            result.push(buf as u8);
            buf = 0;
            buf_len = 0;
        }
    }

    // Handle remaining bytes
    if buf_len == 2 {
        buf <<= 12;
        result.push((buf >> 16) as u8);
    } else if buf_len == 3 {
        buf <<= 6;
        result.push((buf >> 16) as u8);
        result.push((buf >> 8) as u8);
    }

    Ok(result)
}

fn url_encode(data: &str) -> String {
    let mut result = String::new();
    for byte in data.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
            result.push(byte as char);
        } else {
            result.push('%');
            result.push_str(&format!("{:02X}", byte));
        }
    }
    result
}

fn url_decode(data: &str) -> Result<String, String> {
    let mut result = Vec::new();
    let mut chars = data.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() != 2 {
                return Err("Invalid URL encoding".to_string());
            }
            let byte = u8::from_str_radix(&hex, 16).map_err(|e| format!("Invalid hex: {}", e))?;
            result.push(byte);
        } else if c == '+' {
            result.push(b' ');
        } else {
            result.push(c as u8);
        }
    }
    String::from_utf8(result).map_err(|e| format!("Invalid UTF-8: {}", e))
}

fn html_encode(data: &str) -> String {
    let mut result = String::new();
    for c in data.chars() {
        match c {
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '&' => result.push_str("&amp;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&#x27;"),
            _ => result.push(c),
        }
    }
    result
}

fn html_decode(data: &str) -> String {
    let mut result = String::new();
    let mut chars = data.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            let mut entity = String::new();
            while let Some(&ch) = chars.peek() {
                if ch == ';' {
                    chars.next();
                    break;
                }
                entity.push(ch);
                chars.next();
            }
            let decoded = match entity.as_str() {
                "lt" => '<',
                "gt" => '>',
                "amp" => '&',
                "quot" => '"',
                "#x27" | "#39" => '\'',
                "nbsp" => ' ',
                _ => {
                    result.push('&');
                    result.push_str(&entity);
                    result.push(';');
                    continue;
                }
            };
            result.push(decoded);
        } else {
            result.push(c);
        }
    }
    result
}

#[async_trait]
impl Tool for EncodeTool {
    fn name(&self) -> &str {
        "encode"
    }

    fn description(&self) -> &str {
        "Encode/decode data using various schemes: base64, url, html. \
         Supports both encoding and decoding operations."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["encode", "decode"],
                    "description": "Whether to encode or decode"
                },
                "scheme": {
                    "type": "string",
                    "enum": ["base64", "url", "html"],
                    "description": "Encoding scheme to use"
                },
                "data": {
                    "type": "string",
                    "description": "Data to encode/decode"
                }
            },
            "required": ["action", "scheme", "data"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("encode");
        let scheme = params["scheme"].as_str().unwrap_or("base64");
        let data = params["data"].as_str().unwrap_or("");

        if data.is_empty() {
            return ToolResult::error("Data cannot be empty");
        }

        let result = match (action, scheme) {
            ("encode", "base64") => Ok(base64_encode(data)),
            ("decode", "base64") => match base64_decode(data) {
                Ok(bytes) => match String::from_utf8(bytes) {
                    Ok(s) => Ok(s),
                    Err(_) => Ok("[binary data]".to_string()),
                },
                Err(e) => Err(format!("Base64 decode error: {}", e)),
            },
            ("encode", "url") => Ok(url_encode(data)),
            ("decode", "url") => match url_decode(data) {
                Ok(s) => Ok(s),
                Err(e) => Err(format!("URL decode error: {}", e)),
            },
            ("encode", "html") => Ok(html_encode(data)),
            ("decode", "html") => Ok(html_decode(data)),
            _ => Err(format!("Unsupported action/scheme: {}/{}", action, scheme)),
        };

        match result {
            Ok(output) => ToolResult::success_with_data(
                output.clone(),
                json!({
                    "action": action,
                    "scheme": scheme,
                    "input_length": data.len(),
                    "output_length": output.len(),
                    "result": output
                }),
            ),
            Err(e) => ToolResult::error(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_base64_encode() {
        let tool = EncodeTool;
        let params = json!({
            "action": "encode",
            "scheme": "base64",
            "data": "Hello World"
        });
        let context = ToolContext::new(".", "test");

        let result = tool.execute(params, context).await;
        assert!(result.success);
        assert!(result.content.contains("SGVsbG8gV29ybGQ"));
    }

    #[tokio::test]
    async fn test_base64_decode() {
        let tool = EncodeTool;
        let params = json!({
            "action": "decode",
            "scheme": "base64",
            "data": "SGVsbG8gV29ybGQ="
        });
        let context = ToolContext::new(".", "test");

        let result = tool.execute(params, context).await;
        assert!(result.success);
        assert!(result.content.contains("Hello World"));
    }

    #[tokio::test]
    async fn test_url_encode() {
        let tool = EncodeTool;
        let params = json!({
            "action": "encode",
            "scheme": "url",
            "data": "hello world"
        });
        let context = ToolContext::new(".", "test");

        let result = tool.execute(params, context).await;
        assert!(result.success);
        assert!(result.content.contains("hello%20world"));
    }
}
