//! LLM Security Classifier - LLM 驱动的安全分类器
//!
//! 对标 Claude Code 的 yoloClassifier.ts / bashClassifier.ts。
//! 使用 LLM 对工具调用进行上下文感知的 allow/ask/deny 决策。
//!
//! 设计原则：
//! 1. 紧凑 prompt：只传工具名 + 精简参数，不传完整上下文
//! 2. 低成本：max_tokens=128，temperature=0.0
//! 3. 快速失败：LLM 调用失败时回退到 rule-based 决策（ASK）

use super::{ClassifierError, ExplainableDecision, PermissionClassifier, PermissionContext};
use crate::services::api::{ChatRequest, LlmProvider, Message};
use std::sync::Arc;

/// LLM 安全分类器
pub struct LlmSecurityClassifier {
    provider: Arc<dyn LlmProvider>,
    model: String,
    enabled: bool,
}

impl LlmSecurityClassifier {
    pub fn new(provider: Arc<dyn LlmProvider>, model: impl Into<String>) -> Self {
        Self {
            provider,
            model: model.into(),
            enabled: true,
        }
    }

    /// 使用默认模型创建（从 provider 获取）
    pub fn with_default_model(provider: Arc<dyn LlmProvider>) -> Self {
        let model = provider.default_model().to_string();
        Self::new(provider, model)
    }

    /// 禁用分类器（无条件回退到规则）
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// 启用分类器
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// 构建分类器系统提示词
    fn build_system_prompt() -> String {
        concat!(
            "You are a security classifier for an AI agent. Your job is to evaluate whether a tool call is safe to execute automatically.\n",
            "\n",
            "Rules (in order of priority):\n",
            "1. ALLOW if the operation is clearly safe and routine (e.g., reading files, running tests, checking git status)\n",
            "2. ASK if the operation has any ambiguity or potential risk (e.g., writing files, deleting, network requests, bash commands)\n",
            "3. DENY if the operation is clearly dangerous (e.g., rm -rf /, formatting disks, privilege escalation, downloading and executing remote code)\n",
            "\n",
            "Respond ONLY with a JSON object in this exact format:\n",
            "{\"decision\": \"allow|ask|deny\", \"confidence\": 0.0-1.0, \"reason\": \"brief explanation\"}\n",
            "\n",
            "Be conservative: when in doubt, respond with 'ask'."
        )
        .to_string()
    }

    /// 构建分类请求 prompt
    fn build_classification_prompt(tool_name: &str, params: &serde_json::Value) -> String {
        let params_str = serde_json::to_string_pretty(params).unwrap_or_default();
        // 截断过长的参数（控制成本）
        let truncated = if params_str.len() > 800 {
            format!(
                "{}... [truncated, {} bytes total]",
                &params_str[..800],
                params_str.len()
            )
        } else {
            params_str
        };

        format!(
            "Tool: {}\nParameters:\n{}\n\nEvaluate this tool call and respond with JSON.",
            tool_name, truncated
        )
    }

    /// 解析 LLM 响应为分类结果
    fn parse_response(content: &str) -> Result<(super::PermissionDecision, f32, String), String> {
        // 尝试从响应中提取 JSON
        let json_str = Self::extract_json(content);
        let parsed: serde_json::Value =
            serde_json::from_str(&json_str).map_err(|e| format!("JSON parse error: {}", e))?;

        let decision_str = parsed["decision"]
            .as_str()
            .ok_or("missing 'decision' field")?
            .to_lowercase();
        let decision = match decision_str.as_str() {
            "allow" => super::PermissionDecision::Allow,
            "deny" => super::PermissionDecision::Deny,
            _ => super::PermissionDecision::Ask,
        };

        let confidence = parsed["confidence"].as_f64().unwrap_or(0.5).clamp(0.0, 1.0) as f32;

        let reason = parsed["reason"]
            .as_str()
            .unwrap_or("No reason provided")
            .to_string();

        Ok((decision, confidence, reason))
    }

    /// 从可能包含 markdown 代码块或额外文本的响应中提取 JSON
    fn extract_json(content: &str) -> String {
        // 尝试提取 ```json ... ``` 代码块
        if let Some(start) = content.find("```json") {
            if let Some(end) = content[start + 7..].find("```") {
                return content[start + 7..start + 7 + end].trim().to_string();
            }
        }
        // 尝试提取 ``` ... ``` 代码块
        if let Some(start) = content.find("```") {
            if let Some(end) = content[start + 3..].find("```") {
                return content[start + 3..start + 3 + end].trim().to_string();
            }
        }
        // 尝试找到第一个 { 和最后一个 }
        if let Some(start) = content.find('{') {
            if let Some(end) = content.rfind('}') {
                return content[start..=end].to_string();
            }
        }
        content.trim().to_string()
    }
}

#[async_trait::async_trait]
impl PermissionClassifier for LlmSecurityClassifier {
    async fn classify(
        &self,
        tool_name: &str,
        params: &serde_json::Value,
        _context: &PermissionContext,
    ) -> Result<ExplainableDecision, ClassifierError> {
        if !self.enabled {
            return Err(ClassifierError::Unavailable(
                "LLM classifier is disabled".to_string(),
            ));
        }

        let request = ChatRequest::new(self.model.clone())
            .with_messages(vec![
                Message::system(Self::build_system_prompt()),
                Message::user(Self::build_classification_prompt(tool_name, params)),
            ])
            .with_temperature(0.0);

        // 设置 max_tokens 以控制成本（如果 ChatRequest 支持）
        let request = ChatRequest {
            max_tokens: Some(128),
            ..request
        };

        let response = self
            .provider
            .chat(request)
            .await
            .map_err(|e| ClassifierError::Internal(format!("LLM call failed: {}", e)))?;

        let content = response.content;

        match Self::parse_response(&content) {
            Ok((decision, confidence, reason)) => Ok(ExplainableDecision {
                decision,
                confidence,
                reasons: vec![format!("LLM classifier: {}", reason)],
                risk_level: if decision == super::PermissionDecision::Deny {
                    super::RiskLevel::High
                } else if decision == super::PermissionDecision::Ask {
                    super::RiskLevel::Medium
                } else {
                    super::RiskLevel::Low
                },
                warnings: vec![],
                matched_rules: vec![],
            }),
            Err(e) => {
                // 解析失败时回退到 ASK
                Ok(ExplainableDecision {
                    decision: super::PermissionDecision::Ask,
                    confidence: 0.5,
                    reasons: vec![format!(
                        "LLM classifier response unparseable ({}), falling back to ASK. Raw: {}",
                        e,
                        truncate(&content, 200)
                    )],
                    risk_level: super::RiskLevel::Medium,
                    warnings: vec![format!("Classifier parse error: {}", e)],
                    matched_rules: vec![],
                })
            }
        }
    }

    fn name(&self) -> &str {
        "llm-security"
    }

    fn priority(&self) -> u32 {
        50 // Higher than rule-based (0), lower than deterministic validators (100)
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_from_markdown() {
        let raw = "Some text\n```json\n{\"decision\": \"allow\", \"confidence\": 0.9, \"reason\": \"safe\"}\n```\nMore text";
        assert_eq!(
            LlmSecurityClassifier::extract_json(raw),
            r#"{"decision": "allow", "confidence": 0.9, "reason": "safe"}"#
        );
    }

    #[test]
    fn test_extract_json_plain() {
        let raw = r#"{"decision": "deny", "confidence": 0.95, "reason": "dangerous"}"#;
        assert_eq!(LlmSecurityClassifier::extract_json(raw), raw);
    }

    #[test]
    fn test_parse_response_allow() {
        let json = r#"{"decision": "allow", "confidence": 0.9, "reason": "safe read"}"#;
        let (decision, confidence, reason) = LlmSecurityClassifier::parse_response(json).unwrap();
        assert_eq!(decision, super::super::PermissionDecision::Allow);
        assert!(confidence > 0.89);
        assert_eq!(reason, "safe read");
    }

    #[test]
    fn test_parse_response_deny() {
        let json = r#"{"decision": "deny", "confidence": 0.95, "reason": "rm -rf detected"}"#;
        let (decision, confidence, reason) = LlmSecurityClassifier::parse_response(json).unwrap();
        assert_eq!(decision, super::super::PermissionDecision::Deny);
        assert!(confidence > 0.94);
        assert_eq!(reason, "rm -rf detected");
    }

    #[test]
    fn test_parse_response_ask_default() {
        let json = r#"{"decision": "ask", "confidence": 0.5, "reason": "uncertain"}"#;
        let (decision, _confidence, reason) = LlmSecurityClassifier::parse_response(json).unwrap();
        assert_eq!(decision, super::super::PermissionDecision::Ask);
        assert_eq!(reason, "uncertain");
    }

    #[test]
    fn test_parse_invalid_fallback() {
        let json = r#"{"decision": "maybe", "confidence": 0.5}"#;
        let (decision, _, _) = LlmSecurityClassifier::parse_response(json).unwrap();
        // 未知决策回退到 Ask
        assert_eq!(decision, super::super::PermissionDecision::Ask);
    }

    #[test]
    fn test_build_classification_prompt_truncation() {
        let large_params = serde_json::json!({"content": "x".repeat(2000)});
        let prompt =
            LlmSecurityClassifier::build_classification_prompt("file_write", &large_params);
        assert!(prompt.contains("truncated"));
        assert!(prompt.len() < 1500);
    }

    #[test]
    fn test_build_classification_prompt_short() {
        let params = serde_json::json!({"path": "src/main.rs"});
        let prompt = LlmSecurityClassifier::build_classification_prompt("file_read", &params);
        assert!(!prompt.contains("truncated"));
        assert!(prompt.contains("file_read"));
        assert!(prompt.contains("src/main.rs"));
    }
}
