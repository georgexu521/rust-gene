use super::*;

pub(super) async fn handle_sponsor_message_classify_command(
    project_root: &Path,
    args: &str,
    tool_context: ToolContext,
) -> String {
    let (message_id, instructions) = split_once(args);
    let message_id = message_id.trim();
    if message_id.is_empty() {
        return "Usage: /lab messages classify <message_id|latest> [instructions]".to_string();
    }
    let Some(provider) = tool_context.llm_provider.clone() else {
        return "Usage: /lab messages classify <message_id|latest> [instructions] requires an active Lab Mode provider."
            .to_string();
    };
    if tool_context.model.trim().is_empty() {
        return "Usage: /lab messages classify <message_id|latest> [instructions] requires an active model."
            .to_string();
    }
    match crate::lab::draft::classify_sponsor_message_with_provider(
        project_root,
        provider,
        tool_context.model,
        message_id,
        instructions,
    )
    .await
    {
        Ok(outcome) => format!(
            "Professor classified sponsor message: {}\nDecision: {}\nStatus: {:?}\nNote: {}",
            outcome.message.message_id, outcome.decision, outcome.message.status, outcome.note
        ),
        Err(err) => format!(
            "Failed to classify professor side-channel message: {}",
            format_error_chain(&err)
        ),
    }
}

pub(super) fn handle_sponsor_messages_command(
    orchestrator: &LabOrchestrator,
    store: &LabStore,
    args: &str,
) -> String {
    let (action, rest) = split_once(args);
    match action {
        "" | "list" | "messages" => list_sponsor_messages(store),
        "review" | "reviewed" => {
            update_sponsor_message_status(store, rest, SponsorMessageStatus::Reviewed, "review")
        }
        "meeting" => update_sponsor_message_status(
            store,
            rest,
            SponsorMessageStatus::ConvertedToMeeting,
            "meeting",
        ),
        "task" => update_sponsor_message_status(
            store,
            rest,
            SponsorMessageStatus::ConvertedToTask,
            "task",
        ),
        "reject" | "rejected" => {
            update_sponsor_message_status(store, rest, SponsorMessageStatus::Rejected, "reject")
        }
        "decision" | "decide" => render_sponsor_message_decision(store, rest),
        "classify" => {
            "Usage: /lab messages classify <message_id|latest> [instructions] requires the Lab Mode shell provider."
                .to_string()
        }
        "apply" => apply_sponsor_message(orchestrator, store, rest),
        _ => "Usage: /lab messages [list|classify|decision|review|meeting|task|reject|apply <message_id> [note]]"
            .to_string(),
    }
}

fn list_sponsor_messages(store: &LabStore) -> String {
    let run = match store.latest_run() {
        Ok(Some(run)) => run,
        Ok(None) => return "No LabRun found.".to_string(),
        Err(err) => return format!("Failed to load LabRun: {err}"),
    };
    match store.list_sponsor_messages(&run.lab_run_id) {
        Ok(messages) if messages.is_empty() => {
            format!(
                "Professor side-channel inbox is empty for {}.",
                run.lab_run_id
            )
        }
        Ok(messages) => {
            let mut lines = vec![
                format!("Professor side-channel inbox: {}", run.lab_run_id),
                format!("Messages: {}", messages.len()),
            ];
            for message in messages.iter().rev().take(10) {
                lines.push(format!(
                    "- {} [{:?}/{:?}/{}] {}",
                    message.message_id,
                    message.message_type,
                    message.status,
                    message.urgency,
                    compact_message_line(&message.body, 160)
                ));
            }
            lines.join("\n")
        }
        Err(err) => format!("Failed to list professor side-channel messages: {err}"),
    }
}

fn render_sponsor_message_decision(store: &LabStore, args: &str) -> String {
    let message_id = args.trim();
    if message_id.is_empty() {
        return "Usage: /lab messages decision <message_id|latest>".to_string();
    }
    let run = match store.latest_run() {
        Ok(Some(run)) => run,
        Ok(None) => return "No LabRun found.".to_string(),
        Err(err) => return format!("Failed to load LabRun: {err}"),
    };
    let messages = match store.list_sponsor_messages(&run.lab_run_id) {
        Ok(messages) => messages,
        Err(err) => return format!("Failed to list professor side-channel messages: {err}"),
    };
    let Some(message) = select_sponsor_message(&messages, message_id) else {
        return format!("Professor side-channel message not found: {message_id}");
    };
    let (decision, next_action) = sponsor_message_decision_label(message.status);
    let artifact =
        build_professor_steering_decision_artifact(&run.lab_run_id, message, decision, next_action);
    let artifact_id = artifact.artifact_id().to_string();
    let report_path = match store
        .write_stage_artifact(&artifact)
        .and_then(|_| store.write_stage_artifact_report(&artifact))
    {
        Ok(path) => path,
        Err(err) => return format!("Failed to write Professor steering decision: {err}"),
    };
    [
        format!("Professor steering decision: {}", artifact_id),
        format!("Source message: {}", message.message_id),
        format!("Decision: {decision}"),
        format!("Status: {:?}", message.status),
        format!("Type: {:?}", message.message_type),
        format!("Urgency: {}", message.urgency),
        format!("Next action: {next_action}"),
        format!("Report: {}", report_path.display()),
        format!("Message: {}", compact_message_line(&message.body, 240)),
    ]
    .join("\n")
}

fn build_professor_steering_decision_artifact(
    lab_run_id: &str,
    message: &crate::lab::model::SponsorMessage,
    decision: &str,
    next_action: &str,
) -> StageArtifact {
    let decision_id = format!("profsteer_{}", Uuid::new_v4().simple());
    let artifact_id = format!(
        "artifact_professorsteeringdecision_{}",
        Uuid::new_v4().simple()
    );
    let mut artifact = StageArtifact::ProfessorSteeringDecision(LabArtifactEnvelope::new(
        artifact_id,
        lab_run_id.to_string(),
        LabArtifactType::ProfessorSteeringDecision,
        format!("Professor steering decision for {}", message.message_id),
        Utc::now(),
        ProfessorSteeringDecision {
            decision_id,
            source_message_id: message.message_id.clone(),
            decision: decision.to_string(),
            status: message.status,
            message_type: message.message_type,
            urgency: message.urgency.clone(),
            rationale: sponsor_message_decision_rationale(message.status).to_string(),
            next_action: next_action.to_string(),
            message_summary: compact_message_line(&message.body, 240),
        },
    ));
    artifact.set_review_state(
        LabArtifactStatus::ReadyForHandoff,
        Some("decision_recorded_not_applied".to_string()),
    );
    artifact
}

fn sponsor_message_decision_rationale(status: SponsorMessageStatus) -> &'static str {
    match status {
        SponsorMessageStatus::Queued => "Professor has not classified this message yet.",
        SponsorMessageStatus::Reviewed => {
            "Professor reviewed the message and found no workflow mutation pending."
        }
        SponsorMessageStatus::ConvertedToMeeting => {
            "Professor classified the message as a meeting-worthy topic."
        }
        SponsorMessageStatus::ConvertedToTask => {
            "Professor classified the message as a follow-up implementation task."
        }
        SponsorMessageStatus::Applied => "The steering decision has already been applied.",
        SponsorMessageStatus::Rejected => "Professor rejected the message for LabRun action.",
        SponsorMessageStatus::Superseded => "A newer message or decision superseded this one.",
    }
}

fn sponsor_message_decision_label(status: SponsorMessageStatus) -> (&'static str, &'static str) {
    match status {
        SponsorMessageStatus::Queued => (
            "pending_professor_review",
            "Classify with /lab messages classify <message_id> or mark review/meeting/task/reject.",
        ),
        SponsorMessageStatus::Reviewed => (
            "no_change",
            "No LabRun state change is pending; current loop can continue.",
        ),
        SponsorMessageStatus::ConvertedToMeeting => (
            "open_lab_meeting",
            "Apply with /lab messages apply <message_id> to write a read-only meeting report.",
        ),
        SponsorMessageStatus::ConvertedToTask => (
            "create_postdoc_task",
            "Apply with /lab messages apply <message_id> to create a blocked task for postdoc scoping.",
        ),
        SponsorMessageStatus::Applied => (
            "applied",
            "Decision has already been applied; inspect reports or tasks for resulting artifacts.",
        ),
        SponsorMessageStatus::Rejected => (
            "reject",
            "No LabRun action should be taken from this message.",
        ),
        SponsorMessageStatus::Superseded => (
            "superseded",
            "A newer sponsor message or decision replaced this one.",
        ),
    }
}

fn select_sponsor_message<'a>(
    messages: &'a [crate::lab::model::SponsorMessage],
    message_id: &str,
) -> Option<&'a crate::lab::model::SponsorMessage> {
    if message_id == "latest" {
        return messages.iter().next_back();
    }
    messages
        .iter()
        .find(|message| message.message_id == message_id)
}

fn update_sponsor_message_status(
    store: &LabStore,
    args: &str,
    status: SponsorMessageStatus,
    action: &str,
) -> String {
    let (message_id, note) = split_once(args);
    if message_id.trim().is_empty() {
        return format!("Usage: /lab messages {action} <message_id> [note]");
    }
    match store.update_latest_sponsor_message_status(message_id, status, note) {
        Ok(message) => format!(
            "Professor side-channel message updated: {}\nStatus: {:?}\nType: {:?}",
            message.message_id, message.status, message.message_type
        ),
        Err(err) => format!("Failed to update professor side-channel message: {err}"),
    }
}

fn apply_sponsor_message(orchestrator: &LabOrchestrator, store: &LabStore, args: &str) -> String {
    let (message_id, note) = split_once(args);
    let message_id = message_id.trim();
    if message_id.is_empty() {
        return "Usage: /lab messages apply <message_id> [note]".to_string();
    }
    let run = match store.latest_run() {
        Ok(Some(run)) => run,
        Ok(None) => return "No LabRun found.".to_string(),
        Err(err) => return format!("Failed to load LabRun: {err}"),
    };
    let messages = match store.list_sponsor_messages(&run.lab_run_id) {
        Ok(messages) => messages,
        Err(err) => return format!("Failed to list professor side-channel messages: {err}"),
    };
    let Some(message) = select_sponsor_message(&messages, message_id) else {
        return format!("Professor side-channel message not found: {message_id}");
    };

    match message.status {
        SponsorMessageStatus::ConvertedToMeeting => {
            let topic = if note.trim().is_empty() {
                format!("Sponsor message {}: {}", message.message_id, message.body)
            } else {
                note.trim().to_string()
            };
            match orchestrator.create_meeting_summary_for_latest(Some(&topic)) {
                Ok(created) => {
                    let _ = store.update_latest_sponsor_message_status(
                        &message.message_id,
                        SponsorMessageStatus::Applied,
                        &format!("meeting_artifact={}", created.artifact.artifact_id()),
                    );
                    format!(
                        "Professor side-channel message applied as meeting: {}\nArtifact: {}\nReport: {}",
                        message.message_id,
                        created.artifact.artifact_id(),
                        created.report_path.display()
                    )
                }
                Err(err) => format!("Failed to apply professor message as meeting: {err}"),
            }
        }
        SponsorMessageStatus::ConvertedToTask => {
            let title = if note.trim().is_empty() {
                compact_message_line(&message.body, 80)
            } else {
                note.trim().to_string()
            };
            match store.create_graduate_task(
                &run.lab_run_id,
                &title,
                &format!(
                    "Sponsor-requested task from professor side channel {}: {}",
                    message.message_id, message.body
                ),
                Vec::new(),
                vec!["Postdoc must set allowed_scope before graduate execution.".to_string()],
            ) {
                Ok(task) => {
                    if let Err(err) = store.block_graduate_task(
                        &run.lab_run_id,
                        &task.task_id,
                        "Converted from sponsor message; postdoc must assign allowed_scope before execution.",
                    ) {
                        return format!("Failed to block converted graduate task: {err}");
                    }
                    let _ = store.update_latest_sponsor_message_status(
                        &message.message_id,
                        SponsorMessageStatus::Applied,
                        &format!("graduate_task={}", task.task_id),
                    );
                    format!(
                        "Professor side-channel message applied as blocked graduate task: {}\nTask: {}\nReason: postdoc must assign allowed_scope before execution",
                        message.message_id, task.task_id
                    )
                }
                Err(err) => format!("Failed to apply professor message as task: {err}"),
            }
        }
        other => format!(
            "Professor side-channel message {} is {:?}; mark it as meeting or task before apply.",
            message.message_id, other
        ),
    }
}
