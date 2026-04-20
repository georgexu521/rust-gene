//! WebSocket 实时通信
//!
//! 提供双向实时通信支持

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use super::state::ApiState;

/// WebSocket 消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsMessage {
    /// 客户端发送的消息
    Chat {
        session_id: Option<String>,
        message: String,
        stream: Option<bool>,
    },
    /// 工具调用
    ToolCall {
        tool: String,
        params: serde_json::Value,
        session_id: Option<String>,
    },
    /// 心跳
    Ping,
    /// 心跳响应
    Pong,
    /// 订阅事件
    Subscribe { events: Vec<String> },
    /// 取消订阅
    Unsubscribe { events: Vec<String> },
}

/// WebSocket 响应消息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsResponse {
    /// 连接成功
    Connected { session_id: String, version: String },
    /// 聊天响应
    ChatResponse {
        content: String,
        session_id: String,
        done: bool,
    },
    /// 工具调用结果
    ToolResult {
        tool: String,
        success: bool,
        content: String,
        data: Option<serde_json::Value>,
    },
    /// 错误
    Error { code: String, message: String },
    /// 心跳响应
    Pong,
    /// 事件通知
    Event {
        event: String,
        data: serde_json::Value,
    },
}

/// WebSocket 连接管理器
pub struct WebSocketManager {
    state: Arc<ApiState>,
}

impl WebSocketManager {
    pub fn new(state: Arc<ApiState>) -> Self {
        Self { state }
    }

    /// 处理 WebSocket 升级
    pub fn handle_upgrade(&self, ws: WebSocketUpgrade) -> impl IntoResponse {
        let state = self.state.clone();
        ws.on_upgrade(move |socket| handle_socket(socket, state))
    }
}

/// 处理 WebSocket 连接
async fn handle_socket(socket: WebSocket, state: Arc<ApiState>) {
    let session_id = format!("ws_{}", uuid::Uuid::new_v4().simple());
    let (mut sender, mut receiver) = socket.split();

    info!("WebSocket connected: {}", session_id);

    // 发送欢迎消息
    let welcome = WsResponse::Connected {
        session_id: session_id.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    if let Err(e) = send_response(&mut sender, welcome).await {
        error!("Failed to send welcome message: {}", e);
        return;
    }

    // 创建消息通道用于流式响应
    let (tx, mut rx) = mpsc::channel::<WsResponse>(100);

    // 启动发送任务
    let send_task = tokio::spawn(async move {
        while let Some(response) = rx.recv().await {
            if let Err(e) = send_response(&mut sender, response).await {
                debug!("WebSocket send error: {}", e);
                break;
            }
        }
    });

    // 处理接收到的消息
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                debug!("WebSocket received: {}", text);

                match serde_json::from_str::<WsMessage>(&text) {
                    Ok(ws_msg) => {
                        let response = handle_message(ws_msg, &state, &session_id).await;
                        if let Err(e) = tx.send(response).await {
                            error!("Failed to send response to channel: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        let error_resp = WsResponse::Error {
                            code: "invalid_json".to_string(),
                            message: format!("Invalid JSON: {}", e),
                        };
                        if let Err(e) = tx.send(error_resp).await {
                            error!("Failed to send error response: {}", e);
                            break;
                        }
                    }
                }
            }
            Message::Close(_) => {
                info!("WebSocket disconnected: {}", session_id);
                break;
            }
            Message::Ping(_data) => {
                // 自动处理 ping/pong
                debug!("Received ping from {}", session_id);
            }
            _ => {}
        }
    }

    // 清理
    drop(tx);
    let _ = send_task.await;
    info!("WebSocket session ended: {}", session_id);
}

/// 发送响应
async fn send_response(
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
    response: WsResponse,
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string(&response)?;
    sender.send(Message::Text(json)).await?;
    Ok(())
}

/// 处理 WebSocket 消息
async fn handle_message(msg: WsMessage, state: &Arc<ApiState>, session_id: &str) -> WsResponse {
    match msg {
        WsMessage::Chat {
            session_id: msg_session_id,
            message,
            stream,
        } => {
            let target_session = msg_session_id.unwrap_or_else(|| session_id.to_string());

            if stream == Some(true) {
                // 流式响应 - 这里简化处理，实际应该使用 channel
                match handle_streaming_chat(state, &message, &target_session).await {
                    Ok(content) => WsResponse::ChatResponse {
                        content,
                        session_id: target_session,
                        done: true,
                    },
                    Err(e) => WsResponse::Error {
                        code: "chat_error".to_string(),
                        message: e.to_string(),
                    },
                }
            } else {
                // 非流式响应
                match handle_chat(state, &message, &target_session).await {
                    Ok(content) => WsResponse::ChatResponse {
                        content,
                        session_id: target_session,
                        done: true,
                    },
                    Err(e) => WsResponse::Error {
                        code: "chat_error".to_string(),
                        message: e.to_string(),
                    },
                }
            }
        }
        WsMessage::ToolCall {
            tool,
            params,
            session_id: msg_session_id,
        } => {
            let target_session = msg_session_id.unwrap_or_else(|| session_id.to_string());

            match state.call_tool(&tool, params, &target_session).await {
                Ok(result) => WsResponse::ToolResult {
                    tool,
                    success: result.success,
                    content: result.content,
                    data: result.data,
                },
                Err(e) => WsResponse::Error {
                    code: "tool_error".to_string(),
                    message: e.to_string(),
                },
            }
        }
        WsMessage::Ping => WsResponse::Pong,
        WsMessage::Pong => {
            // 客户端发送的 pong，忽略
            WsResponse::Pong
        }
        WsMessage::Subscribe { events } => {
            info!("Client subscribed to events: {:?}", events);
            WsResponse::Event {
                event: "subscribed".to_string(),
                data: json!({ "events": events }),
            }
        }
        WsMessage::Unsubscribe { events } => {
            info!("Client unsubscribed from events: {:?}", events);
            WsResponse::Event {
                event: "unsubscribed".to_string(),
                data: json!({ "events": events }),
            }
        }
    }
}

/// 处理聊天请求
async fn handle_chat(
    state: &Arc<ApiState>,
    message: &str,
    session_id: &str,
) -> anyhow::Result<String> {
    use crate::services::api::{ChatRequest as LlmChatRequest, Message};

    let model = &state.model;
    let llm_req = LlmChatRequest::new(model)
        .with_messages(vec![
            Message::system("You are a helpful AI assistant."),
            Message::user(message),
        ])
        .with_temperature(0.6);

    let response = state.provider.chat(llm_req).await?;

    // 保存消息到会话
    let store = state.session_store.read().await;
    let _ = store.add_message(session_id, "user", message, None, None);
    let _ = store.add_message(session_id, "assistant", &response.content, None, None);

    Ok(response.content)
}

/// 处理流式聊天请求（简化版）
async fn handle_streaming_chat(
    state: &Arc<ApiState>,
    message: &str,
    session_id: &str,
) -> anyhow::Result<String> {
    // 实际实现应该使用流式响应
    // 这里先返回非流式结果
    handle_chat(state, message, session_id).await
}

/// WebSocket 处理函数（供路由使用）
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let manager = WebSocketManager::new(state);
    manager.handle_upgrade(ws)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_message_serialization() {
        let msg = WsMessage::Chat {
            session_id: Some("test".to_string()),
            message: "Hello".to_string(),
            stream: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("chat"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_ws_response_serialization() {
        let resp = WsResponse::ChatResponse {
            content: "Hi".to_string(),
            session_id: "test".to_string(),
            done: true,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("chat_response"));
        assert!(json.contains("Hi"));
    }

    #[test]
    fn test_ping_pong() {
        let ping = WsMessage::Ping;
        let json = serde_json::to_string(&ping).unwrap();
        assert!(json.contains("ping"));

        let pong = WsResponse::Pong;
        let json = serde_json::to_string(&pong).unwrap();
        assert!(json.contains("pong"));
    }
}
