//! Plugin metadata discovery (MVP)
//!
//! This module provides a minimal plugin manifest loader so the agent can
//! discover installed plugins before full execution/injection support lands.

pub mod trust;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TuiSlot {
    SidebarTitle,
    SidebarFooter,
    StatusBar,
    MessageBeforeSend,
    ToolCard,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginTuiContribution {
    #[serde(default)]
    pub slots: Vec<TuiSlot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub entry_command: Option<String>,
    #[serde(default)]
    pub entry_args: Vec<String>,
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub tool_description: Option<String>,
    #[serde(default)]
    pub tool_timeout_secs: Option<u64>,
    #[serde(default)]
    pub signature: Option<String>,
    #[serde(default)]
    pub public_key: Option<String>,
    #[serde(default)]
    pub tui: PluginTuiContribution,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub id: String,
    pub source_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest: PluginManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginValidationIssue {
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginRuntimeStatus {
    Ready,
    Disabled,
    UsableWithWarnings,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRuntimeFacts {
    pub id: String,
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub status: PluginRuntimeStatus,
    pub source_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub entry_command: Option<String>,
    pub entry_args: Vec<String>,
    pub tool_name: Option<String>,
    pub tool_description: Option<String>,
    pub tool_timeout_secs: Option<u64>,
    pub signature_valid: bool,
    pub trust_mode: String,
    pub issues: Vec<PluginValidationIssue>,
    pub contributions: Vec<String>,
    pub diagnostic: String,
    #[serde(default)]
    pub tui_slots: Vec<TuiSlot>,
}

pub fn default_plugin_roots(working_dir: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    roots.push(working_dir.join(".priority-agent").join("plugins"));

    if let Some(config_dir) = dirs::config_dir() {
        roots.push(config_dir.join("priority-agent").join("plugins"));
    }

    roots
}

pub fn discover_plugins(plugin_roots: &[PathBuf]) -> Vec<InstalledPlugin> {
    let mut out = Vec::new();

    for root in plugin_roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };

        for entry in entries.flatten() {
            let plugin_dir = entry.path();
            if !plugin_dir.is_dir() {
                continue;
            }

            let manifest_path = plugin_dir.join("plugin.toml");
            if !manifest_path.exists() {
                continue;
            }

            let Ok(content) = std::fs::read_to_string(&manifest_path) else {
                continue;
            };
            let Ok(manifest) = toml::from_str::<PluginManifest>(&content) else {
                continue;
            };

            out.push(InstalledPlugin {
                id: manifest.name.clone(),
                source_dir: plugin_dir,
                manifest_path,
                manifest,
            });
        }
    }

    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

pub fn validate_manifest(manifest: &PluginManifest) -> Vec<PluginValidationIssue> {
    let mut issues = Vec::new();

    if manifest.name.trim().is_empty() {
        issues.push(PluginValidationIssue {
            severity: "error".to_string(),
            message: "name cannot be empty".to_string(),
        });
    } else {
        let valid = manifest
            .name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.');
        if !valid {
            issues.push(PluginValidationIssue {
                severity: "error".to_string(),
                message: "name must only contain [a-zA-Z0-9._-]".to_string(),
            });
        }
    }

    if manifest.version.trim().is_empty() {
        issues.push(PluginValidationIssue {
            severity: "error".to_string(),
            message: "version cannot be empty".to_string(),
        });
    }

    if manifest.enabled {
        match manifest.entry_command.as_ref().map(|s| s.trim()) {
            Some(cmd) if !cmd.is_empty() => {}
            _ => issues.push(PluginValidationIssue {
                severity: "warning".to_string(),
                message: "enabled plugin has no entry_command".to_string(),
            }),
        }
    }

    if let Some(tool_name) = manifest
        .tool_name
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        let valid = tool_name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.');
        if !valid {
            issues.push(PluginValidationIssue {
                severity: "error".to_string(),
                message: "tool_name must only contain [a-zA-Z0-9._-]".to_string(),
            });
        }
    }

    issues
}

pub fn validate_installed_plugin(plugin: &InstalledPlugin) -> Vec<PluginValidationIssue> {
    let mut issues = validate_manifest(&plugin.manifest);

    if !plugin.source_dir.exists() {
        issues.push(PluginValidationIssue {
            severity: "error".to_string(),
            message: format!(
                "plugin source dir does not exist: {}",
                plugin.source_dir.display()
            ),
        });
    }
    if !plugin.manifest_path.exists() {
        issues.push(PluginValidationIssue {
            severity: "error".to_string(),
            message: format!(
                "manifest path does not exist: {}",
                plugin.manifest_path.display()
            ),
        });
    }

    issues
}

pub fn runtime_facts(
    plugins: &[InstalledPlugin],
    trust_mode: trust::TrustMode,
) -> Vec<PluginRuntimeFacts> {
    let mut facts = plugins
        .iter()
        .map(|plugin| runtime_facts_for_plugin(plugin, trust_mode))
        .collect::<Vec<_>>();
    facts.sort_by(|a, b| a.id.cmp(&b.id));
    facts
}

pub fn runtime_facts_for_plugin(
    plugin: &InstalledPlugin,
    trust_mode: trust::TrustMode,
) -> PluginRuntimeFacts {
    let mut issues = validate_installed_plugin(plugin);
    issues.extend(trust::validate_signature(&plugin.manifest, trust_mode));

    let signature_valid =
        trust::SignatureVerifier::verify_manifest(&plugin.manifest).unwrap_or(false);
    let has_errors = issues.iter().any(|issue| issue.severity == "error");
    let has_warnings = issues.iter().any(|issue| issue.severity == "warning");
    let missing_entry = plugin
        .manifest
        .entry_command
        .as_deref()
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .is_none();
    let strict_unsigned = trust_mode == trust::TrustMode::Strict && !signature_valid;

    let status = if !plugin.manifest.enabled {
        PluginRuntimeStatus::Disabled
    } else if has_errors || missing_entry || strict_unsigned {
        PluginRuntimeStatus::Blocked
    } else if has_warnings {
        PluginRuntimeStatus::UsableWithWarnings
    } else {
        PluginRuntimeStatus::Ready
    };

    let mut contributions = Vec::new();
    if let Some(tool_name) = plugin
        .manifest
        .tool_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        contributions.push(format!("tool:{}", tool_name));
    }
    if plugin.manifest.enabled
        && plugin
            .manifest
            .entry_command
            .as_deref()
            .map(str::trim)
            .filter(|command| !command.is_empty())
            .is_some()
    {
        contributions.push("runtime:process".to_string());
    }
    for slot in &plugin.manifest.tui.slots {
        contributions.push(format!("tui:{:?}", slot));
    }

    let tui_slots = plugin.manifest.tui.slots.clone();

    let diagnostic = match status {
        PluginRuntimeStatus::Ready => "plugin ready".to_string(),
        PluginRuntimeStatus::Disabled => "plugin disabled; enable before use".to_string(),
        PluginRuntimeStatus::UsableWithWarnings => {
            "plugin usable with warnings; run plugin_manage validate for details".to_string()
        }
        PluginRuntimeStatus::Blocked => {
            if strict_unsigned {
                "plugin blocked by strict trust policy; sign manifest or change trust mode"
                    .to_string()
            } else if missing_entry {
                "plugin blocked because enabled manifest has no entry_command".to_string()
            } else {
                "plugin blocked by manifest or signature errors".to_string()
            }
        }
    };

    PluginRuntimeFacts {
        id: plugin.id.clone(),
        name: plugin.manifest.name.clone(),
        version: plugin.manifest.version.clone(),
        enabled: plugin.manifest.enabled,
        status,
        source_dir: plugin.source_dir.clone(),
        manifest_path: plugin.manifest_path.clone(),
        entry_command: plugin.manifest.entry_command.clone(),
        entry_args: plugin.manifest.entry_args.clone(),
        tool_name: plugin.manifest.tool_name.clone(),
        tool_description: plugin.manifest.tool_description.clone(),
        tool_timeout_secs: plugin.manifest.tool_timeout_secs,
        signature_valid,
        trust_mode: trust_mode.as_str().to_string(),
        issues,
        contributions,
        diagnostic,
        tui_slots,
    }
}

pub fn set_plugin_enabled(
    plugin_roots: &[PathBuf],
    plugin_id: &str,
    enabled: bool,
) -> Result<InstalledPlugin, String> {
    let plugins = discover_plugins(plugin_roots);
    let Some(mut plugin) = plugins.into_iter().find(|p| p.id == plugin_id) else {
        return Err(format!("Plugin '{}' not found", plugin_id));
    };

    plugin.manifest.enabled = enabled;
    let manifest_toml = toml::to_string_pretty(&plugin.manifest)
        .map_err(|e| format!("Failed to serialize manifest: {}", e))?;

    std::fs::write(&plugin.manifest_path, manifest_toml)
        .map_err(|e| format!("Failed to update manifest: {}", e))?;

    Ok(plugin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_plugins_from_directory() {
        let temp_dir = std::env::temp_dir().join(format!(
            "priority-agent-plugin-test-{}",
            uuid::Uuid::new_v4()
        ));
        let plugin_dir = temp_dir.join("demo-plugin");
        std::fs::create_dir_all(&plugin_dir).expect("create plugin dir");

        let manifest = r#"
name = "demo-plugin"
version = "0.1.0"
description = "demo"
enabled = true
entry_command = "node"
entry_args = ["index.js"]
"#;
        std::fs::write(plugin_dir.join("plugin.toml"), manifest).expect("write plugin manifest");

        let discovered = discover_plugins(std::slice::from_ref(&temp_dir));
        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].manifest.name, "demo-plugin");
        assert_eq!(discovered[0].manifest.version, "0.1.0");
        assert!(discovered[0].manifest.enabled);

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_validate_manifest_empty_name_is_error() {
        let manifest = PluginManifest {
            name: "".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            enabled: true,
            entry_command: None,
            entry_args: vec![],
            tool_name: None,
            tool_description: None,
            tool_timeout_secs: None,
            signature: None,
            public_key: None,
            tui: Default::default(),
        };

        let issues = validate_manifest(&manifest);
        assert!(issues.iter().any(|i| i.severity == "error"));
    }

    #[test]
    fn test_set_plugin_enabled_updates_manifest_file() {
        let temp_dir = std::env::temp_dir().join(format!(
            "priority-agent-plugin-test-{}",
            uuid::Uuid::new_v4()
        ));
        let plugin_dir = temp_dir.join("demo-plugin");
        std::fs::create_dir_all(&plugin_dir).expect("create plugin dir");
        std::fs::write(
            plugin_dir.join("plugin.toml"),
            r#"
name = "demo-plugin"
version = "0.1.0"
enabled = true
"#,
        )
        .expect("write manifest");

        let updated = set_plugin_enabled(std::slice::from_ref(&temp_dir), "demo-plugin", false)
            .expect("set enabled");
        assert!(!updated.manifest.enabled);

        let refreshed = discover_plugins(std::slice::from_ref(&temp_dir));
        assert_eq!(refreshed.len(), 1);
        assert!(!refreshed[0].manifest.enabled);

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_runtime_facts_classify_ready_disabled_and_blocked_plugins() {
        let temp_dir = std::env::temp_dir().join(format!(
            "priority-agent-plugin-facts-test-{}",
            uuid::Uuid::new_v4()
        ));
        let ready_dir = temp_dir.join("ready");
        let disabled_dir = temp_dir.join("disabled");
        let blocked_dir = temp_dir.join("blocked");
        std::fs::create_dir_all(&ready_dir).expect("create ready plugin");
        std::fs::create_dir_all(&disabled_dir).expect("create disabled plugin");
        std::fs::create_dir_all(&blocked_dir).expect("create blocked plugin");

        std::fs::write(
            ready_dir.join("plugin.toml"),
            r#"
name = "ready"
version = "0.1.0"
enabled = true
entry_command = "sh"
tool_name = "plugin_ready"
"#,
        )
        .expect("write ready manifest");
        std::fs::write(
            disabled_dir.join("plugin.toml"),
            r#"
name = "disabled"
version = "0.1.0"
enabled = false
entry_command = "sh"
"#,
        )
        .expect("write disabled manifest");
        std::fs::write(
            blocked_dir.join("plugin.toml"),
            r#"
name = "blocked"
version = "0.1.0"
enabled = true
"#,
        )
        .expect("write blocked manifest");

        let discovered = discover_plugins(std::slice::from_ref(&temp_dir));
        let facts = runtime_facts(&discovered, trust::TrustMode::Off);

        let ready = facts.iter().find(|fact| fact.id == "ready").unwrap();
        let disabled = facts.iter().find(|fact| fact.id == "disabled").unwrap();
        let blocked = facts.iter().find(|fact| fact.id == "blocked").unwrap();
        assert_eq!(ready.status, PluginRuntimeStatus::Ready);
        assert!(ready
            .contributions
            .contains(&"tool:plugin_ready".to_string()));
        assert!(ready.contributions.contains(&"runtime:process".to_string()));
        assert_eq!(disabled.status, PluginRuntimeStatus::Disabled);
        assert_eq!(blocked.status, PluginRuntimeStatus::Blocked);
        assert!(blocked.diagnostic.contains("entry_command"));

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn manifest_tui_slots_are_parsed_and_validated() {
        let temp_dir =
            std::env::temp_dir().join(format!("pa-plugin-tui-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let ui_dir = temp_dir.join("ui-plugin");
        std::fs::create_dir_all(&ui_dir).unwrap();
        std::fs::write(
            ui_dir.join("plugin.toml"),
            r#"
name = "ui-plugin"
version = "0.1.0"
enabled = true
entry_command = "sh"

[tui]
slots = ["status_bar"]
"#,
        )
        .unwrap();

        let discovered = discover_plugins(std::slice::from_ref(&temp_dir));
        let facts = runtime_facts(&discovered, trust::TrustMode::Off);
        let ui = facts.iter().find(|f| f.id == "ui-plugin").unwrap();
        assert_eq!(ui.tui_slots, vec![TuiSlot::StatusBar]);
        assert!(ui.contributions.contains(&"tui:StatusBar".to_string()));

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
