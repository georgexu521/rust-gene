//! CLI 模块 - 命令行界面
//!
//! 提供命令解析、交互式提示和输出格式化

pub mod commands;
pub mod display;
pub mod interactive;

pub use commands::{Cli, Commands};
pub use display::{format_progress, format_task_tree, print_banner};
pub use interactive::{prompt_project, prompt_task, select_task};

/// 运行 CLI 模式（legacy-cli 模式入口）
#[cfg(feature = "legacy-cli")]
pub async fn run_cli() {
    use crate::cli::commands::Commands;

    let cli = Cli::parse();
    match cli.command {
        Commands::Help => crate::cli::commands::print_help(),
        Commands::Init => println!("Initializing project..."),
        Commands::AddTask { name } => println!("Adding task: {}", name),
        Commands::List => println!("Listing tasks..."),
        Commands::Next => println!("Next recommended task..."),
        Commands::CompleteTask { id } => println!("Completing task: {}", id),
        Commands::Progress => println!("Showing progress..."),
        Commands::Analyze => println!("Analyzing project..."),
        Commands::Snapshot { name } => println!("Creating snapshot: {:?}", name),
        Commands::Restore { id } => println!("Restoring: {}", id),
        Commands::Interactive => {
            println!("\n=== Priority Agent CLI ===");
            crate::cli::commands::print_help();
            println!();
            if let Err(e) = run_interactive().await {
                eprintln!("Interactive error: {}", e);
            }
        }
    }
}

#[cfg(feature = "legacy-cli")]
async fn run_interactive() -> anyhow::Result<()> {
    loop {
        let task = crate::cli::interactive::prompt_task()?;
        println!("Created task: {}", task.name);
    }
}

#[cfg(not(feature = "legacy-cli"))]
pub async fn run_cli() {
    eprintln!("CLI mode not compiled in (missing legacy-cli feature)");
}
