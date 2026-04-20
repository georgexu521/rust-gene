//! Priority Core Library
//!
//! This crate is intended to hold core components (Engine, Tools, Agent, etc.)
//! for sharing across multiple frontends.
//!
//! Currently empty - the actual source code is in the main `priority-agent` crate.
//! This workspace member establishes the project structure for future extraction.

/// Core engine components
pub mod engine {
    // TODO: Extract from src/engine
    // use crate::engine::*;
}

/// Tool implementations
pub mod tools {
    // TODO: Extract from src/tools
}

/// Agent system
pub mod agent {
    // TODO: Extract from src/agent
}

/// LLM Provider abstractions
pub mod services {
    // TODO: Extract from src/services
}
