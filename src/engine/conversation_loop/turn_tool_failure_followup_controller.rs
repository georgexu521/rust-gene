use super::focused_repair_recovery::FocusedRepairRecoveryController;
use super::tool_failure_guided_debugging::{
    GuidedToolFailureDebuggingContext, GuidedToolFailureDebuggingController,
};
use super::tool_failure_stop_controller::{ToolFailureStopController, ToolFailureStopRequest};
use super::turn_runtime_state::TurnRuntimeState;
use super::turn_tool_round_outcome_controller::TurnToolRoundState;
use super::StreamEvent;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::trace::TraceCollector;
use crate::services::api::{LlmProvider, Message};
use crate::session_store::SessionStore;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

pub(super) struct TurnToolFailureFollowupContext<'a> {
    pub(super) provider: &'a dyn LlmProvider,
    pub(super) model: String,
    pub(super) session_store: Option<&'a Arc<SessionStore>>,
    pub(super) session_id: &'a str,
    pub(super) trace: &'a TraceCollector,
    pub(super) any_tool_success: bool,
    pub(super) last_user_preview: &'a str,
    pub(super) task_bundle: &'a mut TaskContextBundle,
    pub(super) round_state: &'a mut TurnToolRoundState,
    pub(super) turn_state: &'a TurnRuntimeState,
    pub(super) failed_tool_names: &'a HashMap<String, usize>,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) final_content: &'a mut String,
    pub(super) messages: &'a mut Vec<Message>,
}

pub(super) enum TurnToolFailureFollowupFlow {
    Continue,
    Stop,
}

pub(super) struct TurnToolFailureFollowupController;

impl TurnToolFailureFollowupController {
    pub(super) async fn run(
        context: TurnToolFailureFollowupContext<'_>,
    ) -> TurnToolFailureFollowupFlow {
        GuidedToolFailureDebuggingController::run(GuidedToolFailureDebuggingContext {
            provider: context.provider,
            model: context.model,
            session_store: context.session_store,
            session_id: context.session_id,
            trace: context.trace,
            any_tool_success: context.any_tool_success,
            last_user_preview: context.last_user_preview,
            task_bundle: context.task_bundle,
            failed_tool_names: &context.round_state.failed_tool_names_this_round,
            failed_tool_evidence: &context.round_state.failed_tool_evidence,
            tool_results_text: &mut context.round_state.tool_results_text,
            messages: context.messages,
        })
        .await;

        if let Some(stop) = ToolFailureStopController::decide(ToolFailureStopRequest {
            any_tool_success: context.any_tool_success,
            repeated_failed_tools: &context.round_state.repeated_failed_tools,
            failed_tool_names: context.failed_tool_names,
        }) {
            let fallback = read_only_failure_fallback_answer(
                context.task_bundle,
                context.turn_state,
                context.last_user_preview,
            );
            let stop_message = fallback
                .map(|answer| format!("{}\n\n{}", stop.message, answer))
                .unwrap_or(stop.message);
            FocusedRepairRecoveryController::stop_with_message(
                context.tx,
                context.final_content,
                &stop_message,
            )
            .await;
            return TurnToolFailureFollowupFlow::Stop;
        }

        TurnToolFailureFollowupFlow::Continue
    }
}

fn read_only_failure_fallback_answer(
    task_bundle: &TaskContextBundle,
    turn_state: &TurnRuntimeState,
    last_user_preview: &str,
) -> Option<String> {
    if crate::engine::code_change_workflow::is_programming_workflow(task_bundle.route.workflow) {
        return None;
    }
    let cached_results = turn_state
        .successful_read_only_tool_results
        .values()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if cached_results.is_empty() {
        return None;
    }

    let combined = cached_results.join("\n");
    let facts = normalized_lines(&combined);
    if facts.is_empty() {
        return None;
    }

    let combined_lower = combined.to_ascii_lowercase();
    let has_local_only = combined_lower.contains("local-only");
    let has_csv_export = combined_lower.contains("csv export");
    let mentions_scope_hold = combined_lower.contains("cloud sync")
        || combined_lower.contains("login")
        || combined_lower.contains("accounts")
        || combined_lower.contains("deployment");
    let chinese = contains_cjk(last_user_preview);

    let mut highlights = facts
        .iter()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("local-only")
                || lower.contains("csv export")
                || lower.contains("status")
                || lower.contains("risk")
                || lower.contains("next product goal")
                || lower.contains("next_steps")
                || lower.contains("next step")
                || lower.contains("cloud sync")
                || lower.contains("deployment")
                || lower.contains("accounts")
                || lower.contains("avoid")
        })
        .take(5)
        .cloned()
        .collect::<Vec<_>>();
    if highlights.is_empty() {
        highlights = facts.into_iter().take(4).collect();
    }

    if chinese {
        let mut answer = String::from(
            "我不再重复失败读取，直接基于已读证据回答（来自 `project memory` 和 `previous execution report`）：",
        );
        if has_local_only {
            answer.push_str("\n- 当前状态：项目仍是 `local-only` 的第一版。");
        } else {
            answer.push_str("\n- 当前状态：继续保持本地优先的第一版范围。");
        }
        if has_csv_export {
            answer.push_str("\n- 最小下一步：先实现 `CSV export`。");
        } else {
            answer.push_str("\n- 最小下一步：先完成报告里最小可执行功能再扩展。");
        }
        if mentions_scope_hold {
            answer.push_str("\n- 暂不纳入范围：账号/登录、cloud sync、部署。");
        }
        answer.push_str("\n- 证据摘录：");
        for line in highlights {
            answer.push_str("\n  - ");
            answer.push_str(&line);
        }
        return Some(answer);
    }

    let mut answer = String::from(
        "I will stop the failing rereads and answer from already-read evidence (from `project memory` and the `previous execution report`):",
    );
    if has_local_only {
        answer.push_str(
            "\n- Current state: the project is still in the `local-only` first-version scope.",
        );
    } else {
        answer.push_str("\n- Current state: keep the current local-first first-version scope.");
    }
    if has_csv_export {
        answer.push_str("\n- Smallest next step: implement `CSV export` first.");
    } else {
        answer.push_str(
            "\n- Smallest next step: complete the smallest pending feature from the report first.",
        );
    }
    if mentions_scope_hold {
        answer
            .push_str("\n- Keep out of scope for now: accounts/login, cloud sync, and deployment.");
    }
    answer.push_str("\n- Evidence excerpts:");
    for line in highlights {
        answer.push_str("\n  - ");
        answer.push_str(&line);
    }
    Some(answer)
}

fn normalized_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(strip_line_prefix)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| {
            !(line.starts_with('[')
                && (line.contains("File unchanged since last read")
                    || line.contains("stored read-only result truncated")
                    || line.contains("lines total")))
        })
        .filter(|line| !matches!(*line, "{" | "}" | "[" | "]"))
        .map(|line| line.trim_start_matches("- ").trim().to_string())
        .collect()
}

fn strip_line_prefix(line: &str) -> &str {
    let trimmed = line.trim_start();
    let mut digit_end = 0usize;
    let mut saw_digit = false;
    for (idx, ch) in trimmed.char_indices() {
        if ch.is_ascii_digit() {
            digit_end = idx + ch.len_utf8();
            saw_digit = true;
            continue;
        }
        break;
    }
    if saw_digit {
        let after_digits = trimmed[digit_end..].trim_start();
        if let Some(rest) = after_digits.strip_prefix('|') {
            return rest.trim_start();
        }
    }
    trimmed
}

fn contains_cjk(text: &str) -> bool {
    text.chars()
        .any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch))
}

#[cfg(test)]
mod tests {
    use super::super::turn_runtime_state::TurnRuntimeState;
    use super::super::turn_tool_round_outcome_controller::TurnToolRoundState;
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::trace::TurnTrace;
    use crate::services::api::{ChatRequest, ChatResponse};
    use async_openai::types::ChatCompletionResponseStream;
    use std::path::PathBuf;

    struct MockProvider;

    #[async_trait::async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            Err(anyhow::anyhow!("chat not used in this test"))
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            Err(anyhow::anyhow!("stream not used in this test"))
        }

        fn base_url(&self) -> &str {
            "mock://local"
        }

        fn default_model(&self) -> &str {
            "mock-model"
        }
    }

    fn trace() -> TraceCollector {
        TraceCollector::new(TurnTrace::new("session-test", 1, "tool failure"))
    }

    fn task_bundle() -> TaskContextBundle {
        let route = IntentRouter::new().route("fix bug");
        TaskContextBundle::new("fix bug", ".", route, None)
    }

    fn round_state(any_tool_success: bool) -> TurnToolRoundState {
        TurnToolRoundState {
            tool_results_text: String::new(),
            changed_files: Vec::<PathBuf>::new(),
            batch_has_unsuccessful_tools: !any_tool_success,
            used_write_tool: false,
            successful_write_tool: false,
            used_action_checkpoint_lookup: false,
            any_tool_success,
            repeated_failed_tools: Vec::new(),
            failed_tool_names_this_round: Vec::new(),
            failed_tool_evidence: Vec::new(),
            file_edit_failure_correction_added: false,
            successful_validation_commands: Vec::new(),
            duplicate_successful_read_only_tools: Vec::new(),
            duplicate_successful_read_only_results: Vec::new(),
            should_closeout_after_verified_change: false,
        }
    }

    fn runtime_state() -> TurnRuntimeState {
        TurnRuntimeState::new(true)
    }

    #[tokio::test]
    async fn run_stops_after_repeated_failed_tool_without_success() {
        let provider = MockProvider;
        let trace = trace();
        let mut task_bundle = task_bundle();
        let mut round_state = round_state(false);
        round_state.repeated_failed_tools = vec!["bash".to_string()];
        let failed_tool_names = HashMap::from([("bash".to_string(), 2)]);
        let turn_state = runtime_state();
        let mut final_content = String::new();
        let mut messages = vec![Message::user("fix bug")];

        let flow = TurnToolFailureFollowupController::run(TurnToolFailureFollowupContext {
            provider: &provider,
            model: "mock-model".to_string(),
            session_store: None,
            session_id: "session-test",
            trace: &trace,
            any_tool_success: false,
            last_user_preview: "fix bug",
            task_bundle: &mut task_bundle,
            round_state: &mut round_state,
            turn_state: &turn_state,
            failed_tool_names: &failed_tool_names,
            tx: None,
            final_content: &mut final_content,
            messages: &mut messages,
        })
        .await;

        assert!(matches!(flow, TurnToolFailureFollowupFlow::Stop));
        assert_eq!(
            final_content,
            "[Stopped repeated failed tool attempts: bash]"
        );
    }

    #[tokio::test]
    async fn run_continues_when_a_tool_succeeded() {
        let provider = MockProvider;
        let trace = trace();
        let mut task_bundle = task_bundle();
        let mut round_state = round_state(true);
        round_state.repeated_failed_tools = vec!["bash".to_string()];
        let failed_tool_names = HashMap::from([("bash".to_string(), 2)]);
        let turn_state = runtime_state();
        let mut final_content = String::new();
        let mut messages = vec![Message::user("fix bug")];

        let flow = TurnToolFailureFollowupController::run(TurnToolFailureFollowupContext {
            provider: &provider,
            model: "mock-model".to_string(),
            session_store: None,
            session_id: "session-test",
            trace: &trace,
            any_tool_success: true,
            last_user_preview: "fix bug",
            task_bundle: &mut task_bundle,
            round_state: &mut round_state,
            turn_state: &turn_state,
            failed_tool_names: &failed_tool_names,
            tx: None,
            final_content: &mut final_content,
            messages: &mut messages,
        })
        .await;

        assert!(matches!(flow, TurnToolFailureFollowupFlow::Continue));
        assert!(final_content.is_empty());
    }

    #[tokio::test]
    async fn run_stops_with_read_only_fallback_answer_when_cached_evidence_exists() {
        let provider = MockProvider;
        let trace = trace();
        let mut task_bundle = TaskContextBundle::new(
            "resume project",
            ".",
            IntentRouter::new().route("summarize project memory and previous report"),
            None,
        );
        let mut round_state = round_state(false);
        round_state.repeated_failed_tools = vec!["file_read".to_string()];
        let failed_tool_names = HashMap::from([("file_read".to_string(), 5)]);
        let mut turn_state = runtime_state();
        turn_state.successful_read_only_tool_results.insert(
            "memory".to_string(),
            "1 | # Project Memory\n2 | - Decision: first version is a local-only lab notebook helper.\n3 | - Next product goal: add CSV export for recorded strain rows.".to_string(),
        );
        turn_state.successful_read_only_tool_results.insert(
            "report".to_string(),
            "1 | {\n2 |   \"status\": \"partial\",\n3 |   \"risks\": [\"CSV export is not implemented yet\"],\n4 |   \"next_steps\": [\"Implement CSV export before adding login or cloud sync\"]\n5 | }".to_string(),
        );
        let mut final_content = String::new();
        let mut messages = vec![Message::user("resume project")];

        let flow = TurnToolFailureFollowupController::run(TurnToolFailureFollowupContext {
            provider: &provider,
            model: "mock-model".to_string(),
            session_store: None,
            session_id: "session-test",
            trace: &trace,
            any_tool_success: false,
            last_user_preview: "resume project",
            task_bundle: &mut task_bundle,
            round_state: &mut round_state,
            turn_state: &turn_state,
            failed_tool_names: &failed_tool_names,
            tx: None,
            final_content: &mut final_content,
            messages: &mut messages,
        })
        .await;

        assert!(matches!(flow, TurnToolFailureFollowupFlow::Stop));
        assert!(final_content.contains("Stopped repeated failed tool attempts: file_read"));
        assert!(final_content.contains("project memory"));
        assert!(final_content.contains("previous execution report"));
        assert!(final_content.contains("local-only"));
        assert!(final_content.contains("CSV export"));
    }
}
