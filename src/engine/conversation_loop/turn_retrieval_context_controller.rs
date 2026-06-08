use crate::engine::intent_router::RetrievalPolicy;
use crate::engine::retrieval_context::RetrievalContext;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::memory::active::{
    run_active_memory_worker, ActiveMemoryConfig, ActiveMemoryEnvironment, ActiveMemoryRequest,
};
use crate::memory::MemoryManager;
use crate::services::api::LlmProvider;
use crate::session_store::SessionStore;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

async fn build_project_retrieval_context(
    query: &str,
    working_dir: &Path,
    policy: RetrievalPolicy,
) -> Option<RetrievalContext> {
    if !policy.allows_project_context() {
        return None;
    }
    let root = working_dir.to_path_buf();
    let query = query.to_string();
    tokio::task::spawn_blocking(move || {
        let mut scanner = crate::tools::project_tool::ProjectScanner::new();
        scanner.scan(&root);
        RetrievalContext::from_project_summary(&query, scanner.tree_summary(), &root, policy)
    })
    .await
    .ok()
    .flatten()
}

async fn build_session_retrieval_context(
    query: &str,
    store: Option<Arc<SessionStore>>,
    policy: RetrievalPolicy,
) -> Option<RetrievalContext> {
    if !policy.allows_memory_context() {
        return None;
    }
    let store = store?;
    let query = fts_phrase_query(query);
    if query.trim().is_empty() {
        return None;
    }
    tokio::task::spawn_blocking(move || {
        store
            .search_messages(&query, 4)
            .ok()
            .and_then(|messages| RetrievalContext::from_session_messages(&query, &messages, policy))
    })
    .await
    .ok()
    .flatten()
}

fn fts_phrase_query(query: &str) -> String {
    let compact = query
        .chars()
        .filter(|ch| !ch.is_control())
        .take(160)
        .collect::<String>()
        .replace('"', "\"\"");
    if compact.trim().is_empty() {
        String::new()
    } else {
        format!("\"{}\"", compact)
    }
}

pub(super) struct TurnRetrievalContextRequest<'a> {
    pub(super) last_user_preview: &'a str,
    pub(super) working_dir: &'a Path,
    pub(super) retrieval_policy: RetrievalPolicy,
    pub(super) session_store: Option<Arc<SessionStore>>,
    pub(super) session_id: Option<&'a str>,
    pub(super) memory_manager: Option<&'a Arc<Mutex<MemoryManager>>>,
    pub(super) memory_recall_mode: &'a str,
    pub(super) provider: &'a dyn LlmProvider,
    pub(super) model: &'a str,
    pub(super) trace: &'a TraceCollector,
}

pub(super) struct TurnRetrievalContextController;

impl TurnRetrievalContextController {
    pub(super) async fn build(
        context: TurnRetrievalContextRequest<'_>,
    ) -> Option<RetrievalContext> {
        let mut turn_retrieval_context = build_project_retrieval_context(
            context.last_user_preview,
            context.working_dir,
            context.retrieval_policy,
        )
        .await;

        if let Some(session_ctx) = build_session_retrieval_context(
            context.last_user_preview,
            context.session_store.clone(),
            context.retrieval_policy,
        )
        .await
        {
            Self::merge_context(&mut turn_retrieval_context, session_ctx);
        }

        if context.retrieval_policy.allows_memory_context() {
            if let Some(memory_ctx) = Self::build_memory_context(&context).await {
                Self::record_memory_prefetch(context.trace, &memory_ctx);
                Self::merge_context(&mut turn_retrieval_context, memory_ctx);
            }
            if let Some(active_memory_ctx) = Self::build_active_memory_context(&context).await {
                Self::merge_context(&mut turn_retrieval_context, active_memory_ctx);
            }
        }

        apply_memory_recall_mode(&mut turn_retrieval_context, context.memory_recall_mode);

        if let Some(ref ctx) = turn_retrieval_context {
            Self::record_context_built(context.trace, ctx);
        }

        turn_retrieval_context
    }

    async fn build_memory_context(
        context: &TurnRetrievalContextRequest<'_>,
    ) -> Option<RetrievalContext> {
        let memory_manager = context.memory_manager?;
        let mut memory = memory_manager.lock().await;
        memory.reset_turn();
        let ctx = memory
            .prefetch_retrieval_context_with_llm_rerank(
                context.last_user_preview,
                context.provider,
                context.model,
                context.retrieval_policy,
            )
            .await?;
        if ctx.items.is_empty() {
            None
        } else {
            Some(ctx)
        }
    }

    async fn build_active_memory_context(
        context: &TurnRetrievalContextRequest<'_>,
    ) -> Option<RetrievalContext> {
        let config = ActiveMemoryConfig::from_env();
        let request = ActiveMemoryRequest {
            query: context.last_user_preview,
            retrieval_policy: context.retrieval_policy,
            session_id: context.session_id,
            memory_enabled: context.memory_manager.is_some(),
            user_facing: true,
            timeout_budget_available: config.timeout.as_millis() > 0,
            environment: ActiveMemoryEnvironment::from_process(),
        };
        let Some(memory_manager) = context.memory_manager else {
            let gate = crate::memory::active::evaluate_active_memory_gate(request, config);
            context.trace.record(TraceEvent::ActiveMemoryEvaluated {
                status: "skipped".to_string(),
                reason: gate.reason,
                items: 0,
                timeout_ms: config.timeout.as_millis() as u64,
                elapsed_ms: 0,
            });
            return None;
        };

        let memory = memory_manager.lock().await;
        let outcome = run_active_memory_worker(&memory, request, config).await;
        context.trace.record(TraceEvent::ActiveMemoryEvaluated {
            status: outcome.status.clone(),
            reason: outcome.reason.clone(),
            items: outcome.items,
            timeout_ms: outcome.timeout_ms,
            elapsed_ms: outcome.elapsed_ms,
        });
        outcome.context
    }

    fn merge_context(
        turn_retrieval_context: &mut Option<RetrievalContext>,
        next_context: RetrievalContext,
    ) {
        if let Some(ctx) = turn_retrieval_context {
            ctx.extend(next_context);
        } else {
            *turn_retrieval_context = Some(next_context);
        }
    }

    fn record_memory_prefetch(trace: &TraceCollector, context: &RetrievalContext) {
        trace.record(TraceEvent::MemoryPrefetch {
            chars: context
                .items
                .iter()
                .map(|item| item.content_preview.chars().count())
                .sum(),
        });
    }

    fn record_context_built(trace: &TraceCollector, context: &RetrievalContext) {
        trace.record(TraceEvent::RetrievalContextBuilt {
            policy: format!("{:?}", context.policy),
            sources: context
                .items
                .iter()
                .map(|item| format!("{:?}", item.source))
                .collect(),
            items: context.items.len(),
            estimated_tokens: context.token_estimate,
            provenance: context.provenance_summaries(),
            conflicts: context.conflict_count(),
        });
    }
}

fn apply_memory_recall_mode(context: &mut Option<RetrievalContext>, mode: &str) {
    let Some(ctx) = context.as_mut() else {
        return;
    };
    match mode.trim().to_ascii_lowercase().as_str() {
        "strict" => {
            ctx.items.retain(|item| item.score >= 0.70);
        }
        "preference-only" | "preference_only" | "preferences" => {
            ctx.items.retain(is_preference_memory_item);
        }
        _ => {}
    }
    ctx.token_estimate = ctx.items.iter().map(|item| item.token_estimate).sum();
    if ctx.items.is_empty() {
        *context = None;
    }
}

fn is_preference_memory_item(item: &crate::engine::retrieval_context::RetrievalItem) -> bool {
    if item.source != crate::engine::retrieval_context::RetrievalSource::Memory {
        return false;
    }
    let title = item.title.to_ascii_lowercase();
    let provenance = item.provenance.to_ascii_lowercase();
    let content = item.content_preview.to_ascii_lowercase();
    title.contains("preference")
        || title.contains("collaboration style")
        || provenance.contains("user")
        || content.contains("preference")
        || content.contains("collaboration style")
        || item.content_preview.contains("偏好")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::retrieval_context::{RetrievalItem, RetrievalSource, TrustLevel};
    use crate::engine::trace::{TurnStatus, TurnTrace};
    use crate::services::api::{ChatRequest, ChatResponse};
    use async_openai::types::ChatCompletionResponseStream;

    struct MockProvider;

    #[async_trait::async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            Err(anyhow::anyhow!("chat not used in this test"))
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            Err(anyhow::anyhow!("stream not used in this test"))
        }

        fn base_url(&self) -> &str {
            "mock://local"
        }

        fn default_model(&self) -> &str {
            "mock-model"
        }
    }

    #[test]
    fn merge_context_adds_items_without_replacing_existing_context() {
        let mut project_context = RetrievalContext::from_project_summary(
            "fix bug",
            "src/main.rs\nsrc/lib.rs",
            "/tmp/project",
            RetrievalPolicy::Project,
        );
        let memory_context = RetrievalContext::from_memory_prefetch(
            "fix bug",
            "run cargo test after changes",
            RetrievalPolicy::Project,
        )
        .expect("memory context");

        TurnRetrievalContextController::merge_context(&mut project_context, memory_context);

        let context = project_context.expect("merged context");
        assert_eq!(context.item_count_by_source(RetrievalSource::Project), 1);
        assert_eq!(context.item_count_by_source(RetrievalSource::Memory), 1);
        assert_eq!(context.items.len(), 2);
    }

    #[test]
    fn preference_only_recall_keeps_only_preference_memory_items() {
        let mut context = Some(RetrievalContext::new(
            "preferences",
            RetrievalPolicy::Project,
        ));
        let ctx = context.as_mut().expect("context");
        ctx.add_item(RetrievalItem::new(
            RetrievalSource::Project,
            "Project index summary",
            "src/main.rs",
            0.90,
            "project.index:/tmp/repo",
            TrustLevel::High,
        ));
        ctx.add_item(RetrievalItem::new(
            RetrievalSource::Session,
            "Session previous user",
            "Temporary debugging note",
            0.80,
            "session.message:s1:1",
            TrustLevel::Medium,
        ));
        ctx.add_item(RetrievalItem::new(
            RetrievalSource::Memory,
            "Build habit",
            "Run cargo check after edits",
            0.85,
            "memory.match:MEMORY.md:score=90",
            TrustLevel::Medium,
        ));
        ctx.add_item(RetrievalItem::new(
            RetrievalSource::Memory,
            "User preference",
            "User preference: keep final answers concise",
            0.82,
            "memory.match:USER.md:score=88",
            TrustLevel::High,
        ));

        apply_memory_recall_mode(&mut context, "preference-only");

        let context = context.expect("preference memory should remain");
        assert_eq!(context.items.len(), 1);
        assert_eq!(context.items[0].title, "User preference");
        assert_eq!(context.item_count_by_source(RetrievalSource::Project), 0);
        assert_eq!(context.item_count_by_source(RetrievalSource::Session), 0);
    }

    #[test]
    fn strict_recall_filters_all_low_confidence_sources() {
        let mut context = Some(RetrievalContext::new("strict", RetrievalPolicy::Project));
        let ctx = context.as_mut().expect("context");
        ctx.add_item(RetrievalItem::new(
            RetrievalSource::Project,
            "Project index summary",
            "src/main.rs",
            0.75,
            "project.index:/tmp/repo",
            TrustLevel::High,
        ));
        ctx.add_item(RetrievalItem::new(
            RetrievalSource::Memory,
            "Weak memory",
            "Maybe related",
            0.40,
            "memory.match:MEMORY.md:score=40",
            TrustLevel::Medium,
        ));

        apply_memory_recall_mode(&mut context, "strict");

        let context = context.expect("strict context should retain high-confidence project item");
        assert_eq!(context.items.len(), 1);
        assert_eq!(context.items[0].title, "Project index summary");
    }

    #[test]
    fn record_context_built_preserves_trace_fields() {
        let trace = TraceCollector::new(TurnTrace::new("session-test", 1, "fix bug"));
        let context = RetrievalContext::from_project_summary(
            "fix bug",
            "src/main.rs\nsrc/lib.rs",
            "/tmp/project",
            RetrievalPolicy::Project,
        )
        .expect("project context");

        TurnRetrievalContextController::record_context_built(&trace, &context);

        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| {
            matches!(
                event,
                TraceEvent::RetrievalContextBuilt {
                    policy,
                    sources,
                    items: 1,
                    estimated_tokens,
                    provenance,
                    conflicts: 0,
                } if policy == "Project"
                    && sources == &vec!["Project".to_string()]
                    && *estimated_tokens > 0
                    && provenance.len() == 1
            )
        }));
    }

    #[tokio::test]
    async fn build_skips_sources_when_retrieval_policy_is_none() {
        let provider = MockProvider;
        let trace = TraceCollector::new(TurnTrace::new("session-test", 1, "hello"));

        let context = TurnRetrievalContextController::build(TurnRetrievalContextRequest {
            last_user_preview: "hello",
            working_dir: Path::new("/tmp"),
            retrieval_policy: RetrievalPolicy::None,
            session_store: None,
            session_id: Some("session-test"),
            memory_manager: None,
            memory_recall_mode: "balanced",
            provider: &provider,
            model: "mock-model",
            trace: &trace,
        })
        .await;

        assert!(context.is_none());
        let finished = trace.finish(TurnStatus::Completed);
        assert!(!finished
            .events
            .iter()
            .any(|event| matches!(event, TraceEvent::RetrievalContextBuilt { .. })));
    }

    #[test]
    fn fts_phrase_query_strips_controls_and_escapes_quotes() {
        assert_eq!(
            fts_phrase_query("hello\u{0} \"world\""),
            "\"hello \"\"world\"\"\""
        );
    }

    #[test]
    fn fts_phrase_query_returns_empty_for_blank_control_only_query() {
        assert_eq!(fts_phrase_query("\u{0}\n\t"), "");
    }
}
