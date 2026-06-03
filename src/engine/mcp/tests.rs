use super::tool::ensure_mcp_server_allowed;
use super::*;

#[test]
fn test_mcp_server_config() {
    let config = McpServerConfig {
        name: "test-server".to_string(),
        transport: McpTransport::Stdio,
        command: "npx".to_string(),
        args: vec!["-y".to_string(), "test-mcp".to_string()],
        env: HashMap::new(),
        websocket_url: None,
        http_url: None,
        headers: HashMap::new(),
        oauth: None,
        oauth_token_url: None,
    };
    assert_eq!(config.name, "test-server");
    assert_eq!(config.command, "npx");
}

#[test]
fn test_mcp_manager_creation() {
    let manager = McpManager::new();
    assert!(manager.server_names().is_empty());
}

#[test]
fn test_mcp_tool_def() {
    let tool = McpToolDef {
        name: "read_file".to_string(),
        description: "Read a file".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" }
            }
        }),
        server_name: "filesystem".to_string(),
    };
    assert_eq!(tool.name, "read_file");
    assert_eq!(tool.server_name, "filesystem");
}

#[test]
fn test_mcp_manage_tool_schema_includes_resource_and_repair_actions() {
    use crate::tools::Tool;

    let schema = McpManageTool.parameters();
    let actions = schema["properties"]["action"]["enum"]
        .as_array()
        .expect("action enum");
    let action_names = actions
        .iter()
        .filter_map(|value| value.as_str())
        .collect::<Vec<_>>();

    assert!(action_names.contains(&"list_resources"));
    assert!(action_names.contains(&"read_resource"));
    assert!(action_names.contains(&"repair_server"));
    assert!(action_names.contains(&"status"));
    assert!(schema["properties"].get("uri").is_some());
}

#[tokio::test]
async fn test_mcp_runtime_facts_report_pending_approval_without_discovery() {
    let mut manager = McpManager::new();
    manager.add_server(McpServerConfig {
        name: "filesystem".to_string(),
        transport: McpTransport::Stdio,
        command: "mcp-filesystem".to_string(),
        args: vec![],
        env: HashMap::new(),
        websocket_url: None,
        http_url: None,
        headers: HashMap::new(),
        oauth: None,
        oauth_token_url: None,
    });

    let facts = manager.runtime_facts().await;

    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].name, "filesystem");
    assert_eq!(facts[0].health, McpHealthStatus::Pending);
    assert!(!facts[0].approved);
    assert_eq!(facts[0].tool_count, 0);
    assert_eq!(facts[0].resource_count, 0);
    assert_eq!(facts[0].prompt_count, 0);
    assert_eq!(facts[0].repair_hint, "/mcp approve filesystem");
    assert!(facts[0].diagnostic.contains("pending approval"));
}

#[tokio::test]
async fn test_mcp_status_action_returns_runtime_facts_data() {
    use crate::tools::Tool;

    let mut manager = McpManager::new();
    manager.add_server(McpServerConfig {
        name: "filesystem".to_string(),
        transport: McpTransport::Stdio,
        command: "mcp-filesystem".to_string(),
        args: vec![],
        env: HashMap::new(),
        websocket_url: None,
        http_url: None,
        headers: HashMap::new(),
        oauth: None,
        oauth_token_url: None,
    });
    let context = crate::tools::ToolContext::new(".", "test").with_mcp_manager(Arc::new(manager));

    let result = McpManageTool
        .execute(json!({ "action": "status" }), context)
        .await;

    assert!(result.success);
    assert!(result.content.contains("MCP runtime status"));
    assert_eq!(
        result.data.unwrap()["servers"][0]["repair_hint"],
        "/mcp approve filesystem"
    );
}

#[test]
fn test_mcp_manage_scope_rejects_unlisted_server() {
    let mut context = crate::tools::ToolContext::new(".", "test");
    context.metadata.insert(
        "allowed_mcp_servers".to_string(),
        "filesystem,github".to_string(),
    );

    assert!(ensure_mcp_server_allowed(&context, "filesystem").is_ok());
    assert!(ensure_mcp_server_allowed(&context, "slack").is_err());
}

#[tokio::test]
async fn test_mcp_manage_resource_listing_requires_explicit_scoped_server() {
    use crate::tools::Tool;

    let manager = Arc::new(McpManager::new());
    let mut context = crate::tools::ToolContext::new(".", "test").with_mcp_manager(manager);
    context.metadata.insert(
        "allowed_mcp_servers".to_string(),
        "filesystem,github".to_string(),
    );

    let result = McpManageTool
        .execute(json!({ "action": "list_resources" }), context)
        .await;

    assert!(!result.success);
    assert!(result
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("Pass server_name explicitly"));
}

#[test]
fn test_mcp_request_serialization() {
    let request = McpRequest {
        jsonrpc: "2.0".to_string(),
        id: 1,
        method: "tools/list".to_string(),
        params: json!({}),
    };
    let json_str = serde_json::to_string(&request).unwrap();
    assert!(json_str.contains("jsonrpc"));
    assert!(json_str.contains("tools/list"));
}

#[test]
fn test_mcp_manager_approval() {
    let manager = McpManager::new();
    assert!(!manager.is_server_approved("test-server"));

    manager.approve_server("test-server");
    assert!(manager.is_server_approved("test-server"));

    manager.revoke_server("test-server");
    assert!(!manager.is_server_approved("test-server"));
}

#[test]
fn test_mcp_manager_approval_disabled() {
    let manager = McpManager::new();
    manager.set_require_server_approval(false);
    assert!(manager.is_server_approved("any-server"));
}

#[test]
fn test_mcp_manager_approved_names() {
    let manager = McpManager::new();
    manager.approve_server("server-a");
    manager.approve_server("server-b");
    let names = manager.approved_server_names();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"server-a".to_string()));
    assert!(names.contains(&"server-b".to_string()));
}

#[test]
fn test_parse_oauth_token_response() {
    let v = json!({
        "access_token": "acc-1",
        "refresh_token": "ref-1",
        "token_type": "Bearer",
        "expires_in": 3600,
        "scope": "read write"
    });
    let token = parse_oauth_token_response(&v).expect("parse token");
    assert_eq!(token.access_token, "acc-1");
    assert_eq!(token.refresh_token.as_deref(), Some("ref-1"));
    assert_eq!(token.token_type, "Bearer");
    assert!(token.expires_at.is_some());
}

#[test]
fn test_http_endpoint_summary() {
    let config = McpServerConfig {
        name: "http-test".to_string(),
        transport: McpTransport::Http,
        command: String::new(),
        args: vec![],
        env: HashMap::new(),
        websocket_url: None,
        http_url: Some("https://mcp.example.com/rpc".to_string()),
        headers: HashMap::new(),
        oauth: None,
        oauth_token_url: None,
    };
    let client = McpClient::new(config);
    assert!(client
        .endpoint_summary()
        .contains("http:https://mcp.example.com/rpc"));
}

#[tokio::test]
async fn test_mcp_websocket_disconnect_detected() {
    let config = McpServerConfig {
        name: "ws-test".to_string(),
        transport: McpTransport::WebSocket,
        command: String::new(),
        args: vec![],
        env: HashMap::new(),
        websocket_url: Some("ws://localhost:9999".to_string()),
        http_url: None,
        headers: HashMap::new(),
        oauth: None,
        oauth_token_url: None,
    };
    let client = McpClient::new(config);

    // 手动构造一个已断开的 WebSocket 连接
    let (write_tx, _write_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let handle = tokio::spawn(async {});
    handle.abort();
    let disconnected = Arc::new(std::sync::atomic::AtomicBool::new(true));

    let fake_conn = McpConnection {
        pending: Arc::new(Mutex::new(HashMap::new())),
        transport: McpTransportConnection::WebSocket {
            write_tx,
            read_handle: handle,
            disconnected,
        },
    };

    {
        let mut conn = client.connection.lock().await;
        *conn = Some(fake_conn);
    }

    // health_check 应检测到断开并返回错误
    let result = client.health_check().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("disconnected"));

    // 连接应被清空
    let conn = client.connection.lock().await;
    assert!(conn.is_none());
}

#[tokio::test]
async fn test_mcp_http_transport_list_tools() {
    use axum::{extract::Json, routing::post, Router};
    use std::net::SocketAddr;

    async fn rpc_handler(Json(req): Json<Value>) -> Json<Value> {
        let id = req["id"].as_u64().unwrap_or(0);
        let method = req["method"].as_str().unwrap_or("");
        if method == "tools/list" {
            Json(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": [{
                        "name": "echo_http",
                        "description": "Echo tool",
                        "inputSchema": {"type":"object","properties":{"x":{"type":"string"}}}
                    }]
                }
            }))
        } else {
            Json(json!({
                "jsonrpc":"2.0",
                "id": id,
                "error": {"code": -32601, "message": "method not found"}
            }))
        }
    }

    let app = Router::new().route("/rpc", post(rpc_handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test server");
    let addr: SocketAddr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let config = McpServerConfig {
        name: "http-local".to_string(),
        transport: McpTransport::Http,
        command: String::new(),
        args: vec![],
        env: HashMap::new(),
        websocket_url: None,
        http_url: Some(format!("http://{}/rpc", addr)),
        headers: HashMap::new(),
        oauth: None,
        oauth_token_url: None,
    };
    let client = McpClient::new(config);
    let tools = client.discover_tools().await.expect("discover tools");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "echo_http");
}
