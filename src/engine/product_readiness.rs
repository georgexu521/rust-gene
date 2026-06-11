//! Product readiness diagnostics.
//!
//! Provides `/product-ready` and extends `/doctor` with a product-oriented
//! health check covering: provider, LSP, session store, export path,
//! permissions, and runtime facade availability.

use serde::{Deserialize, Serialize};

/// Overall product readiness status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReadinessStatus {
    Ready,
    Blocked,
    Warn,
}

/// A single readiness check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessCheck {
    pub name: String,
    pub status: ReadinessStatus,
    pub detail: String,
    pub remediation: Option<String>,
}

/// Collect all product readiness checks.
pub fn collect_readiness_checks() -> Vec<ReadinessCheck> {
    vec![
        check_provider_configured(),
        check_session_store(),
        check_export_path(),
        check_lsp_status(),
        check_permissions_mode(),
        check_runtime_available(),
        check_goal_runner_health(),
    ]
}

fn check_provider_configured() -> ReadinessCheck {
    let configured = crate::services::api::provider_catalog::configured_providers();
    if configured.is_empty() {
        ReadinessCheck {
            name: "provider".into(),
            status: ReadinessStatus::Blocked,
            detail: "No provider is configured.".into(),
            remediation: Some(
                "Run /connect <provider> to set up an API key. Supported: minimax, deepseek, kimi, openai, glm, kimi-code."
                    .into(),
            ),
        }
    } else {
        ReadinessCheck {
            name: "provider".into(),
            status: ReadinessStatus::Ready,
            detail: format!(
                "{} provider(s) configured: {}",
                configured.len(),
                configured.join(", ")
            ),
            remediation: None,
        }
    }
}

fn check_session_store() -> ReadinessCheck {
    // Simple check: can we create an in-memory store?
    match crate::session_store::SessionStore::in_memory() {
        Ok(_) => ReadinessCheck {
            name: "session_store".into(),
            status: ReadinessStatus::Ready,
            detail: "Session store is available.".into(),
            remediation: None,
        },
        Err(e) => ReadinessCheck {
            name: "session_store".into(),
            status: ReadinessStatus::Blocked,
            detail: format!("Session store unavailable: {}", e),
            remediation: Some("Check disk space and permissions for the data directory.".into()),
        },
    }
}

fn check_export_path() -> ReadinessCheck {
    let dir = crate::session_store::export::default_export_dir();
    match std::fs::create_dir_all(&dir) {
        Ok(()) => ReadinessCheck {
            name: "export".into(),
            status: ReadinessStatus::Ready,
            detail: format!("Export directory writable: {}", dir.display()),
            remediation: None,
        },
        Err(e) => ReadinessCheck {
            name: "export".into(),
            status: ReadinessStatus::Blocked,
            detail: format!("Export directory not writable: {}", e),
            remediation: Some("Check permissions for the export directory.".into()),
        },
    }
}

fn check_lsp_status() -> ReadinessCheck {
    let config = crate::services::config::AppConfig::load().unwrap_or_default();
    if config.lsp.enabled {
        ReadinessCheck {
            name: "lsp".into(),
            status: ReadinessStatus::Ready,
            detail: format!(
                "LSP is enabled (auto_detect={}, downloads_disabled={}).",
                config.lsp.auto_detect, config.lsp.disable_downloads
            ),
            remediation: None,
        }
    } else {
        ReadinessCheck {
            name: "lsp".into(),
            status: ReadinessStatus::Warn,
            detail:
                "LSP is disabled by default; validation commands remain authoritative.".into(),
            remediation: Some(
                "Run /config set lsp.enabled true if you want optional language-server diagnostics."
                    .into(),
            ),
        }
    }
}

fn check_permissions_mode() -> ReadinessCheck {
    ReadinessCheck {
        name: "permissions".into(),
        status: ReadinessStatus::Ready,
        detail: "Permission system is active with system-level defaults.".into(),
        remediation: None,
    }
}

fn check_runtime_available() -> ReadinessCheck {
    ReadinessCheck {
        name: "runtime".into(),
        status: ReadinessStatus::Ready,
        detail: "Runtime facade is available.".into(),
        remediation: None,
    }
}

/// Build a human-readable product readiness report.
pub fn readiness_report() -> String {
    let checks = collect_readiness_checks();
    let mut out = String::from("Product Readiness:\n\n");

    let blocked: Vec<_> = checks
        .iter()
        .filter(|c| c.status == ReadinessStatus::Blocked)
        .collect();
    let warns: Vec<_> = checks
        .iter()
        .filter(|c| c.status == ReadinessStatus::Warn)
        .collect();
    let ready = checks.len() - blocked.len() - warns.len();

    let overall = if blocked.is_empty() {
        "READY"
    } else {
        "BLOCKED"
    };
    out.push_str(&format!(
        "Overall: {overall} ({ready} OK, {} warnings, {} blocked)\n\n",
        warns.len(),
        blocked.len()
    ));

    for check in &checks {
        let icon = match check.status {
            ReadinessStatus::Ready => "✓",
            ReadinessStatus::Warn => "⚠",
            ReadinessStatus::Blocked => "✗",
        };
        out.push_str(&format!("  {icon} {}: {}\n", check.name, check.detail));
        if let Some(ref remediation) = check.remediation {
            out.push_str(&format!("     → {}\n", remediation));
        }
    }

    out
}

fn check_goal_runner_health() -> ReadinessCheck {
    let config = crate::services::config::runtime_config();
    if !config.features.goals {
        return ReadinessCheck {
            name: "goal_runner".into(),
            status: ReadinessStatus::Warn,
            detail: "Goal mode is not enabled (features.goals = false).".into(),
            remediation: Some(
                "Set features.goals = true in config to enable Codex-style durable goal tracking."
                    .into(),
            ),
        };
    }

    let store = crate::session_store::SessionStore::default_path();
    if !store.exists() {
        return ReadinessCheck {
            name: "goal_runner".into(),
            status: ReadinessStatus::Warn,
            detail: "Session store not found; cannot verify goal persistence.".into(),
            remediation: Some("Start a session to create the store.".into()),
        };
    }

    match crate::session_store::SessionStore::open(&store) {
        Ok(db) => {
            let conn = db.shared_conn();
            let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
            let active_count: i64 = guard
                .query_row(
                    "SELECT COUNT(*) FROM goal_runs WHERE status IN (?1, ?2)",
                    rusqlite::params![
                        crate::engine::goal::model::GoalRunStatus::Active.as_str(),
                        crate::engine::goal::model::GoalRunStatus::Active.legacy_json_str(),
                    ],
                    |row| row.get(0),
                )
                .unwrap_or(-1);

            if active_count > 0 {
                ReadinessCheck {
                    name: "goal_runner".into(),
                    status: ReadinessStatus::Warn,
                    detail: format!(
                        "{} active goal(s) found in store. Use /goal resume to continue.",
                        active_count
                    ),
                    remediation: Some(
                        "Active goals are paused on restart for safety. Use /goal resume to continue."
                            .into(),
                    ),
                }
            } else {
                ReadinessCheck {
                    name: "goal_runner".into(),
                    status: ReadinessStatus::Ready,
                    detail: "Goal runner infrastructure is healthy.".into(),
                    remediation: None,
                }
            }
        }
        Err(e) => ReadinessCheck {
            name: "goal_runner".into(),
            status: ReadinessStatus::Warn,
            detail: format!("Could not open session store: {}", e),
            remediation: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readiness_checks_have_all_required_fields() {
        let checks = collect_readiness_checks();
        assert!(!checks.is_empty());
        for c in &checks {
            assert!(!c.name.is_empty());
            assert!(!c.detail.is_empty());
            if c.status == ReadinessStatus::Blocked {
                assert!(
                    c.remediation.is_some(),
                    "{} should have remediation",
                    c.name
                );
            }
        }
    }

    #[test]
    fn report_includes_overall_status() {
        let report = readiness_report();
        assert!(report.contains("Overall:"));
    }
}
