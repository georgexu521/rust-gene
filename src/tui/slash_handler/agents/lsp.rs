use super::*;

/// /lsp - LSP 服务器管理
pub fn handle_lsp(app: &TuiApp, args: &str) -> String {
    if let Some(ref mgr) = app.lsp_manager {
        let parts: Vec<&str> = args.split_whitespace().collect();
        if parts.is_empty() || parts[0] == "list" {
            let servers = mgr.server_names();
            if servers.is_empty() {
                "No LSP servers running.".to_string()
            } else {
                format!("LSP servers ({}):\n{}", servers.len(), servers.join("\n"))
            }
        } else if parts[0] == "restart" && parts.len() >= 2 {
            let _name = parts[1];
            format!("Restarting LSP server: {}...", _name)
        } else if parts[0] == "stop" && parts.len() >= 2 {
            let _name = parts[1];
            format!("Stopping LSP server: {}...", _name)
        } else {
            "Usage: /lsp [list|restart <name>|stop <name>]".to_string()
        }
    } else {
        "LSP manager not available.".to_string()
    }
}
