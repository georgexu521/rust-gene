use crate::engine::streaming::StreamingQueryEngine;
use crate::shell::{self, ShellOptions};
use std::sync::Arc;

pub async fn run_cli(engine: Arc<StreamingQueryEngine>, no_footer: bool) -> anyhow::Result<()> {
    shell::run_shell_with_options(
        engine,
        ShellOptions {
            no_footer,
            lab_mode: false,
        },
    )
    .await
}
