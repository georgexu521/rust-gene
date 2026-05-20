use crate::engine::destructive_scope::DestructiveScopeContract;
use crate::engine::intent_router::IntentRoute;
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::streaming::StreamEvent;
use crate::engine::trace::TraceCollector;
use crate::tools::ToolContextRetainedContext;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

#[derive(Clone, Copy)]
pub(super) struct TurnRuntimeContext<'a> {
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) trace: &'a TraceCollector,
    pub(super) route: &'a IntentRoute,
    pub(super) resource_policy: &'a ResourcePolicy,
    pub(super) exposed_tool_names: &'a HashSet<String>,
    pub(super) working_dir: &'a Path,
    pub(super) last_user_preview: &'a str,
    pub(super) required_validation_commands: &'a [String],
    pub(super) destructive_scope: &'a DestructiveScopeContract,
    pub(super) baseline_git_status_files: &'a HashSet<PathBuf>,
    pub(super) retained_context: &'a ToolContextRetainedContext,
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct SessionRuntimeState {
    pub(super) companion_context_keys: HashSet<String>,
    pub(super) failed_tool_fingerprints: HashMap<String, usize>,
    pub(super) failed_tool_names: HashMap<String, usize>,
    pub(super) successful_required_validation_commands: HashSet<String>,
}

impl SessionRuntimeState {
    #[cfg(test)]
    pub(super) fn new() -> Self {
        Self::default()
    }
}

impl Default for SessionRuntimeState {
    fn default() -> Self {
        Self {
            companion_context_keys: HashSet::new(),
            failed_tool_fingerprints: HashMap::new(),
            failed_tool_names: HashMap::new(),
            successful_required_validation_commands: HashSet::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::destructive_scope::DestructiveScopeContract;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::resource_policy::ResourcePolicy;
    use crate::engine::trace::{TraceCollector, TurnTrace};

    #[test]
    fn session_runtime_state_starts_empty() {
        let state = SessionRuntimeState::new();

        assert!(state.companion_context_keys.is_empty());
        assert!(state.failed_tool_fingerprints.is_empty());
        assert!(state.failed_tool_names.is_empty());
        assert!(state.successful_required_validation_commands.is_empty());
    }

    #[test]
    fn turn_runtime_context_groups_per_turn_refs() {
        let route = IntentRouter::new().route("inspect files");
        let policy = ResourcePolicy::from_route(&route);
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "inspect files"));
        let destructive_scope =
            DestructiveScopeContract::from_user_request("inspect files", Path::new("."));
        let exposed = HashSet::from(["grep".to_string()]);
        let baseline = HashSet::new();
        let required = vec!["cargo check -q".to_string()];
        let retained_context = ToolContextRetainedContext::default();
        let context = TurnRuntimeContext {
            tx: None,
            trace: &trace,
            route: &route,
            resource_policy: &policy,
            exposed_tool_names: &exposed,
            working_dir: Path::new("."),
            last_user_preview: "inspect files",
            required_validation_commands: &required,
            destructive_scope: &destructive_scope,
            baseline_git_status_files: &baseline,
            retained_context: &retained_context,
        };

        assert_eq!(context.exposed_tool_names.len(), 1);
        assert_eq!(context.required_validation_commands[0], "cargo check -q");
        assert_eq!(context.last_user_preview, "inspect files");
        assert!(context.retained_context.is_empty());
    }
}
