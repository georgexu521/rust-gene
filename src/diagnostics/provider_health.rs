use crate::services::api::{ChatRequest, LlmProvider, Message, Tool, ToolCall, ToolChoice};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderHealthStatus {
    Ok,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealthStep {
    pub name: String,
    pub status: ProviderHealthStatus,
    pub duration_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealthReport {
    pub status: ProviderHealthStatus,
    pub model: String,
    pub base_url: String,
    pub timeout_secs: u64,
    pub duration_ms: u128,
    pub steps: Vec<ProviderHealthStep>,
}

impl ProviderHealthReport {
    pub fn is_ok(&self) -> bool {
        self.status == ProviderHealthStatus::Ok
    }

    pub fn failure_summary(&self) -> String {
        self.steps
            .iter()
            .find(|step| step.status == ProviderHealthStatus::Failed)
            .and_then(|step| {
                step.error
                    .as_deref()
                    .map(|error| (step.name.as_str(), error))
            })
            .map(|(name, error)| format!("{name}: {error}"))
            .unwrap_or_else(|| "provider health failed".to_string())
    }
}

pub async fn run_provider_health(
    provider: Arc<dyn LlmProvider>,
    model: impl Into<String>,
    timeout: Duration,
) -> ProviderHealthReport {
    let model = model.into();
    let base_url = provider.base_url().to_string();
    let started = Instant::now();
    let mut steps = Vec::new();

    steps.push(run_plain_chat(provider.clone(), &model, timeout).await);

    let (tool_step, tool_call) = run_tool_call(provider.clone(), &model, timeout).await;
    steps.push(tool_step);

    if let Some(tool_call) = tool_call {
        steps.push(run_tool_result_continuation(provider, &model, timeout, tool_call).await);
    }

    let status = if steps
        .iter()
        .all(|step| step.status == ProviderHealthStatus::Ok)
    {
        ProviderHealthStatus::Ok
    } else {
        ProviderHealthStatus::Failed
    };

    ProviderHealthReport {
        status,
        model,
        base_url,
        timeout_secs: timeout.as_secs(),
        duration_ms: started.elapsed().as_millis(),
        steps,
    }
}

async fn run_plain_chat(
    provider: Arc<dyn LlmProvider>,
    model: &str,
    timeout: Duration,
) -> ProviderHealthStep {
    let mut request = ChatRequest::new(model)
        .with_temperature(0.0)
        .with_messages(vec![
            Message::system("You are a provider health probe. Answer tersely."),
            Message::user("Reply with exactly: provider-health-ok"),
        ]);
    request.max_tokens = Some(256);

    run_step("plain_chat", timeout, async move {
        let response = provider.chat(request).await?;
        if response.content.trim().is_empty() {
            anyhow::bail!("provider returned empty plain chat content");
        }
        Ok(format!(
            "content_chars={}",
            response.content.chars().count()
        ))
    })
    .await
}

async fn run_tool_call(
    provider: Arc<dyn LlmProvider>,
    model: &str,
    timeout: Duration,
) -> (ProviderHealthStep, Option<ToolCall>) {
    let tool = provider_health_tool();
    let mut request = ChatRequest::new(model)
        .with_temperature(0.0)
        .with_tools(vec![tool])
        .with_tool_choice(ToolChoice::Function("provider_health_echo".to_string()))
        .with_messages(vec![
            Message::system("You are a provider health probe. Use the available tool when asked."),
            Message::user(
                "Call provider_health_echo exactly once with value \"provider-health-ok\".",
            ),
        ]);
    request.max_tokens = Some(256);

    let started = Instant::now();
    match tokio::time::timeout(timeout, provider.chat(request)).await {
        Ok(Ok(response)) => {
            let Some(calls) = response.tool_calls else {
                return (
                    failed_step(
                        "tool_call",
                        started,
                        "provider returned no tool call",
                        Some("provider_semantics"),
                    ),
                    None,
                );
            };
            let Some(call) = calls
                .into_iter()
                .find(|call| call.name == "provider_health_echo")
            else {
                return (
                    failed_step(
                        "tool_call",
                        started,
                        "provider returned tool calls, but not provider_health_echo",
                        Some("provider_semantics"),
                    ),
                    None,
                );
            };
            (
                ok_step("tool_call", started, format!("tool_call_id={}", call.id)),
                Some(call),
            )
        }
        Ok(Err(error)) => (
            failed_step(
                "tool_call",
                started,
                error.to_string(),
                Some(provider_health_error_category(&error.to_string())),
            ),
            None,
        ),
        Err(_) => (
            failed_step(
                "tool_call",
                started,
                format!(
                    "provider health step timed out after {}s",
                    timeout.as_secs()
                ),
                Some("timeout"),
            ),
            None,
        ),
    }
}

async fn run_tool_result_continuation(
    provider: Arc<dyn LlmProvider>,
    model: &str,
    timeout: Duration,
    tool_call: ToolCall,
) -> ProviderHealthStep {
    let mut request = ChatRequest::new(model)
        .with_temperature(0.0)
        .with_messages(vec![
            Message::system("You are a provider health probe. Continue after the tool result."),
            Message::user("Use the tool result and reply with a one-line Closeout."),
            Message::assistant_with_tools("", vec![tool_call.clone()]),
            Message::tool(tool_call.id, "Result: OK\nprovider-health-ok"),
        ]);
    request.max_tokens = Some(256);

    run_step("tool_result_continuation", timeout, async move {
        let response = provider.chat(request).await?;
        let content = response.content.trim();
        if content.is_empty() {
            anyhow::bail!("provider returned empty continuation content");
        }
        Ok(format!("content_chars={}", content.chars().count()))
    })
    .await
}

async fn run_step<F>(name: &'static str, timeout: Duration, future: F) -> ProviderHealthStep
where
    F: std::future::Future<Output = anyhow::Result<String>>,
{
    let started = Instant::now();
    match tokio::time::timeout(timeout, future).await {
        Ok(Ok(detail)) => ok_step(name, started, detail),
        Ok(Err(error)) => failed_step(
            name,
            started,
            error.to_string(),
            Some(provider_health_error_category(&error.to_string())),
        ),
        Err(_) => failed_step(
            name,
            started,
            format!(
                "provider health step timed out after {}s",
                timeout.as_secs()
            ),
            Some("timeout"),
        ),
    }
}

fn provider_health_tool() -> Tool {
    Tool::new(
        "provider_health_echo",
        "Health-check tool. Echo the required value exactly as requested.",
    )
    .with_parameters(json!({
        "type": "object",
        "properties": {
            "value": {
                "type": "string",
                "description": "The exact health-check value."
            }
        },
        "required": ["value"],
        "additionalProperties": false
    }))
}

fn ok_step(
    name: impl Into<String>,
    started: Instant,
    detail: impl Into<String>,
) -> ProviderHealthStep {
    ProviderHealthStep {
        name: name.into(),
        status: ProviderHealthStatus::Ok,
        duration_ms: started.elapsed().as_millis(),
        detail: Some(detail.into()),
        error: None,
        error_category: None,
    }
}

fn failed_step(
    name: impl Into<String>,
    started: Instant,
    error: impl Into<String>,
    category: Option<&str>,
) -> ProviderHealthStep {
    ProviderHealthStep {
        name: name.into(),
        status: ProviderHealthStatus::Failed,
        duration_ms: started.elapsed().as_millis(),
        detail: None,
        error: Some(error.into()),
        error_category: category.map(ToString::to_string),
    }
}

pub fn provider_health_error_category(error: &str) -> &'static str {
    let lower = error.to_ascii_lowercase();
    if lower.contains("unauthorized")
        || lower.contains("invalid api key")
        || lower.contains("401")
        || lower.contains("403")
    {
        "auth"
    } else if lower.contains("rate limit")
        || lower.contains("too many requests")
        || lower.contains("429")
    {
        "rate_limit"
    } else if lower.contains("does not follow tool call")
        || lower.contains("tool call result")
        || lower.contains("tool_call_id")
    {
        "protocol"
    } else if lower.contains("invalid params")
        || lower.contains("bad_request")
        || lower.contains("schema")
        || lower.contains("400")
    {
        "schema"
    } else if lower.contains("timed out") || lower.contains("timeout") {
        "timeout"
    } else if lower.contains("error sending request")
        || lower.contains("connection refused")
        || lower.contains("connection reset")
        || lower.contains("dns")
        || lower.contains("network")
        || lower.contains("provider unavailable")
    {
        "transport"
    } else {
        "provider"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categorizes_common_provider_errors() {
        assert_eq!(
            provider_health_error_category("error sending request for url"),
            "transport"
        );
        assert_eq!(
            provider_health_error_category(
                "invalid params, tool call result does not follow tool call (2013)"
            ),
            "protocol"
        );
        assert_eq!(
            provider_health_error_category("bad_request_error: invalid params"),
            "schema"
        );
        assert_eq!(
            provider_health_error_category("429 rate limit"),
            "rate_limit"
        );
        assert_eq!(provider_health_error_category("401 unauthorized"), "auth");
    }

    #[test]
    fn provider_health_tool_requires_value() {
        let tool = provider_health_tool();
        assert_eq!(tool.name, "provider_health_echo");
        assert_eq!(tool.parameters["required"][0], "value");
    }

    #[test]
    fn report_failure_summary_names_first_failed_step() {
        let report = ProviderHealthReport {
            status: ProviderHealthStatus::Failed,
            model: "model".to_string(),
            base_url: "https://example.test".to_string(),
            timeout_secs: 30,
            duration_ms: 1,
            steps: vec![failed_step(
                "plain_chat",
                Instant::now(),
                "connection reset",
                Some("transport"),
            )],
        };

        assert!(report.failure_summary().contains("plain_chat"));
        assert!(!report.is_ok());
    }
}
