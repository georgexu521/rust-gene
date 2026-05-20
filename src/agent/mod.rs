//! Agent 系统模块
//!
//! 实现子 Agent 的创建、管理和通信

// 允许 module inception，因为 agent 模块下的 agent.rs 是核心实体文件
#![allow(clippy::module_inception)]

pub mod a2a_transcript;
pub mod agent;
pub mod envelope;
pub mod forked_context;
pub mod manager;
pub mod memory;
pub mod profiles;
pub mod roles;
pub mod types;
pub mod verification_agent;

pub use manager::AgentManager;
