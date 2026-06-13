//! Local deterministic provider for end-to-end TUI/runtime tests.
//!
//! This provider is registered only when `PRIORITY_AGENT_TEST_PROVIDER_SCENARIO`
//! is set. It lets PTY smoke tests exercise provider/tool-turn edge cases
//! without relying on external network timing.

use super::{ChatRequest, ChatResponse, LlmProvider, Message, ToolCall};
use async_openai::types::ChatCompletionResponseStream;
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

#[derive(Debug)]
pub struct TestProvider {
    scenario: TestProviderScenario,
    calls: AtomicUsize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestProviderScenario {
    ToolPwd,
    ToolFail,
    ToolLong,
    ToolSleep,
    ToolInvalidArgs,
    ToolMulti,
    ToolPartial,
    ToolMalformed,
    ToolTimeoutAfterResult,
    TextOk,
}

impl TestProvider {
    pub fn from_env() -> Option<Self> {
        let scenario = std::env::var("PRIORITY_AGENT_TEST_PROVIDER_SCENARIO")
            .ok()
            .and_then(|value| TestProviderScenario::parse(&value))?;
        Some(Self {
            scenario,
            calls: AtomicUsize::new(0),
        })
    }

    fn tool_response(tool_calls: Vec<ToolCall>) -> ChatResponse {
        ChatResponse {
            content: String::new(),
            tool_calls: Some(tool_calls),
            usage: None,
            tool_call_repair: None,
        }
    }

    fn text_response(content: impl Into<String>) -> ChatResponse {
        ChatResponse {
            content: content.into(),
            tool_calls: None,
            usage: None,
            tool_call_repair: None,
        }
    }

    fn first_round(&self) -> ChatResponse {
        match self.scenario {
            TestProviderScenario::TextOk => Self::text_response("OK"),
            TestProviderScenario::ToolPwd | TestProviderScenario::ToolTimeoutAfterResult => {
                Self::tool_response(vec![ToolCall {
                    id: "call_test_pwd".to_string(),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({"command": "pwd"}),
                }])
            }
            TestProviderScenario::ToolFail => Self::tool_response(vec![ToolCall {
                id: "call_test_fail".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({"command": "sh -lc 'exit 7'"}),
            }]),
            TestProviderScenario::ToolLong => Self::tool_response(vec![ToolCall {
                id: "call_test_long".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({"command": "python3 -c 'print(\"x\" * 5000)'"}),
            }]),
            TestProviderScenario::ToolSleep => Self::tool_response(vec![ToolCall {
                id: "call_test_sleep".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({"command": "sleep 20"}),
            }]),
            TestProviderScenario::ToolInvalidArgs => Self::tool_response(vec![ToolCall {
                id: "call_test_invalid".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({}),
            }]),
            TestProviderScenario::ToolMulti => Self::tool_response(vec![
                ToolCall {
                    id: "call_test_pwd".to_string(),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({"command": "pwd"}),
                },
                ToolCall {
                    id: "call_test_echo".to_string(),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({"command": "echo multi-tool-ok"}),
                },
            ]),
            TestProviderScenario::ToolPartial => Self::tool_response(vec![
                ToolCall {
                    id: "call_test_partial_ok".to_string(),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({"command": "echo partial-ok"}),
                },
                ToolCall {
                    id: "call_test_partial_fail".to_string(),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({"command": "sh -lc 'echo partial-fail >&2; exit 9'"}),
                },
            ]),
            TestProviderScenario::ToolMalformed => {
                let mut report =
                    crate::services::api::tool_call_repair::ToolCallRepairReport::new(
                        crate::services::api::provider_protocol::ProviderProtocolFamily::OpenAiCompatible,
                    );
                report.malformed_tool_calls = 1;
                report.argument_repairs = 1;
                report
                    .warnings
                    .push("fixture simulated malformed tool call repaired".to_string());
                ChatResponse {
                    content: String::new(),
                    tool_calls: Some(vec![ToolCall {
                        id: "call_test_malformed_repaired".to_string(),
                        name: "bash".to_string(),
                        arguments: serde_json::json!({"command": "echo malformed-repaired"}),
                    }]),
                    usage: None,
                    tool_call_repair: Some(report),
                }
            }
        }
    }

    async fn final_round(&self, request: &ChatRequest) -> anyhow::Result<ChatResponse> {
        if self.scenario == TestProviderScenario::ToolTimeoutAfterResult {
            let sleep_secs = std::env::var("PRIORITY_AGENT_TEST_PROVIDER_SLEEP_SECS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(35);
            tokio::time::sleep(Duration::from_secs(sleep_secs)).await;
        }

        let tool_results = request
            .messages
            .iter()
            .filter_map(|message| match message {
                Message::Tool { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let failed = tool_results
            .iter()
            .any(|content| content.contains("Result: ERROR"));
        let invalid = tool_results
            .iter()
            .any(|content| content.contains("schema_validation"));
        let multi = self.scenario == TestProviderScenario::ToolMulti;
        let partial = self.scenario == TestProviderScenario::ToolPartial;
        let long = self.scenario == TestProviderScenario::ToolLong;
        let content = if invalid {
            "工具参数无效，运行时已经把错误结果回传给模型。"
        } else if partial {
            "多工具部分失败，成功和失败结果都已经回传给模型。"
        } else if failed {
            "命令执行失败，工具错误已经被模型看到。"
        } else if multi {
            "两个工具都执行完成并回传给模型。"
        } else if self.scenario == TestProviderScenario::ToolMalformed {
            "畸形工具调用已在 provider 边界修复，工具结果已经回传给模型。"
        } else if self.scenario == TestProviderScenario::ToolSleep {
            "等待工具执行完成并回传给模型。"
        } else if long {
            "长输出工具执行完成，模型只收到了压缩观察。"
        } else {
            "工具执行完成并回传给模型。"
        };
        Ok(Self::text_response(content))
    }
}

impl TestProviderScenario {
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "text-ok" | "provider-ok" => Some(Self::TextOk),
            "tool-pwd" => Some(Self::ToolPwd),
            "tool-fail" => Some(Self::ToolFail),
            "tool-long" => Some(Self::ToolLong),
            "tool-sleep" => Some(Self::ToolSleep),
            "tool-invalid-args" | "invalid-args" => Some(Self::ToolInvalidArgs),
            "tool-multi" | "multi-tool" => Some(Self::ToolMulti),
            "tool-partial" | "multi-tool-partial" | "partial-failure" => Some(Self::ToolPartial),
            "tool-malformed" | "malformed-tool-call" => Some(Self::ToolMalformed),
            "tool-timeout-after-result" | "provider-timeout" => Some(Self::ToolTimeoutAfterResult),
            _ => None,
        }
    }
}

#[async_trait]
impl LlmProvider for TestProvider {
    async fn chat(&self, request: ChatRequest) -> anyhow::Result<ChatResponse> {
        let call_index = self.calls.fetch_add(1, Ordering::SeqCst);
        let has_tool_result = request
            .messages
            .iter()
            .any(|message| matches!(message, Message::Tool { .. }));
        if call_index == 0 || !has_tool_result {
            return Ok(self.first_round());
        }
        self.final_round(&request).await
    }

    async fn chat_stream(
        &self,
        _request: ChatRequest,
    ) -> anyhow::Result<ChatCompletionResponseStream> {
        anyhow::bail!("test provider does not implement streaming")
    }

    fn base_url(&self) -> &str {
        "test://provider"
    }

    fn default_model(&self) -> &str {
        "test-fixture-model"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_test_provider_scenarios() {
        assert_eq!(
            TestProviderScenario::parse("tool-timeout-after-result"),
            Some(TestProviderScenario::ToolTimeoutAfterResult)
        );
        assert_eq!(
            TestProviderScenario::parse("multi-tool"),
            Some(TestProviderScenario::ToolMulti)
        );
        assert_eq!(
            TestProviderScenario::parse("partial-failure"),
            Some(TestProviderScenario::ToolPartial)
        );
        assert_eq!(
            TestProviderScenario::parse("malformed-tool-call"),
            Some(TestProviderScenario::ToolMalformed)
        );
        assert_eq!(TestProviderScenario::parse("unknown"), None);
    }
}
