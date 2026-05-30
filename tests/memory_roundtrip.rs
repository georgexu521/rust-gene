//! Integration test: memory write → flush → search roundtrip.
//!
//! Validates the core memory pipeline without requiring an LLM provider.

use priority_agent::memory::manager::MemoryManager;

mod common;

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pa-int-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn memory_search_finds_content_in_memory_file() {
    let base = temp_dir("mem-search");
    // Write content directly to MEMORY.md to test the retrieval path.
    std::fs::write(
        base.join("MEMORY.md"),
        "## Rust Lifetimes\nUse the 'a syntax for lifetime annotations.\n\n## TUI Design\nUses ratatui for rendering.\n",
    )
    .unwrap();

    let mgr = MemoryManager::with_base_dir(base.clone());
    // Freeze so the content is captured in the snapshot.
    let mut mgr = mgr;
    mgr.freeze_snapshot();

    let results = mgr.search("lifetimes");
    assert!(
        results.iter().any(|r| r.contains("lifetime")),
        "search for 'lifetimes' should find results: {:?}",
        results
    );

    let results = mgr.search("ratatui");
    assert!(
        results.iter().any(|r| r.contains("ratatui")),
        "search for 'ratatui' should find results: {:?}",
        results
    );

    // Tiered search.
    let project = mgr.search_tier(
        "lifetimes",
        priority_agent::memory::reports::MemoryTier::Project,
    );
    assert!(!project.is_empty(), "project tier search should work");

    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn memory_prefetch_finds_relevant_context() {
    let base = temp_dir("mem-prefetch");
    std::fs::write(
        base.join("MEMORY.md"),
        "## Language Preference\nUser prefers Chinese for UI messages.\n",
    )
    .unwrap();

    let mut mgr = MemoryManager::with_base_dir(base.clone());
    mgr.freeze_snapshot();

    let ctx = mgr.prefetch("language preference");
    assert!(
        ctx.contains("Chinese") || ctx.contains("UI messages"),
        "prefetch should find the preference: '{}'",
        ctx
    );

    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn memory_summary_reflects_memory_file() {
    let base = temp_dir("mem-summary");
    std::fs::write(
        base.join("MEMORY.md"),
        "# Project Memory\nConvention: use snake_case.\n",
    )
    .unwrap();

    let mut mgr = MemoryManager::with_base_dir(base.clone());
    mgr.freeze_snapshot();

    let summary = mgr.memory_summary();
    assert!(
        summary.project_memory_chars > 0,
        "MEMORY.md content should be reflected in summary"
    );

    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn memory_search_no_match_returns_empty() {
    let base = temp_dir("mem-nomatch");
    std::fs::write(base.join("MEMORY.md"), "## Notes\nGeneral notes.\n").unwrap();

    let mut mgr = MemoryManager::with_base_dir(base.clone());
    mgr.freeze_snapshot();

    let results = mgr.search("nonexistent_term_xyzzy");
    assert!(results.is_empty(), "should return empty for no match");

    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn memory_user_tier_uses_user_md() {
    let base = temp_dir("mem-user");
    std::fs::write(
        base.join("USER.md"),
        "language: chinese\nprefer: compact output\n",
    )
    .unwrap();

    let mut mgr = MemoryManager::with_base_dir(base.clone());
    mgr.freeze_snapshot();

    let results = mgr.search_tier(
        "language",
        priority_agent::memory::reports::MemoryTier::User,
    );
    assert!(
        !results.is_empty(),
        "user tier search should find USER.md content"
    );

    let _ = std::fs::remove_dir_all(base);
}
