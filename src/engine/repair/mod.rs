//! Tool-call repair pipeline
//!
//! Three-layer repair that runs before tool dispatch:
//! 1. Storm breaker — detect and suppress repeated tool calls
//! 2. Truncation repair — fix malformed JSON from truncated LLM output
//! 3. Git rollback — auto-git-stash before edits, recover on failure

pub mod rollback;
pub mod storm;
pub mod truncation;
