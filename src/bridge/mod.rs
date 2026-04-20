//! 桥接客户端 - 远程会话管理
//!
//! 简单的 HTTP 桥接，用于分布式工作

use reqwest::Client;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

static BRIDGE_URL: OnceLock<String> = OnceLock::new();

/// 设置全局桥接服务器 URL（线程安全，只能设置一次）
pub fn set_bridge_url(url: String) -> Result<(), String> {
    BRIDGE_URL
        .set(url)
        .map_err(|_| "Bridge URL already set".to_string())
}

/// 获取全局桥接服务器 URL
pub fn get_bridge_url() -> Option<&'static String> {
    BRIDGE_URL.get()
}

fn bridge_cursor_path() -> PathBuf {
    if let Ok(custom) = std::env::var("PRIORITY_AGENT_BRIDGE_CURSOR_FILE") {
        if !custom.trim().is_empty() {
            return PathBuf::from(custom);
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".priority-agent")
        .join("bridge_cursors.json")
}

/// 读取远程会话回放游标
pub fn load_replay_cursor(session_id: &str) -> Option<i64> {
    let path = bridge_cursor_path();
    let content = std::fs::read_to_string(path).ok()?;
    let map: HashMap<String, i64> = serde_json::from_str(&content).ok()?;
    map.get(session_id).copied()
}

/// 保存远程会话回放游标
pub fn save_replay_cursor(session_id: &str, cursor: i64) -> anyhow::Result<()> {
    let path = bridge_cursor_path();
    let mut map: HashMap<String, i64> = if path.exists() {
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashMap::new()
    };
    map.insert(session_id.to_string(), cursor);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(&map)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// 桥接客户端
pub struct BridgeClient {
    base_url: String,
    auth_token: Option<String>,
    tenant_id: Option<String>,
    client: Client,
}

impl BridgeClient {
    /// 创建新的桥接客户端
    pub fn new(base_url: impl Into<String>, auth_token: Option<String>) -> Self {
        Self::with_tenant(base_url, auth_token, None)
    }

    /// 创建带租户标识的桥接客户端
    pub fn with_tenant(
        base_url: impl Into<String>,
        auth_token: Option<String>,
        tenant_id: Option<String>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            auth_token,
            tenant_id,
            client: Client::new(),
        }
    }

    fn build_request(
        &self,
        method: reqwest::Method,
        path: &str,
    ) -> anyhow::Result<reqwest::RequestBuilder> {
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), path);
        let mut req = self.client.request(method, &url);
        if let Some(token) = &self.auth_token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }
        if let Some(tenant_id) = &self.tenant_id {
            if !tenant_id.trim().is_empty() {
                req = req.header("X-Tenant-Id", tenant_id.trim());
            }
        }
        Ok(req)
    }

    /// 列出远程会话
    pub async fn list_sessions(&self) -> anyhow::Result<serde_json::Value> {
        let req = self.build_request(reqwest::Method::GET, "/v1/sessions")?;
        let resp = req.send().await?;
        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "List sessions failed: {}",
                resp.text().await?
            ))
        }
    }

    /// 获取单个远程会话
    pub async fn get_session(&self, id: &str) -> anyhow::Result<serde_json::Value> {
        let req = self.build_request(reqwest::Method::GET, &format!("/v1/sessions/{}", id))?;
        let resp = req.send().await?;
        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Get session failed: {}",
                resp.text().await?
            ))
        }
    }

    /// 创建远程会话
    pub async fn create_session(&self, prompt: &str) -> anyhow::Result<serde_json::Value> {
        let req = self
            .build_request(reqwest::Method::POST, "/v1/sessions")?
            .json(&json!({ "prompt": prompt }));
        let resp = req.send().await?;
        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Create session failed: {}",
                resp.text().await?
            ))
        }
    }

    /// 运行远程触发器
    pub async fn run_trigger(
        &self,
        id: &str,
        body: Option<serde_json::Value>,
    ) -> anyhow::Result<serde_json::Value> {
        let req = self.build_request(reqwest::Method::POST, &format!("/v1/triggers/{}/run", id))?;
        let req = if let Some(b) = body {
            req.json(&b)
        } else {
            req
        };
        let resp = req.send().await?;
        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Run trigger failed: {}",
                resp.text().await?
            ))
        }
    }

    /// 获取会话同步状态
    pub async fn get_session_status(&self, id: &str) -> anyhow::Result<serde_json::Value> {
        let req =
            self.build_request(reqwest::Method::GET, &format!("/v1/sessions/{}/status", id))?;
        let resp = req.send().await?;
        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Get session status failed: {}",
                resp.text().await?
            ))
        }
    }

    /// 拉取会话消息（可按 since_id 增量拉取）
    pub async fn get_session_messages(
        &self,
        id: &str,
        limit: Option<u32>,
        since_id: Option<i64>,
    ) -> anyhow::Result<serde_json::Value> {
        let mut req = self.build_request(
            reqwest::Method::GET,
            &format!("/v1/sessions/{}/messages", id),
        )?;
        let mut query: Vec<(&str, String)> = Vec::new();
        if let Some(limit) = limit {
            query.push(("limit", limit.to_string()));
        }
        if let Some(since_id) = since_id {
            query.push(("since_id", since_id.to_string()));
        }
        if !query.is_empty() {
            req = req.query(&query);
        }
        let resp = req.send().await?;
        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Get session messages failed: {}",
                resp.text().await?
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_client_new() {
        let client = BridgeClient::new("http://localhost:8080", Some("token".to_string()));
        assert_eq!(client.base_url, "http://localhost:8080");
        assert_eq!(client.tenant_id, None);
    }

    #[test]
    fn test_bridge_client_with_tenant() {
        let client = BridgeClient::with_tenant(
            "http://localhost:8080",
            Some("token".to_string()),
            Some("team-a".to_string()),
        );
        assert_eq!(client.base_url, "http://localhost:8080");
        assert_eq!(client.tenant_id.as_deref(), Some("team-a"));
    }
}
