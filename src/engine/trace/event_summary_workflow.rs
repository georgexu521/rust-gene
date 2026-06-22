//! Trace summarization support.
//!
//! Converts runtime event traces into compact summaries for diagnostics and learning loops.

use super::{preview, short_id, TraceEvent};

pub(super) fn workflow_summary(event: &TraceEvent) -> String {
    match event {
            TraceEvent::UserPromptSubmitted { chars } => format!("user prompt: {} chars", chars),
            TraceEvent::IntentRouted {
                agent_mode,
                intent,
                workflow,
                retrieval,
                confidence,
                risk,
                reason,
            } => {
                let mode = agent_mode
                    .as_deref()
                    .filter(|mode| !mode.is_empty())
                    .unwrap_or("auto");
                format!(
                    "mode={} intent={} workflow={} retrieval={} risk={} confidence={:.2}: {}",
                    mode,
                    intent,
                    workflow,
                    retrieval,
                    risk,
                    confidence,
                    preview(reason)
                )
            }
            TraceEvent::RouteCandidateEvaluated {
                intent,
                confidence,
                matched_signals,
                reason,
            } => format!(
                "route candidate intent={} confidence={:.2} signals={} reason={}",
                intent,
                confidence,
                if matched_signals.is_empty() {
                    "none".to_string()
                } else {
                    matched_signals.join(",")
                },
                preview(reason)
            ),
            TraceEvent::RouteCompetitionSummary {
                selected_intent,
                selected_confidence,
                runner_up_intent,
                runner_up_confidence,
                candidate_count,
                delta,
            } => format!(
                "route competition selected={}({:.2}) runner_up={}({:.2}) candidates={} delta={:.2}",
                selected_intent,
                selected_confidence,
                runner_up_intent,
                runner_up_confidence,
                candidate_count,
                delta
            ),
            TraceEvent::ContextTokenBreakdown {
                total_chars,
                system_chars,
                history_chars,
                tool_result_chars,
                dynamic_zone_chars,
                last_user_chars,
            } => format!(
                "context tokens chars total={} system={} history={} tool={} dynamic={} last_user={}",
                total_chars,
                system_chars,
                history_chars,
                tool_result_chars,
                dynamic_zone_chars,
                last_user_chars
            ),
            TraceEvent::ResourcePolicySelected {
                latency,
                target_ms,
                cost_ceiling_usd,
                reasoning,
                parallelism_limit,
                max_tool_calls,
                context_budget_tokens,
                allow_fallback_model,
                reason,
            } => format!(
                "resource policy: latency={} target={}ms cost<=${:.2} reasoning={} parallel={} tools={} ctx={} fallback={} ({})",
                latency,
                target_ms,
                cost_ceiling_usd,
                reasoning,
                parallelism_limit,
                max_tool_calls,
                context_budget_tokens,
                allow_fallback_model,
                preview(reason)
            ),
            TraceEvent::TaskContextBuilt {
                task_id,
                workflow,
                files,
                constraints,
                risks,
                acceptance_checks,
            } => format!(
                "task context {} workflow={} files={} constraints={} risks={} checks={}",
                short_id(task_id),
                workflow,
                files,
                constraints,
                risks,
                acceptance_checks
            ),
            TraceEvent::TaskContractMaterialized {
                task_id,
                task_type,
                model_profile,
                assumptions,
                scope_files,
                validation_commands,
                proof_required,
                risk,
            } => format!(
                "task contract {} type={} profile={} assumptions={} files={} validations={} proof_required={} risk={}",
                short_id(task_id),
                task_type,
                model_profile,
                assumptions,
                scope_files,
                validation_commands,
                proof_required,
                risk
            ),
            TraceEvent::ContextPackMaterialized {
                task_id,
                project_facts,
                memory_records,
                recent_observations,
                failure_summaries,
                estimated_tokens,
                max_tokens,
                overflow_items,
                fingerprint,
            } => format!(
                "context pack {} project_facts={} memory_records={} observations={} failures={} tokens~{}/{} overflow={} fp={}",
                short_id(task_id),
                project_facts,
                memory_records,
                recent_observations,
                failure_summaries,
                estimated_tokens,
                max_tokens,
                overflow_items,
                preview(fingerprint)
            ),
            TraceEvent::ImplementationIntentRecorded {
                task_id,
                workflow,
                target_files,
                validation_commands,
                risks,
                reason,
            } => format!(
                "implementation intent {} workflow={} targets={} validations={} risks={} reason={}",
                short_id(task_id),
                workflow,
                target_files,
                validation_commands.len(),
                risks,
                preview(reason)
            ),
            TraceEvent::WorkflowJudgmentCompleted {
                task_type,
                complexity,
                risk,
                plan_steps,
                acceptance_checks,
                questions,
                guided_reasoning,
            } => format!(
                "workflow judgment type={} complexity={} risk={} steps={} checks={} questions={} guided={}",
                preview(task_type),
                complexity,
                risk,
                plan_steps,
                acceptance_checks,
                questions,
                guided_reasoning
            ),
            TraceEvent::WorkflowPlanProgress {
                total_steps,
                completed_steps,
                active_step,
                top_priority,
                top_importance_score,
                top_weight_share,
                weight_source,
                reweighted,
            } => format!(
                "workflow plan {}/{} active={} priority={} importance={} share={} source={} reweighted={}",
                completed_steps,
                total_steps,
                active_step
                    .as_deref()
                    .map(preview)
                    .unwrap_or_else(|| "none".to_string()),
                top_priority.as_deref().unwrap_or("none"),
                top_importance_score
                    .map(|score| format!("{:.2}", score))
                    .unwrap_or_else(|| "none".to_string()),
                top_weight_share
                    .map(|share| format!("{:.2}", share))
                    .unwrap_or_else(|| "none".to_string()),
                weight_source.as_deref().unwrap_or("none"),
                reweighted
            ),
            TraceEvent::WorkflowLearningAdjusted {
                adjustments,
                before_top_step,
                after_top_step,
                reason,
            } => format!(
                "workflow learning adjusted count={} before={} after={} reason={}",
                adjustments,
                before_top_step
                    .as_deref()
                    .map(preview)
                    .unwrap_or_else(|| "none".to_string()),
                after_top_step
                    .as_deref()
                    .map(preview)
                    .unwrap_or_else(|| "none".to_string()),
                preview(reason)
            ),
            TraceEvent::StageValidationCompleted {
                step,
                status,
                changed_files,
                evidence_items,
            } => format!(
                "stage validation step={} status={} files={} evidence={}",
                step.as_deref()
                    .map(preview)
                    .unwrap_or_else(|| "none".to_string()),
                status,
                changed_files,
                evidence_items
            ),
            TraceEvent::ReflectionPassCompleted {
                pass_id,
                task_id,
                status,
                findings,
                unresolved,
            } => format!(
                "reflection {} task={} status={} findings={} unresolved={}",
                short_id(pass_id),
                short_id(task_id),
                status,
                findings,
                unresolved
            ),
            TraceEvent::SessionGoalUpdated {
                goal_id,
                title,
                status,
                reason,
            } => format!(
                "goal {} {}: {} ({})",
                short_id(goal_id),
                status,
                preview(title),
                preview(reason)
            ),
            TraceEvent::GoalDriftDetected {
                goal_id,
                tool,
                call_id,
                level,
                reason,
                suggested_action,
            } => format!(
                "{} {} drift={} goal={} reason={} suggested={}",
                tool,
                short_id(call_id),
                level,
                short_id(goal_id),
                preview(reason),
                suggested_action.as_deref().unwrap_or("none")
            ),
            TraceEvent::DestructiveScopeChecked {
                tool,
                call_id,
                operation,
                target,
                allowed,
                reason,
            } => format!(
                "{} {} destructive_scope op={} target={} allowed={} reason={}",
                tool,
                short_id(call_id),
                operation,
                target.as_deref().map(preview).unwrap_or_else(|| "none".to_string()),
                allowed,
                preview(reason)
            ),
            TraceEvent::WorkflowRouted { decision, reason } => {
                format!("workflow decision: {} ({})", decision, preview(reason))
            }
            TraceEvent::WorkflowCompleted { steps } => {
                format!("workflow completed: {} steps", steps)
            }
            TraceEvent::WorkflowFallback { error } => {
                format!("workflow fallback: {}", preview(error))
            }
            TraceEvent::AgentLoopStepEvaluated {
                route_workflow,
                route_risk,
                task_mode,
                stage_before,
                stage_after,
                mva_stage_before,
                mva_stage_after,
                stage_transition_policy,
                exposed_tools,
                selected_tool_calls,
                action_score_records,
                latest_action_score,
                observations_delta,
                key_findings_delta,
                stop_status,
                stop_reason,
                stop_action,
                terminal_status,
                state_delta,
            } => format!(
                "agent loop: mode={} workflow={} risk={} stage={}->{} mva_stage={}->{} transition={} tools_exposed={} calls={} scores={} latest_score={} obs_delta={} findings_delta={} stop={}/{}/{} terminal={} delta={} ",
                task_mode,
                route_workflow,
                route_risk,
                stage_before,
                stage_after,
                mva_stage_before,
                mva_stage_after,
                stage_transition_policy,
                exposed_tools,
                selected_tool_calls,
                action_score_records,
                latest_action_score
                    .map(|score| score.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                observations_delta,
                key_findings_delta,
                stop_status,
                stop_reason,
                stop_action,
                terminal_status.as_deref().unwrap_or("none"),
                preview(state_delta)
            ),
            TraceEvent::StopCheckEvaluated {
                status,
                reason,
                stage,
                terminal_status,
                action,
                no_code_progress_rounds,
                action_checkpoint_active,
                summary,
                evidence_items,
                failure_type,
                recovery_plan_id,
                rollback_recommended,
                next_action,
            } => format!(
                "stop check status={} reason={} terminal={} action={} stage={} no_progress={} checkpoint={} evidence={} failure={} recovery={} rollback={} next={} ({})",
                status,
                reason,
                terminal_status.as_deref().unwrap_or("none"),
                if action.is_empty() { "none" } else { action },
                stage,
                no_code_progress_rounds,
                action_checkpoint_active,
                evidence_items,
                failure_type.as_deref().unwrap_or("none"),
                recovery_plan_id.as_deref().unwrap_or("none"),
                rollback_recommended,
                next_action.as_deref().unwrap_or("none"),
                preview(summary)
            ),
            TraceEvent::WorkflowContractActivation {
                mode,
                phase,
                active,
                reason,
            } => format!(
                "workflow contract {} phase={} active={} reason={}",
                mode,
                phase,
                active,
                preview(reason)
            ),
            TraceEvent::RiskSignalAssessed {
                phase,
                level,
                entry_contract,
                reasons,
            } => format!(
                "risk signal phase={} level={} entry_contract={} reasons={}",
                phase,
                level,
                entry_contract,
                preview(&reasons.join("; "))
            ),
            TraceEvent::AdaptiveWorkflowTriggered {
                trigger,
                depth,
                require_workflow_judgment,
                require_stage_validation,
                max_repair_attempts,
                reason,
            } => format!(
                "workflow trigger={} depth={} judgment={} stage_validation={} repairs={} reason={}",
                trigger,
                depth,
                require_workflow_judgment,
                require_stage_validation,
                max_repair_attempts,
                preview(reason)
            ),
        _ => unreachable!("workflow trace summary called for non-workflow event"),
    }
}
