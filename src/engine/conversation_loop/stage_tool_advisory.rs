//! Runtime hint injection for stage-aware tool exposure advisories.

use crate::engine::dynamic_context::{
    tagged_block, DynamicContextBlockBuilder, RECENT_OBSERVATION_TAG,
};
use crate::engine::trace::{TraceCollector, TraceEvent};

pub(super) fn inject_stage_tool_advisory_zone(
    trace: &TraceCollector,
    dynamic_blocks: &mut DynamicContextBlockBuilder,
) {
    let snapshot = trace.snapshot();
    let Some((task_stage, recommended_tools, missing_tools, policy)) =
        snapshot.events.iter().rev().find_map(|event| {
            if let TraceEvent::StageToolExposureAdvisory {
                task_stage,
                recommended_tools,
                missing_tools,
                policy,
                ..
            } = event
            {
                Some((
                    task_stage,
                    recommended_tools,
                    missing_tools,
                    policy.as_str(),
                ))
            } else {
                None
            }
        })
    else {
        return;
    };
    let missing = if missing_tools.is_empty() {
        "none".to_string()
    } else {
        missing_tools.join(",")
    };
    let body = format!(
        "stage_tool_advisory:\n  current_stage={}\n  recommended={}\n  missing={}\n  policy={}; exposed tools were not filtered or auto-added",
        task_stage,
        recommended_tools.join(","),
        missing,
        policy
    );
    if let Some(block) = tagged_block(RECENT_OBSERVATION_TAG, body) {
        dynamic_blocks.push(block);
    }
}
