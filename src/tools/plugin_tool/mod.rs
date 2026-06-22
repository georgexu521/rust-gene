//! Plugin discovery and installation tool boundary.
//!
//! Surfaces available plugin actions to the model while leaving install approval and execution policy to the runtime.

use crate::plugins;
use crate::tools::{Tool, ToolContext, ToolRegistry, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{sleep, timeout, Duration};
use tracing::{info, warn};

/// 列出本地已安装插件（MVP：仅发现与清单展示）
pub struct PluginListTool;
pub struct PluginManageTool;
pub struct PluginRuntimeTool {
    tool_name: String,
    tool_description: String,
    plugin: plugins::InstalledPlugin,
    default_timeout_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginRegistrationReport {
    pub roots: Vec<std::path::PathBuf>,
    pub discovered_count: usize,
    pub enabled_count: usize,
    pub injected_count: usize,
    pub injected_tool_names: Vec<String>,
    pub skipped_disabled: usize,
    pub skipped_missing_entry: usize,
    pub skipped_unsigned: usize,
    pub skipped_name_collision: usize,
    pub trust_mode: String,
}

impl PluginRegistrationReport {
    fn skipped_count(&self) -> usize {
        self.skipped_disabled
            + self.skipped_missing_entry
            + self.skipped_unsigned
            + self.skipped_name_collision
    }

    pub fn summary(&self) -> String {
        let injected = if self.injected_tool_names.is_empty() {
            "none".to_string()
        } else {
            self.injected_tool_names.join(", ")
        };
        format!(
            "Plugins reloaded: discovered={} enabled={} injected={} skipped={} trust_mode={}\nInjected tools: {}\nSkipped: disabled={} missing_entry={} unsigned={} name_collision={}",
            self.discovered_count,
            self.enabled_count,
            self.injected_count,
            self.skipped_count(),
            self.trust_mode,
            injected,
            self.skipped_disabled,
            self.skipped_missing_entry,
            self.skipped_unsigned,
            self.skipped_name_collision
        )
    }

    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "roots": self.roots,
            "discovered_count": self.discovered_count,
            "enabled_count": self.enabled_count,
            "injected_count": self.injected_count,
            "injected_tool_names": self.injected_tool_names,
            "skipped": {
                "total": self.skipped_count(),
                "disabled": self.skipped_disabled,
                "missing_entry": self.skipped_missing_entry,
                "unsigned": self.skipped_unsigned,
                "name_collision": self.skipped_name_collision,
            },
            "trust_mode": self.trust_mode,
        })
    }
}

struct PluginExecutionOutput {
    success: bool,
    stdout: String,
    stderr: String,
    exit_code: i32,
}

fn discovered_plugins_to_json(discovered: &[plugins::InstalledPlugin]) -> Vec<serde_json::Value> {
    discovered
        .iter()
        .map(|p| {
            let sig_valid =
                plugins::trust::SignatureVerifier::verify_manifest(&p.manifest).unwrap_or(false);
            json!({
                "id": p.id,
                "name": p.manifest.name,
                "version": p.manifest.version,
                "description": p.manifest.description,
                "enabled": p.manifest.enabled,
                "entry_command": p.manifest.entry_command,
                "entry_args": p.manifest.entry_args,
                "tool_name": p.manifest.tool_name,
                "tool_description": p.manifest.tool_description,
                "tool_timeout_secs": p.manifest.tool_timeout_secs,
                "signature_valid": sig_valid,
                "has_public_key": p.manifest.public_key.is_some(),
                "has_signature": p.manifest.signature.is_some(),
                "source_dir": p.source_dir,
                "manifest_path": p.manifest_path
            })
        })
        .collect::<Vec<_>>()
}

fn current_trust_mode() -> plugins::trust::TrustMode {
    let mode_str = crate::services::config::AppConfig::load()
        .map(|c| c.features.plugin_trust_mode)
        .unwrap_or_else(|_| "warn".to_string());
    plugins::trust::TrustMode::parse_lossy(&mode_str)
}

fn sanitize_tool_name(id: &str) -> String {
    let mut out = String::with_capacity(id.len() + 7);
    out.push_str("plugin_");
    for ch in id.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    out
}

/// 验证插件源目录是否位于允许的插件根目录内，防止 symlink 逃逸
fn validate_plugin_source_dir(
    plugin: &plugins::InstalledPlugin,
    roots: &[std::path::PathBuf],
) -> Result<(), String> {
    let source_dir = std::path::Path::new(&plugin.source_dir);
    let canonical_source = crate::tools::file_tool::canonicalize_or_normalize(source_dir);

    for root in roots {
        let canonical_root = crate::tools::file_tool::canonicalize_or_normalize(root);
        if canonical_source.starts_with(&canonical_root) {
            return Ok(());
        }
    }

    Err(format!(
        "Plugin '{}' source directory '{}' is outside allowed plugin roots",
        plugin.id,
        plugin.source_dir.display()
    ))
}

async fn execute_plugin_process(
    plugin: &plugins::InstalledPlugin,
    timeout_secs: u64,
    stdin_json: Option<&serde_json::Value>,
    roots: &[std::path::PathBuf],
) -> Result<PluginExecutionOutput, String> {
    let command = match plugin
        .manifest
        .entry_command
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        Some(cmd) => cmd.to_string(),
        None => return Err(format!("Plugin '{}' has no entry_command", plugin.id)),
    };

    // 工作目录安全校验
    validate_plugin_source_dir(plugin, roots)?;

    let mut cmd = Command::new(&command);
    cmd.args(&plugin.manifest.entry_args)
        .current_dir(&plugin.source_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(unix)]
    unsafe {
        // 让子进程成为新的进程组 leader，超时时可一次性 kill 整棵进程树
        cmd.pre_exec(|| {
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn plugin '{}': {}", plugin.id, e))?;

    if let Some(input) = stdin_json {
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            let payload = serde_json::to_vec(input)
                .map_err(|e| format!("Failed to encode stdin json: {}", e))?;
            stdin
                .write_all(&payload)
                .await
                .map_err(|e| format!("Failed to write stdin payload: {}", e))?;
            stdin
                .write_all(b"\n")
                .await
                .map_err(|e| format!("Failed to finalize stdin payload: {}", e))?;
        }
    }

    let child_pid = child.id().map(|id| id as i32);
    let wait_fut = child.wait_with_output();
    tokio::pin!(wait_fut);

    let output = tokio::select! {
        res = &mut wait_fut => match res {
            Ok(o) => o,
            Err(e) => return Err(format!("Plugin '{}' execution failed: {}", plugin.id, e)),
        },
        _ = sleep(Duration::from_secs(timeout_secs)) => {
            warn!("Plugin '{}' timed out after {}s, killing process tree (pid: {:?})", plugin.id, timeout_secs, child_pid);
            kill_process_tree(child_pid);
            match timeout(Duration::from_secs(2), &mut wait_fut).await {
                Ok(Ok(o)) => o,
                _ => {
                    return Err(format!(
                        "Plugin '{}' timed out after {}s",
                        plugin.id, timeout_secs
                    ));
                }
            }
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    Ok(PluginExecutionOutput {
        success: output.status.success(),
        stdout,
        stderr,
        exit_code,
    })
}

pub fn register_enabled_plugin_tools(registry: &mut ToolRegistry, working_dir: &Path) -> usize {
    register_enabled_plugin_tools_with_report(registry, working_dir).injected_count
}

pub fn register_enabled_plugin_tools_with_report(
    registry: &mut ToolRegistry,
    working_dir: &Path,
) -> PluginRegistrationReport {
    let roots = plugins::default_plugin_roots(working_dir);
    let discovered = plugins::discover_plugins(&roots);
    let trust_mode = current_trust_mode();
    let mut report = PluginRegistrationReport {
        roots: roots.clone(),
        discovered_count: discovered.len(),
        enabled_count: discovered
            .iter()
            .filter(|plugin| plugin.manifest.enabled)
            .count(),
        injected_count: 0,
        injected_tool_names: Vec::new(),
        skipped_disabled: 0,
        skipped_missing_entry: 0,
        skipped_unsigned: 0,
        skipped_name_collision: 0,
        trust_mode: trust_mode.as_str().to_string(),
    };

    for plugin in discovered {
        if !plugin.manifest.enabled {
            report.skipped_disabled += 1;
            continue;
        }

        if trust_mode == plugins::trust::TrustMode::Strict {
            let sig_valid = plugins::trust::SignatureVerifier::verify_manifest(&plugin.manifest)
                .unwrap_or(false);
            if !sig_valid {
                report.skipped_unsigned += 1;
                warn!(
                    "Skipping unsigned plugin '{}' in strict trust mode",
                    plugin.id
                );
                continue;
            }
        }

        let Some(command) = plugin
            .manifest
            .entry_command
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        else {
            report.skipped_missing_entry += 1;
            warn!(
                "Skipping enabled plugin '{}' without entry_command",
                plugin.id
            );
            continue;
        };

        let tool_name = plugin
            .manifest
            .tool_name
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| sanitize_tool_name(&plugin.id));

        if registry.has(&tool_name) {
            report.skipped_name_collision += 1;
            warn!(
                "Skipping plugin '{}' tool '{}' due to name collision",
                plugin.id, tool_name
            );
            continue;
        }

        let tool_description = plugin
            .manifest
            .tool_description
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("Run plugin '{}' via {}", plugin.id, command));

        let timeout_secs = plugin
            .manifest
            .tool_timeout_secs
            .map(|v| v.clamp(1, 600))
            .unwrap_or(30);

        registry.register(PluginRuntimeTool {
            tool_name: tool_name.clone(),
            tool_description,
            plugin,
            default_timeout_secs: timeout_secs,
        });
        report.injected_count += 1;
        report.injected_tool_names.push(tool_name);
    }

    if report.injected_count > 0 {
        info!("Registered {} plugin runtime tools", report.injected_count);
    }
    report
}

#[async_trait]
impl Tool for PluginListTool {
    fn name(&self) -> &str {
        "plugin_list"
    }

    fn description(&self) -> &str {
        "List discovered local plugins and their metadata (name/version/enabled/entrypoint)."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _params: serde_json::Value, context: ToolContext) -> ToolResult {
        let roots = plugins::default_plugin_roots(&context.working_dir);
        let discovered = plugins::discover_plugins(&roots);

        if discovered.is_empty() {
            return ToolResult::success_with_data(
                "No plugins discovered".to_string(),
                json!({
                    "count": 0,
                    "roots": roots,
                    "plugins": []
                }),
            );
        }

        let summary = discovered
            .iter()
            .map(|p| {
                format!(
                    "- {}@{} ({})",
                    p.manifest.name,
                    p.manifest.version,
                    if p.manifest.enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let plugins_json = discovered_plugins_to_json(&discovered);

        ToolResult::success_with_data(
            format!("Discovered {} plugins:\n{}", plugins_json.len(), summary),
            json!({
                "count": plugins_json.len(),
                "roots": roots,
                "plugins": plugins_json
            }),
        )
    }
}

#[async_trait]
impl Tool for PluginManageTool {
    fn name(&self) -> &str {
        "plugin_manage"
    }

    fn description(&self) -> &str {
        "Manage plugins (list, status, validate, reload, enable, disable, run)."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "status", "validate", "reload", "enable", "disable", "run", "sign", "generate_key"],
                    "description": "Management action"
                },
                "plugin_id": {
                    "type": "string",
                    "description": "Required for enable/disable/sign; optional for validate (all plugins if omitted)"
                },
                "timeout_secs": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 600,
                    "description": "Only for run action. Default 30."
                },
                "private_key": {
                    "type": "string",
                    "description": "Only for sign action. Base64-encoded 32-byte Ed25519 private key. Falls back to PRIORITY_AGENT_PLUGIN_SIGN_KEY env var."
                },
                "write_manifest": {
                    "type": "boolean",
                    "description": "Only for sign action. If true (default), writes signature and public_key back to plugin.toml."
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("");
        if action.is_empty() {
            return ToolResult::error("action is required");
        }
        let plugin_id = params["plugin_id"]
            .as_str()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());

        let roots = plugins::default_plugin_roots(&context.working_dir);
        let discovered = plugins::discover_plugins(&roots);

        match action {
            "list" => {
                let plugins_json = discovered_plugins_to_json(&discovered);
                ToolResult::success_with_data(
                    format!("Discovered {} plugins", plugins_json.len()),
                    json!({
                        "count": plugins_json.len(),
                        "roots": roots,
                        "plugins": plugins_json
                    }),
                )
            }
            "status" => {
                let trust_mode = current_trust_mode();
                let facts = plugins::runtime_facts(&discovered, trust_mode);
                let rows = facts
                    .iter()
                    .map(|fact| {
                        format!(
                            "- {}@{} status={:?} enabled={} signature_valid={} contributions={} diagnostic={}",
                            fact.id,
                            fact.version,
                            fact.status,
                            fact.enabled,
                            fact.signature_valid,
                            if fact.contributions.is_empty() {
                                "none".to_string()
                            } else {
                                fact.contributions.join(",")
                            },
                            fact.diagnostic
                        )
                    })
                    .collect::<Vec<_>>();
                ToolResult::success_with_data(
                    if rows.is_empty() {
                        "No plugins discovered".to_string()
                    } else {
                        format!("Plugin runtime status:\n{}", rows.join("\n"))
                    },
                    json!({
                        "count": facts.len(),
                        "roots": roots,
                        "trust_mode": trust_mode.as_str(),
                        "plugins": facts
                    }),
                )
            }
            "validate" => {
                let selected = if let Some(pid) = plugin_id {
                    let Some(plugin) = discovered.iter().find(|p| p.id == pid) else {
                        return ToolResult::error(format!("Plugin '{}' not found", pid));
                    };
                    vec![plugin]
                } else {
                    discovered.iter().collect::<Vec<_>>()
                };

                let trust_mode = current_trust_mode();

                let reports = selected
                    .into_iter()
                    .map(|plugin| {
                        let mut issues = plugins::validate_installed_plugin(plugin);
                        let sig_issues =
                            plugins::trust::validate_signature(&plugin.manifest, trust_mode);
                        issues.extend(sig_issues);
                        let sig_valid =
                            plugins::trust::SignatureVerifier::verify_manifest(&plugin.manifest)
                                .unwrap_or(false);
                        let runtime_facts = plugins::runtime_facts_for_plugin(plugin, trust_mode);
                        json!({
                            "id": plugin.id,
                            "valid": !issues.iter().any(|i| i.severity == "error"),
                            "signature_valid": sig_valid,
                            "trust_mode": trust_mode.as_str(),
                            "runtime_status": runtime_facts.status,
                            "diagnostic": runtime_facts.diagnostic,
                            "contributions": runtime_facts.contributions,
                            "issues": issues
                        })
                    })
                    .collect::<Vec<_>>();

                let invalid_count = reports
                    .iter()
                    .filter(|r| r["valid"].as_bool() == Some(false))
                    .count();

                ToolResult::success_with_data(
                    format!(
                        "Validated {} plugin(s), {} invalid (trust_mode={})",
                        reports.len(),
                        invalid_count,
                        trust_mode.as_str()
                    ),
                    json!({
                        "count": reports.len(),
                        "invalid_count": invalid_count,
                        "trust_mode": trust_mode.as_str(),
                        "reports": reports
                    }),
                )
            }
            "reload" => {
                let mut registry = ToolRegistry::default_registry();
                let report =
                    register_enabled_plugin_tools_with_report(&mut registry, &context.working_dir);
                ToolResult::success_with_data(report.summary(), report.to_json())
            }
            "enable" | "disable" => {
                let Some(pid) = plugin_id else {
                    return ToolResult::error("plugin_id is required for enable/disable");
                };
                let enabled = action == "enable";

                if enabled {
                    let trust_mode = current_trust_mode();
                    if trust_mode == plugins::trust::TrustMode::Strict {
                        let Some(plugin) = discovered.iter().find(|p| p.id == pid) else {
                            return ToolResult::error(format!("Plugin '{}' not found", pid));
                        };
                        let sig_valid =
                            plugins::trust::SignatureVerifier::verify_manifest(&plugin.manifest)
                                .unwrap_or(false);
                        if !sig_valid {
                            return ToolResult::error(format!(
                                "Plugin '{}' cannot be enabled: trust_mode=strict and signature is invalid or missing. Run plugin_manage validate for details.",
                                pid
                            ));
                        }
                    }
                }

                match plugins::set_plugin_enabled(&roots, pid, enabled) {
                    Ok(updated) => ToolResult::success_with_data(
                        format!(
                            "Plugin '{}' is now {}",
                            updated.id,
                            if updated.manifest.enabled {
                                "enabled"
                            } else {
                                "disabled"
                            }
                        ),
                        json!({
                            "id": updated.id,
                            "enabled": updated.manifest.enabled,
                            "manifest_path": updated.manifest_path
                        }),
                    ),
                    Err(e) => ToolResult::error(e),
                }
            }
            "run" => {
                let Some(pid) = plugin_id else {
                    return ToolResult::error("plugin_id is required for run");
                };
                let Some(plugin) = discovered.iter().find(|p| p.id == pid) else {
                    return ToolResult::error(format!("Plugin '{}' not found", pid));
                };
                if !plugin.manifest.enabled {
                    return ToolResult::error(format!(
                        "Plugin '{}' is disabled; enable it before run",
                        pid
                    ));
                }
                let timeout_secs = params["timeout_secs"]
                    .as_u64()
                    .map(|v| v.clamp(1, 600))
                    .unwrap_or(30);
                let output = match execute_plugin_process(plugin, timeout_secs, None, &roots).await
                {
                    Ok(o) => o,
                    Err(e) => return ToolResult::error(e),
                };
                let mut merged = String::new();
                if !output.stdout.is_empty() {
                    merged.push_str(&output.stdout);
                }
                if !output.stderr.is_empty() {
                    if !merged.is_empty() {
                        merged.push_str("\n\n[stderr]\n");
                    } else {
                        merged.push_str("[stderr]\n");
                    }
                    merged.push_str(&output.stderr);
                }
                if merged.is_empty() {
                    merged = "(no output)".to_string();
                }

                if output.success {
                    ToolResult::success_with_data(
                        format!("Plugin '{}' executed successfully\n{}", pid, merged),
                        json!({
                            "plugin_id": pid,
                            "command": plugin.manifest.entry_command.clone().unwrap_or_default(),
                            "args": plugin.manifest.entry_args,
                            "exit_code": output.exit_code,
                            "stdout": output.stdout,
                            "stderr": output.stderr,
                        }),
                    )
                } else {
                    ToolResult::error(format!(
                        "Plugin '{}' exited with code {}\n{}",
                        pid, output.exit_code, merged
                    ))
                }
            }
            "sign" => {
                let Some(pid) = plugin_id else {
                    return ToolResult::error("plugin_id is required for sign");
                };
                let Some(plugin) = discovered.iter().find(|p| p.id == pid) else {
                    return ToolResult::error(format!("Plugin '{}' not found", pid));
                };

                // 读取私钥：参数优先，其次环境变量
                let private_key_b64 = params["private_key"]
                    .as_str()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .or_else(|| {
                        std::env::var("PRIORITY_AGENT_PLUGIN_SIGN_KEY")
                            .ok()
                            .map(|s| s.trim().to_string())
                    });

                let Some(key_b64) = private_key_b64 else {
                    return ToolResult::error(
                        "No private key provided. Set 'private_key' parameter or PRIORITY_AGENT_PLUGIN_SIGN_KEY env var."
                    );
                };

                let (public_key_b64, signature_b64) =
                    match plugins::trust::sign_manifest(&plugin.manifest, &key_b64) {
                        Ok(pair) => pair,
                        Err(e) => return ToolResult::error(format!("Signing failed: {}", e)),
                    };

                let write_manifest = params["write_manifest"].as_bool().unwrap_or(true);

                if write_manifest {
                    let mut updated_manifest = plugin.manifest.clone();
                    updated_manifest.public_key = Some(public_key_b64.clone());
                    updated_manifest.signature = Some(signature_b64.clone());

                    let manifest_toml = match toml::to_string_pretty(&updated_manifest) {
                        Ok(s) => s,
                        Err(e) => {
                            return ToolResult::error(format!(
                                "Failed to serialize manifest: {}",
                                e
                            ))
                        }
                    };

                    if let Err(e) = std::fs::write(&plugin.manifest_path, manifest_toml) {
                        return ToolResult::error(format!("Failed to write manifest: {}", e));
                    }
                }

                ToolResult::success_with_data(
                    format!(
                        "Plugin '{}' signed successfully.\npublic_key: {}\nsignature: {}",
                        pid, public_key_b64, signature_b64
                    ),
                    json!({
                        "plugin_id": pid,
                        "public_key": public_key_b64,
                        "signature": signature_b64,
                        "manifest_path": plugin.manifest_path.to_string_lossy().to_string(),
                        "written": write_manifest,
                    }),
                )
            }
            "generate_key" => {
                let (private_b64, public_b64) = plugins::trust::generate_keypair();
                ToolResult::success_with_data(
                    format!(
                        "Generated Ed25519 keypair.\n\nprivate_key: {}\npublic_key: {}\n\nStore the private key securely (e.g. in PRIORITY_AGENT_PLUGIN_SIGN_KEY env var).",
                        private_b64, public_b64
                    ),
                    json!({
                        "private_key": private_b64,
                        "public_key": public_b64,
                    }),
                )
            }
            _ => ToolResult::error(format!("Unknown action: {}", action)),
        }
    }

    fn requires_confirmation(&self, params: &serde_json::Value) -> bool {
        params["action"].as_str() == Some("run")
    }

    fn confirmation_prompt(&self, params: &serde_json::Value) -> Option<String> {
        if params["action"].as_str() != Some("run") {
            return None;
        }
        let plugin_id = params["plugin_id"].as_str().unwrap_or("unknown");
        Some(format!(
            "This will execute plugin '{}'. Continue?",
            plugin_id
        ))
    }
}

#[async_trait]
impl Tool for PluginRuntimeTool {
    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.tool_description
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "description": "Arbitrary JSON payload passed to plugin stdin (plus optional timeout_secs).",
            "properties": {
                "timeout_secs": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 600,
                    "description": "Override execution timeout for this call."
                }
            },
            "additionalProperties": true
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let timeout_secs = params["timeout_secs"]
            .as_u64()
            .map(|v| v.clamp(1, 600))
            .unwrap_or(self.default_timeout_secs);

        let roots = plugins::default_plugin_roots(&_context.working_dir);
        let output =
            match execute_plugin_process(&self.plugin, timeout_secs, Some(&params), &roots).await {
                Ok(o) => o,
                Err(e) => return ToolResult::error(e),
            };

        let mut merged = String::new();
        if !output.stdout.is_empty() {
            merged.push_str(&output.stdout);
        }
        if !output.stderr.is_empty() {
            if !merged.is_empty() {
                merged.push_str("\n\n[stderr]\n");
            } else {
                merged.push_str("[stderr]\n");
            }
            merged.push_str(&output.stderr);
        }
        if merged.is_empty() {
            merged = "(no output)".to_string();
        }

        if output.success {
            ToolResult::success_with_data(
                format!("Plugin tool '{}' succeeded\n{}", self.tool_name, merged),
                json!({
                    "tool_name": self.tool_name,
                    "plugin_id": self.plugin.id,
                    "exit_code": output.exit_code,
                    "stdout": output.stdout,
                    "stderr": output.stderr,
                }),
            )
        } else {
            ToolResult::error(format!(
                "Plugin tool '{}' failed with code {}\n{}",
                self.tool_name, output.exit_code, merged
            ))
        }
    }
}

#[cfg(unix)]
fn kill_process_tree(child_pid: Option<i32>) {
    if let Some(pid) = child_pid {
        // kill(-pgid) 发送到整个进程组，避免遗留后台子进程
        let _ = unsafe { libc::kill(-pid, libc::SIGKILL) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;

    #[tokio::test]
    async fn test_plugin_manage_enable_disable_roundtrip() {
        let tmp = std::env::temp_dir().join(format!(
            "priority-agent-plugin-tool-test-{}",
            uuid::Uuid::new_v4()
        ));
        let plugins_root = tmp.join(".priority-agent").join("plugins").join("demo");
        std::fs::create_dir_all(&plugins_root).expect("create dirs");
        std::fs::write(
            plugins_root.join("plugin.toml"),
            r#"
name = "demo"
version = "0.1.0"
enabled = false
"#,
        )
        .expect("write manifest");

        let tool = PluginManageTool;
        let ctx = crate::tools::ToolContext::new(&tmp, "s1");

        let enable_result = tool
            .execute(json!({"action":"enable","plugin_id":"demo"}), ctx.clone())
            .await;
        assert!(enable_result.success);

        let disable_result = tool
            .execute(json!({"action":"disable","plugin_id":"demo"}), ctx.clone())
            .await;
        assert!(disable_result.success);

        let validate_result = tool.execute(json!({"action":"validate"}), ctx).await;
        assert!(validate_result.success);

        let _ = std::fs::remove_dir_all(tmp);
    }

    #[tokio::test]
    async fn test_plugin_manage_status_returns_runtime_facts() {
        let tmp = std::env::temp_dir().join(format!(
            "priority-agent-plugin-status-test-{}",
            uuid::Uuid::new_v4()
        ));
        let plugins_root = tmp.join(".priority-agent").join("plugins").join("demo");
        std::fs::create_dir_all(&plugins_root).expect("create dirs");
        std::fs::write(
            plugins_root.join("plugin.toml"),
            r#"
name = "demo"
version = "0.1.0"
enabled = true
entry_command = "sh"
tool_name = "plugin_demo"
"#,
        )
        .expect("write manifest");

        let tool = PluginManageTool;
        let ctx = crate::tools::ToolContext::new(&tmp, "s1");
        let result = tool.execute(json!({"action":"status"}), ctx).await;

        assert!(result.success, "{:?}", result.error);
        assert!(result.content.contains("Plugin runtime status"));
        let data = result.data.as_ref().unwrap();
        assert_eq!(data["count"], 1);
        assert_eq!(data["plugins"][0]["id"], "demo");
        assert_eq!(data["plugins"][0]["status"], "usable_with_warnings");
        assert_eq!(data["plugins"][0]["contributions"][0], "tool:plugin_demo");

        let _ = std::fs::remove_dir_all(tmp);
    }

    #[tokio::test]
    async fn test_plugin_manage_run_action() {
        let tmp = std::env::temp_dir().join(format!(
            "priority-agent-plugin-run-test-{}",
            uuid::Uuid::new_v4()
        ));
        let plugin_dir = tmp.join(".priority-agent").join("plugins").join("demo");
        std::fs::create_dir_all(&plugin_dir).expect("create plugin dir");
        std::fs::write(
            plugin_dir.join("plugin.toml"),
            r#"
name = "demo"
version = "0.1.0"
enabled = true
entry_command = "sh"
entry_args = ["-c", "echo plugin-ok"]
"#,
        )
        .expect("write manifest");

        let tool = PluginManageTool;
        let ctx = crate::tools::ToolContext::new(&tmp, "s1");
        let res = tool
            .execute(
                json!({"action":"run","plugin_id":"demo","timeout_secs":5}),
                ctx,
            )
            .await;
        assert!(res.success, "{}", res.content);
        assert!(res.content.contains("plugin-ok"));

        let _ = std::fs::remove_dir_all(tmp);
    }

    #[tokio::test]
    async fn test_register_enabled_plugin_tools_and_execute() {
        let tmp = std::env::temp_dir().join(format!(
            "priority-agent-plugin-register-test-{}",
            uuid::Uuid::new_v4()
        ));
        let plugin_dir = tmp.join(".priority-agent").join("plugins").join("echoer");
        std::fs::create_dir_all(&plugin_dir).expect("create plugin dir");
        std::fs::write(
            plugin_dir.join("plugin.toml"),
            r#"
name = "echoer"
version = "0.1.0"
enabled = true
tool_name = "plugin_echoer"
entry_command = "sh"
entry_args = ["-c", "cat"]
"#,
        )
        .expect("write manifest");

        let mut registry = crate::tools::ToolRegistry::new();
        let count = register_enabled_plugin_tools(&mut registry, &tmp);
        assert_eq!(count, 1);
        assert!(registry.has("plugin_echoer"));

        let tool = registry.get("plugin_echoer").expect("tool exists");
        let ctx = crate::tools::ToolContext::new(&tmp, "s2");
        let result = tool.execute(json!({"hello":"world"}), ctx).await;
        assert!(result.success, "{:?}", result.error);
        assert!(result.content.contains("world"));

        let _ = std::fs::remove_dir_all(tmp);
    }

    #[test]
    fn test_plugin_registration_report_counts_reload_lifecycle() {
        let tmp = std::env::temp_dir().join(format!(
            "priority-agent-plugin-report-test-{}",
            uuid::Uuid::new_v4()
        ));
        let plugins_root = tmp.join(".priority-agent").join("plugins");
        let enabled_dir = plugins_root.join("enabled");
        let disabled_dir = plugins_root.join("disabled");
        let missing_entry_dir = plugins_root.join("missing-entry");
        std::fs::create_dir_all(&enabled_dir).expect("create enabled plugin");
        std::fs::create_dir_all(&disabled_dir).expect("create disabled plugin");
        std::fs::create_dir_all(&missing_entry_dir).expect("create missing-entry plugin");

        std::fs::write(
            enabled_dir.join("plugin.toml"),
            r#"
name = "enabled"
version = "0.1.0"
enabled = true
entry_command = "sh"
entry_args = ["-c", "echo ok"]
tool_name = "plugin_enabled"
"#,
        )
        .expect("write enabled manifest");
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
            missing_entry_dir.join("plugin.toml"),
            r#"
name = "missing-entry"
version = "0.1.0"
enabled = true
"#,
        )
        .expect("write missing-entry manifest");

        let mut registry = ToolRegistry::new();
        let report = register_enabled_plugin_tools_with_report(&mut registry, &tmp);

        assert_eq!(report.discovered_count, 3);
        assert_eq!(report.enabled_count, 2);
        assert_eq!(report.injected_count, 1);
        assert_eq!(report.skipped_disabled, 1);
        assert_eq!(report.skipped_missing_entry, 1);
        assert_eq!(report.injected_tool_names, vec!["plugin_enabled"]);
        assert!(registry.has("plugin_enabled"));

        let _ = std::fs::remove_dir_all(tmp);
    }

    #[tokio::test]
    async fn test_plugin_manage_generate_key_and_sign() {
        let tmp = std::env::temp_dir().join(format!(
            "priority-agent-plugin-sign-test-{}",
            uuid::Uuid::new_v4()
        ));
        let plugin_dir = tmp.join(".priority-agent").join("plugins").join("demo");
        std::fs::create_dir_all(&plugin_dir).expect("create plugin dir");
        std::fs::write(
            plugin_dir.join("plugin.toml"),
            r#"
name = "demo"
version = "0.1.0"
enabled = true
entry_command = "sh"
entry_args = ["-c", "echo hello"]
"#,
        )
        .expect("write manifest");

        let tool = PluginManageTool;
        let ctx = crate::tools::ToolContext::new(&tmp, "s1");

        // 1. generate_key
        let gen_res = tool
            .execute(json!({"action":"generate_key"}), ctx.clone())
            .await;
        assert!(gen_res.success, "generate_key failed: {:?}", gen_res.error);
        let data = gen_res.data.as_ref().unwrap();
        let private_key = data["private_key"].as_str().unwrap();
        let public_key = data["public_key"].as_str().unwrap();
        assert!(!private_key.is_empty());
        assert!(!public_key.is_empty());

        // 2. sign with the generated private key (dry-run first)
        let sign_dry = tool
            .execute(
                json!({
                    "action": "sign",
                    "plugin_id": "demo",
                    "private_key": private_key,
                    "write_manifest": false
                }),
                ctx.clone(),
            )
            .await;
        assert!(
            sign_dry.success,
            "sign dry-run failed: {:?}",
            sign_dry.error
        );
        let sign_dry_data = sign_dry.data.as_ref().unwrap();
        let sig = sign_dry_data["signature"].as_str().unwrap();
        let pk = sign_dry_data["public_key"].as_str().unwrap();
        assert_eq!(pk, public_key);
        assert!(!sig.is_empty());

        // 3. sign and write manifest
        let sign_write = tool
            .execute(
                json!({
                    "action": "sign",
                    "plugin_id": "demo",
                    "private_key": private_key,
                    "write_manifest": true
                }),
                ctx.clone(),
            )
            .await;
        assert!(
            sign_write.success,
            "sign write failed: {:?}",
            sign_write.error
        );

        // 4. validate should now report signature valid
        let validate = tool
            .execute(json!({"action":"validate","plugin_id":"demo"}), ctx)
            .await;
        assert!(validate.success);
        let val_data = validate.data.as_ref().unwrap();
        let reports = val_data["reports"].as_array().unwrap();
        let report = reports.iter().find(|r| r["id"] == "demo").unwrap();
        assert_eq!(report["signature_valid"], true);

        let _ = std::fs::remove_dir_all(tmp);
    }

    #[tokio::test]
    async fn test_plugin_manage_sign_without_key_fails() {
        let tmp = std::env::temp_dir().join(format!(
            "priority-agent-plugin-sign-fail-test-{}",
            uuid::Uuid::new_v4()
        ));
        let plugin_dir = tmp.join(".priority-agent").join("plugins").join("demo");
        std::fs::create_dir_all(&plugin_dir).expect("create plugin dir");
        std::fs::write(
            plugin_dir.join("plugin.toml"),
            r#"
name = "demo"
version = "0.1.0"
enabled = true
"#,
        )
        .expect("write manifest");

        let tool = PluginManageTool;
        let ctx = crate::tools::ToolContext::new(&tmp, "s1");

        // sign without any key should fail
        let res = tool
            .execute(json!({"action":"sign","plugin_id":"demo"}), ctx)
            .await;
        assert!(!res.success);
        assert!(res.error.as_ref().unwrap().contains("No private key"));

        let _ = std::fs::remove_dir_all(tmp);
    }
}

#[cfg(not(unix))]
fn kill_process_tree(child_pid: Option<i32>) {
    if let Some(pid) = child_pid {
        if pid > 0 {
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
    }
}
