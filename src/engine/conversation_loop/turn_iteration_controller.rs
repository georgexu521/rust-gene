use super::tool_context_helpers::tool_call_fingerprint;
use super::tool_execution::is_read_only;
use super::turn_focused_repair_flow_controller::{
    TurnFocusedRepairFlow, TurnFocusedRepairFlowContext, TurnFocusedRepairFlowController,
};
use super::turn_iteration_setup_controller::{
    TurnIterationSetupContext, TurnIterationSetupController, TurnIterationSetupFlow,
};
use super::turn_loop_state_controller::TurnLoopState;
use super::turn_model_step_controller::{
    TurnModelStepContext, TurnModelStepController, TurnModelStepFlow,
};
use super::turn_post_change_closeout_controller::{
    TurnPostChangeCloseoutContext, TurnPostChangeCloseoutController, TurnPostChangeCloseoutFlow,
};
use super::turn_runtime_context::TurnRuntimeContext;
use super::turn_runtime_state::TurnRuntimeState;
use super::turn_tool_failure_followup_controller::{
    TurnToolFailureFollowupContext, TurnToolFailureFollowupController, TurnToolFailureFollowupFlow,
};
use super::turn_tool_round_step_controller::{
    TurnToolRoundStepContext, TurnToolRoundStepController,
};
use super::ConversationLoop;
use crate::engine::code_change_workflow::CodeChangeWorkflowRunner;
use crate::engine::destructive_scope::DestructiveScopeContract;
use crate::engine::intent_router::IntentRoute;
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::retrieval_context::RetrievalContext;
use crate::engine::stop_checker::{StopCheckInput, StopChecker};
use crate::engine::streaming::StreamEvent;
use crate::engine::task_context::{AgentToolRoundObservation, TaskContextBundle};
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::{Message, Tool, ToolCall};
use crate::tools::ToolContextRetainedContext;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

pub(super) struct TurnIterationContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) iteration: usize,
    pub(super) route: &'a IntentRoute,
    pub(super) code_workflow: &'a mut CodeChangeWorkflowRunner,
    pub(super) task_bundle: &'a mut TaskContextBundle,
    pub(super) turn_retrieval_context: Option<&'a RetrievalContext>,
    pub(super) retained_context: &'a ToolContextRetainedContext,
    pub(super) base_tools: &'a [Tool],
    pub(super) loop_state: &'a mut TurnLoopState,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) no_diff_audit_closeout_allowed: bool,
    pub(super) code_write_tools_forbidden: bool,
    pub(super) resource_policy: &'a ResourcePolicy,
    pub(super) working_dir: &'a Path,
    pub(super) last_user_preview: &'a str,
    pub(super) required_validation_commands: &'a [String],
    pub(super) destructive_scope: &'a DestructiveScopeContract,
    pub(super) baseline_git_status_files: &'a HashSet<PathBuf>,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) trace: &'a TraceCollector,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
}

pub(super) enum TurnIterationFlow {
    Continue,
    Break,
}

pub(super) struct TurnIterationController;

impl TurnIterationController {
    pub(super) async fn run(
        context: TurnIterationContext<'_>,
    ) -> anyhow::Result<TurnIterationFlow> {
        let exposure_plan = match TurnIterationSetupController::run(TurnIterationSetupContext {
            iteration: context.iteration,
            max_iterations: context.conversation.max_iterations,
            turn_state: &mut *context.turn_state,
            memory_manager: context.conversation.memory_manager.as_ref(),
            trace: context.trace,
            route_workflow: context.route.workflow,
            task_stage: context.task_bundle.agent_state.stage,
            baseline_git_status_files: context.baseline_git_status_files,
            base_tools: context.base_tools,
        })
        .await
        {
            TurnIterationSetupFlow::Continue { exposure_plan } => exposure_plan,
            TurnIterationSetupFlow::Stop => return Ok(TurnIterationFlow::Break),
        };
        let tools = exposure_plan.tools;
        let exposed_tool_names = exposure_plan.exposed_tool_names;

        let (content, tool_calls, pre_executed) =
            match TurnModelStepController::run(TurnModelStepContext {
                conversation: context.conversation,
                iteration: context.iteration + 1,
                route: context.route,
                code_workflow: &*context.code_workflow,
                task_bundle: &*context.task_bundle,
                turn_retrieval_context: context.turn_retrieval_context,
                focused_repair_prompt: exposure_plan.focused_repair_prompt,
                tools: &tools,
                exposed_tool_names: &exposed_tool_names,
                resource_policy: context.resource_policy,
                loop_state: &mut *context.loop_state,
                turn_state: &mut *context.turn_state,
                messages: &mut *context.messages,
                trace: context.trace,
                tx: context.tx,
            })
            .await?
            {
                TurnModelStepFlow::Retry => return Ok(TurnIterationFlow::Continue),
                TurnModelStepFlow::Finish => return Ok(TurnIterationFlow::Break),
                TurnModelStepFlow::ToolRound {
                    content,
                    tool_calls,
                    pre_executed,
                } => (content, tool_calls, pre_executed),
            };

        if let Some(message) = duplicate_successful_read_only_pre_execution_closeout(
            &tool_calls,
            context.turn_state,
            context.last_user_preview,
            context.conversation.session_store.as_ref(),
            &context.conversation.session_id,
        ) {
            context.loop_state.final_content.push_str(&message);
            if let Some(tx) = context.tx {
                let _ = tx.send(StreamEvent::TextChunk(message)).await;
            }
            return Ok(TurnIterationFlow::Break);
        }

        let mut tool_round_state = TurnToolRoundStepController::run(TurnToolRoundStepContext {
            conversation: context.conversation,
            content: &content,
            tool_calls: &tool_calls,
            pre_executed,
            runtime: TurnRuntimeContext {
                tx: context.tx,
                trace: context.trace,
                route: context.route,
                resource_policy: context.resource_policy,
                task_stage: context.task_bundle.agent_state.stage,
                exposed_tool_names: &exposed_tool_names,
                working_dir: context.working_dir,
                last_user_preview: context.last_user_preview,
                required_validation_commands: context.required_validation_commands,
                destructive_scope: context.destructive_scope,
                baseline_git_status_files: context.baseline_git_status_files,
                retained_context: context.retained_context,
            },
            turn_state: &mut *context.turn_state,
            task_bundle: &mut *context.task_bundle,
            messages: &mut *context.messages,
            is_programming_workflow: crate::engine::code_change_workflow::is_programming_workflow(
                context.route.workflow,
            ),
            loop_state: &mut *context.loop_state,
        })
        .await;
        context
            .task_bundle
            .agent_state
            .observe_tool_round(AgentToolRoundObservation {
                any_tool_success: tool_round_state.any_tool_success,
                batch_has_unsuccessful_tools: tool_round_state.batch_has_unsuccessful_tools,
                used_write_tool: tool_round_state.used_write_tool,
                successful_write_tool: tool_round_state.successful_write_tool,
                has_worktree_changes: tool_round_state.has_worktree_changes(),
                has_successful_validation_commands: tool_round_state
                    .has_successful_validation_commands(),
                failed_tool_evidence_present: tool_round_state.failed_tool_evidence_present(),
            });
        record_stop_check(
            context.trace,
            context.task_bundle,
            context.turn_state,
            &tool_round_state,
            false,
        );

        if let Some(message) =
            duplicate_successful_read_only_closeout(&tool_round_state, context.last_user_preview)
        {
            context.loop_state.final_content.push_str(&message);
            if let Some(tx) = context.tx {
                let _ = tx.send(StreamEvent::TextChunk(message)).await;
            }
            return Ok(TurnIterationFlow::Break);
        }

        let focused_repair_flow =
            TurnFocusedRepairFlowController::run(TurnFocusedRepairFlowContext {
                conversation: context.conversation,
                workflow: context.route.workflow,
                no_diff_audit_closeout_allowed: context.no_diff_audit_closeout_allowed,
                code_write_tools_forbidden: context.code_write_tools_forbidden,
                trace: context.trace,
                code_workflow: &mut *context.code_workflow,
                turn_state: &mut *context.turn_state,
                round_state: &mut tool_round_state,
                exposed_tool_names: &exposed_tool_names,
                tx: context.tx,
                resource_policy: context.resource_policy,
                destructive_scope: context.destructive_scope,
                baseline_git_status_files: context.baseline_git_status_files,
                last_user_preview: context.last_user_preview,
                messages: &mut *context.messages,
                final_content: &mut context.loop_state.final_content,
                final_tool_calls: &mut context.loop_state.final_tool_calls,
            })
            .await;
        record_stop_check(
            context.trace,
            context.task_bundle,
            context.turn_state,
            &tool_round_state,
            matches!(
                focused_repair_flow,
                TurnFocusedRepairFlow::Continue | TurnFocusedRepairFlow::Stop
            ),
        );
        match focused_repair_flow {
            TurnFocusedRepairFlow::Continue => return Ok(TurnIterationFlow::Continue),
            TurnFocusedRepairFlow::Stop => return Ok(TurnIterationFlow::Break),
            TurnFocusedRepairFlow::Proceed => {}
        }

        match TurnToolFailureFollowupController::run(TurnToolFailureFollowupContext {
            provider: context.conversation.provider.as_ref(),
            model: context.conversation.model.clone(),
            session_store: context.conversation.session_store.as_ref(),
            session_id: &context.conversation.session_id,
            trace: context.trace,
            any_tool_success: tool_round_state.any_tool_success,
            last_user_preview: context.last_user_preview,
            task_bundle: &mut *context.task_bundle,
            round_state: &mut tool_round_state,
            failed_tool_names: &context.loop_state.failed_tool_names,
            tx: context.tx,
            final_content: &mut context.loop_state.final_content,
            messages: &mut *context.messages,
        })
        .await
        {
            TurnToolFailureFollowupFlow::Continue => {}
            TurnToolFailureFollowupFlow::Stop => return Ok(TurnIterationFlow::Break),
        }

        match TurnPostChangeCloseoutController::run(TurnPostChangeCloseoutContext {
            conversation: context.conversation,
            trace: context.trace,
            route: context.route,
            code_workflow: &mut *context.code_workflow,
            task_bundle: &mut *context.task_bundle,
            round_state: &mut tool_round_state,
            required_validation_commands: context.required_validation_commands,
            successful_required_validation_commands: &mut context
                .loop_state
                .successful_required_validation_commands,
            turn_state: &mut *context.turn_state,
            final_content: &mut context.loop_state.final_content,
            messages: &mut *context.messages,
            last_user_preview: context.last_user_preview,
        })
        .await
        {
            TurnPostChangeCloseoutFlow::Continue => Ok(TurnIterationFlow::Continue),
            TurnPostChangeCloseoutFlow::Break => Ok(TurnIterationFlow::Break),
        }
    }
}

fn record_stop_check(
    trace: &TraceCollector,
    task_bundle: &mut TaskContextBundle,
    turn_state: &TurnRuntimeState,
    tool_round_state: &super::turn_tool_round_outcome_controller::TurnToolRoundState,
    force_patch_synthesis_after_no_change: bool,
) {
    let decision = StopChecker::evaluate(StopCheckInput {
        any_tool_success: tool_round_state.any_tool_success,
        successful_write_tool: tool_round_state.successful_write_tool,
        has_successful_validation_commands: tool_round_state.has_successful_validation_commands(),
        no_code_progress_rounds: turn_state.focused_repair.no_code_progress_rounds,
        action_checkpoint_active: turn_state.focused_repair.action_checkpoint_active,
        action_checkpoint_no_change_rounds: turn_state
            .focused_repair
            .action_checkpoint_no_change_rounds,
        force_patch_synthesis_after_no_change,
        repeated_failed_tools: tool_round_state.repeated_failed_tools.len(),
        duplicate_read_only_tools: tool_round_state.duplicate_successful_read_only_tools.len(),
    });
    StopChecker::apply_to_task_state(&mut task_bundle.agent_state, &decision);
    trace.record(TraceEvent::StopCheckEvaluated {
        status: decision.status.label().to_string(),
        reason: decision.reason.label().to_string(),
        stage: format!("{:?}", task_bundle.agent_state.stage),
        no_code_progress_rounds: decision.no_code_progress_rounds,
        action_checkpoint_active: decision.action_checkpoint_active,
        summary: decision.summary,
    });
}

fn duplicate_successful_read_only_pre_execution_closeout(
    tool_calls: &[ToolCall],
    turn_state: &TurnRuntimeState,
    last_user_preview: &str,
    session_store: Option<&std::sync::Arc<crate::session_store::SessionStore>>,
    session_id: &str,
) -> Option<String> {
    if tool_calls.is_empty() {
        return None;
    }

    let mut duplicate_results = Vec::new();
    for tool_call in tool_calls {
        if !is_read_only(&tool_call.name) {
            return None;
        }
        let fingerprint = tool_call_fingerprint(tool_call);
        if !turn_state
            .successful_read_only_tool_fingerprints
            .contains_key(&fingerprint)
        {
            return None;
        }
        let cached = turn_state
            .successful_read_only_tool_results
            .get(&fingerprint)?
            .trim();
        if cached.is_empty() {
            return None;
        }
        let ledger_summary =
            duplicate_tool_call_ledger_summary(session_store, session_id, tool_call);
        if is_read_cache_notice(cached) && ledger_summary.is_none() {
            return None;
        }
        duplicate_results.push((tool_call.name.as_str(), cached, ledger_summary));
    }

    let (tool_name, result_text, ledger_summary) = duplicate_results.last()?;
    Some(synthesize_read_only_duplicate_answer(
        tool_name,
        result_text,
        last_user_preview,
        ledger_summary.as_deref(),
    ))
}

fn duplicate_successful_read_only_closeout(
    round_state: &super::turn_tool_round_outcome_controller::TurnToolRoundState,
    last_user_preview: &str,
) -> Option<String> {
    if round_state.duplicate_successful_read_only_tools.is_empty() {
        return None;
    }
    let duplicate = round_state.duplicate_successful_read_only_results.last()?;
    let result_text = duplicate.result_text.trim();
    if result_text.is_empty() {
        return None;
    }
    Some(synthesize_read_only_duplicate_answer(
        &duplicate.tool_name,
        result_text,
        last_user_preview,
        duplicate.ledger_summary.as_deref(),
    ))
}

fn synthesize_read_only_duplicate_answer(
    tool_name: &str,
    result_text: &str,
    last_user_preview: &str,
    ledger_summary: Option<&str>,
) -> String {
    let chinese = contains_cjk(last_user_preview);
    let summary = if is_read_cache_notice(result_text) {
        ledger_summary
            .map(|summary| ledger_reuse_answer(summary, chinese))
            .unwrap_or_else(|| summarize_read_only_result(result_text, chinese))
    } else {
        summarize_read_only_result(result_text, chinese)
    };
    let provenance = ledger_summary
        .filter(|summary| !summary.trim().is_empty())
        .map(|summary| {
            if chinese {
                format!("\n\n复用依据：{summary}")
            } else {
                format!("\n\nReuse basis: {summary}")
            }
        })
        .unwrap_or_default();
    if chinese {
        format!(
            "我已经读到需要的信息；模型重复请求 `{tool_name}` 时我已停止继续读取，下面直接根据已有结果回答。\n\n{summary}{provenance}"
        )
    } else {
        format!(
            "I already had the needed information, so I stopped the repeated `{tool_name}` read and answered from the existing tool output.\n\n{summary}{provenance}"
        )
    }
}

fn ledger_reuse_answer(ledger_summary: &str, chinese: bool) -> String {
    if chinese {
        format!(
            "这次重复读取被已有会话上下文接住了。{ledger_summary}。如果需要逐字内容，下一步应读取具体行范围，而不是重复全文读取。"
        )
    } else {
        format!(
            "The repeated read was handled from session context. {ledger_summary}. If exact text is needed, the next step should read a targeted line range instead of rereading the whole file."
        )
    }
}

fn duplicate_tool_call_ledger_summary(
    session_store: Option<&std::sync::Arc<crate::session_store::SessionStore>>,
    session_id: &str,
    tool_call: &ToolCall,
) -> Option<String> {
    let store = session_store?;
    match tool_call.name.as_str() {
        "file_read" => {
            let path = tool_call.arguments.get("path")?.as_str()?.trim();
            let events = store.recent_context_ledger_events(session_id, 20).ok()?;
            events.into_iter().find_map(|event| {
                let entry = crate::engine::context_ledger::file_read_entry_from_event(&event)?;
                let matches_path = entry.path == path
                    || entry.resolved_path == path
                    || entry.resolved_path.ends_with(path.trim_start_matches("~/"));
                matches_path.then(|| {
                    format!(
                        "ledger: file `{}` was read previously ({} displayed / {} total lines, hash {})",
                        if entry.path.is_empty() { entry.resolved_path } else { entry.path },
                        entry.displayed_lines,
                        entry.total_lines,
                        entry.content_hash
                    )
                })
            })
        }
        "bash" => {
            let command = tool_call.arguments.get("command")?.as_str()?.trim();
            let events = store.recent_context_ledger_events(session_id, 20).ok()?;
            events.into_iter().find_map(|event| {
                if event.kind != crate::engine::context_ledger::CONTEXT_LEDGER_BASH_READ_KIND {
                    return None;
                }
                let event_command = event.payload.get("command")?.as_str()?.trim();
                if event_command != command {
                    return None;
                }
                let exit_code = event
                    .payload
                    .get("exit_code")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0);
                Some(format!(
                    "ledger: read-only command `{command}` ran previously with exit {exit_code}"
                ))
            })
        }
        _ => None,
    }
}

fn summarize_read_only_result(result_text: &str, chinese: bool) -> String {
    let lines = normalized_result_lines(result_text);
    let title = lines
        .iter()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .or_else(|| {
            lines
                .iter()
                .find(|line| !line.starts_with('#') && !line.starts_with('['))
                .map(|line| line.as_str())
        })
        .unwrap_or("tool result");
    let description = lines
        .iter()
        .filter(|line| !line.starts_with('#'))
        .find(|line| {
            let line = line.trim();
            !line.is_empty()
                && !line.starts_with("- ")
                && !line.starts_with("* ")
                && !line.starts_with('`')
                && !line.starts_with('[')
        });
    let bullets: Vec<String> = lines
        .iter()
        .filter_map(|line| markdown_bullet_text(line))
        .take(5)
        .collect();

    if chinese {
        let mut answer = match description {
            Some(description) if description != title => {
                format!("根据 README，这是 **{title}**：{description}")
            }
            _ => format!("根据已读内容，这是 **{title}**。"),
        };
        if !bullets.is_empty() {
            answer.push_str("\n\n主要信息：");
            for bullet in bullets {
                answer.push_str("\n- ");
                answer.push_str(&bullet);
            }
        }
        return answer;
    }

    let mut answer = match description {
        Some(description) if description != title => {
            format!("Based on the README, this is **{title}**: {description}")
        }
        _ => format!("Based on the already-read content, this is **{title}**."),
    };
    if !bullets.is_empty() {
        answer.push_str("\n\nKey points:");
        for bullet in bullets {
            answer.push_str("\n- ");
            answer.push_str(&bullet);
        }
    }
    answer
}

fn normalized_result_lines(result_text: &str) -> Vec<String> {
    result_text
        .lines()
        .map(strip_file_read_line_prefix)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| {
            !(line.starts_with("[File unchanged since last read:")
                || line.starts_with("[stored read-only result truncated]")
                || line.starts_with('[') && line.contains("lines total"))
        })
        .map(ToString::to_string)
        .collect()
}

fn strip_file_read_line_prefix(line: &str) -> &str {
    let trimmed = line.trim_start();
    let mut digit_end = 0;
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

fn markdown_bullet_text(line: &str) -> Option<String> {
    line.strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToString::to_string)
}

fn contains_cjk(text: &str) -> bool {
    text.chars()
        .any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch))
}

fn is_read_cache_notice(text: &str) -> bool {
    text.trim_start()
        .starts_with("[File unchanged since last read:")
}

#[cfg(test)]
mod tests {
    use super::super::tool_batch_result_processor::DuplicateSuccessfulReadOnlyToolResult;
    use super::super::turn_loop_state_controller::TurnLoopStateController;
    use super::*;
    use crate::engine::destructive_scope::DestructiveScopeContract;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::trace::{TraceEvent, TurnStatus, TurnTrace};
    use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
    use crate::tools::ToolRegistry;
    use async_openai::types::ChatCompletionResponseStream;
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;
    use tokio::sync::Mutex;

    struct MockProvider {
        responses: StdMutex<VecDeque<anyhow::Result<ChatResponse>>>,
    }

    #[async_trait::async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("mock response")
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

    fn conversation(response: ChatResponse) -> ConversationLoop {
        ConversationLoop::new(
            Arc::new(MockProvider {
                responses: StdMutex::new(VecDeque::from(vec![Ok(response)])),
            }),
            Arc::new(ToolRegistry::new()),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "mock-model".to_string(),
        )
    }

    #[tokio::test]
    async fn plain_model_response_breaks_iteration() {
        let conversation = conversation(ChatResponse {
            content: "done".to_string(),
            tool_calls: None,
            usage: None,
        });
        let route = IntentRouter::new().route("hello");
        let resource_policy = ResourcePolicy::from_route(&route);
        let working_dir = std::env::current_dir().expect("current dir");
        let destructive_scope = DestructiveScopeContract::from_user_request("hello", &working_dir);
        let mut task_bundle = TaskContextBundle::new("hello", &working_dir, route.clone(), None);
        let mut code_workflow = CodeChangeWorkflowRunner::new(&task_bundle);
        let mut turn_state = TurnRuntimeState::new(true);
        let mut loop_state = TurnLoopStateController::initial_state();
        let mut messages = vec![Message::user("hello")];
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "hello"));
        let base_tools = Vec::new();
        let baseline_git_status_files = HashSet::new();
        let retained_context = crate::tools::ToolContextRetainedContext::default();

        let flow = TurnIterationController::run(TurnIterationContext {
            conversation: &conversation,
            iteration: 0,
            route: &route,
            code_workflow: &mut code_workflow,
            task_bundle: &mut task_bundle,
            turn_retrieval_context: None,
            retained_context: &retained_context,
            base_tools: &base_tools,
            loop_state: &mut loop_state,
            turn_state: &mut turn_state,
            no_diff_audit_closeout_allowed: false,
            code_write_tools_forbidden: false,
            resource_policy: &resource_policy,
            working_dir: &working_dir,
            last_user_preview: "hello",
            required_validation_commands: &[],
            destructive_scope: &destructive_scope,
            baseline_git_status_files: &baseline_git_status_files,
            messages: &mut messages,
            trace: &trace,
            tx: None,
        })
        .await
        .expect("iteration");

        assert!(matches!(flow, TurnIterationFlow::Break));
        assert_eq!(loop_state.final_content, "done");
        assert_eq!(turn_state.iterations_used, 1);
        assert!(!loop_state.tool_calls_made);
    }

    #[test]
    fn repeated_read_only_closeout_summarizes_existing_readme_result() {
        let round_state = super::super::turn_tool_round_outcome_controller::TurnToolRoundState {
            tool_results_text: String::new(),
            changed_files: Vec::new(),
            batch_has_unsuccessful_tools: false,
            used_write_tool: false,
            successful_write_tool: false,
            used_action_checkpoint_lookup: false,
            any_tool_success: true,
            repeated_failed_tools: Vec::new(),
            failed_tool_names_this_round: Vec::new(),
            failed_tool_evidence: Vec::new(),
            file_edit_failure_correction_added: false,
            successful_validation_commands: Vec::new(),
            duplicate_successful_read_only_tools: vec!["file_read".to_string()],
            duplicate_successful_read_only_results: vec![
                DuplicateSuccessfulReadOnlyToolResult {
                    tool_name: "file_read".to_string(),
                    result_text: "   1 | # PhageMatch - 噬菌体匹配平台\n   2 | 一个专注的噬菌体-耐药菌匹配平台。\n   3 | - 菌株上传\n   4 | - 噬菌体匹配"
                        .to_string(),
                    ledger_summary: Some(
                        "ledger: file `README.md` was read previously (4 displayed / 4 total lines, hash abc)"
                            .to_string(),
                    ),
                },
            ],
            should_closeout_after_verified_change: false,
        };

        let message = duplicate_successful_read_only_closeout(
            &round_state,
            "再帮我看一下桌面里面的phageGPT的文件夹是什么项目",
        )
        .expect("closeout");

        assert!(message.contains("停止继续读取"));
        assert!(message.contains("PhageMatch - 噬菌体匹配平台"));
        assert!(message.contains("菌株上传"));
        assert!(message.contains("噬菌体匹配"));
    }

    #[test]
    fn repeated_read_only_pre_execution_closeout_blocks_duplicate_tool_run() {
        let tool_call = ToolCall {
            id: "call_read_again".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({"path": "~/Desktop/phageGPT/README.md"}),
        };
        let fingerprint = tool_call_fingerprint(&tool_call);
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state
            .successful_read_only_tool_fingerprints
            .insert(fingerprint.clone(), 1);
        turn_state.successful_read_only_tool_results.insert(
            fingerprint,
            "   1 | # PhageMatch - 噬菌体匹配平台\n   2 | 一个专注的噬菌体-耐药菌匹配平台。"
                .to_string(),
        );

        let message = duplicate_successful_read_only_pre_execution_closeout(
            std::slice::from_ref(&tool_call),
            &turn_state,
            "再帮我看一下桌面里面的phageGPT的文件夹是什么项目",
            None,
            "session",
        )
        .expect("duplicate read should close out before execution");

        assert!(message.contains("停止继续读取"));
        assert!(message.contains("PhageMatch - 噬菌体匹配平台"));
    }

    #[test]
    fn repeated_read_only_pre_execution_closeout_allows_new_read() {
        let tool_call = ToolCall {
            id: "call_new_read".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({"path": "README.md"}),
        };
        let turn_state = TurnRuntimeState::new(true);

        assert!(duplicate_successful_read_only_pre_execution_closeout(
            std::slice::from_ref(&tool_call),
            &turn_state,
            "read README",
            None,
            "session",
        )
        .is_none());
    }

    #[test]
    fn repeated_read_only_pre_execution_closeout_uses_ledger_for_cache_notice() {
        let tool_call = ToolCall {
            id: "call_read_again".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({"path": "README.md"}),
        };
        let fingerprint = tool_call_fingerprint(&tool_call);
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state
            .successful_read_only_tool_fingerprints
            .insert(fingerprint.clone(), 1);
        turn_state.successful_read_only_tool_results.insert(
            fingerprint,
            "[File unchanged since last read: README.md] (2 lines)".to_string(),
        );
        let store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
        store.create_session("session", "Test", "model").unwrap();
        store
            .add_learning_event(
                "session",
                crate::engine::context_ledger::CONTEXT_LEDGER_FILE_READ_KIND,
                "file_read",
                "Read README.md",
                1.0,
                &serde_json::json!({
                    "path": "README.md",
                    "resolved_path": "/tmp/project/README.md",
                    "content_hash": "abc123",
                    "size_bytes": 10,
                    "total_lines": 2,
                    "displayed_lines": 2,
                    "line_start": 1,
                    "line_end": 2,
                    "targeted_read": false,
                    "truncated": false
                }),
            )
            .unwrap();

        let message = duplicate_successful_read_only_pre_execution_closeout(
            std::slice::from_ref(&tool_call),
            &turn_state,
            "read README",
            Some(&store),
            "session",
        )
        .expect("ledger should recover cache notice");

        assert!(message.contains("abc123"));
        assert!(message.contains("Reuse basis"));
    }

    #[test]
    fn stop_check_records_no_progress_in_task_state_and_trace() {
        let route = IntentRouter::new().route("fix src/main.rs");
        let mut task_bundle = TaskContextBundle::new("fix src/main.rs", ".", route, None);
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state.focused_repair.no_code_progress_rounds = 2;
        turn_state.focused_repair.action_checkpoint_active = true;
        let round_state = super::super::turn_tool_round_outcome_controller::TurnToolRoundState {
            tool_results_text: String::new(),
            changed_files: Vec::new(),
            batch_has_unsuccessful_tools: false,
            used_write_tool: false,
            successful_write_tool: false,
            used_action_checkpoint_lookup: false,
            any_tool_success: true,
            repeated_failed_tools: Vec::new(),
            failed_tool_names_this_round: Vec::new(),
            failed_tool_evidence: Vec::new(),
            file_edit_failure_correction_added: false,
            successful_validation_commands: Vec::new(),
            duplicate_successful_read_only_tools: Vec::new(),
            duplicate_successful_read_only_results: Vec::new(),
            should_closeout_after_verified_change: false,
        };
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "fix src/main.rs"));

        record_stop_check(&trace, &mut task_bundle, &turn_state, &round_state, false);

        let stop_check = task_bundle
            .agent_state
            .stop_checks
            .last()
            .expect("stop check");
        assert_eq!(
            stop_check.status,
            crate::engine::task_context::StopCheckStatus::Checkpoint
        );
        assert_eq!(
            stop_check.reason,
            crate::engine::task_context::StopCheckReason::NoProgress
        );
        assert_eq!(
            task_bundle.agent_state.stage,
            crate::engine::task_context::AgentTaskStage::Repair
        );

        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::StopCheckEvaluated {
                status,
                reason,
                no_code_progress_rounds: 2,
                action_checkpoint_active: true,
                ..
            } if status == "checkpoint" && reason == "no_progress"
        )));
    }
}
