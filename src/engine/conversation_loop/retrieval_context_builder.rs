use crate::engine::intent_router::RetrievalPolicy;
use crate::engine::retrieval_context::RetrievalContext;
use crate::session_store::SessionStore;
use std::path::Path;
use std::sync::Arc;

pub(super) async fn build_project_retrieval_context(
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

pub(super) async fn build_session_retrieval_context(
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

#[cfg(test)]
mod tests {
    use super::*;

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
