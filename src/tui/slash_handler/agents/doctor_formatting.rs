pub fn format_prompt_cache_doctor_line(tracker: &crate::cost_tracker::CostTracker) -> String {
    let hit_rate = if tracker.total_tokens.prompt == 0 {
        0.0
    } else {
        tracker.total_tokens.cached.min(tracker.total_tokens.prompt) as f64
            / tracker.total_tokens.prompt as f64
            * 100.0
    };
    let base = format!(
        "requests={} prompt={} cached={} miss={} hit_rate={:.1}%",
        tracker.total_requests,
        tracker.total_tokens.prompt,
        tracker.total_tokens.cached,
        tracker.total_tokens.cache_miss,
        hit_rate
    );
    let Some(last) = tracker.prompt_cache_diagnostics.last() else {
        return format!("{base}; no per-turn cache-shape diagnostics yet");
    };
    format!(
        "{base}; last_reason={} detail={} tools={} tool_fp={} dynamic_zones={} before_last_user={}",
        last.miss_reason,
        last.miss_reason_detail,
        last.tool_count,
        short_hash(&last.tool_schema_fingerprint),
        last.dynamic_zone_messages,
        last.dynamic_zones_before_last_user
    )
}

pub fn short_hash(hash: &str) -> String {
    hash.chars().take(12).collect()
}

pub fn format_provider_status_summary() -> String {
    let mut parts = Vec::new();

    // Provider timeout
    let timeout = crate::services::config::runtime_config()
        .llm_request_timeout()
        .as_secs()
        .to_string();
    parts.push(format!("timeout={}s", timeout));

    // Reconnect attempts
    let reconnect = std::env::var("PRIORITY_AGENT_PROVIDER_RECONNECT_ATTEMPTS")
        .unwrap_or_else(|_| "5".to_string());
    parts.push(format!("reconnect={}", reconnect));

    // Fallback model
    let fallback = crate::services::config::runtime_config()
        .fallback_model()
        .unwrap_or_else(|| "none".to_string());
    parts.push(format!("fallback={}", fallback));

    // Tool profile
    let tool_profile = crate::services::config::runtime_config().tool_profile();
    parts.push(format!("tool_profile={}", tool_profile));

    parts.join(" | ")
}

pub fn format_effective_config_summary() -> String {
    let mut parts = Vec::new();

    // Memory settings
    let write_policy = crate::services::config::runtime_config()
        .auto_memory_write_policy()
        .to_string();
    parts.push(format!("memory_write={}", write_policy));

    let active_memory =
        std::env::var("PRIORITY_AGENT_ACTIVE_MEMORY").unwrap_or_else(|_| "0".to_string());
    parts.push(format!("active_memory={}", active_memory));

    // Route scoped tools
    let route_scoped = if crate::services::config::runtime_config().route_scoped_tools_enabled() {
        "true"
    } else {
        "false"
    };
    parts.push(format!("route_scoped={}", route_scoped));

    // Stream idle timeout
    let stream_idle = crate::services::config::runtime_config()
        .stream_idle_timeout()
        .as_secs()
        .to_string();
    parts.push(format!("stream_idle={}s", stream_idle));

    parts.join(" | ")
}

pub fn exposure_label(report: &crate::engine::tool_exposure::ToolExposureReport) -> &'static str {
    if report.model_exposed {
        "exposed"
    } else {
        "hidden"
    }
}

pub fn format_terminal_bash_exposure(
    report: &crate::engine::tool_exposure::ToolExposureReport,
) -> String {
    let scope = if report.route_scoped_tools {
        "route_scoped=on"
    } else {
        "route_scoped=off"
    };
    let schema = if report.provider_schema_compatible {
        "schema=ok".to_string()
    } else {
        format!(
            "schema=bad: {}",
            report
                .provider_schema_reason
                .as_deref()
                .unwrap_or("unknown schema issue")
        )
    };
    if report.model_exposed {
        format!("exposed for terminal requests ({}, {})", scope, schema)
    } else {
        format!(
            "hidden for terminal requests: {} ({}, {})",
            report.hidden_reason.as_deref().unwrap_or("unknown reason"),
            scope,
            schema
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductReadiness {
    pub ready: bool,
    pub label: &'static str,
    pub status: crate::diagnostics::CheckStatus,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
}

impl ProductReadiness {
    pub fn to_check_result(&self) -> crate::diagnostics::CheckResult {
        match self.status {
            crate::diagnostics::CheckStatus::Ok => crate::diagnostics::CheckResult::ok(
                "product_ready",
                "READY: install and runtime can run coding sessions",
            ),
            crate::diagnostics::CheckStatus::Warning => crate::diagnostics::CheckResult::warn(
                "product_ready",
                format!("USABLE_WITH_WARNINGS: {} warning(s)", self.warnings.len()),
                self.warnings.join("; "),
            ),
            crate::diagnostics::CheckStatus::Error => crate::diagnostics::CheckResult::error(
                "product_ready",
                format!("BLOCKED: {} blocker(s)", self.blockers.len()),
                self.blockers.join("; "),
            ),
            crate::diagnostics::CheckStatus::Info => {
                crate::diagnostics::CheckResult::info("product_ready", self.label)
            }
        }
    }

    pub fn format_text(&self) -> String {
        let mut lines = vec![
            "Product Readiness".to_string(),
            format!("Status: {}", self.label),
        ];
        if self.blockers.is_empty() {
            lines.push("Blockers: none".to_string());
        } else {
            lines.push(format!("Blockers: {}", self.blockers.join("; ")));
        }
        if self.warnings.is_empty() {
            lines.push("Warnings: none".to_string());
        } else {
            lines.push(format!("Warnings: {}", self.warnings.join("; ")));
        }
        lines.join("\n")
    }
}

pub fn evaluate_product_readiness(
    report: &crate::diagnostics::DiagnosticReport,
    runtime: &crate::state::RuntimeStatusSnapshot,
) -> ProductReadiness {
    let blockers = report
        .checks
        .iter()
        .filter(|check| check.status == crate::diagnostics::CheckStatus::Error)
        .map(|check| format!("{}: {}", check.name, check.message))
        .collect::<Vec<_>>();
    let mut warnings = report
        .checks
        .iter()
        .filter(|check| check.status == crate::diagnostics::CheckStatus::Warning)
        .map(|check| format!("{}: {}", check.name, check.message))
        .collect::<Vec<_>>();

    if runtime.failed_tool_count > 0 {
        warnings.push(format!(
            "runtime tools failed={}",
            runtime.failed_tool_count
        ));
    }
    if runtime.backgrounded_tool_count > 0 {
        warnings.push(format!(
            "backgrounded tools={}",
            runtime.backgrounded_tool_count
        ));
    }
    if let Some(pending) = runtime.pending_permission.as_ref() {
        warnings.push(format!("approval pending: {}", pending));
    }
    if !runtime.mcp_repair_hints.is_empty() {
        warnings.push(format!(
            "mcp repair: {}",
            runtime.mcp_repair_hints.join(", ")
        ));
    }

    let (ready, label, status) = if !blockers.is_empty() {
        (false, "BLOCKED", crate::diagnostics::CheckStatus::Error)
    } else if !warnings.is_empty() {
        (
            false,
            "USABLE_WITH_WARNINGS",
            crate::diagnostics::CheckStatus::Warning,
        )
    } else {
        (true, "READY", crate::diagnostics::CheckStatus::Ok)
    };

    ProductReadiness {
        ready,
        label,
        status,
        blockers,
        warnings,
    }
}
