//! Helper predicates and renderers for code-change workflow state.
//!
//! These helpers keep workflow state formatting and route checks separate from
//! the controller that owns runtime transitions.

use super::{PlanStepRuntimeState, PlanStepRuntimeStatus, StageValidationRecord};
use crate::engine::intent_router::WorkflowKind;
use crate::engine::task_context::TaskContextBundle;
use std::path::Path;

pub fn is_programming_workflow(workflow: WorkflowKind) -> bool {
    matches!(workflow, WorkflowKind::CodeChange | WorkflowKind::BugFix)
}

pub(super) fn route_allows_no_diff_closeout(reason: &str) -> bool {
    let lower = reason.to_ascii_lowercase();
    lower.contains("code diff is optional")
        || lower.contains("audit/regression")
        || lower.contains("already satisfied")
}

pub(super) fn runtime_validation_label_passed(label: Option<&str>) -> bool {
    let Some(label) = label else {
        return false;
    };
    let lower = label.trim().to_ascii_lowercase();
    lower.starts_with("passed:") && !lower.starts_with("passed:0/")
}

pub(super) fn append_bullets(out: &mut String, items: &[String]) {
    if items.is_empty() {
        out.push_str("  - none\n");
    } else {
        for item in items {
            out.push_str(&format!("  - {}\n", item));
        }
    }
}

pub(super) fn step_states_from_bundle(bundle: &TaskContextBundle) -> Vec<PlanStepRuntimeState> {
    bundle
        .workflow_judgment
        .as_ref()
        .map(|judgment| {
            judgment
                .sorted_plan()
                .into_iter()
                .map(|step| PlanStepRuntimeState {
                    id: step.id,
                    description: step.description,
                    status: PlanStepRuntimeStatus::Pending,
                    priority: format!("{:?}", step.priority),
                    last_evidence: None,
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn push_unique(items: &mut Vec<String>, value: String) {
    if !value.trim().is_empty() && !items.contains(&value) {
        items.push(value);
    }
}

pub(super) fn append_reason(current: &str, addition: &str) -> String {
    if current.contains(addition) {
        current.to_string()
    } else if current.trim().is_empty() {
        addition.to_string()
    } else {
        format!("{}; {}", current, addition)
    }
}

pub(super) fn preview(text: &str, max_chars: usize) -> String {
    let mut out = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

pub(super) fn validation_evidence_summary(evidence: &str) -> Option<String> {
    let trimmed = evidence.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("[Changed-file diff evidence]")
        || trimmed.starts_with("[Code review]")
    {
        return None;
    }

    if trimmed.starts_with("[Manual validation passed after code changes]") {
        let commands = trimmed
            .lines()
            .filter_map(|line| line.trim().strip_prefix("$ "))
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        if !commands.is_empty() {
            return Some(preview(&format!("manual: {}", commands.join("; ")), 140));
        }
    }

    let normalized = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    let summary = normalized
        .split_once("] ")
        .map(|(_, rest)| rest)
        .unwrap_or(normalized.as_str());
    Some(preview(summary.trim_end_matches('.'), 160))
}

pub(super) fn select_validation_evidence(record: &StageValidationRecord) -> Option<String> {
    let usable = record
        .evidence
        .iter()
        .filter(|item| validation_evidence_summary(item).is_some())
        .collect::<Vec<_>>();
    if usable.is_empty() {
        return record.evidence.first().cloned();
    }

    let changed_labels = record
        .changed_files
        .iter()
        .flat_map(|path| {
            let file_name = Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string);
            [Some(path.clone()), file_name]
        })
        .flatten()
        .collect::<Vec<_>>();
    if let Some(matched) = usable.iter().find(|item| {
        changed_labels
            .iter()
            .any(|label| !label.is_empty() && item.contains(label))
    }) {
        return Some((*matched).clone());
    }

    usable.first().map(|item| (*item).clone())
}
