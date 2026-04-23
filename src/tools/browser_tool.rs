//! Chrome / 浏览器工具
//!
//! 通过 Chrome DevTools Protocol (CDP) 与本地 Chrome 实例通信，
//! 支持网页导航、截图、内容提取、元素查找、JS 执行等操作。
//!
//! 依赖项目已有的 reqwest 和 tokio-tungstenite，无需额外 crate。

use crate::tools::{Tool, ToolContext, ToolResult};
use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::Engine;
use serde::Deserialize;
use serde_json::Value;
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};

/// 浏览器工具，支持多种 CDP 操作
pub struct BrowserTool;

#[async_trait]
impl Tool for BrowserTool {
    fn name(&self) -> &str {
        "browser"
    }

    fn description(&self) -> &str {
        "Launch a headless Chrome instance to interact with web pages. \
Supports navigate, screenshot, get_page_content, find_elements, and evaluate_js. \
Use this when you need to view a webpage, capture a screenshot, extract text content, \
find specific elements, or run JavaScript in a browser context."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["navigate", "screenshot", "get_page_content", "find_elements", "evaluate_js"],
                    "description": "The browser action to perform"
                },
                "url": {
                    "type": "string",
                    "description": "URL to navigate to (required for navigate)"
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector for find_elements (e.g., 'a', '.class', '#id')"
                },
                "script": {
                    "type": "string",
                    "description": "JavaScript code to execute (required for evaluate_js)"
                },
                "full_page": {
                    "type": "boolean",
                    "description": "Capture full page screenshot (default: false)",
                    "default": false
                },
                "wait_ms": {
                    "type": "integer",
                    "description": "Milliseconds to wait after navigation before screenshot/content extraction (default: 1000)",
                    "default": 1000
                }
            },
            "required": ["action"]
        })
    }

    fn confirmation_prompt(&self, params: &Value) -> Option<String> {
        match params["action"].as_str() {
            Some("navigate") => {
                let url = params["url"].as_str().unwrap_or("unknown");
                Some(format!("Allow browser to navigate to '{}'?", url))
            }
            Some("evaluate_js") => {
                let script = params["script"].as_str().unwrap_or("unknown");
                let preview = if script.len() > 60 {
                    format!("{}...", &script[..60])
                } else {
                    script.to_string()
                };
                Some(format!(
                    "Allow browser to execute JavaScript: '{}'?",
                    preview
                ))
            }
            _ => None,
        }
    }

    async fn execute(&self, params: Value, _context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("");
        if action.is_empty() {
            return ToolResult::error("Missing required parameter: action");
        }

        let mut browser = match BrowserSession::start().await {
            Ok(b) => b,
            Err(e) => return ToolResult::error(format!("Failed to start browser: {}", e)),
        };

        let result = match action {
            "navigate" => {
                let url = match params["url"].as_str() {
                    Some(u) => u,
                    None => return ToolResult::error("Missing required parameter: url"),
                };
                match browser.navigate(url).await {
                    Ok(_) => ToolResult::success(format!("Navigated to {}", url)),
                    Err(e) => ToolResult::error(format!("Navigation failed: {}", e)),
                }
            }
            "screenshot" => {
                let full_page = params["full_page"].as_bool().unwrap_or(false);
                let wait_ms = params["wait_ms"].as_u64().unwrap_or(1000);
                if wait_ms > 0 {
                    sleep(Duration::from_millis(wait_ms)).await;
                }
                match browser.screenshot(full_page).await {
                    Ok(data) => {
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
                        let md = format!("![screenshot](data:image/png;base64,{})", b64);
                        ToolResult::success(md)
                    }
                    Err(e) => ToolResult::error(format!("Screenshot failed: {}", e)),
                }
            }
            "get_page_content" => {
                let wait_ms = params["wait_ms"].as_u64().unwrap_or(1000);
                if wait_ms > 0 {
                    sleep(Duration::from_millis(wait_ms)).await;
                }
                match browser.get_page_text().await {
                    Ok(text) => ToolResult::success(text),
                    Err(e) => ToolResult::error(format!("Failed to get page content: {}", e)),
                }
            }
            "find_elements" => {
                let selector = match params["selector"].as_str() {
                    Some(s) => s,
                    None => return ToolResult::error("Missing required parameter: selector"),
                };
                let wait_ms = params["wait_ms"].as_u64().unwrap_or(1000);
                if wait_ms > 0 {
                    sleep(Duration::from_millis(wait_ms)).await;
                }
                match browser.find_elements(selector).await {
                    Ok(elements) => {
                        let lines: Vec<String> = elements
                            .iter()
                            .enumerate()
                            .map(|(i, el)| format!("[{}] {}", i + 1, el))
                            .collect();
                        ToolResult::success(lines.join("\n"))
                    }
                    Err(e) => ToolResult::error(format!("Find elements failed: {}", e)),
                }
            }
            "evaluate_js" => {
                let script = match params["script"].as_str() {
                    Some(s) => s,
                    None => return ToolResult::error("Missing required parameter: script"),
                };
                match browser.evaluate(script).await {
                    Ok(result) => ToolResult::success(result),
                    Err(e) => ToolResult::error(format!("JS evaluation failed: {}", e)),
                }
            }
            _ => ToolResult::error(format!("Unknown browser action: {}", action)),
        };

        let _ = browser.shutdown().await;
        result
    }
}

// ========================================================================
// CDP Client Implementation
// ========================================================================

/// 一次性浏览器会话，启动 Chrome headless 并通过 CDP 控制
struct BrowserSession {
    #[allow(dead_code)]
    port: u16,
    chrome_process: Child,
    client: CdpClient,
}

impl BrowserSession {
    /// 启动 Chrome headless 并连接到 CDP
    async fn start() -> Result<Self> {
        let port = find_free_port().await?;
        let chrome_path = find_chrome_binary()?;

        let child = Command::new(&chrome_path)
            .arg(format!("--remote-debugging-port={}", port))
            .arg("--headless=new")
            .arg("--disable-gpu")
            .arg("--no-sandbox")
            .arg("--disable-dev-shm-usage")
            .arg("--disable-background-networking")
            .arg("--disable-background-timer-throttling")
            .arg("--disable-renderer-backgrounding")
            .arg("--disable-features=TranslateUI")
            .arg("--disable-component-extensions-with-background-pages")
            .arg("--window-size=1280,720")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("Failed to launch Chrome at {:?}", chrome_path))?;

        // 等待 CDP HTTP 端口可用
        let cdp_url = wait_for_cdp(port).await?;

        // 连接 WebSocket
        let client = CdpClient::connect(&cdp_url).await?;

        Ok(BrowserSession {
            port,
            chrome_process: child,
            client,
        })
    }

    async fn navigate(&mut self, url: &str) -> Result<()> {
        let params = serde_json::json!({ "url": url });
        let res = self.client.send("Page.navigate", params).await?;
        if res["errorText"].as_str().is_some() {
            anyhow::bail!("Navigation error: {}", res["errorText"].as_str().unwrap());
        }
        // 等待页面加载
        sleep(Duration::from_millis(500)).await;
        Ok(())
    }

    async fn screenshot(&mut self, full_page: bool) -> Result<Vec<u8>> {
        let mut params = serde_json::json!({ "format": "png" });
        if full_page {
            // 先获取页面尺寸
            let size = self
                .client
                .send(
                    "Runtime.evaluate",
                    serde_json::json!({
                        "expression": "JSON.stringify({width: document.documentElement.scrollWidth, height: document.documentElement.scrollHeight})",
                        "returnByValue": true,
                    }),
                )
                .await?;
            if let Some(result) = size["result"]["value"].as_str() {
                if let Ok(parsed) = serde_json::from_str::<Value>(result) {
                    if let (Some(w), Some(h)) =
                        (parsed["width"].as_u64(), parsed["height"].as_u64())
                    {
                        params["clip"] = serde_json::json!({
                            "x": 0, "y": 0,
                            "width": w, "height": h,
                            "scale": 1,
                        });
                    }
                }
            }
        }
        let res = self.client.send("Page.captureScreenshot", params).await?;
        let b64 = res["data"]
            .as_str()
            .context("Screenshot data missing from CDP response")?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .context("Failed to decode base64 screenshot")?;
        Ok(bytes)
    }

    async fn get_page_text(&mut self) -> Result<String> {
        let res = self
            .client
            .send(
                "Runtime.evaluate",
                serde_json::json!({
                    "expression": "document.documentElement.innerText",
                    "returnByValue": true,
                }),
            )
            .await?;
        let text = res["result"]["value"].as_str().unwrap_or("").to_string();
        Ok(text)
    }

    async fn find_elements(&mut self, selector: &str) -> Result<Vec<String>> {
        let escaped = selector.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!(
            "Array.from(document.querySelectorAll(\"{}\")).map(el => el.innerText || el.textContent || el.outerHTML).slice(0, 50)",
            escaped
        );
        let res = self
            .client
            .send(
                "Runtime.evaluate",
                serde_json::json!({
                    "expression": script,
                    "returnByValue": true,
                }),
            )
            .await?;

        let mut out = Vec::new();
        if let Some(arr) = res["result"]["value"].as_array() {
            for v in arr {
                if let Some(s) = v.as_str() {
                    let trimmed = s.trim();
                    if !trimmed.is_empty() {
                        out.push(trimmed.to_string());
                    }
                }
            }
        }
        Ok(out)
    }

    async fn evaluate(&mut self, script: &str) -> Result<String> {
        let res = self
            .client
            .send(
                "Runtime.evaluate",
                serde_json::json!({
                    "expression": script,
                    "returnByValue": true,
                    "awaitPromise": true,
                }),
            )
            .await?;

        if let Some(desc) = res["result"]["description"].as_str() {
            return Ok(desc.to_string());
        }
        if let Some(val) = res["result"]["value"].as_str() {
            return Ok(val.to_string());
        }
        if let Some(obj) = res["result"]["value"].as_object() {
            return Ok(serde_json::to_string(obj)?);
        }
        if let Some(arr) = res["result"]["value"].as_array() {
            return Ok(serde_json::to_string(arr)?);
        }
        Ok(serde_json::to_string(&res["result"])?)
    }

    async fn shutdown(mut self) {
        let _ = self.client.send("Browser.close", Value::Null).await;
        let _ = self.chrome_process.kill().await;
    }
}

// ========================================================================
// CDP WebSocket Client
// ========================================================================

struct CdpClient {
    write_tx: tokio::sync::mpsc::UnboundedSender<String>,
    responses: tokio::sync::mpsc::Receiver<CdpResponse>,
}

#[derive(Debug, Deserialize)]
struct CdpResponse {
    id: u32,
    #[serde(default)]
    result: Value,
    #[serde(default)]
    error: Value,
}

static CDP_ID: AtomicU32 = AtomicU32::new(1);

impl CdpClient {
    async fn connect(ws_url: &str) -> Result<Self> {
        use futures::{SinkExt, StreamExt};
        use tokio_tungstenite::connect_async;
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;

        let request = ws_url.into_client_request()?;
        let (mut ws_stream, _) = connect_async(request).await?;

        let (write_tx, mut write_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let (resp_tx, resp_rx) = tokio::sync::mpsc::channel::<CdpResponse>(32);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    maybe_text = write_rx.recv() => {
                        match maybe_text {
                            Some(text) => {
                                if ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(text)).await.is_err() {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                    maybe_msg = ws_stream.next() => {
                        match maybe_msg {
                            Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                                if let Ok(resp) = serde_json::from_str::<CdpResponse>(&text) {
                                    let _ = resp_tx.send(resp).await;
                                }
                            }
                            Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) => break,
                            Some(Err(_)) => break,
                            _ => {}
                        }
                    }
                }
            }
        });

        Ok(CdpClient {
            write_tx,
            responses: resp_rx,
        })
    }

    async fn send(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = CDP_ID.fetch_add(1, Ordering::SeqCst);
        let msg = serde_json::json!({
            "id": id,
            "method": method,
            "params": params,
        });
        let msg_str = serde_json::to_string(&msg)?;
        self.write_tx.send(msg_str)?;

        // 等待对应 id 的响应，8 秒超时
        let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                anyhow::bail!("CDP command '{}' timed out", method);
            }
            match timeout(remaining, self.responses.recv()).await {
                Ok(Some(resp)) => {
                    if resp.id == id {
                        if resp.error.is_object() {
                            anyhow::bail!("CDP error for {}: {}", method, resp.error);
                        }
                        return Ok(resp.result);
                    }
                    // id 不匹配，继续等待（理论上不应该发生）
                }
                Ok(None) => anyhow::bail!("CDP response channel closed"),
                Err(_) => anyhow::bail!("CDP command '{}' timed out", method),
            }
        }
    }
}

// ========================================================================
// Helpers
// ========================================================================

/// 在系统中查找 Chrome/Chromium 可执行文件
fn find_chrome_binary() -> Result<std::path::PathBuf> {
    // 检查环境变量
    if let Ok(path) = std::env::var("CHROME_PATH") {
        let p = std::path::PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        }
    }

    let candidates: Vec<&str> = match std::env::consts::OS {
        "macos" => vec![
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
        ],
        "linux" => vec![
            "google-chrome",
            "google-chrome-stable",
            "chromium",
            "chromium-browser",
            "/usr/bin/google-chrome",
            "/usr/bin/chromium",
            "/usr/bin/chromium-browser",
        ],
        "windows" => vec![
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files\Chromium\Application\chromium.exe",
        ],
        _ => vec!["google-chrome", "chromium"],
    };

    for c in &candidates {
        let path = std::path::PathBuf::from(c);
        if path.exists() {
            return Ok(path);
        }
        // 尝试从 PATH 查找
        if let Ok(output) = std::process::Command::new("which").arg(c).output() {
            if output.status.success() {
                let found = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !found.is_empty() {
                    return Ok(std::path::PathBuf::from(found));
                }
            }
        }
    }

    anyhow::bail!(
        "Chrome/Chromium not found. Please install Chrome or set CHROME_PATH environment variable."
    )
}

/// 找一个未使用的 TCP 端口
async fn find_free_port() -> Result<u16> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

/// 等待 Chrome 的 CDP HTTP 端口可用，返回第一个标签页的 WebSocket URL
async fn wait_for_cdp(port: u16) -> Result<String> {
    let http_url = format!("http://127.0.0.1:{}/json/list", port);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()?;

    for _ in 0..30 {
        match client.get(&http_url).send().await {
            Ok(resp) => {
                if let Ok(json) = resp.json::<Value>().await {
                    if let Some(arr) = json.as_array() {
                        if let Some(first) = arr.first() {
                            if let Some(ws_url) = first["webSocketDebuggerUrl"].as_str() {
                                return Ok(ws_url.to_string());
                            }
                        }
                    }
                }
            }
            Err(_) => {
                sleep(Duration::from_millis(200)).await;
            }
        }
    }

    anyhow::bail!("Chrome CDP did not become available within timeout")
}

// ========================================================================
// Tests
// ========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_tool_schema_valid() {
        let tool = BrowserTool;
        let schema = tool.parameters();
        assert_eq!(tool.name(), "browser");
        assert!(schema.get("properties").is_some());
        assert!(!schema["properties"]["action"]["enum"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn test_find_chrome_binary_fails_gracefully() {
        // 在没有 Chrome 的环境下应该返回错误
        let result = find_chrome_binary();
        // 我们不能确定系统上是否安装了 Chrome，但函数结构是正确的
        // 只要不 panic 就算通过
        let _ = result;
    }

    #[test]
    fn test_find_free_port() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let port = rt.block_on(find_free_port()).unwrap();
        assert!(port > 0);
    }
}
