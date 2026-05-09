use super::{ConversationLoop, RuntimeDietSnapshot};
use crate::engine::code_change_workflow::CodeChangeWorkflowRunner;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::ToolCall;
use tokio::sync::mpsc;

pub(super) struct FinalCloseoutContext<'a> {
    pub(super) trace: &'a TraceCollector,
    pub(super) code_workflow: &'a CodeChangeWorkflowRunner,
    pub(super) task_bundle: &'a TaskContextBundle,
    pub(super) runtime_diet: &'a mut RuntimeDietSnapshot,
    pub(super) final_content: &'a mut String,
    pub(super) final_tool_calls: &'a [ToolCall],
    pub(super) iterations_used: usize,
    pub(super) max_iterations: usize,
    pub(super) tx: Option<&'a mpsc::Sender<super::super::streaming::StreamEvent>>,
}

impl ConversationLoop {
    pub(super) async fn apply_final_closeout(context: FinalCloseoutContext<'_>) {
        if let Some(closeout) = context.code_workflow.build_closeout(context.task_bundle) {
            context.trace.record(TraceEvent::FinalCloseoutPrepared {
                status: closeout.status.label().to_string(),
                changed_files: closeout.changed_files.len(),
                validation_items: closeout.validation.len(),
                acceptance_items: closeout.acceptance.len(),
                residual_risks: closeout.residual_risks.len(),
            });
            context.runtime_diet.closeout_visibility =
                format!("{:?}", closeout.visibility_from_env()).to_ascii_lowercase();
            context.runtime_diet.validation_evidence = closeout.status.label().to_string();
            let closeout_text = closeout.format_for_user_response();
            if !closeout_text.is_empty() && !context.final_content.contains("Closeout:") {
                context.final_content.push_str(&closeout_text);
                if let Some(tx) = context.tx {
                    let _ = tx
                        .send(super::super::streaming::StreamEvent::TextChunk(
                            closeout_text,
                        ))
                        .await;
                }
            }
        }

        if context.iterations_used >= context.max_iterations
            && !context.final_tool_calls.is_empty()
            && !context.final_content.contains("Closeout:")
        {
            let stop_msg = "\n\n[Stopped after reaching the tool-iteration budget before a final closeout. Review the last tool results and continue if the task is not complete.]\n";
            context.final_content.push_str(stop_msg);
            if let Some(tx) = context.tx {
                let _ = tx
                    .send(super::super::streaming::StreamEvent::TextChunk(
                        stop_msg.to_string(),
                    ))
                    .await;
            }
            context.trace.record(TraceEvent::WorkflowFallback {
                error: "tool iteration budget exhausted before final closeout".to_string(),
            });
        }
    }
}
