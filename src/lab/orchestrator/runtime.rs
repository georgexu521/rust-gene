use super::*;

pub(super) fn transition_for_stage(stage: &str) -> Option<StageTransition> {
    STAGE_TRANSITIONS
        .iter()
        .copied()
        .find(|transition| transition.from_stage == stage)
}

pub(super) fn artifact_type_for_stage(stage: &str) -> anyhow::Result<LabArtifactType> {
    match stage {
        "professor_discussion" => Ok(LabArtifactType::ProfessorPlan),
        "postdoc_plan" => Ok(LabArtifactType::PostdocPlan),
        "graduate_work" => Ok(LabArtifactType::GraduateResult),
        "postdoc_review" => Ok(LabArtifactType::PostdocIntegrationSummary),
        "professor_review" => Ok(LabArtifactType::ProfessorReview),
        _ => Err(anyhow!("unknown LabRun artifact stage: {stage}")),
    }
}

#[derive(Debug, Default)]
pub(super) struct PostdocWorktreeProof {
    pub(super) accepted_results: Vec<String>,
    pub(super) remaining_risks: Vec<String>,
    pub(super) evidence_refs: Vec<String>,
}

pub(super) fn collect_graduate_worktree_proof_for_postdoc(
    store: &LabStore,
    lab_run_id: &str,
    limit: usize,
) -> anyhow::Result<PostdocWorktreeProof> {
    let events = store.list_run_events(lab_run_id)?;
    let mut proof = PostdocWorktreeProof::default();
    let mut recent_events = events
        .iter()
        .rev()
        .filter(|event| event.event_type == "lab_graduate_worktree_action")
        .take(limit)
        .collect::<Vec<_>>();
    recent_events.reverse();

    for event in recent_events {
        proof
            .evidence_refs
            .push(format!("event:{}", event.event_id));
        let summary = format_graduate_worktree_proof_for_postdoc(event);
        if event
            .payload
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            proof
                .accepted_results
                .push(format!("runtime worktree proof: {summary}"));
        } else {
            proof
                .remaining_risks
                .push(format!("runtime worktree proof failed: {summary}"));
        }
    }

    Ok(proof)
}

pub(super) fn format_graduate_worktree_proof_for_postdoc(event: &LabEvent) -> String {
    let payload = &event.payload;
    let result_data = payload.get("result_data").unwrap_or(&Value::Null);
    let action = payload
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let task_id = payload
        .get("task_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let agent_ref_kind = payload
        .get("agent_ref_kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let agent_ref = payload
        .get("agent_ref")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let merge_kind = result_data
        .get("merge_kind")
        .and_then(Value::as_str)
        .unwrap_or("n/a");
    let dirty = result_data
        .get("dirty")
        .and_then(Value::as_bool)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "n/a".to_string());
    let path = result_data
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or("n/a");
    format!(
        "{} task={} ref={}:{} merge_kind={} dirty={} path={}",
        action, task_id, agent_ref_kind, agent_ref, merge_kind, dirty, path
    )
}

pub(super) fn collect_graduate_workspace_snapshot_proof_for_postdoc(
    store: &LabStore,
    lab_run_id: &str,
    limit: usize,
) -> anyhow::Result<PostdocWorktreeProof> {
    let events = store.list_run_events(lab_run_id)?;
    let mut proof = PostdocWorktreeProof::default();
    let mut recent_events = events
        .iter()
        .rev()
        .filter(|event| event.event_type == "lab_graduate_workspace_snapshot")
        .take(limit)
        .collect::<Vec<_>>();
    recent_events.reverse();

    for event in recent_events {
        proof
            .evidence_refs
            .push(format!("event:{}", event.event_id));
        let summary = format_graduate_workspace_snapshot_for_postdoc(event);
        let phase = event
            .payload
            .get("phase")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let dirty_count = event
            .payload
            .get("dirty_path_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let changed_count = event
            .payload
            .get("changed_path_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if phase == "before" && dirty_count > 0 {
            proof
                .remaining_risks
                .push(format!("pre-existing workspace changes: {summary}"));
        } else if phase == "after" && changed_count > 0 {
            proof
                .accepted_results
                .push(format!("runtime workspace delta: {summary}"));
        }
    }

    Ok(proof)
}

pub(super) fn format_graduate_workspace_snapshot_for_postdoc(event: &LabEvent) -> String {
    let payload = &event.payload;
    let phase = payload
        .get("phase")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let task_id = payload
        .get("task_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let dispatch_id = payload
        .get("dispatch_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let dirty_paths = value_string_list(payload.get("dirty_paths"));
    let changed_paths = value_string_list(payload.get("changed_paths"));
    format!(
        "{} task={} dispatch={} dirty=[{}] changed=[{}]",
        phase,
        task_id,
        dispatch_id,
        summarize_paths_for_runtime_proof(&dirty_paths),
        summarize_paths_for_runtime_proof(&changed_paths)
    )
}

pub(super) fn value_string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn summarize_paths_for_runtime_proof(paths: &[String]) -> String {
    if paths.is_empty() {
        return "none".to_string();
    }
    let mut shown = paths.iter().take(5).cloned().collect::<Vec<_>>();
    if paths.len() > shown.len() {
        shown.push(format!("+{} more", paths.len() - shown.len()));
    }
    shown.join(",")
}

pub(super) fn build_stage_artifact(
    run: &LabRun,
    artifact_type: LabArtifactType,
    note: &str,
) -> StageArtifact {
    let now = Utc::now();
    let note = note.trim();
    let title = if note.is_empty() {
        format!("{} for {}", artifact_type.as_str(), run.lab_run_id)
    } else {
        note.lines().next().unwrap_or(note).trim().to_string()
    };
    let artifact_id = format!(
        "artifact_{}_{}",
        artifact_type.as_str().to_ascii_lowercase(),
        Uuid::new_v4().simple()
    );
    match artifact_type {
        LabArtifactType::ProfessorPlan => StageArtifact::ProfessorPlan(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            artifact_type,
            title,
            now,
            ProfessorPlan {
                problem_statement: run.user_goal.clone(),
                strategic_direction: note_or_placeholder(note, "Initial professor direction."),
                success_criteria: vec![
                    "User-visible result is reviewed before closeout.".to_string()
                ],
                constraints: vec![
                    "Do not bypass runtime permission, checkpoint, or validation gates."
                        .to_string(),
                ],
                risks: vec![
                    "Plan content is a runtime draft until reviewed by the professor model."
                        .to_string(),
                ],
                handoff_to_postdoc:
                    "Create an implementation plan with slices, expected files, and validation."
                        .to_string(),
            },
        )),
        LabArtifactType::PostdocPlan => StageArtifact::PostdocPlan(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            artifact_type,
            title,
            now,
            PostdocPlan {
                implementation_summary: note_or_placeholder(
                    note,
                    "Postdoc implementation plan draft.",
                ),
                slices: vec!["Implement the smallest verifiable next slice.".to_string()],
                files_expected: Vec::new(),
                validation_plan: vec!["Run the narrowest relevant validation gate.".to_string()],
                graduate_handoff:
                    "Execute the current slice and report changed files, proof, and blockers."
                        .to_string(),
            },
        )),
        LabArtifactType::GraduateResult => StageArtifact::GraduateResult(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            artifact_type,
            title,
            now,
            GraduateResult {
                task_summary: note_or_placeholder(note, "Graduate task result draft."),
                changed_files: Vec::new(),
                validation_attempts: Vec::new(),
                blockers: Vec::new(),
                handoff_to_postdoc: "Review implementation quality and integration readiness."
                    .to_string(),
            },
        )),
        LabArtifactType::PostdocIntegrationSummary => {
            StageArtifact::PostdocIntegrationSummary(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                title,
                now,
                PostdocIntegrationSummary {
                    integration_summary: note_or_placeholder(
                        note,
                        "Postdoc integration summary draft.",
                    ),
                    accepted_results: Vec::new(),
                    validation_status: "not_verified".to_string(),
                    remaining_risks: Vec::new(),
                    handoff_to_professor:
                        "Review strategic fit, completeness, and user-facing closeout.".to_string(),
                },
            ))
        }
        LabArtifactType::ProfessorReview => {
            StageArtifact::ProfessorReview(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                title,
                now,
                ProfessorReview {
                    review_summary: note_or_placeholder(note, "Professor review draft."),
                    strategic_assessment: "Strategic assessment requires professor model review."
                        .to_string(),
                    accepted: false,
                    required_revisions: Vec::new(),
                    user_report: "Prepare a concise user-facing report before closeout."
                        .to_string(),
                },
            ))
        }
        LabArtifactType::CycleSummary => StageArtifact::CycleSummary(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            artifact_type,
            title,
            now,
            LabCycleSummary {
                cycle_id: run.cycle_count.to_string(),
                current_stage: run.current_stage.clone(),
                owner: run.internal_owner,
                summary: note_or_placeholder(note, "Cycle summary draft."),
                completed_items: Vec::new(),
                evidence_ids: Vec::new(),
                total_tokens: 0,
                cache_hit_rate_percent: 0.0,
                estimated_cost_usd: 0.0,
                next_action: "Continue LabRun orchestration from the current stage.".to_string(),
            },
        )),
        LabArtifactType::CompressionSummary => {
            StageArtifact::CompressionSummary(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                title,
                now,
                LabCompressionSummary {
                    decision_id: String::new(),
                    role: run.internal_owner,
                    action: LabCompressionAction::None,
                    reason: "Compression summary placeholder.".to_string(),
                    before_tokens: 0,
                    target_budget_tokens: 0,
                    usage_ratio_percent: 0.0,
                    stable_prefix_fingerprint: String::new(),
                    dynamic_tail_fingerprint: String::new(),
                    retained_layers: Vec::new(),
                    evidence_ids: Vec::new(),
                    compressed_summary: note_or_placeholder(note, "Compression summary draft."),
                    next_action: "Continue LabRun orchestration from the current stage."
                        .to_string(),
                },
            ))
        }
        LabArtifactType::LabMeetingRequest => {
            StageArtifact::LabMeetingRequest(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                format!(
                    "Runtime escalation signal meeting request for {}",
                    run.current_stage
                ),
                now,
                LabMeetingRequest {
                    request_id: "meeting_request_placeholder".to_string(),
                    topic: note_or_placeholder(note, "General LabRun meeting request."),
                    current_stage: run.current_stage.clone(),
                    reason: "runtime_placeholder".to_string(),
                    signals: Vec::new(),
                    requested_by: LabRole::Runtime,
                    next_action: "open_read_only_lab_meeting".to_string(),
                },
            ))
        }
        LabArtifactType::LabMeetingSummary => {
            StageArtifact::LabMeetingSummary(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                format!("Lab meeting summary for {}", run.lab_run_id),
                now,
                LabMeetingSummary {
                    meeting_id: "meeting_placeholder".to_string(),
                    topic: note_or_placeholder(note, "General LabRun meeting."),
                    current_stage: run.current_stage.clone(),
                    professor_view: "Runtime placeholder professor view.".to_string(),
                    postdoc_view: "Runtime placeholder postdoc view.".to_string(),
                    decision: "continue_current_plan".to_string(),
                    next_actions: vec!["continue_labrun".to_string()],
                    evidence_ids: Vec::new(),
                    total_tokens: 0,
                    cache_hit_rate_percent: 0.0,
                },
            ))
        }
        LabArtifactType::LabBlockerReport => {
            StageArtifact::LabBlockerReport(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                format!("Postdoc blocker report for {}", run.current_stage),
                now,
                LabBlockerReport {
                    blocker_id: "blocker_placeholder".to_string(),
                    current_stage: run.current_stage.clone(),
                    summary: note_or_placeholder(note, "No blocker summary provided."),
                    blocked_tasks: Vec::new(),
                    failed_dispatches: Vec::new(),
                    failure_count: run.failure_count,
                    recommendation: "continue_current_plan".to_string(),
                    handoff_to_professor: "Review blocker state.".to_string(),
                },
            ))
        }
        LabArtifactType::LabRevisionTask => {
            StageArtifact::LabRevisionTask(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                format!("Postdoc revision task for {}", run.lab_run_id),
                now,
                LabRevisionTask {
                    revision_id: "revision_placeholder".to_string(),
                    source_review_artifact_id: String::new(),
                    assigned_role: LabRole::Postdoc,
                    summary: note_or_placeholder(note, "Professor requested postdoc revision."),
                    required_revisions: Vec::new(),
                    evidence_ids: Vec::new(),
                    next_action:
                        "Revise postdoc integration before professor review can close out."
                            .to_string(),
                },
            ))
        }
        LabArtifactType::ProfessorSteeringDecision => {
            StageArtifact::ProfessorSteeringDecision(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                format!("Professor steering decision for {}", run.lab_run_id),
                now,
                ProfessorSteeringDecision {
                    decision_id: "professor_steering_placeholder".to_string(),
                    source_message_id: String::new(),
                    decision: "pending_professor_review".to_string(),
                    status: SponsorMessageStatus::Queued,
                    message_type: SponsorMessageType::Concern,
                    urgency: "normal".to_string(),
                    rationale: note_or_placeholder(note, "No steering rationale provided."),
                    next_action: "Review sponsor message before applying any LabRun change."
                        .to_string(),
                    message_summary: String::new(),
                },
            ))
        }
    }
}

pub(super) fn note_or_placeholder(note: &str, placeholder: &str) -> String {
    if note.trim().is_empty() {
        placeholder.to_string()
    } else {
        note.trim().to_string()
    }
}

pub(super) fn postdoc_plan_task_marker(artifact_id: &str) -> String {
    format!("postdoc_plan_artifact_id={}", artifact_id.trim())
}

pub(super) fn compact_task_title(value: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= 72 {
        return compact;
    }
    let mut out = compact.chars().take(69).collect::<String>();
    out.push_str("...");
    out
}

pub(super) fn compact_result_preview(value: &str, limit: usize) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= limit {
        return compact;
    }
    let keep = limit.saturating_sub(3);
    let mut out = compact.chars().take(keep).collect::<String>();
    out.push_str("...");
    out
}

pub(super) fn clean_string_vec(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

pub(super) fn validate_changed_files_within_scope(
    allowed_scope: &[String],
    changed_files: &[String],
) -> anyhow::Result<()> {
    if changed_files.is_empty() {
        return Ok(());
    }
    if allowed_scope.is_empty() {
        return Err(anyhow!(
            "graduate result cannot report changed files without allowed_scope"
        ));
    }
    let outside = changed_files
        .iter()
        .find(|file| !path_matches_any_scope(file, allowed_scope));
    if let Some(file) = outside {
        return Err(anyhow!(
            "graduate result changed file '{}' is outside allowed_scope ({})",
            file,
            allowed_scope.join(", ")
        ));
    }
    Ok(())
}

pub(super) fn path_matches_any_scope(file: &str, allowed_scope: &[String]) -> bool {
    let file = file.trim().trim_start_matches("./");
    allowed_scope.iter().any(|scope| {
        let scope = scope.trim().trim_start_matches("./");
        if scope.is_empty() {
            return false;
        }
        file == scope || file.starts_with(&format!("{}/", scope.trim_end_matches('/')))
    })
}

pub(super) fn durable_graduate_task_is_completed(
    context: &ToolContext,
    task: &GraduateTask,
) -> bool {
    let Some(store) = context.session_store.as_ref() else {
        return false;
    };
    let agent_task_id = graduate_agent_task_id(task);
    matches!(
        store.agent_task_state(&context.session_id, &agent_task_id),
        Ok(Some(state))
            if state.profile.as_deref() == Some("lab-graduate")
                && state.status == "completed"
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GraduateRuntimeEvidence {
    pub(super) changed_files: Vec<String>,
    pub(super) validation_attempts: Vec<String>,
}

pub(super) fn runtime_verify_graduate_task_result(
    task: &GraduateTask,
    context: &ToolContext,
    agent_id: Option<&str>,
    agent_task_id: &str,
    parent_changed_files: &[String],
) -> anyhow::Result<GraduateRuntimeEvidence> {
    let verification_root = agent_id
        .and_then(|agent_id| agent_worktree_path(context, agent_id))
        .or_else(|| agent_worktree_path(context, agent_task_id))
        .unwrap_or_else(|| context.working_dir.clone());
    if !verification_root.exists() {
        return Err(anyhow!(
            "graduate runtime verification worktree does not exist: {}",
            verification_root.display()
        ));
    }

    let changed_files = if same_filesystem_path(&verification_root, &context.working_dir) {
        clean_string_vec(parent_changed_files.to_vec())
    } else {
        current_git_changed_paths(&verification_root, Some(&context.working_dir))
    };
    if changed_files.is_empty() {
        return Err(anyhow!(
            "graduate runtime verification found no actual file changes in {}",
            verification_root.display()
        ));
    }
    validate_changed_files_within_scope(&task.allowed_scope, &changed_files)?;
    let validation_attempts =
        run_required_validation_commands(&verification_root, &task.required_validation)?;

    Ok(GraduateRuntimeEvidence {
        changed_files,
        validation_attempts,
    })
}

pub(super) fn agent_worktree_path(context: &ToolContext, agent_id: &str) -> Option<PathBuf> {
    let store = context.session_store.as_ref()?;
    let state = store
        .agent_task_state(&context.session_id, agent_id)
        .ok()
        .flatten()?;
    state
        .payload
        .get("isolated_worktree")?
        .get("path")?
        .as_str()
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
}

pub(super) fn same_filesystem_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

pub(super) fn current_git_changed_paths(
    worktree_root: &Path,
    target_root: Option<&Path>,
) -> Vec<String> {
    let mut paths = workspace_change_snapshot(worktree_root)
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    if let Some(target_root) = target_root {
        if let Some(base) = git_stdout(target_root, &["rev-parse", "HEAD"]) {
            if let Some(committed) = git_stdout(
                worktree_root,
                &["diff", "--name-only", &format!("{}...HEAD", base)],
            ) {
                paths.extend(
                    committed
                        .lines()
                        .map(str::trim)
                        .filter(|line| !line.is_empty() && !is_internal_lab_runtime_path(line))
                        .map(str::to_string),
                );
            }
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

pub(super) fn git_stdout(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn run_required_validation_commands(
    cwd: &Path,
    commands: &[String],
) -> anyhow::Result<Vec<String>> {
    let mut attempts = Vec::new();
    for command in commands {
        let command = command.trim();
        if command.is_empty() {
            continue;
        }
        let output = Command::new("sh")
            .arg("-lc")
            .arg(command)
            .current_dir(cwd)
            .output()
            .map_err(|err| anyhow!("failed to run required validation `{command}`: {err}"))?;
        if output.status.success() {
            attempts.push(format!("runtime validation `{command}` passed"));
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "required validation `{}` failed with status {:?}; stdout={}; stderr={}",
                command,
                output.status.code(),
                compact_result_preview(&stdout, 240),
                compact_result_preview(&stderr, 240)
            ));
        }
    }
    Ok(attempts)
}

pub(super) fn workspace_change_snapshot(project_root: &Path) -> BTreeMap<String, String> {
    let output = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=all"])
        .current_dir(project_root)
        .output();
    let Ok(output) = output else {
        return BTreeMap::new();
    };
    if !output.status.success() {
        return BTreeMap::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_git_status_path)
        .filter(|path| !is_internal_lab_runtime_path(path))
        .map(|path| {
            let fingerprint = workspace_path_fingerprint(project_root, &path);
            (path, fingerprint)
        })
        .collect()
}

pub(super) fn parse_git_status_path(line: &str) -> Option<String> {
    if line.len() < 4 {
        return None;
    }
    let path = line[3..].trim();
    if path.is_empty() {
        return None;
    }
    Some(
        path.rsplit_once(" -> ")
            .map(|(_, renamed)| renamed)
            .unwrap_or(path)
            .trim_matches('"')
            .trim_start_matches("./")
            .to_string(),
    )
}

pub(super) fn closeout_status_from_gate(gate: &ArtifactGate) -> LabCloseoutStatus {
    match gate
        .validation_status
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "verified" | "validated" | "passed" | "success" => LabCloseoutStatus::CompletedVerified,
        "partial" | "partially_verified" | "partially_completed" => LabCloseoutStatus::Partial,
        "blocked" | "blocked_needs_user" | "needs_user" => LabCloseoutStatus::BlockedNeedsUser,
        "failed" | "failure" => LabCloseoutStatus::Failed,
        _ => LabCloseoutStatus::CompletedNotVerified,
    }
}

pub(super) fn workspace_path_fingerprint(project_root: &Path, path: &str) -> String {
    let full_path = project_root.join(path);
    let Ok(metadata) = std::fs::metadata(&full_path) else {
        return "missing".to_string();
    };
    if !metadata.is_file() {
        return format!("non_file:{}", metadata.len());
    }
    let Ok(bytes) = std::fs::read(&full_path) else {
        return format!("unreadable:{}", metadata.len());
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("file:{}:{:x}", metadata.len(), hasher.finish())
}

pub(super) fn changed_paths_between(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> Vec<String> {
    after
        .iter()
        .filter_map(|(path, fingerprint)| {
            (!is_internal_lab_runtime_path(path) && before.get(path) != Some(fingerprint))
                .then_some(path.clone())
        })
        .collect()
}

pub(super) fn is_internal_lab_runtime_path(path: &str) -> bool {
    let path = path.trim().trim_start_matches("./");
    path.starts_with(".priority-agent/")
        || path == ".priority-agent"
        || path.starts_with(".git/")
        || path == ".git"
        || path.starts_with(".claude/worktrees/")
        || path == ".claude/worktrees"
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedGraduateAgentResult {
    pub(super) task_summary: String,
    pub(super) changed_files: Vec<String>,
    pub(super) validation_attempts: Vec<String>,
    pub(super) blockers: Vec<String>,
    pub(super) evidence_ids: Vec<String>,
}

pub(super) fn parse_graduate_agent_result(
    data: Option<&Value>,
    content: &str,
) -> Option<ParsedGraduateAgentResult> {
    if let Some(data) = data {
        if let Some(parsed) = parse_graduate_agent_result_value(data) {
            return Some(parsed);
        }
        if let Some(result) = data.get("result").and_then(Value::as_str) {
            if let Some(value) = parse_json_value_from_text(result) {
                if let Some(parsed) = parse_graduate_agent_result_value(&value) {
                    return Some(parsed);
                }
            }
        }
    }
    parse_json_value_from_text(content).and_then(|value| parse_graduate_agent_result_value(&value))
}

pub(super) fn parse_json_value_from_text(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return Some(value);
    }
    if let Some(fenced) = trimmed.strip_prefix("```") {
        let body = fenced.lines().skip(1).collect::<Vec<_>>().join("\n");
        let body = body
            .trim()
            .strip_suffix("```")
            .unwrap_or(body.trim())
            .trim();
        if let Ok(value) = serde_json::from_str::<Value>(body) {
            return Some(value);
        }
    }
    let start = trimmed.find('{')?;
    for end in trimmed.rmatch_indices('}').map(|(idx, _)| idx + 1) {
        if end <= start {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(&trimmed[start..end]) {
            return Some(value);
        }
    }
    None
}

pub(super) fn parse_graduate_agent_result_value(
    value: &Value,
) -> Option<ParsedGraduateAgentResult> {
    let value = value
        .get("graduate_result")
        .or_else(|| value.get("result_json"))
        .unwrap_or(value);
    let task_summary = string_field(value, &["task_summary", "summary", "handoff_summary"])?;
    let validation_attempts = string_array_field(
        value,
        &["validation_attempts", "validation_results", "validation"],
    );
    if validation_attempts.is_empty() {
        return None;
    }
    Some(ParsedGraduateAgentResult {
        task_summary,
        changed_files: string_array_field(value, &["changed_files", "files_changed"]),
        validation_attempts,
        blockers: string_array_field(value, &["blockers", "risks"]),
        evidence_ids: string_array_field(value, &["evidence_ids", "evidence_refs"]),
    })
}

pub(super) fn string_field(value: &Value, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| value.get(*name).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn string_array_field(value: &Value, names: &[&str]) -> Vec<String> {
    names
        .iter()
        .find_map(|name| value.get(*name))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}
