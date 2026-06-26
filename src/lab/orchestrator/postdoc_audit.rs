//! Postdoc audit and proof collection helpers.

use super::artifact_factory::compact_result_preview;
use super::*;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Default)]
pub(super) struct PostdocWorktreeProof {
    pub(super) accepted_results: Vec<String>,
    pub(super) remaining_risks: Vec<String>,
    pub(super) evidence_refs: Vec<String>,
}

const MAX_POSTDOC_AUDIT_FILE_BYTES: u64 = 256 * 1024;
const MAX_POSTDOC_AUDIT_DIFF_BYTES: usize = 512 * 1024;

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

pub(super) fn collect_postdoc_read_only_audit_proof(
    store: &LabStore,
    run: &LabRun,
    graduate_results: &[LabArtifactEnvelope<GraduateResult>],
) -> anyhow::Result<PostdocWorktreeProof> {
    let mut proof = PostdocWorktreeProof::default();
    let mut audited_results = Vec::new();
    let mut inspected_paths = Vec::new();
    let mut diff_summaries = Vec::new();
    let mut file_snippets = Vec::new();
    let mut audit_risks = Vec::new();
    let mut validation_event_refs = Vec::new();
    for result in graduate_results {
        audited_results.push(result.artifact_id.clone());
        let result_validation_refs =
            postdoc_validation_event_refs_for_result(store, &run.lab_run_id, result)?;
        if result_validation_refs.is_empty() {
            audit_risks.push(format!(
                "{} audit risk: no task-bound Lab validation command events found",
                result.artifact_id
            ));
        }
        validation_event_refs.extend(result_validation_refs);
        let artifact_path = store
            .root()
            .join("runs")
            .join(&run.lab_run_id)
            .join("artifacts")
            .join(format!("{}.json", result.artifact_id));
        if result.body.changed_files.is_empty() {
            audit_risks.push(format!(
                "{} audit risk: no changed_files to inspect",
                result.artifact_id
            ));
        }
        if result.body.validation_attempts.is_empty() {
            audit_risks.push(format!(
                "{} audit risk: no validation attempts to inspect",
                result.artifact_id
            ));
        }
        for changed_file in &result.body.changed_files {
            match crate::lab::path_scope::normalize_lab_relative_path(changed_file) {
                Ok(path) => {
                    let audit_root = postdoc_audit_root_for_result(store, result);
                    let audit_path = audit_root.join(&path);
                    let exists_in_audit_root = audit_path.exists();
                    inspected_paths.push(serde_json::json!({
                        "artifact_id": result.artifact_id,
                        "path": path.clone(),
                        "audit_root": audit_root.display().to_string(),
                        "verification_root": result.body.provenance.verification_root,
                        "exists_in_audit_root": exists_in_audit_root,
                        "graduate_result_artifact": artifact_path.display().to_string(),
                    }));
                    if !exists_in_audit_root {
                        audit_risks.push(format!(
                            "{} audit risk: {} is not present in audit root {}",
                            result.artifact_id,
                            changed_file,
                            audit_root.display()
                        ));
                    } else {
                        file_snippets.push(audit_file_snippet_payload(
                            &result.artifact_id,
                            &path,
                            &audit_path,
                        ));
                    }
                    if let Some(diff_capture) = git_diff_for_path(&audit_root, &path) {
                        diff_summaries.push(audit_diff_summary_payload(
                            &result.artifact_id,
                            &path,
                            &diff_capture,
                        ));
                    } else {
                        audit_risks.push(format!(
                            "{} audit risk: no git diff evidence available for {} in audit root {}",
                            result.artifact_id,
                            changed_file,
                            audit_root.display()
                        ));
                    }
                }
                Err(err) => {
                    audit_risks.push(format!(
                        "{} audit risk: invalid changed file path: {}",
                        result.artifact_id, err
                    ));
                }
            }
        }
    }
    validation_event_refs.sort();
    validation_event_refs.dedup();
    let audit_status = if graduate_results.is_empty() {
        "postdoc_audit_not_verified"
    } else if audit_risks.iter().any(|risk| risk.contains("risk:")) {
        "postdoc_audit_needs_revision"
    } else {
        "postdoc_audit_verified"
    };
    let audit_id = format!("postdoc_audit_{}", Uuid::new_v4().simple());
    let audit_dir = store
        .root()
        .join("runs")
        .join(&run.lab_run_id)
        .join("postdoc_audits");
    std::fs::create_dir_all(&audit_dir)?;
    let audit_path = audit_dir.join(format!("{audit_id}.json"));
    let payload = serde_json::json!({
        "schema_version": crate::lab::model::LAB_SCHEMA_VERSION,
        "audit_id": audit_id,
        "lab_run_id": run.lab_run_id,
        "cycle_id": run.cycle_count.to_string(),
        "role": "postdoc",
        "mode": "read_only",
        "audit_status": audit_status,
        "audited_results": audited_results,
        "graduate_result_artifact_ids": graduate_results
            .iter()
            .map(|result| result.artifact_id.as_str())
            .collect::<Vec<_>>(),
        "inspected_paths": inspected_paths,
        "changed_files_inspected": graduate_results
            .iter()
            .flat_map(|result| result.body.changed_files.iter().map(String::as_str))
            .collect::<Vec<_>>(),
        "diff_summaries": diff_summaries,
        "file_snippets": file_snippets,
        "validation_event_refs": validation_event_refs,
        "audit_binding": "graduate_result_provenance",
        "risks": audit_risks.clone(),
        "forbidden_actions": ["file_write", "file_edit", "file_patch", "arbitrary_shell"],
    });
    std::fs::write(&audit_path, serde_json::to_vec_pretty(&payload)?)?;
    proof.evidence_refs.push(audit_path.display().to_string());
    proof.accepted_results.push(format!(
        "postdoc read-only audit {} reviewed {} graduate result artifact(s)",
        audit_status,
        graduate_results.len()
    ));
    proof.remaining_risks.extend(audit_risks);
    store.record_run_event(
        &run.lab_run_id,
        "postdoc_read_only_audit_written",
        serde_json::json!({
            "audit_path": audit_path.display().to_string(),
            "graduate_result_count": graduate_results.len(),
            "audit_status": audit_status,
        }),
    )?;
    Ok(proof)
}

fn postdoc_validation_event_refs_for_result(
    store: &LabStore,
    lab_run_id: &str,
    result: &LabArtifactEnvelope<GraduateResult>,
) -> anyhow::Result<Vec<String>> {
    let provenance = &result.body.provenance;
    let wanted_ids = provenance
        .validation_event_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut refs = store
        .list_run_events(lab_run_id)?
        .into_iter()
        .filter(|event| {
            matches!(
                event.event_type.as_str(),
                "lab_validation_command_passed"
                    | "lab_validation_command_failed"
                    | "lab_validation_command_blocked"
            )
        })
        .filter(|event| {
            if !wanted_ids.is_empty() {
                return wanted_ids.contains(&event.event_id)
                    && validation_event_matches_provenance(event, provenance);
            }
            validation_event_matches_provenance(event, provenance)
        })
        .map(|event| format!("event:{}", event.event_id))
        .collect::<Vec<_>>();
    refs.sort();
    refs.dedup();
    Ok(refs)
}

fn validation_event_matches_provenance(
    event: &LabEvent,
    provenance: &LabEvidenceProvenance,
) -> bool {
    if let Some(task_id) = provenance.graduate_task_id.as_deref() {
        let event_task_id = event
            .payload
            .get("graduate_task_id")
            .or_else(|| event.payload.get("task_id"))
            .and_then(Value::as_str);
        if event_task_id != Some(task_id) {
            return false;
        }
    }
    if let Some(dispatch_id) = provenance.dispatch_id.as_deref() {
        if event.payload.get("dispatch_id").and_then(Value::as_str) != Some(dispatch_id) {
            return false;
        }
    }
    if let Some(root) = provenance.verification_root.as_deref() {
        if event
            .payload
            .get("verification_root")
            .and_then(Value::as_str)
            != Some(root)
        {
            return false;
        }
    }
    true
}

fn postdoc_audit_root_for_result(
    store: &LabStore,
    result: &LabArtifactEnvelope<GraduateResult>,
) -> PathBuf {
    result
        .body
        .provenance
        .verification_root
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| store.project_root().to_path_buf())
}

fn audit_file_snippet_payload(artifact_id: &str, path: &str, parent_path: &Path) -> Value {
    let metadata = match std::fs::metadata(parent_path) {
        Ok(metadata) => metadata,
        Err(err) => {
            return serde_json::json!({
                "artifact_id": artifact_id,
                "path": path,
                "snippet": "unreadable",
                "read_error": err.to_string(),
                "redaction_applied": false,
                "redaction_reasons": [],
            });
        }
    };
    let byte_len = metadata.len();
    if let Some(reason) = crate::lab::audit_redaction::sensitive_audit_path_reason(path) {
        return serde_json::json!({
            "artifact_id": artifact_id,
            "path": path,
            "snippet_redacted": true,
            "redaction_applied": true,
            "redaction_reasons": [reason],
            "content_hash": audit_file_hash(parent_path).ok(),
            "byte_len": byte_len,
        });
    }
    if let Some(reason) = crate::lab::audit_redaction::bulky_audit_path_reason(path) {
        return serde_json::json!({
            "artifact_id": artifact_id,
            "path": path,
            "snippet_omitted": true,
            "audit_omission_reason": reason,
            "redaction_applied": false,
            "redaction_reasons": [],
            "content_hash": audit_file_hash(parent_path).ok(),
            "byte_len": byte_len,
        });
    }
    if byte_len > MAX_POSTDOC_AUDIT_FILE_BYTES {
        return serde_json::json!({
            "artifact_id": artifact_id,
            "path": path,
            "snippet_omitted": true,
            "audit_omission_reason": "omitted_large_file",
            "redaction_applied": false,
            "redaction_reasons": [],
            "content_hash": audit_file_hash(parent_path).ok(),
            "byte_len": byte_len,
            "max_audit_file_bytes": MAX_POSTDOC_AUDIT_FILE_BYTES,
        });
    }
    let bytes = match std::fs::read(parent_path) {
        Ok(bytes) => bytes,
        Err(err) => {
            return serde_json::json!({
                "artifact_id": artifact_id,
                "path": path,
                "snippet": "unreadable",
                "read_error": err.to_string(),
                "redaction_applied": false,
                "redaction_reasons": [],
                "byte_len": byte_len,
            });
        }
    };
    let content = String::from_utf8_lossy(&bytes);
    let redacted = crate::lab::audit_redaction::redact_lab_audit_text(&content);
    serde_json::json!({
        "artifact_id": artifact_id,
        "path": path,
        "snippet": compact_result_preview(&redacted.text, 1600),
        "redaction_applied": redacted.redaction_applied,
        "redaction_reasons": redacted.redaction_reasons,
        "byte_len": byte_len,
    })
}

fn audit_diff_summary_payload(
    artifact_id: &str,
    path: &str,
    diff_capture: &crate::lab::audit_redaction::AuditCapturedBytes,
) -> Value {
    if let Some(reason) = crate::lab::audit_redaction::sensitive_audit_path_reason(path) {
        return serde_json::json!({
            "artifact_id": artifact_id,
            "path": path,
            "diff_redacted": true,
            "redaction_applied": true,
            "redaction_reasons": [reason],
            "diff_hash": diff_capture.content_hash,
            "byte_len": diff_capture.byte_len,
            "diff_truncated": diff_capture.truncated,
        });
    }
    if let Some(reason) = crate::lab::audit_redaction::bulky_audit_path_reason(path) {
        return serde_json::json!({
            "artifact_id": artifact_id,
            "path": path,
            "diff_omitted": true,
            "audit_omission_reason": reason,
            "redaction_applied": false,
            "redaction_reasons": [],
            "diff_hash": diff_capture.content_hash,
            "byte_len": diff_capture.byte_len,
            "diff_truncated": diff_capture.truncated,
        });
    }
    if diff_capture.truncated {
        let preview = String::from_utf8_lossy(&diff_capture.preview);
        let redacted = crate::lab::audit_redaction::redact_lab_audit_text(&preview);
        return serde_json::json!({
            "artifact_id": artifact_id,
            "path": path,
            "diff_omitted": true,
            "audit_omission_reason": "omitted_large_diff",
            "summary_preview": compact_result_preview(&redacted.text, 1600),
            "redaction_applied": redacted.redaction_applied,
            "redaction_reasons": redacted.redaction_reasons,
            "diff_hash": diff_capture.content_hash,
            "byte_len": diff_capture.byte_len,
            "diff_truncated": true,
            "max_audit_diff_bytes": MAX_POSTDOC_AUDIT_DIFF_BYTES,
        });
    }
    let diff_text = String::from_utf8_lossy(&diff_capture.preview);
    let redacted = crate::lab::audit_redaction::redact_lab_audit_text(&diff_text);
    serde_json::json!({
        "artifact_id": artifact_id,
        "path": path,
        "summary": compact_result_preview(&redacted.text, 1600),
        "redaction_applied": redacted.redaction_applied,
        "redaction_reasons": redacted.redaction_reasons,
        "diff_hash": diff_capture.content_hash,
        "byte_len": diff_capture.byte_len,
        "diff_truncated": false,
    })
}

fn audit_file_hash(path: &Path) -> std::io::Result<String> {
    let file = std::fs::File::open(path)?;
    crate::lab::audit_redaction::audit_reader_hash(file)
}

fn git_diff_for_path(
    project_root: &Path,
    path: &str,
) -> Option<crate::lab::audit_redaction::AuditCapturedBytes> {
    let mut child = Command::new("git")
        .args(["diff", "--no-ext-diff", "--", path])
        .current_dir(project_root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let captured =
        crate::lab::audit_redaction::capture_reader_with_hash(stdout, MAX_POSTDOC_AUDIT_DIFF_BYTES)
            .ok()?;
    let status = child.wait().ok()?;
    if !status.success() || captured.byte_len == 0 {
        return None;
    }
    if String::from_utf8_lossy(&captured.preview).trim().is_empty() {
        return None;
    }
    Some(captured)
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
