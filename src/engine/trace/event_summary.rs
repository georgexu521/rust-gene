use super::event_summary_workflow::workflow_summary;
use super::{
    compact_id_list, compact_label_list, preview, runtime_diet_level, short_id, TraceEvent,
};

impl TraceEvent {
    pub fn summary(&self) -> String {
        match self {
            TraceEvent::UserPromptSubmitted { .. }
            | TraceEvent::IntentRouted { .. }
            | TraceEvent::ResourcePolicySelected { .. }
            | TraceEvent::TaskContextBuilt { .. }
            | TraceEvent::TaskContractMaterialized { .. }
            | TraceEvent::ContextPackMaterialized { .. }
            | TraceEvent::ImplementationIntentRecorded { .. }
            | TraceEvent::WorkflowJudgmentCompleted { .. }
            | TraceEvent::WorkflowPlanProgress { .. }
            | TraceEvent::WorkflowLearningAdjusted { .. }
            | TraceEvent::StageValidationCompleted { .. }
            | TraceEvent::ReflectionPassCompleted { .. }
            | TraceEvent::SessionGoalUpdated { .. }
            | TraceEvent::GoalDriftDetected { .. }
            | TraceEvent::DestructiveScopeChecked { .. }
            | TraceEvent::WorkflowRouted { .. }
            | TraceEvent::WorkflowCompleted { .. }
            | TraceEvent::WorkflowFallback { .. }
            | TraceEvent::AgentLoopStepEvaluated { .. }
            | TraceEvent::StopCheckEvaluated { .. }
            | TraceEvent::WorkflowContractActivation { .. }
            | TraceEvent::RiskSignalAssessed { .. }
            | TraceEvent::AdaptiveWorkflowTriggered { .. } => workflow_summary(self),
            TraceEvent::MemorySnapshotInjected { chars } => {
                format!("pinned memory snapshot injected: {} chars", chars)
            }
            TraceEvent::MemoryPrefetch { chars } => format!("memory prefetch: {} chars", chars),
            TraceEvent::ActiveMemoryEvaluated {
                status,
                reason,
                items,
                timeout_ms,
                elapsed_ms,
            } => format!(
                "active memory: status={} items={} elapsed={}ms timeout={}ms reason={}",
                status, items, elapsed_ms, timeout_ms, reason
            ),
            TraceEvent::SelfEvolutionGuidanceInjected {
                records,
                chars,
                provenance,
            } => format!(
                "self-evolution guidance injected: records={} chars={} provenance={}",
                records,
                chars,
                if provenance.is_empty() {
                    "none".to_string()
                } else {
                    provenance
                        .iter()
                        .take(3)
                        .map(|item| preview(item))
                        .collect::<Vec<_>>()
                        .join(" | ")
                }
            ),
            TraceEvent::RetrievalContextBuilt {
                policy,
                sources,
                items,
                estimated_tokens,
                provenance,
                conflicts,
            } => {
                let provenance = if provenance.is_empty() {
                    "none".to_string()
                } else {
                    provenance
                        .iter()
                        .take(3)
                        .map(|item| preview(item))
                        .collect::<Vec<_>>()
                        .join(" | ")
                };
                format!(
                    "retrieval context: policy={} sources={} items={} tokens~{} conflicts={} provenance={}",
                    policy,
                    sources.join(","),
                    items,
                    estimated_tokens,
                    conflicts,
                    provenance
                )
            }
            TraceEvent::ContextZonesMaterialized {
                stable_prefix_tokens,
                task_state_tokens,
                relevant_material_tokens,
                recent_observation_tokens,
                current_decision_request_tokens,
                stable_prefix_fingerprint,
                task_state_fingerprint,
                relevant_material_fingerprint,
                recent_observation_fingerprint,
                current_decision_request_fingerprint,
                stable_prefix_budget_tokens,
                task_state_budget_tokens,
                relevant_material_budget_tokens,
                recent_observation_budget_tokens,
                current_decision_request_budget_tokens,
                stable_prefix_overflow,
                task_state_overflow,
                relevant_material_overflow,
                recent_observation_overflow,
                current_decision_request_overflow,
                task_state_empty,
                current_decision_request_empty,
                relevant_material_items,
                recent_observation_items,
                zone_envelope_messages,
                zone_source_messages,
                zone_duplicate_blocks_removed,
                zone_provenance_markers,
            } => format!(
                "context zones: stable={}t/{} task_state={}t/{} relevant={}t/{}/{} items observation={}t/{}/{} items decision={}t/{} empty_task={} empty_decision={} envelope={} source_msgs={} dedupe_removed={} provenance={} fp={}/{}/{}/{}/{} overflow={}/{}/{}/{}/{}",
                stable_prefix_tokens,
                stable_prefix_budget_tokens,
                task_state_tokens,
                task_state_budget_tokens,
                relevant_material_tokens,
                relevant_material_budget_tokens,
                relevant_material_items,
                recent_observation_tokens,
                recent_observation_budget_tokens,
                recent_observation_items,
                current_decision_request_tokens,
                current_decision_request_budget_tokens,
                task_state_empty,
                current_decision_request_empty,
                zone_envelope_messages,
                zone_source_messages,
                zone_duplicate_blocks_removed,
                zone_provenance_markers,
                preview(stable_prefix_fingerprint),
                preview(task_state_fingerprint),
                preview(relevant_material_fingerprint),
                preview(recent_observation_fingerprint),
                preview(current_decision_request_fingerprint),
                stable_prefix_overflow,
                task_state_overflow,
                relevant_material_overflow,
                recent_observation_overflow,
                current_decision_request_overflow
            ),
            TraceEvent::CacheStabilitySnapshot {
                stable_prefix_fingerprint,
                tool_schema_fingerprint,
                tool_schema_tokens,
                tool_count,
                dynamic_zone_messages,
                dynamic_zones_before_last_user,
                message_count,
            } => format!(
                "cache stability: stable_fp={} tool_fp={} tools={} tool_tokens={} dynamic_zones={} before_last_user={} messages={}",
                preview(stable_prefix_fingerprint),
                preview(tool_schema_fingerprint),
                tool_count,
                tool_schema_tokens,
                dynamic_zone_messages,
                dynamic_zones_before_last_user,
                message_count
            ),
            TraceEvent::PromptCacheUsageRecorded {
                model,
                prompt_tokens,
                cached_tokens,
                cache_miss_tokens,
                hit_rate,
            } => format!(
                "prompt cache usage: model={} prompt={} cached={} miss={} hit_rate={:.1}%",
                model,
                prompt_tokens,
                cached_tokens,
                cache_miss_tokens,
                hit_rate * 100.0
            ),
            TraceEvent::MemoryBoundaryEvaluated {
                read_status,
                stale_conflict_demotion_status,
                closeout_write_candidate_status,
                reason,
            } => format!(
                "memory boundary: read={} stale_conflict={} closeout_write={} ({})",
                read_status,
                stale_conflict_demotion_status,
                closeout_write_candidate_status,
                preview(reason)
            ),
            TraceEvent::MemoryProposalPrepared {
                task_id,
                status,
                candidates,
                candidate_kinds,
                evidence_items,
                write_policy,
                write_performed,
                reason,
            } => format!(
                "memory proposal {} status={} candidates={} kinds={} evidence={} write_policy={} wrote={} ({})",
                short_id(task_id),
                status,
                candidates,
                if candidate_kinds.is_empty() {
                    "none".to_string()
                } else {
                    candidate_kinds.join(",")
                },
                evidence_items,
                write_policy,
                write_performed,
                preview(reason)
            ),
            TraceEvent::CloseoutBackgroundStage {
                stage,
                status,
                duration_ms,
                timeout_ms,
                detail,
            } => format!(
                "closeout background: stage={} status={} duration={}ms timeout={}ms ({})",
                stage,
                status,
                duration_ms,
                timeout_ms,
                preview(detail)
            ),
            TraceEvent::MemorySynced { mode } => format!("memory synced: {}", mode),
            TraceEvent::ContextCompacted {
                before_tokens,
                after_tokens,
                strategy,
                trigger,
                token_pressure,
                boundary_id,
                sequence,
                messages_before,
                messages_after,
                preserved_tail_count,
                retained_items,
                provenance,
            } => format!(
                "context compacted: {} -> {} tokens ({}) trigger={} pressure={} boundary={} seq={} msgs={}->{} preserved={} retained={} provenance={}",
                before_tokens,
                after_tokens,
                strategy,
                trigger.as_deref().unwrap_or("unknown"),
                token_pressure.as_deref().unwrap_or("unknown"),
                boundary_id.as_deref().unwrap_or("none"),
                sequence
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                messages_before
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                messages_after
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                preserved_tail_count
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                if retained_items.is_empty() {
                    "none".to_string()
                } else {
                    retained_items.join(",")
                },
                if provenance.is_empty() {
                    "none".to_string()
                } else {
                    provenance.join(",")
                }
            ),
            TraceEvent::RuntimeDietReport {
                prompt_tokens,
                tool_schema_tokens,
                total_request_tokens,
                max_context_tokens,
                remaining_context_tokens,
                tool_result_chars,
                tool_result_tokens,
                truncated_tool_results,
                tool_result_artifacts,
                exposed_tools,
                memory_snapshot_chars,
                memory_snapshot_tokens,
                retrieval_items,
                retrieval_tokens,
                skill_list_chars,
                skill_list_tokens,
                route_scoped_tools,
                workflow_context,
                closeout_visibility,
                validation_evidence,
                warnings,
            } => {
                let total = if *total_request_tokens > 0 {
                    *total_request_tokens
                } else {
                    prompt_tokens.saturating_add(*tool_schema_tokens)
                };
                let level = runtime_diet_level(*prompt_tokens, *exposed_tools);
                let context_budget = match (remaining_context_tokens, max_context_tokens) {
                    (Some(remaining), Some(max)) => {
                        format!(" context_remaining={}/{}", remaining, max)
                    }
                    _ => String::new(),
                };
                format!(
                    "{} prompt={} tool_schema={} total={} tools={} tool_results={}ch/~{}t truncated={} artifacts={} pinned_memory={}ch/~{}t retrieval={}items/~{}t skills={}ch/~{}t route_scoped={} workflow={} closeout={} validation={} warnings={}{}",
                    level,
                    prompt_tokens,
                    tool_schema_tokens,
                    total,
                    exposed_tools,
                    tool_result_chars,
                    tool_result_tokens,
                    truncated_tool_results,
                    tool_result_artifacts,
                    memory_snapshot_chars,
                    memory_snapshot_tokens,
                    retrieval_items,
                    retrieval_tokens,
                    skill_list_chars,
                    skill_list_tokens,
                    route_scoped_tools,
                    workflow_context,
                    closeout_visibility,
                    validation_evidence,
                    compact_label_list(warnings),
                    context_budget
                )
            }
            TraceEvent::ApiRequestStarted {
                iteration,
                model,
                tools,
                provider_family,
                nonstreaming_tools_required,
                tool_result_adjacency_required,
            } => format!(
                "api request #{}: model={}, tools={}, provider={}, nonstreaming_tools={}, tool_adjacency={}",
                iteration,
                model,
                tools,
                provider_family.as_deref().unwrap_or("unknown"),
                nonstreaming_tools_required,
                tool_result_adjacency_required
            ),
            TraceEvent::ProviderMessageSequenceNormalized {
                provider_family,
                requires_tool_result_adjacency,
                requires_merged_system_messages,
                system_messages_merged,
                input_messages,
                output_messages,
                valid_tool_call_pairs,
                dropped_assistant_tool_calls,
                dropped_tool_results,
                valid_tool_call_ids,
                dropped_assistant_tool_call_ids,
                dropped_tool_result_ids,
            } => format!(
                "provider protocol normalized: provider={} messages={}->{} tool_pairs={} dropped_calls={} dropped_results={} merged_system={} adjacency={} merge_system={} valid_ids={} dropped_call_ids={} dropped_result_ids={}",
                provider_family,
                input_messages,
                output_messages,
                valid_tool_call_pairs,
                dropped_assistant_tool_calls,
                dropped_tool_results,
                system_messages_merged,
                requires_tool_result_adjacency,
                requires_merged_system_messages,
                compact_id_list(valid_tool_call_ids),
                compact_id_list(dropped_assistant_tool_call_ids),
                compact_id_list(dropped_tool_result_ids)
            ),
            TraceEvent::ProviderToolCallRepairApplied {
                provider_family,
                schema_flattened_tools,
                schema_flattened_fields,
                scavenged_tool_calls,
                argument_repairs,
                unflattened_arguments,
                dropped_duplicate_calls,
                malformed_tool_calls,
                warnings,
            } => format!(
                "provider tool repair: provider={} flattened_tools={} flattened_fields={} scavenged={} argument_repairs={} unflattened={} dropped_duplicates={} malformed={} warnings={}",
                provider_family,
                schema_flattened_tools,
                schema_flattened_fields,
                scavenged_tool_calls,
                argument_repairs,
                unflattened_arguments,
                dropped_duplicate_calls,
                malformed_tool_calls,
                compact_label_list(warnings)
            ),
            TraceEvent::StreamingToolExecutionShadow {
                mode,
                provider_family,
                provider_supports_streaming_tool_calls,
                streamed_request_path,
                observed_tool_calls,
                read_only_tool_calls,
                concurrency_safe_tool_calls,
                eligible_tool_calls,
                schema_complete_tool_calls,
                latency_upper_bound_ms,
                reason,
            } => format!(
                "streaming tool shadow: mode={} provider={} supports_streaming_tools={} streamed_path={} calls={} read_only={} concurrency_safe={} schema_complete={} eligible={} latency_upper_bound={}ms reason={}",
                mode,
                provider_family,
                provider_supports_streaming_tool_calls,
                streamed_request_path,
                observed_tool_calls,
                read_only_tool_calls,
                concurrency_safe_tool_calls,
                schema_complete_tool_calls,
                eligible_tool_calls,
                latency_upper_bound_ms,
                preview(reason)
            ),
            TraceEvent::ApiRequestCompleted {
                iteration,
                tool_calls,
                content_chars,
            } => format!(
                "api response #{}: {} tool calls, {} chars",
                iteration, tool_calls, content_chars
            ),
            TraceEvent::ProviderRequestStarted {
                provider_family,
                model,
                request_shape,
                timeout_secs,
                slow_warning_threshold_secs,
                message_count,
                tool_count,
                is_known_slow_path,
            } => format!(
                "provider request started: family={} model={} shape={} timeout={}s slow_warning={}s msgs={} tools={} slow_path={}",
                provider_family, model, request_shape, timeout_secs, slow_warning_threshold_secs, message_count, tool_count, is_known_slow_path
            ),
            TraceEvent::ProviderRequestRetrying {
                provider_family,
                model,
                request_shape,
                attempt,
                max_attempts,
                delay_ms,
                elapsed_ms,
                error_preview,
            } => format!(
                "provider request retrying: family={} model={} shape={} attempt={}/{} delay={}ms elapsed={}ms error={}",
                provider_family, model, request_shape, attempt, max_attempts, delay_ms, elapsed_ms, preview(error_preview)
            ),
            TraceEvent::ProviderRequestSlowWarning {
                provider_family,
                model,
                request_shape,
                elapsed_ms,
                timeout_ms,
                message,
            } => format!(
                "provider slow warning: family={} model={} shape={} elapsed={}ms timeout={}ms: {}",
                provider_family, model, request_shape, elapsed_ms, timeout_ms, message
            ),
            TraceEvent::ProviderRequestCompleted {
                provider_family,
                model,
                request_shape,
                elapsed_ms,
                success,
            } => format!(
                "provider request {}: family={} model={} shape={} elapsed={}ms",
                if *success { "completed" } else { "failed" },
                provider_family,
                model,
                request_shape,
                elapsed_ms
            ),
            TraceEvent::ProviderRequestTimeout {
                provider_family,
                model,
                request_shape,
                elapsed_ms,
                timeout_ms,
            } => format!(
                "provider request timeout: family={} model={} shape={} elapsed={}ms timeout={}ms",
                provider_family, model, request_shape, elapsed_ms, timeout_ms
            ),
            TraceEvent::ProviderRequestCancelled {
                provider_family,
                model,
                request_shape,
                elapsed_ms,
            } => format!(
                "provider request cancelled: family={} model={} shape={} elapsed={}ms",
                provider_family, model, request_shape, elapsed_ms
            ),
            TraceEvent::ToolStarted {
                tool,
                call_id,
                parallel,
                pre_executed,
            } => format!(
                "{} {} started{}{}",
                tool,
                short_id(call_id),
                if *parallel { " in parallel" } else { "" },
                if *pre_executed { " (pre-executed)" } else { "" }
            ),
            TraceEvent::ActionDecisionEvaluated {
                tool,
                call_id,
                stage,
                value,
                risk,
                uncertainty_reduction,
                cost,
                reversibility,
                scope_fit,
                action_score,
                formula_stage,
                formula_version,
                phase_aligned,
                mutates_workspace,
                broad_shell,
                modifiers,
                requires_confirmation,
                reason,
            } => format!(
                "{} {} action decision: stage={} formula={}/{} score={} value={} risk={} uncertainty={} cost={} reversible={} scope_fit={} aligned={} mutates={} broad_shell={} modifiers={} confirm={} ({})",
                tool,
                short_id(call_id),
                stage,
                formula_stage,
                formula_version,
                action_score,
                value,
                risk,
                uncertainty_reduction,
                cost,
                reversibility,
                scope_fit,
                phase_aligned,
                mutates_workspace,
                broad_shell,
                modifiers.len(),
                requires_confirmation,
                preview(reason)
            ),
            TraceEvent::CandidateActionsEvaluated {
                mode,
                candidate_count,
                selected_id,
                selected_tool,
                selected_score,
                selected_runtime_score,
                selected_model_score,
                runtime_model_score_delta,
                runtime_selected_differs_from_model_order,
                calibration_reason,
                selected_factor_score,
                model_factor_coverage,
                memory_evidence_items,
                selected_factor_rationale,
                rejected,
                reason,
            } => format!(
                "candidate actions: mode={} count={} selected={} tool={} score={} runtime_score={} model_score={} factor_score={} factor_coverage={} memory_evidence={} delta={} differs={} rejected={} rationale={} calibration={} ({})",
                mode,
                candidate_count,
                selected_id.as_deref().unwrap_or("none"),
                selected_tool.as_deref().unwrap_or("none"),
                selected_score
                    .map(|score| score.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                selected_runtime_score
                    .map(|score| score.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                selected_model_score
                    .map(|score| score.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                selected_factor_score
                    .map(|score| score.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                model_factor_coverage,
                memory_evidence_items,
                runtime_model_score_delta
                    .map(|score| score.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                runtime_selected_differs_from_model_order,
                rejected,
                preview(selected_factor_rationale.as_deref().unwrap_or("none")),
                preview(calibration_reason),
                preview(reason)
            ),
            TraceEvent::ActionReviewed {
                tool,
                call_id,
                decision,
                reason,
                permission,
                scope_allowed,
                budget_allowed,
                checkpoint,
                network,
                external_effect,
                recovery,
            } => format!(
                "{} {} action review: decision={} reason={} permission={} scope_allowed={} budget_allowed={} checkpoint={} network={} external_effect={} recovery={}",
                tool,
                short_id(call_id),
                decision,
                reason,
                permission.as_deref().unwrap_or("none"),
                scope_allowed,
                budget_allowed,
                checkpoint,
                network,
                external_effect,
                preview(recovery)
            ),
            TraceEvent::MemoryRecallScored {
                item_count,
                injected,
                available,
                omitted,
                conflict_capped,
                top_score,
                budget_exhausted,
                policy,
            } => format!(
                "memory recall scored: items={} injected={} available={} omitted={} conflict_capped={} top_score={:.2} budget_exhausted={} policy={}",
                item_count, injected, available, omitted, conflict_capped, top_score, budget_exhausted, policy
            ),
            TraceEvent::MemoryWriteScored {
                candidate_id,
                kind,
                status,
                score,
                threshold,
                explicit,
                duplication,
                reason,
            } => format!(
                "memory write scored: id={} kind={} status={} score={:.2} threshold={:.2} explicit={} duplication={:.2} reason={}",
                short_id(&candidate_id), kind, status, score, threshold, explicit, duplication, reason
            ),
            TraceEvent::MemoryKeepScored {
                record_id,
                kind,
                action,
                score,
                contradiction_risk,
                redundancy,
                reason,
            } => format!(
                "memory keep scored: id={} kind={} action={} score={:.2} contradiction={:.2} redundancy={:.2} reason={}",
                short_id(&record_id), kind, action, score, contradiction_risk, redundancy, reason
            ),
            TraceEvent::PermissionRequested {
                tool,
                call_id,
                prompt,
                review,
            } => {
                let mut summary = format!(
                    "{} {} requested permission: {}",
                    tool,
                    short_id(call_id),
                    preview(prompt)
                );
                if let Some(review) = review {
                    summary.push_str(&format!(
                        " review={:?}/{}",
                        review.kind,
                        review.risk.as_str()
                    ));
                    if !review.risk_facts.is_empty() {
                        summary.push_str(&format!(" facts={}", review.risk_facts.join(",")));
                    }
                }
                summary
            }
            TraceEvent::PermissionResolved {
                tool,
                call_id,
                approved,
                source,
                decision,
                persistence_scope,
                rule_pattern,
                persisted_path,
                review,
            } => {
                let mut summary = format!(
                    "{} {} permission {}",
                    tool,
                    short_id(call_id),
                    if *approved { "approved" } else { "denied" }
                );
                if let Some(source) = source {
                    summary.push_str(&format!(" source={}", source));
                }
                if let Some(decision) = decision {
                    summary.push_str(&format!(" decision={}", decision));
                }
                if let Some(scope) = persistence_scope {
                    summary.push_str(&format!(" scope={}", scope));
                }
                if let Some(pattern) = rule_pattern {
                    summary.push_str(&format!(" rule={}", pattern));
                }
                if let Some(path) = persisted_path {
                    summary.push_str(&format!(" saved={}", path));
                }
                if let Some(review) = review {
                    if let Some(user_decision) = review.user_decision.as_deref() {
                        summary.push_str(&format!(" user_decision={}", user_decision));
                    }
                    if let Some(hint) = review.recovery_hint.as_deref() {
                        summary.push_str(&format!(" recovery={}", preview(hint)));
                    }
                }
                summary
            }
            TraceEvent::ToolCompleted {
                tool,
                call_id,
                success,
                duration_ms,
                output_chars,
            } => format!(
                "{} {} {} in {}ms ({} chars)",
                tool,
                short_id(call_id),
                if *success { "ok" } else { "failed" },
                duration_ms.unwrap_or_default(),
                output_chars
            ),
            TraceEvent::ToolObservationRecorded {
                tool,
                call_id,
                status,
                result_kind,
                model_visibility,
                include_in_next_context,
                store_in_state,
                key_findings,
                evidence_items,
                failure_type,
                recovery_plan_id,
                recovery_kind,
                raw_result_ref,
                quality_warnings,
                quality_warning_labels,
                files_read,
                files_changed,
                checkpoint_id,
                summary,
            } => format!(
                "{} {} observation: kind={} status={} visibility={} context={} state={} findings={} evidence={} warnings={} warning_labels={} failure={} recovery_plan={} recovery_kind={} files_read={} files_changed={} checkpoint={} raw={} ({})",
                tool,
                short_id(call_id),
                if result_kind.is_empty() {
                    "generic"
                } else {
                    result_kind.as_str()
                },
                status,
                if model_visibility.is_empty() {
                    "unknown"
                } else {
                    model_visibility.as_str()
                },
                include_in_next_context,
                store_in_state,
                key_findings,
                evidence_items,
                quality_warnings,
                compact_label_list(quality_warning_labels),
                failure_type.as_deref().unwrap_or("none"),
                recovery_plan_id.as_deref().unwrap_or("none"),
                recovery_kind.as_deref().unwrap_or("none"),
                files_read,
                files_changed,
                checkpoint_id.as_deref().unwrap_or("none"),
                raw_result_ref.as_deref().unwrap_or("none"),
                preview(summary)
            ),
            TraceEvent::HookCompleted {
                event,
                provider,
                hook_name,
                call_id,
                tool,
                success,
                blocked,
                duration_ms,
                error,
                output_preview,
            } => {
                let detail = error
                    .as_deref()
                    .or(output_preview.as_deref())
                    .map(preview)
                    .unwrap_or_else(|| "no output".to_string());
                format!(
                    "{} hook '{}' provider={} for {} {}{} in {}ms: {}",
                    event,
                    hook_name,
                    provider,
                    tool.as_deref().unwrap_or(call_id),
                    if *success { "ok" } else { "failed" },
                    if *blocked { " blocked" } else { "" },
                    duration_ms,
                    detail
                )
            }
            TraceEvent::SubagentStarted {
                agent_id,
                profile,
                role,
                description,
                timeout_secs,
                allowed_tools,
            } => format!(
                "subagent {} started role={} profile={} timeout={}s tools={} task={}",
                agent_id,
                role,
                profile.as_deref().unwrap_or("none"),
                timeout_secs,
                allowed_tools,
                preview(description)
            ),
            TraceEvent::SubagentCompleted {
                agent_id,
                status,
                duration_ms,
                output_chars,
                tools_used,
            } => format!(
                "subagent {} {} in {}ms ({} chars, {} tools)",
                agent_id, status, duration_ms, output_chars, tools_used
            ),
            TraceEvent::VerificationCompleted {
                changed_files,
                passed,
                check_passed,
                tests_passed,
                review_passed,
                failed_commands,
            } => format!(
                "verification {} for {} changed files (check={} tests={} review={} failed={})",
                if *passed { "passed" } else { "failed" },
                changed_files,
                check_passed,
                tests_passed,
                review_passed,
                failed_commands.len()
            ),
            TraceEvent::RequiredValidationHeartbeat {
                command_preview,
                elapsed_secs,
                timeout_secs,
            } => format!(
                "required validation still running: elapsed={}s timeout={} command={}",
                elapsed_secs,
                timeout_secs
                    .map(|secs| format!("{secs}s"))
                    .unwrap_or_else(|| "unlimited".to_string()),
                preview(command_preview)
            ),
            TraceEvent::AcceptanceReviewCompleted {
                accepted,
                confidence,
                criteria,
                unresolved,
                next_action,
            } => format!(
                "acceptance accepted={} confidence={} criteria={} unresolved={} next={}",
                accepted, confidence, criteria, unresolved, next_action
            ),
            TraceEvent::GuidedDebuggingCompleted {
                blocker,
                next_action,
                causes,
                evidence_items,
                ask_user,
            } => format!(
                "guided debug blocker={} next={} causes={} evidence={} ask_user={}",
                blocker, next_action, causes, evidence_items, ask_user
            ),
            TraceEvent::RecoveryApplied { error, action } => {
                format!("recovery: {} -> {}", preview(error), action)
            }
            TraceEvent::RecoveryPlan {
                plan_id,
                source,
                category,
                failure_type,
                recovery_kind,
                action,
                retryable,
                safe_retry,
                allowed_alternatives,
                retry_budget,
                side_effect_uncertain,
                requires_user_decision,
                suggested_command,
                status,
            } => format!(
                "{} {} {} failure_type={} recovery_kind={} action={} retryable={} safe_retry={} alternatives={} retry_budget={} side_effect_uncertain={} requires_user={} suggested={} status={}",
                source,
                short_id(plan_id),
                category,
                if failure_type.is_empty() { "none" } else { failure_type },
                if recovery_kind.is_empty() { "none" } else { recovery_kind },
                preview(action),
                retryable,
                safe_retry,
                allowed_alternatives.len(),
                retry_budget
                    .map(|budget| budget.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                side_effect_uncertain,
                requires_user_decision,
                suggested_command.as_deref().unwrap_or("none"),
                status
            ),
            TraceEvent::McpResourceAccessed {
                server,
                uri,
                action,
                success,
                content_chars,
            } => format!(
                "{} resource {} on {} success={} ({} chars)",
                action,
                preview(uri),
                server,
                success,
                content_chars
            ),
            TraceEvent::RemoteBridgeAction {
                tool,
                call_id,
                action,
                target,
                risk,
                permission_hint,
                success,
                error_code,
            } => format!(
                "{} {} remote action={} target={} risk={} success={} error={} hint={}",
                tool,
                short_id(call_id),
                action,
                target.as_deref().unwrap_or("none"),
                risk,
                success,
                error_code.as_deref().unwrap_or("none"),
                preview(permission_hint)
            ),
            TraceEvent::AssistantResponded { chars, iterations } => {
                format!(
                    "assistant responded: {} chars, {} iterations",
                    chars, iterations
                )
            }
            TraceEvent::CompletionContractEvaluated {
                mode,
                workflow,
                status,
                terminal_status,
                requires_validation,
                verification_status,
                verification_proof_status,
                changed_files,
                reason,
            } => format!(
                "completion contract: mode={} workflow={} status={} terminal={} validation_required={} verification={} proof={} changed_files={} ({})",
                mode,
                workflow,
                status,
                terminal_status,
                requires_validation,
                verification_status,
                verification_proof_status,
                changed_files,
                preview(reason)
            ),
            TraceEvent::FinalCloseoutPrepared {
                status,
                terminal_status,
                stop_reason,
                stop_action,
                failure_type,
                recovery_plan_id,
                rollback_status,
                changed_files,
                validation_items,
                tool_records,
                tool_evidence,
                verification_proof_status,
                verification_proof_summary,
                verification_proof_kind_summary,
                verification_proof_support_status,
                verification_proof_support_summary,
                verification_proof_supports_verified,
                verification_proof_residual_risk,
                acceptance_items,
                residual_risks,
            } => format!(
                "final closeout status={} terminal={} stop_reason={} stop_action={} failure={} recovery={} rollback={} files={} validation={} tool_records={} tool_evidence={} proof={} proof_summary={} proof_kinds={} proof_support={} proof_supports_verified={} proof_residual_risk={} proof_support_summary={} acceptance={} risks={}",
                status,
                terminal_status.as_deref().unwrap_or("none"),
                stop_reason.as_deref().unwrap_or("none"),
                stop_action.as_deref().unwrap_or("none"),
                failure_type.as_deref().unwrap_or("none"),
                recovery_plan_id.as_deref().unwrap_or("none"),
                rollback_status.as_deref().unwrap_or("none"),
                changed_files,
                validation_items,
                tool_records,
                tool_evidence.as_deref().unwrap_or("none"),
                verification_proof_status.as_deref().unwrap_or("none"),
                verification_proof_summary.as_deref().unwrap_or("none"),
                verification_proof_kind_summary.as_deref().unwrap_or("none"),
                verification_proof_support_status.as_deref().unwrap_or("none"),
                verification_proof_supports_verified
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                verification_proof_residual_risk
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                verification_proof_support_summary.as_deref().unwrap_or("none"),
                acceptance_items,
                residual_risks
            ),
            TraceEvent::ExecutionReportPrepared {
                task_id,
                status,
                changed_files,
                validation_evidence,
                risks,
                next_steps,
            } => format!(
                "execution report {} status={} files={} validation={} risks={} next_steps={}",
                short_id(task_id),
                status,
                changed_files,
                validation_evidence,
                risks,
                next_steps
            ),
            TraceEvent::Error { message } => format!("error: {}", preview(message)),
        }
    }
}
