//! Experimental priority model crate.
//!
//! This crate currently contains task weighting and priority-model types shared
//! by the main Priority Agent crate. Runtime engine, tools, providers, and
//! agent orchestration still live in the root crate until those contracts
//! stabilize.

pub mod errors;
pub mod weight_engine;
