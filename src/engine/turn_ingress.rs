#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnIngressLane {
    SideQuestion,
    AgentTask,
}

impl TurnIngressLane {
    pub fn is_lightweight(self) -> bool {
        matches!(self, Self::SideQuestion)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::SideQuestion => "side_question",
            Self::AgentTask => "agent_task",
        }
    }
}

pub fn classify_turn_ingress(message: &str, has_contexts: bool) -> TurnIngressLane {
    if has_contexts {
        return TurnIngressLane::AgentTask;
    }

    let trimmed = message.trim();
    if trimmed.is_empty() {
        return TurnIngressLane::AgentTask;
    }

    if strip_btw_prefix(trimmed).is_some() {
        return TurnIngressLane::SideQuestion;
    }
    TurnIngressLane::AgentTask
}

pub fn lightweight_user_text(message: &str, lane: TurnIngressLane) -> String {
    let trimmed = message.trim();
    if lane == TurnIngressLane::SideQuestion {
        if let Some(question) = strip_btw_prefix(trimmed) {
            return question.trim().to_string();
        }
    }
    trimmed.to_string()
}

fn strip_btw_prefix(message: &str) -> Option<&str> {
    let lower = message.to_lowercase();
    if lower == "/btw" {
        return Some("");
    }
    lower.strip_prefix("/btw ").map(|_| {
        message[message
            .char_indices()
            .nth(5)
            .map(|(i, _)| i)
            .unwrap_or(message.len())..]
            .trim_start()
    })
}

#[cfg(test)]
mod tests {
    use super::{classify_turn_ingress, lightweight_user_text, TurnIngressLane};

    #[test]
    fn greeting_uses_main_agent_loop() {
        assert_eq!(
            classify_turn_ingress("你好", false),
            TurnIngressLane::AgentTask
        );
    }

    #[test]
    fn chinese_webpage_request_is_agent_task() {
        assert_eq!(
            classify_turn_ingress("帮我做一个天气预报网页", false),
            TurnIngressLane::AgentTask
        );
    }

    #[test]
    fn contexts_force_agent_task() {
        assert_eq!(
            classify_turn_ingress("你好", true),
            TurnIngressLane::AgentTask
        );
    }

    #[test]
    fn btw_is_side_question_and_strips_prefix() {
        let lane = classify_turn_ingress("/btw Rust 的 trait 是什么？", false);
        assert_eq!(lane, TurnIngressLane::SideQuestion);
        assert_eq!(
            lightweight_user_text("/btw Rust 的 trait 是什么？", lane),
            "Rust 的 trait 是什么？"
        );
    }
}
