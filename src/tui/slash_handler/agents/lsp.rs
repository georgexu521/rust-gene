use super::*;

/// /lsp - LSP server management.
pub fn handle_lsp(app: &TuiApp, args: &str) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();

    if parts.is_empty() || parts[0] == "list" {
        return match &app.lsp_manager {
            Some(mgr) => {
                let servers = mgr.server_names();
                if servers.is_empty() {
                    "No LSP servers running. Enable LSP with /config lsp.enabled true".to_string()
                } else {
                    format!("LSP servers ({}):\n{}", servers.len(), servers.join("\n"))
                }
            }
            None => "LSP manager not available. Enable with /config lsp.enabled true.".to_string(),
        };
    }

    if parts[0] == "restart" && parts.len() >= 2 {
        let name = parts[1].to_string();
        return match &app.lsp_manager {
            Some(mgr) => {
                if !mgr.is_registered(&name) {
                    format!("Server '{}' is not registered.", name)
                } else {
                    // LspManager requires &mut self for unregister/register.
                    // Since it's behind Arc, we can't mutate it directly from
                    // this handler.  For full restart, re-run detect_servers()
                    // after toggling LSP off/on.
                    format!(
                        "Server '{}' is running. To restart, run:\n  /config lsp.enabled false\n  /config lsp.enabled true\n\nThis will restart all detected LSP servers.",
                        name
                    )
                }
            }
            None => "LSP manager not available.".to_string(),
        };
    }

    if parts[0] == "stop" && parts.len() >= 2 {
        let name = parts[1].to_string();
        return match &app.lsp_manager {
            Some(mgr) => {
                if !mgr.is_registered(&name) {
                    format!("Server '{}' is not registered.", name)
                } else {
                    format!(
                        "Server '{}' is running. To stop all LSP servers, run:\n  /config lsp.enabled false\n\nSelective stop will be available in a future update.",
                        name
                    )
                }
            }
            None => "LSP manager not available.".to_string(),
        };
    }

    "Usage: /lsp [list|restart <name>|stop <name>]".to_string()
}
