//! Runtime repair helpers
//!
//! Keep hard runtime safety here. Provider-specific tool-call argument repair
//! lives in `services::api::tool_call_repair`, where raw model responses are
//! normalized before dispatch.

pub mod rollback;
pub mod storm;
