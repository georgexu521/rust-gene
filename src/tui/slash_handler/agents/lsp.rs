use super::*;

/// /lsp - LSP server management.
pub async fn handle_lsp(app: &TuiApp, args: &str) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();

    if parts.is_empty() || parts[0] == "list" {
        return match &app.lsp_manager {
            Some(mgr) => {
                let servers = mgr.server_names();
                if servers.is_empty() {
                    "No LSP servers running. Enable LSP with /config set lsp.enabled true"
                        .to_string()
                } else {
                    format!("LSP servers ({}):\n{}", servers.len(), servers.join("\n"))
                }
            }
            None => {
                "LSP manager not available. Enable with /config set lsp.enabled true.".to_string()
            }
        };
    }

    if parts[0] == "restart" && parts.len() >= 2 {
        let name = parts[1].to_string();
        return match &app.lsp_manager {
            Some(mgr) => {
                if !mgr.is_registered(&name) {
                    format!("Server '{}' is not registered.", name)
                } else {
                    match mgr.restart_server(&name).await {
                        Ok(()) => format!("Restarted LSP server: {}", name),
                        Err(err) => format!("Failed to restart LSP server '{}': {}", name, err),
                    }
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
                    match mgr.stop_server(&name).await {
                        Ok(()) => format!("Stopped LSP server: {}", name),
                        Err(err) => format!("Failed to stop LSP server '{}': {}", name, err),
                    }
                }
            }
            None => "LSP manager not available.".to_string(),
        };
    }

    "Usage: /lsp [list|restart <name>|stop <name>]".to_string()
}
