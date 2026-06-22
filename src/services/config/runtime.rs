//! Runtime configuration service helpers.
//!
//! Collects configuration values used by service startup without owning provider or session behavior.

use super::{AppConfig, ConfigError};

impl AppConfig {
    /// Turn execution timeout as Duration.
    ///
    /// Backward compatibility: `PRIORITY_AGENT_TURN_TIMEOUT_SECS` still
    /// overrides the config file field during the migration window.
    pub fn turn_timeout(&self) -> std::time::Duration {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_TURN_TIMEOUT_SECS") {
            if let Ok(secs) = raw.trim().parse::<u64>() {
                return std::time::Duration::from_secs(secs.clamp(60, 7200));
            }
        }
        std::time::Duration::from_secs(self.engine.turn_timeout_secs.clamp(60, 7200))
    }

    /// Session-end memory flush timeout as Duration.
    ///
    /// Backward compatibility: `PRIORITY_AGENT_SESSION_END_MEMORY_FLUSH_TIMEOUT_SECS`
    /// still overrides the config file field during the migration window.
    pub fn session_end_memory_flush_timeout(&self) -> std::time::Duration {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_SESSION_END_MEMORY_FLUSH_TIMEOUT_SECS") {
            if let Ok(secs) = raw.trim().parse::<u64>() {
                return std::time::Duration::from_secs(secs.clamp(1, 60));
            }
        }
        std::time::Duration::from_secs(
            self.engine
                .session_end_memory_flush_timeout_secs
                .clamp(1, 60),
        )
    }

    /// LLM request timeout as Duration (backward compat: checks PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS).
    pub fn llm_request_timeout(&self) -> std::time::Duration {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS") {
            if let Ok(secs) = raw.trim().parse::<u64>() {
                return std::time::Duration::from_secs(secs.clamp(30, 600));
            }
        }
        std::time::Duration::from_secs(self.engine.llm_request_timeout_secs.max(10))
    }

    /// Stream idle timeout as Duration (backward compat: checks PRIORITY_AGENT_STREAM_IDLE_TIMEOUT_SECS).
    pub fn stream_idle_timeout(&self) -> std::time::Duration {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_STREAM_IDLE_TIMEOUT_SECS") {
            if let Ok(secs) = raw.trim().parse::<u64>() {
                return std::time::Duration::from_secs(secs.clamp(30, 600));
            }
        }
        std::time::Duration::from_secs(self.engine.stream_idle_timeout_secs.max(5))
    }

    /// Explicit LLM request timeout from env var (for provider profile override).
    /// Returns None if the env var is not set; callers should fall back to provider-specific timeout.
    pub fn explicit_llm_request_timeout(&self) -> Option<std::time::Duration> {
        std::env::var("PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS")
            .ok()
            .and_then(|raw| raw.trim().parse::<u64>().ok())
            .map(|secs| std::time::Duration::from_secs(secs.clamp(30, 600)))
    }

    /// Fallback model name (backward compat: checks PRIORITY_AGENT_FALLBACK_MODEL).
    pub fn fallback_model(&self) -> Option<String> {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_FALLBACK_MODEL") {
            let v = raw.trim().to_string();
            if !v.is_empty() && !v.eq_ignore_ascii_case("none") {
                return Some(v);
            }
        }
        self.engine.fallback_model.clone()
    }

    /// Runtime profile (backward compat: checks PRIORITY_AGENT_RUNTIME_PROFILE).
    pub fn runtime_profile(&self) -> String {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_RUNTIME_PROFILE") {
            let v = raw.trim().to_ascii_lowercase();
            if !v.is_empty() {
                return v;
            }
        }
        self.engine.runtime_profile.clone()
    }

    /// Whether self-correction is enabled (backward compat: checks PRIORITY_AGENT_SELF_CORRECTION).
    pub fn self_correction_enabled(&self) -> bool {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_SELF_CORRECTION") {
            let v = raw.trim().to_ascii_lowercase();
            if v == "0" || v == "false" || v == "no" || v == "off" {
                return false;
            }
            return true;
        }
        self.engine.self_correction_enabled
    }

    /// Closeout visibility.
    pub fn closeout_visibility(&self) -> &str {
        &self.engine.closeout_visibility
    }

    /// Approval timeout as Duration (backward compat: checks PRIORITY_AGENT_APPROVAL_TIMEOUT_SECS).
    pub fn approval_timeout(&self) -> std::time::Duration {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_APPROVAL_TIMEOUT_SECS") {
            if let Ok(secs) = raw.trim().parse::<u64>() {
                return std::time::Duration::from_secs(secs.clamp(30, 1800));
            }
        }
        std::time::Duration::from_secs(self.engine.approval_timeout_secs.clamp(30, 1800))
    }

    /// Closeout background stage timeout as Duration (backward compat: checks PRIORITY_AGENT_CLOSEOUT_BACKGROUND_TIMEOUT_SECS).
    pub fn closeout_background_timeout(&self) -> std::time::Duration {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_CLOSEOUT_BACKGROUND_TIMEOUT_SECS") {
            if let Ok(secs) = raw.trim().parse::<u64>() {
                return std::time::Duration::from_secs(secs.clamp(1, 60));
            }
        }
        std::time::Duration::from_secs(self.engine.closeout_background_timeout_secs.clamp(1, 60))
    }

    /// Whether patch synthesis is enabled (backward compat: checks PRIORITY_AGENT_PATCH_SYNTHESIS).
    pub fn patch_synthesis_enabled(&self) -> bool {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_PATCH_SYNTHESIS") {
            let v = raw.trim().to_ascii_lowercase();
            if v == "0" || v == "false" || v == "no" {
                return false;
            }
            return true;
        }
        self.engine.patch_synthesis_enabled
    }

    /// Whether deterministic patch synthesis is enabled (backward compat: checks PRIORITY_AGENT_DETERMINISTIC_PATCH_SYNTHESIS).
    pub fn deterministic_patch_synthesis_enabled(&self) -> bool {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_DETERMINISTIC_PATCH_SYNTHESIS") {
            let v = raw.trim().to_ascii_lowercase();
            if v == "0" || v == "false" || v == "no" {
                return false;
            }
            return true;
        }
        self.engine.deterministic_patch_synthesis_enabled
    }

    /// Streaming tool execution shadow mode (backward compat: checks PRIORITY_AGENT_STREAMING_TOOL_EXECUTION).
    pub fn streaming_tool_execution_shadow(&self) -> Option<String> {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_STREAMING_TOOL_EXECUTION") {
            let v = raw.trim().to_ascii_lowercase();
            if v == "shadow" {
                return Some("shadow".to_string());
            }
            return None;
        }
        if self.engine.streaming_tool_execution == "shadow" {
            Some("shadow".to_string())
        } else {
            None
        }
    }

    /// Auto memory write policy (backward compat: checks PRIORITY_AGENT_AUTO_MEMORY_WRITE).
    pub fn auto_memory_write_policy(&self) -> &str {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_AUTO_MEMORY_WRITE") {
            let v = raw.trim().to_ascii_lowercase();
            if v == "legacy" || v == "unsafe" || v == "all" || v == "1" || v == "true" || v == "on"
            {
                return "legacy";
            }
            if v == "narrow" || v == "verified" || v == "explicit" {
                return "narrow";
            }
            return "review_only";
        }
        &self.engine.auto_memory_write
    }

    /// Memory dialectic depth (backward compat: checks PRIORITY_AGENT_MEMORY_DIALECTIC_DEPTH).
    pub fn memory_dialectic_depth(&self) -> usize {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_MEMORY_DIALECTIC_DEPTH") {
            if let Ok(n) = raw.trim().parse::<usize>() {
                return n;
            }
        }
        self.engine.memory_dialectic_depth
    }

    /// Required validation timeout (backward compat: checks PRIORITY_AGENT_REQUIRED_VALIDATION_TIMEOUT_SECS).
    pub fn required_validation_timeout(&self) -> Option<std::time::Duration> {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_REQUIRED_VALIDATION_TIMEOUT_SECS") {
            let v = raw.trim().to_ascii_lowercase();
            if v.is_empty()
                || v == "0"
                || v == "none"
                || v == "off"
                || v == "false"
                || v == "unlimited"
            {
                return None;
            }
            if let Ok(secs) = v.parse::<u64>() {
                return Some(std::time::Duration::from_secs(secs.max(30)));
            }
        }
        self.engine
            .required_validation_timeout_secs
            .map(|secs| std::time::Duration::from_secs(secs.max(30)))
    }

    /// Whether tool dispatch should be forced serial (backward compat: checks PRIORITY_AGENT_TOOL_DISPATCH).
    pub fn force_serial_tool_dispatch(&self) -> bool {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_TOOL_DISPATCH") {
            return raw.trim().eq_ignore_ascii_case("serial");
        }
        self.engine.tool_dispatch_serial
    }

    /// Read-only tool concurrency (backward compat: checks PRIORITY_AGENT_READ_ONLY_TOOL_CONCURRENCY).
    pub fn read_only_tool_concurrency(&self) -> usize {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_READ_ONLY_TOOL_CONCURRENCY") {
            if let Ok(n) = raw.trim().parse::<usize>() {
                if n > 0 {
                    return n;
                }
            }
        }
        self.engine.read_only_tool_concurrency.max(1)
    }

    /// Tool profile (backward compat: checks PRIORITY_AGENT_TOOL_PROFILE).
    pub fn tool_profile(&self) -> String {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_TOOL_PROFILE") {
            return raw.trim().to_ascii_lowercase();
        }
        self.engine.tool_profile.clone()
    }

    /// Workflow contract mode string (backward compat: checks PRIORITY_AGENT_WORKFLOW_CONTRACT).
    pub fn workflow_contract(&self) -> String {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_WORKFLOW_CONTRACT") {
            return raw.trim().to_ascii_lowercase();
        }
        self.engine.workflow_contract.clone()
    }

    /// Whether task guidance is enabled (backward compat: checks PRIORITY_AGENT_TASK_GUIDANCE).
    pub fn task_guidance_enabled(&self) -> bool {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_TASK_GUIDANCE") {
            let v = raw.trim().to_ascii_lowercase();
            return v == "1" || v == "true" || v == "yes" || v == "on";
        }
        self.engine.task_guidance_enabled
    }

    /// Whether the runtime profile is MVA (minimum viable agent).
    pub fn is_mva_profile(&self) -> bool {
        matches!(
            self.runtime_profile().as_str(),
            "minimum_viable_agent" | "mva"
        )
    }

    /// Whether the runtime profile enables structured closeout.
    pub fn is_structured_closeout_profile(&self) -> bool {
        matches!(
            self.runtime_profile().as_str(),
            "minimum_viable_agent" | "mva" | "project_partner_alignment"
        )
    }

    /// Whether route-scoped tools are enabled (backward compat: checks PRIORITY_AGENT_ROUTE_SCOPED_TOOLS).
    pub fn route_scoped_tools_enabled(&self) -> bool {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS") {
            let v = raw.trim().to_ascii_lowercase();
            if v == "0" || v == "false" || v == "no" || v == "off" {
                return false;
            }
            return true;
        }
        self.engine.route_scoped_tools_enabled
    }

    /// Whether debug tool exposure is enabled (backward compat: checks PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE).
    pub fn debug_tool_exposure_enabled(&self) -> bool {
        if let Ok(raw) = std::env::var("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE") {
            let v = raw.trim().to_ascii_lowercase();
            return v == "1" || v == "true" || v == "yes" || v == "on";
        }
        self.engine.debug_tool_exposure
    }
}

static RUNTIME_CONFIG: std::sync::OnceLock<std::sync::RwLock<AppConfig>> =
    std::sync::OnceLock::new();

fn runtime_config_lock() -> &'static std::sync::RwLock<AppConfig> {
    RUNTIME_CONFIG.get_or_init(|| {
        let config = AppConfig::load().unwrap_or_else(|err| {
            tracing::warn!("Failed to load AppConfig, using defaults: {}", err);
            AppConfig::default()
        });
        std::sync::RwLock::new(config)
    })
}

/// Get the current global runtime configuration snapshot.
pub fn runtime_config() -> AppConfig {
    runtime_config_lock()
        .read()
        .unwrap_or_else(|err| err.into_inner())
        .clone()
}

/// Replace the runtime configuration snapshot.
pub fn init_runtime_config(config: AppConfig) {
    *runtime_config_lock()
        .write()
        .unwrap_or_else(|err| err.into_inner()) = config;
}

/// Reload the runtime configuration from disk/env and replace the snapshot.
pub fn reload_runtime_config() -> Result<AppConfig, ConfigError> {
    let config = AppConfig::load()?;
    init_runtime_config(config.clone());
    Ok(config)
}
