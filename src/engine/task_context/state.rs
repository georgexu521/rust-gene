use super::*;

impl AgentTaskState {
    pub fn from_initial_context(
        prompt: &str,
        working_dir: &Path,
        route: &IntentRoute,
        goal: Option<&SessionGoal>,
    ) -> Self {
        let mode_score = TaskModeScore::from_route(prompt, route);
        let lightweight_plan = LightweightPlanner::plan(prompt, route, &mode_score);
        let main_goal = goal
            .map(|goal| goal.title.clone())
            .unwrap_or_else(|| preview(prompt, 160));
        let mut allowed_scope = vec![format!("working_dir: {}", working_dir.display())];
        if let Some(goal) = goal {
            allowed_scope.push(format!("goal: {}", goal.title));
        }

        Self {
            main_goal: if main_goal.trim().is_empty() {
                "current user request".to_string()
            } else {
                main_goal
            },
            mode: mode_score.mode,
            mode_score,
            lightweight_plan,
            stage: AgentTaskStage::initial_for(route),
            allowed_scope,
            forbidden_actions: default_forbidden_actions(route),
            completed_steps: Vec::new(),
            observations: Vec::new(),
            key_findings: Vec::new(),
            hypotheses: Vec::new(),
            candidate_focus: Vec::new(),
            edit_snapshots: Vec::new(),
            active_files: Vec::new(),
            risks: Vec::new(),
            verification_plan: VerificationPlan {
                required_checks: Vec::new(),
                status: VerificationStatus::initial_for(route),
            },
            done_condition: DoneCondition {
                summary: done_condition_for(route),
                satisfied: false,
            },
            stop_checks: Vec::new(),
            terminal_status: None,
            uncertainty_not_reduced_steps: 0,
            consecutive_validation_failures: 0,
            consecutive_edit_failures: 0,
            consecutive_command_failures: 0,
            consecutive_permission_blocks: 0,
            last_failure_family: None,
            last_progress_signal: None,
            rollback_candidates: Vec::new(),
            failed_strategies: Vec::new(),
            action_score_history: Vec::new(),
            stage_transitions: Vec::new(),
        }
    }

    pub fn add_active_file(&mut self, path: impl Into<PathBuf>) {
        let path = path.into();
        if !self.active_files.contains(&path) {
            self.active_files.push(path);
        }
    }

    pub fn add_risk(&mut self, risk: impl Into<String>) {
        push_unique(&mut self.risks, risk.into());
    }

    pub fn add_required_check(&mut self, check: impl Into<String>) {
        push_unique(&mut self.verification_plan.required_checks, check.into());
        if self.verification_plan.status == VerificationStatus::NotRequired {
            self.verification_plan.status = VerificationStatus::Pending;
        }
    }

    pub fn record_observation(&mut self, source: impl Into<String>, summary: impl Into<String>) {
        let source = source.into();
        let summary = summary.into();
        if summary.trim().is_empty() {
            return;
        }
        if self
            .observations
            .iter()
            .any(|item| item.source == source && item.summary == summary)
        {
            return;
        }
        self.observations
            .push(ObservationSummary { source, summary });
        trim_front(&mut self.observations, MAX_OBSERVATIONS);
    }

    pub fn record_key_finding(
        &mut self,
        source: impl Into<String>,
        summary: impl Into<String>,
        evidence: Vec<String>,
    ) {
        let source = source.into();
        let summary = summary.into();
        if summary.trim().is_empty() {
            return;
        }
        if self
            .key_findings
            .iter()
            .any(|item| item.source == source && item.summary == summary)
        {
            return;
        }
        self.key_findings.push(TaskFinding {
            source,
            summary,
            evidence: evidence.into_iter().take(3).collect(),
        });
        trim_front(&mut self.key_findings, MAX_KEY_FINDINGS);
    }

    pub fn record_hypothesis(
        &mut self,
        hypothesis: impl Into<String>,
        confidence: u8,
        evidence: Vec<String>,
    ) {
        let hypothesis = hypothesis.into();
        if hypothesis.trim().is_empty() {
            return;
        }
        if let Some(existing) = self
            .hypotheses
            .iter_mut()
            .find(|item| item.hypothesis == hypothesis)
        {
            existing.confidence = existing.confidence.max(confidence.min(100));
            for item in evidence.into_iter().take(3) {
                push_unique(&mut existing.evidence, item);
            }
            trim_front(&mut existing.evidence, 5);
            return;
        }
        self.hypotheses.push(TaskHypothesis {
            hypothesis,
            confidence: confidence.min(100),
            evidence: evidence.into_iter().take(3).collect(),
        });
        trim_front(&mut self.hypotheses, MAX_HYPOTHESES);
    }

    pub fn record_candidate_focus(
        &mut self,
        target: impl Into<String>,
        reason: impl Into<String>,
        confidence: u8,
    ) {
        let target = target.into();
        let reason = reason.into();
        if target.trim().is_empty() {
            return;
        }
        if let Some(existing) = self
            .candidate_focus
            .iter_mut()
            .find(|item| item.target == target)
        {
            existing.confidence = existing.confidence.max(confidence.min(100));
            if !reason.trim().is_empty() {
                existing.reason = reason;
            }
            return;
        }
        self.candidate_focus.push(TaskFocus {
            target,
            reason,
            confidence: confidence.min(100),
        });
        trim_front(&mut self.candidate_focus, MAX_CANDIDATE_FOCUS);
    }

    pub fn record_completed_step(&mut self, stage: AgentTaskStage, summary: impl Into<String>) {
        let summary = summary.into();
        if summary.trim().is_empty() {
            return;
        }
        if self
            .completed_steps
            .iter()
            .any(|item| item.stage == stage && item.summary == summary)
        {
            return;
        }
        self.completed_steps.push(CompletedStep { stage, summary });
        trim_front(&mut self.completed_steps, MAX_COMPLETED_STEPS);
    }

    pub fn record_edit_snapshot(&mut self, label: impl Into<String>) {
        let label = label.into();
        if label.trim().is_empty() {
            return;
        }
        let snapshot = EditStateSnapshot {
            label,
            stage: self.stage,
            active_files: self.active_files.clone(),
            verification_status: self.verification_plan.status,
            recent_step: self.completed_steps.last().map(|step| step.summary.clone()),
            recent_observation: self
                .observations
                .last()
                .map(|observation| observation.summary.clone()),
        };
        if self.edit_snapshots.last() == Some(&snapshot) {
            return;
        }
        self.edit_snapshots.push(snapshot);
        trim_front(&mut self.edit_snapshots, MAX_EDIT_SNAPSHOTS);
    }

    pub fn observe_tool_context_evidence(
        &mut self,
        tool_call: &ToolCall,
        result: &ToolResult,
    ) -> usize {
        let mut observed = 0;
        for entry in tool_context_evidence_entries(tool_call, result) {
            self.observe_context_ledger_entry(entry);
            observed += 1;
        }
        observed
    }

    pub fn set_stage(&mut self, stage: AgentTaskStage) {
        self.transition_to_stage(stage, "manual", "stage set by runtime caller", 0);
    }

    pub fn mark_done(&mut self, summary: impl Into<String>) {
        self.transition_to_stage(
            AgentTaskStage::Done,
            "done_condition",
            "task marked done",
            1,
        );
        self.done_condition.summary = summary.into();
        self.done_condition.satisfied = true;
    }

    pub fn transition_to_stage(
        &mut self,
        next: AgentTaskStage,
        source: impl Into<String>,
        reason: impl Into<String>,
        evidence_items: usize,
    ) {
        if self.stage == next {
            return;
        }
        let previous = self.stage;
        self.stage = next;
        self.stage_transitions.push(TaskStageTransition {
            from: previous,
            to: next,
            mva_from: previous.mva_stage_label().to_string(),
            mva_to: next.mva_stage_label().to_string(),
            policy: mva_stage_transition_policy(previous, next).to_string(),
            source: source.into(),
            reason: reason.into(),
            evidence_items,
        });
        trim_front(&mut self.stage_transitions, MAX_STAGE_TRANSITIONS);
    }

    pub fn record_stop_check(&mut self, record: StopCheckRecord) {
        if let Some(status) = record.terminal_status {
            self.terminal_status = Some(status);
        }
        if let Some(failure_type) = &record.failure_type {
            self.last_failure_family = Some(failure_type.clone());
        }
        if record.action == StopAction::RecommendRollback {
            if let Some(candidate) = &record.rollback_candidate {
                self.record_rollback_candidate(candidate.clone());
            }
        }
        if matches!(
            record.status,
            StopCheckStatus::Checkpoint | StopCheckStatus::Stop
        ) && matches!(
            record.reason,
            StopCheckReason::NoProgress
                | StopCheckReason::FocusedRepairStalled
                | StopCheckReason::RepeatedToolFailure
                | StopCheckReason::ConsecutiveValidationFailures
                | StopCheckReason::ConsecutiveEditFailures
                | StopCheckReason::ConsecutiveCommandFailures
                | StopCheckReason::ConsecutivePermissionBlocks
                | StopCheckReason::UncertaintyNotReduced
                | StopCheckReason::ModelOutputInvalid
                | StopCheckReason::LowActionValueLoop
                | StopCheckReason::ScoreNotReducingUncertainty
                | StopCheckReason::RepeatedActionRevision
        ) {
            self.record_failed_strategy(FailedStrategyRecord {
                failed_strategy: record.reason.label().to_string(),
                reason: record.summary.clone(),
                better_strategy: record
                    .next_action
                    .clone()
                    .unwrap_or_else(|| record.action.label().to_string()),
                recovery_plan_id: record.recovery_plan_id.clone(),
                rollback_status: record.rollback_candidate.as_ref().map(|candidate| {
                    if candidate.auto_allowed {
                        "candidate_auto_allowed".to_string()
                    } else {
                        "candidate_requires_review".to_string()
                    }
                }),
            });
        }
        self.stop_checks.push(record);
        const MAX_STOP_CHECKS: usize = 8;
        if self.stop_checks.len() > MAX_STOP_CHECKS {
            let overflow = self.stop_checks.len() - MAX_STOP_CHECKS;
            self.stop_checks.drain(0..overflow);
        }
    }

    pub fn record_action_score(&mut self, record: ActionScoreRecord) {
        self.action_score_history.push(record);
        trim_front(&mut self.action_score_history, MAX_ACTION_SCORE_HISTORY);
    }

    pub fn consecutive_low_action_scores(&self) -> usize {
        self.action_score_history
            .iter()
            .rev()
            .take_while(|record| record.action_score <= 3)
            .count()
    }

    pub fn consecutive_high_risk_low_value_actions(&self) -> usize {
        self.action_score_history
            .iter()
            .rev()
            .take_while(|record| record.risk >= 8 && record.value <= 5)
            .count()
    }

    pub fn score_without_uncertainty_reduction_rounds(&self) -> usize {
        self.action_score_history
            .iter()
            .rev()
            .take_while(|record| {
                record.action_score <= 8
                    || (record.uncertainty_reduction <= 3 && !record.reduced_uncertainty)
            })
            .count()
    }

    pub fn repeated_revised_action_count(&self) -> usize {
        self.action_score_history
            .iter()
            .rev()
            .take_while(|record| {
                record
                    .review_decision
                    .as_deref()
                    .map(|decision| matches!(decision, "revise" | "denied" | "deny"))
                    .unwrap_or(false)
            })
            .count()
    }

    pub fn record_rollback_candidate(&mut self, candidate: RollbackCandidate) {
        if candidate.paths.is_empty()
            && candidate.checkpoint_id.is_none()
            && candidate.file_change_id.is_none()
            && candidate.tool_round_id.is_none()
        {
            return;
        }
        if self.rollback_candidates.iter().any(|existing| {
            existing.checkpoint_id == candidate.checkpoint_id
                && existing.file_change_id == candidate.file_change_id
                && existing.tool_round_id == candidate.tool_round_id
                && existing.paths == candidate.paths
        }) {
            return;
        }
        self.rollback_candidates.push(candidate);
        trim_front(&mut self.rollback_candidates, MAX_ROLLBACK_CANDIDATES);
    }

    pub fn record_failed_strategy(&mut self, record: FailedStrategyRecord) {
        if record.failed_strategy.trim().is_empty() || record.reason.trim().is_empty() {
            return;
        }
        if self.failed_strategies.iter().any(|existing| {
            existing.failed_strategy == record.failed_strategy && existing.reason == record.reason
        }) {
            return;
        }
        self.failed_strategies.push(record);
        trim_front(&mut self.failed_strategies, MAX_FAILED_STRATEGIES);
    }

    fn observe_context_ledger_entry(&mut self, entry: ContextLedgerEntry) {
        match entry {
            ContextLedgerEntry::FileEdit(entry) => {
                for path in entry.paths.iter().chain(entry.resolved_paths.iter()) {
                    self.add_active_file(path);
                }
                let target = display_evidence_paths(&entry.paths, &entry.resolved_paths);
                if entry.success {
                    self.consecutive_edit_failures = 0;
                    self.mark_progress(format!("edit succeeded: {}", target));
                    self.record_completed_step(
                        AgentTaskStage::Edit,
                        format!(
                            "{} changed {} file(s): {}",
                            entry.tool, entry.file_count, target
                        ),
                    );
                    if !matches!(self.stage, AgentTaskStage::Closeout | AgentTaskStage::Done) {
                        self.transition_to_stage(
                            AgentTaskStage::Validate,
                            "context_ledger.file_edit",
                            "successful edit requires validation",
                            1,
                        );
                    }
                    self.record_edit_snapshot(format!("edit succeeded: {}", target));
                } else {
                    self.consecutive_edit_failures += 1;
                    self.last_failure_family = Some("edit".to_string());
                    self.record_observation(
                        "context_ledger.file_edit",
                        format!(
                            "{} attempted change on {} but did not succeed",
                            entry.tool, target
                        ),
                    );
                    if !matches!(self.stage, AgentTaskStage::Done) {
                        self.transition_to_stage(
                            AgentTaskStage::Repair,
                            "context_ledger.file_edit",
                            "failed edit requires repair",
                            1,
                        );
                    }
                    self.record_edit_snapshot(format!("edit failed: {}", target));
                }
            }
            ContextLedgerEntry::Diff(entry) => {
                let target = entry
                    .command
                    .as_deref()
                    .or(entry.path.as_deref())
                    .or(entry.action.as_deref())
                    .unwrap_or("diff");
                self.record_observation(
                    "context_ledger.diff",
                    format!(
                        "{} inspected {}: changed={}, success={}",
                        entry.tool, target, entry.changed, entry.success
                    ),
                );
            }
            ContextLedgerEntry::Validation(entry) => {
                let status = if entry.success { "passed" } else { "failed" };
                self.record_observation(
                    "context_ledger.validation",
                    format!(
                        "validation {} {} with exit {}",
                        entry.command,
                        status,
                        entry
                            .exit_code
                            .map(|code| code.to_string())
                            .unwrap_or_else(|| "unknown".to_string())
                    ),
                );
                if entry.success {
                    self.consecutive_validation_failures = 0;
                    self.consecutive_command_failures = 0;
                    self.mark_progress(format!("validation passed: {}", entry.command));
                    self.record_completed_step(
                        AgentTaskStage::Validate,
                        format!("validation passed: {}", entry.command),
                    );
                    self.verification_plan.status = VerificationStatus::Verified;
                    if !matches!(self.stage, AgentTaskStage::Done) {
                        self.transition_to_stage(
                            AgentTaskStage::Closeout,
                            "context_ledger.validation",
                            "successful validation is ready for closeout",
                            1,
                        );
                    }
                } else {
                    self.consecutive_validation_failures += 1;
                    self.consecutive_command_failures += 1;
                    self.last_failure_family = Some("validation".to_string());
                    self.verification_plan.status = VerificationStatus::Failed;
                    if !matches!(self.stage, AgentTaskStage::Done) {
                        self.transition_to_stage(
                            AgentTaskStage::Repair,
                            "context_ledger.validation",
                            "failed validation requires repair",
                            1,
                        );
                    }
                    self.record_edit_snapshot(format!("validation failed: {}", entry.command));
                }
            }
            ContextLedgerEntry::UserConfirmation(entry) => {
                let kind = entry.kind.as_deref().unwrap_or("permission");
                self.record_observation(
                    "context_ledger.user_confirmation",
                    format!(
                        "user {} {} for {}",
                        if entry.approved { "approved" } else { "denied" },
                        kind,
                        entry.tool
                    ),
                );
                if !entry.approved {
                    self.consecutive_permission_blocks += 1;
                    self.last_failure_family = Some("permission".to_string());
                    if matches!(
                        self.verification_plan.status,
                        VerificationStatus::Pending | VerificationStatus::NotRequired
                    ) {
                        self.verification_plan.status = VerificationStatus::UserDeferred;
                    }
                    if !matches!(self.stage, AgentTaskStage::Done) {
                        self.transition_to_stage(
                            AgentTaskStage::Repair,
                            "context_ledger.user_confirmation",
                            "user denied or blocked the action",
                            1,
                        );
                    }
                } else {
                    self.consecutive_permission_blocks = 0;
                    self.mark_progress(format!("user approved {} for {}", kind, entry.tool));
                }
            }
            ContextLedgerEntry::ToolObservation(entry) => {
                for path in entry.files_read.iter().chain(entry.files_changed.iter()) {
                    self.add_active_file(path);
                }
                if entry.store_in_state
                    || !entry.key_findings.is_empty()
                    || !entry.evidence.is_empty()
                {
                    self.record_observation(
                        "tool_observation",
                        format!(
                            "{} {}: {}",
                            entry.tool,
                            entry.status,
                            preview(&entry.summary, 160)
                        ),
                    );
                }
                for finding in &entry.key_findings {
                    self.record_key_finding(
                        format!("tool_observation.{}", entry.result_kind),
                        finding.clone(),
                        entry.evidence.clone(),
                    );
                }
                if let Some(impact) = &entry.impact_on_goal {
                    self.record_key_finding(
                        "tool_observation.impact",
                        impact.clone(),
                        entry.evidence.clone(),
                    );
                }
                for attention in &entry.next_attention {
                    self.record_key_finding(
                        "tool_observation.next_attention",
                        attention.clone(),
                        entry.evidence.clone(),
                    );
                }
                for hypothesis in &entry.hypothesis_updates {
                    self.record_hypothesis(
                        hypothesis.clone(),
                        entry.confidence.unwrap_or(70),
                        entry.evidence.clone(),
                    );
                }
                for focus in entry
                    .candidate_focus
                    .iter()
                    .chain(entry.files_read.iter())
                    .chain(entry.files_changed.iter())
                {
                    self.record_candidate_focus(
                        focus.clone(),
                        format!("{} observation", entry.result_kind),
                        entry.confidence.unwrap_or(70),
                    );
                }
                if let Some(risk_note) = &entry.risk_note {
                    self.add_risk(risk_note.clone());
                }
                self.record_action_score_from_tool_observation(&entry);
                self.update_progress_from_tool_observation(&entry);
            }
        }
    }

    fn record_action_score_from_tool_observation(
        &mut self,
        entry: &crate::engine::context_ledger::ToolObservationLedgerEntry,
    ) {
        let Some(action_score) = entry.action_score else {
            return;
        };
        let Some(value) = entry.action_value else {
            return;
        };
        let Some(risk) = entry.action_risk else {
            return;
        };
        let Some(uncertainty_reduction) = entry.action_uncertainty_reduction else {
            return;
        };
        let Some(cost) = entry.action_cost else {
            return;
        };
        let Some(reversibility) = entry.action_reversibility else {
            return;
        };
        let Some(scope_fit) = entry.action_scope_fit else {
            return;
        };

        self.record_action_score(ActionScoreRecord {
            tool: entry.tool.clone(),
            stage: entry
                .action_stage
                .clone()
                .unwrap_or_else(|| format!("{:?}", self.stage)),
            action_score,
            value,
            risk,
            uncertainty_reduction,
            cost,
            reversibility,
            scope_fit,
            formula_stage: entry.action_formula_stage.clone(),
            formula_version: entry.action_formula_version.clone(),
            review_decision: entry.action_review_decision.clone(),
            reduced_uncertainty: entry.reduced_uncertainty,
        });
    }

    fn mark_progress(&mut self, signal: String) {
        if signal.trim().is_empty() {
            return;
        }
        self.uncertainty_not_reduced_steps = 0;
        self.last_progress_signal = Some(preview(&signal, 160));
    }

    fn mark_uncertainty_not_reduced(&mut self) {
        self.uncertainty_not_reduced_steps += 1;
    }

    fn update_progress_from_tool_observation(
        &mut self,
        entry: &crate::engine::context_ledger::ToolObservationLedgerEntry,
    ) {
        let status = entry.status.as_str();
        let result_kind = entry.result_kind.as_str();
        let success = status == "success" || status == "ok" || status == "passed";
        let failed = matches!(
            status,
            "failed" | "error" | "denied" | "rejected" | "blocked"
        );

        if entry.reduced_uncertainty || success || !entry.key_findings.is_empty() {
            self.mark_progress(format!(
                "{} {} observation reduced uncertainty",
                entry.tool, result_kind
            ));
        } else if entry.include_in_next_context || entry.store_in_state {
            self.mark_uncertainty_not_reduced();
        }

        if matches!(result_kind, "validation" | "test" | "command_validation") {
            if success {
                self.consecutive_validation_failures = 0;
                self.consecutive_command_failures = 0;
            } else if failed {
                self.consecutive_validation_failures += 1;
                self.consecutive_command_failures += 1;
                self.last_failure_family = entry
                    .failure_type
                    .clone()
                    .or_else(|| Some("validation".to_string()));
            }
        } else if matches!(result_kind, "edit" | "file_edit" | "patch" | "write") {
            if success {
                self.consecutive_edit_failures = 0;
            } else if failed {
                self.consecutive_edit_failures += 1;
                self.last_failure_family = entry
                    .failure_type
                    .clone()
                    .or_else(|| Some("edit".to_string()));
            }
        } else if matches!(result_kind, "command" | "bash" | "shell") {
            if success {
                self.consecutive_command_failures = 0;
            } else if failed {
                self.consecutive_command_failures += 1;
                self.last_failure_family = entry
                    .failure_type
                    .clone()
                    .or_else(|| Some("command".to_string()));
            }
        } else if failed && entry.failure_type.is_some() {
            self.last_failure_family = entry.failure_type.clone();
        }

        let permission_denied = entry
            .permission_decision
            .as_deref()
            .is_some_and(|decision| matches!(decision, "denied" | "blocked" | "rejected"))
            || matches!(status, "denied" | "blocked");
        if permission_denied {
            self.consecutive_permission_blocks += 1;
            self.last_failure_family = Some("permission".to_string());
        }

        let should_recommend_rollback = failed
            && entry.checkpoint_id.is_some()
            && (!entry.files_changed.is_empty()
                || matches!(result_kind, "edit" | "file_edit" | "patch" | "write"));
        if should_recommend_rollback {
            self.record_rollback_candidate(RollbackCandidate {
                checkpoint_id: entry.checkpoint_id.clone(),
                file_change_id: None,
                tool_round_id: Some(entry.call_id.clone()),
                paths: entry.files_changed.clone(),
                reason: entry.risk_note.clone().unwrap_or_else(|| {
                    format!("{} failed after a checkpointed change", entry.tool)
                }),
                confidence: entry.confidence.unwrap_or(75),
                auto_allowed: false,
            });
        }

        if success && matches!(entry.tool.as_str(), "rewind" | "rollback") {
            self.terminal_status = Some(TaskTerminalStatus::RolledBack);
        }
    }

    pub fn observe_tool_round(&mut self, observation: AgentToolRoundObservation) {
        if observation.has_successful_validation_commands {
            self.verification_plan.status = VerificationStatus::Verified;
            self.record_completed_step(AgentTaskStage::Validate, "validation succeeded");
            self.transition_to_stage(
                AgentTaskStage::Closeout,
                "tool_round",
                "validation succeeded",
                1,
            );
            return;
        }

        if observation.batch_has_unsuccessful_tools || observation.failed_tool_evidence_present {
            if matches!(self.verification_plan.status, VerificationStatus::Pending) {
                self.verification_plan.status = VerificationStatus::Failed;
            }
            self.record_observation("tool_round", "tool failure requires repair");
            self.transition_to_stage(
                AgentTaskStage::Repair,
                "tool_round",
                "tool failure requires repair",
                1,
            );
            self.record_edit_snapshot("tool round requires repair");
            return;
        }

        if observation.successful_write_tool
            || observation.used_write_tool
            || observation.has_worktree_changes
        {
            self.record_completed_step(AgentTaskStage::Edit, "code changes were applied");
            self.transition_to_stage(
                AgentTaskStage::Validate,
                "tool_round",
                "code changes were applied",
                1,
            );
            self.record_edit_snapshot("tool round applied changes");
            return;
        }

        if observation.any_tool_success
            && matches!(
                self.stage,
                AgentTaskStage::Understand | AgentTaskStage::Plan
            )
        {
            self.record_completed_step(self.stage, "initial context was inspected");
            self.transition_to_stage(
                AgentTaskStage::Edit,
                "tool_round",
                "initial context was inspected",
                1,
            );
        }
    }

    pub fn format_for_context_zone(&self) -> String {
        let active_files = if self.active_files.is_empty() {
            "none".to_string()
        } else {
            self.active_files
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let risks = if self.risks.is_empty() {
            "none".to_string()
        } else {
            self.risks.join("; ")
        };
        let checks = if self.verification_plan.required_checks.is_empty() {
            "none".to_string()
        } else {
            self.verification_plan.required_checks.join("; ")
        };
        let stop_check = self
            .stop_checks
            .last()
            .map(|record| {
                let terminal = record
                    .terminal_status
                    .map(|status| status.label())
                    .unwrap_or("none");
                let failure = record.failure_type.as_deref().unwrap_or("none");
                let recovery = record.recovery_plan_id.as_deref().unwrap_or("none");
                let rollback = record
                    .rollback_candidate
                    .as_ref()
                    .and_then(|candidate| candidate.checkpoint_id.as_deref())
                    .unwrap_or("none");
                format!(
                    "{}: reason={} terminal={} action={} failure={} recovery={} rollback={} summary={}",
                    record.status.label(),
                    record.reason.label(),
                    terminal,
                    record.action.label(),
                    failure,
                    recovery,
                    rollback,
                    preview(&record.summary, 160)
                )
            })
            .unwrap_or_else(|| "none".to_string());
        let terminal_status = self
            .terminal_status
            .map(|status| status.label())
            .unwrap_or("none");
        let failure_counters = format!(
            "uncertainty={}, validation={}, edit={}, command={}, permission={}, low_score={}, score_no_uncertainty={}, revised_actions={}",
            self.uncertainty_not_reduced_steps,
            self.consecutive_validation_failures,
            self.consecutive_edit_failures,
            self.consecutive_command_failures,
            self.consecutive_permission_blocks,
            self.consecutive_low_action_scores(),
            self.score_without_uncertainty_reduction_rounds(),
            self.repeated_revised_action_count()
        );
        let rollback_candidates = if self.rollback_candidates.is_empty() {
            "none".to_string()
        } else {
            self.rollback_candidates
                .iter()
                .rev()
                .take(2)
                .map(|candidate| {
                    format!(
                        "checkpoint={} paths={} reason={}",
                        candidate.checkpoint_id.as_deref().unwrap_or("none"),
                        preview(&candidate.paths.join(", "), 80),
                        preview(&candidate.reason, 80)
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        let recent_steps = if self.completed_steps.is_empty() {
            "none".to_string()
        } else {
            self.completed_steps
                .iter()
                .rev()
                .take(3)
                .map(|step| format!("{:?}: {}", step.stage, preview(&step.summary, 120)))
                .collect::<Vec<_>>()
                .join("; ")
        };
        let stage_transitions = if self.stage_transitions.is_empty() {
            "none".to_string()
        } else {
            self.stage_transitions
                .iter()
                .rev()
                .take(3)
                .map(|transition| {
                    format!(
                        "{:?}->{:?} via {}: {} evidence={}",
                        transition.from,
                        transition.to,
                        transition.source,
                        preview(&transition.reason, 100),
                        transition.evidence_items
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        let recent_observations = if self.observations.is_empty() {
            "none".to_string()
        } else {
            self.observations
                .iter()
                .rev()
                .take(3)
                .map(|observation| {
                    format!(
                        "{}: {}",
                        observation.source,
                        preview(&observation.summary, 120)
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        let recent_snapshots = if self.edit_snapshots.is_empty() {
            "none".to_string()
        } else {
            self.edit_snapshots
                .iter()
                .rev()
                .take(3)
                .map(|snapshot| {
                    let files = if snapshot.active_files.is_empty() {
                        "none".to_string()
                    } else {
                        snapshot
                            .active_files
                            .iter()
                            .take(3)
                            .map(|path| path.display().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    format!(
                        "{}: stage={:?}, verification={:?}, files={}",
                        preview(&snapshot.label, 80),
                        snapshot.stage,
                        snapshot.verification_status,
                        files
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        let key_findings = if self.key_findings.is_empty() {
            "none".to_string()
        } else {
            self.key_findings
                .iter()
                .rev()
                .take(3)
                .map(|finding| {
                    let evidence = if finding.evidence.is_empty() {
                        String::new()
                    } else {
                        format!(" evidence={}", preview(&finding.evidence.join(" | "), 120))
                    };
                    format!(
                        "{}: {}{}",
                        finding.source,
                        preview(&finding.summary, 120),
                        evidence
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        let hypotheses = if self.hypotheses.is_empty() {
            "none".to_string()
        } else {
            self.hypotheses
                .iter()
                .rev()
                .take(3)
                .map(|hypothesis| {
                    format!(
                        "{} ({}%)",
                        preview(&hypothesis.hypothesis, 120),
                        hypothesis.confidence
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        let candidate_focus = if self.candidate_focus.is_empty() {
            "none".to_string()
        } else {
            self.candidate_focus
                .iter()
                .rev()
                .take(4)
                .map(|focus| {
                    format!(
                        "{} ({}%, {})",
                        preview(&focus.target, 80),
                        focus.confidence,
                        preview(&focus.reason, 80)
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        let lightweight_plan = self
            .lightweight_plan
            .as_ref()
            .map(LightweightPlan::format_for_context_zone)
            .unwrap_or_else(|| "none".to_string());
        let action_scores = if self.action_score_history.is_empty() {
            "none".to_string()
        } else {
            self.action_score_history
                .iter()
                .rev()
                .take(3)
                .map(|record| {
                    format!(
                        "{} stage={} score={} value={} risk={} uncertainty={} scope={} review={} reduced_uncertainty={}",
                        record.tool,
                        record.stage,
                        record.action_score,
                        record.value,
                        record.risk,
                        record.uncertainty_reduction,
                        record.scope_fit,
                        record.review_decision.as_deref().unwrap_or("none"),
                        record.reduced_uncertainty
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };

        format!(
            "Goal: {}\nMode: {:?}\nMode score: {}\nLightweight plan: {}\nStage: {:?}\nTerminal status: {}\nActive files: {}\nRisks: {}\nVerification: {:?}; checks: {}\nFailure counters: {}\nRecent action scores: {}\nRecent steps: {}\nStage transitions: {}\nRecent observations: {}\nKey findings: {}\nHypotheses: {}\nCandidate focus: {}\nRecent edit snapshots: {}\nRollback candidates: {}\nStop check: {}\nDone: {}",
            self.main_goal,
            self.mode,
            self.mode_score.compact_summary(),
            lightweight_plan,
            self.stage,
            terminal_status,
            active_files,
            risks,
            self.verification_plan.status,
            checks,
            failure_counters,
            action_scores,
            recent_steps,
            stage_transitions,
            recent_observations,
            key_findings,
            hypotheses,
            candidate_focus,
            recent_snapshots,
            rollback_candidates,
            stop_check,
            self.done_condition.satisfied
        )
    }
}
