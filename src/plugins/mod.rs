//! Plugin metadata discovery (MVP)
//!
//! This module provides a minimal plugin manifest loader so the agent can
//! discover installed plugins before full execution/injection support lands.

pub mod trust;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
}
