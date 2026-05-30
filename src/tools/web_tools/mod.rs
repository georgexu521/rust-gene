use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// WebFetch 工具 - 获取网页内容
pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Download a URL and return its visible text content (HTML pages get \
         scripts/styles/nav stripped). Truncated at the tool-result cap. \
         Use after web_search when a snippet isn't enough to answer the question."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch"
                },
                "max_chars": {
                    "type": "integer",
                    "description": "Maximum characters to return (default: 10000)",
                    "default": 10000
                }
            },
            "required": ["url"]
        })
    }

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        let url = params["url"].as_str().unwrap_or("");
        format!("web_fetch: {}", url)
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let url = params["url"].as_str().unwrap_or("");
        if url.is_empty() {
            return ToolResult::error("URL cannot be empty");
        }

        let max_chars = params["max_chars"].as_u64().unwrap_or(10000) as usize;

        // 安全校验：只允许 http/https
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return ToolResult::error("Only http:// and https:// URLs are allowed");
        }
        // 阻止内网地址（通过 DNS 解析后检查 IP）
        if is_internal_url(url) {
            return ToolResult::error("Internal network URLs are not allowed");
        }

        // 注意：不跟随重定向，防止 SSRF 绕过内网检查
        let output = match tokio::process::Command::new("curl")
            .args([
                "-s",
                "--max-time",
                "30",
                "--max-redirs",
                "0",
                "-A",
                "Mozilla/5.0",
                url,
            ])
            .output()
            .await
        {
            Ok(output) => output,
            Err(e) => return ToolResult::error(format!("Failed to fetch URL: {}", e)),
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return ToolResult::error(format!("Failed to fetch URL: {}", stderr));
        }

        let content = String::from_utf8_lossy(&output.stdout);

        // 简单的 HTML 清理（去除标签）
        let cleaned = strip_html_tags(&content);

        // 截断到指定长度
        let truncated: String = cleaned.chars().take(max_chars).collect();
        let truncated = if cleaned.len() > max_chars {
            format!("{}\n\n[... truncated at {} chars]", truncated, max_chars)
        } else {
            truncated
        };

        let result_content = format!(
            "Fetched: {}\nBytes: {}\nChars returned: {}\n\n{}",
            url,
            content.len(),
            truncated.len(),
            truncated
        );

        ToolResult::success_with_data(
            result_content,
            json!({ "url": url, "size_bytes": content.len(), "chars_returned": truncated.len() }),
        )
    }
}

/// WebSearch 工具 - 使用搜索引擎搜索
pub struct WebSearchTool;

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the public web. Returns ranked results with title, url, and snippet. \
         Call this when the answer's correctness depends on current state — \
         anything that changes over time (events, prices, releases, status of \
         a thing in the real world). Composing such answers from training \
         memory invents stale numbers; search first, then ground the answer \
         in the results. For evergreen/definitional questions you don't need this."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                },
                "num_results": {
                    "type": "integer",
                    "description": "Number of results to return (default: 5)",
                    "default": 5
                }
            },
            "required": ["query"]
        })
    }

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        let query = params["query"].as_str().unwrap_or("");
        format!("web_search: {}", query)
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let query = params["query"].as_str().unwrap_or("");
        if query.is_empty() {
            return ToolResult::error("Query cannot be empty");
        }

        let num_results = params["num_results"].as_u64().unwrap_or(5);

        // 使用 DuckDuckGo HTML 搜索（不需 API key）
        let encoded = url_encode(query);
        let search_url = format!("https://html.duckduckgo.com/html/?q={}", encoded);

        let output = match tokio::process::Command::new("curl")
            .args(["-sL", "--max-time", "15", "-A", "Mozilla/5.0", &search_url])
            .output()
            .await
        {
            Ok(output) => output,
            Err(e) => return ToolResult::error(format!("Search failed: {}", e)),
        };

        if !output.status.success() {
            return ToolResult::error("Search request failed".to_string());
        }

        let html = String::from_utf8_lossy(&output.stdout);
        let results = parse_ddg_results(&html, num_results as usize);

        if results.is_empty() {
            return ToolResult::success("No search results found");
        }

        let mut output_text = format!("Search results for '{}':\n\n", query);
        for (i, (title, url, snippet)) in results.iter().enumerate() {
            output_text.push_str(&format!(
                "{}. {}\n   URL: {}\n   {}\n\n",
                i + 1,
                title,
                url,
                snippet
            ));
        }

        ToolResult::success(output_text)
    }
}

/// 检查 URL 是否指向内网地址
fn is_internal_url(url: &str) -> bool {
    // 阻止 userinfo 绕过（如 http://evil.com@127.0.0.1）
    if url.contains('@') {
        return true;
    }

    // 提取 host 部分
    let after_scheme = if let Some(idx) = url.find("://") {
        &url[idx + 3..]
    } else {
        url
    };

    let host_part = after_scheme.split('/').next().unwrap_or(after_scheme);

    // 去掉端口，正确处理 IPv6
    let host = if host_part.starts_with('[') {
        if let Some(bracket_end) = host_part.find(']') {
            &host_part[1..bracket_end]
        } else {
            host_part
        }
    } else if let Some(colon_pos) = host_part.rfind(':') {
        &host_part[..colon_pos]
    } else {
        host_part
    };

    if host.is_empty() {
        return true;
    }

    // 阻止常见的内部主机名
    let host_lower = host.to_lowercase();
    if host_lower == "localhost"
        || host_lower == "localhost.localdomain"
        || host_lower.ends_with(".local")
        || host_lower.ends_with(".internal")
    {
        return true;
    }

    // 解析为 IP 直接检查
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return is_internal_ip(&ip);
    }

    // DNS 解析后检查 IP 是否属于内网
    use std::net::ToSocketAddrs;
    if let Ok(addrs) = (host, 80).to_socket_addrs() {
        for addr in addrs {
            if is_internal_ip(&addr.ip()) {
                return true;
            }
        }
    }

    false
}

/// 检查 IP 是否属于内网地址
fn is_internal_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ip) => {
            ip.is_loopback() || ip.is_private() || ip.is_link_local() || ip.is_multicast()
        }
        std::net::IpAddr::V6(ip) => {
            let octets = ip.octets();
            // fe80::/10 - IPv6 link local
            let is_link_local = octets[0] == 0xfe && (octets[1] & 0xc0) == 0x80;
            // fc00::/7 - IPv6 unique local
            let is_unique_local = (octets[0] & 0xfe) == 0xfc;
            ip.is_loopback() || ip.is_multicast() || is_link_local || is_unique_local
        }
    }
}

fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_script = false;
    let mut chars = html.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '<' {
            let rest: String = chars.clone().take(20).collect();
            if rest.to_lowercase().starts_with("script") {
                in_script = true;
            } else if rest.to_lowercase().starts_with("/script") {
                in_script = false;
            }
            in_tag = true;
            continue;
        }
        if c == '>' {
            in_tag = false;
            continue;
        }
        if !in_tag && !in_script {
            result.push(c);
        }
    }

    // 清理多余空白
    result
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_ddg_results(html: &str, max: usize) -> Vec<(String, String, String)> {
    let mut results = Vec::new();
    let lines: Vec<&str> = html.lines().collect();
    let mut i = 0;

    while i < lines.len() && results.len() < max {
        let line = lines[i];

        if line.contains("result__title") {
            let title =
                extract_between(line, "class=\"result__title\">", "</a>").unwrap_or_default();
            let url = extract_between(line, "href=\"", "\"").unwrap_or_default();
            let mut snippet = String::new();

            for line in lines.iter().take((i + 5).min(lines.len())).skip(i + 1) {
                if line.contains("result__snippet") {
                    snippet = strip_html_tags(line);
                    break;
                }
            }

            if !title.is_empty() && !url.is_empty() {
                let real_url = if url.contains("uddg=") {
                    extract_between(&url, "uddg=", "&").unwrap_or(url.clone())
                } else {
                    url.clone()
                };
                results.push((strip_html_tags(&title), real_url, snippet));
            }
        }
        i += 1;
    }

    results
}

fn extract_between(text: &str, start: &str, end: &str) -> Option<String> {
    let start_idx = text.find(start)? + start.len();
    let end_idx = text[start_idx..].find(end)? + start_idx;
    Some(text[start_idx..end_idx].to_string())
}

fn url_encode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html() {
        assert_eq!(strip_html_tags("<p>Hello <b>World</b></p>"), "Hello World");
    }

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("hello world"), "hello+world");
        assert_eq!(url_encode("test&foo"), "test%26foo");
    }

    #[test]
    fn test_is_internal_url() {
        assert!(is_internal_url("http://127.0.0.1/test"));
        assert!(is_internal_url("http://localhost/test"));
        assert!(is_internal_url("http://192.168.1.1/test"));
        assert!(is_internal_url("http://10.0.0.1/test"));
        assert!(is_internal_url("http://example.com@127.0.0.1/test"));
        assert!(is_internal_url("http://[::1]/test"));
        assert!(!is_internal_url("http://example.com/test"));
        assert!(!is_internal_url("https://github.com/test"));
    }
}
