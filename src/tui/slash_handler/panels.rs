//! Runtime panel slash-command handlers.

use crate::tui::app::TuiApp;
use crate::tui::runtime_panels::{render_runtime_panel, RuntimePanelKind};

pub async fn handle_panel(app: &TuiApp, args: &str) -> String {
    match RuntimePanelKind::parse(args) {
        Some(kind) => render_runtime_panel(app, kind).await,
        None => RuntimePanelKind::usage().to_string(),
    }
}
