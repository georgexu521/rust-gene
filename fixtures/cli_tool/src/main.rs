use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use serde::Deserialize;

#[derive(Parser, Debug)]
#[command(name = "cli_tool", about = "A small CLI tool with config", version)]
struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Path to a custom config file
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Create a default config file at ~/.cli_tool/config.toml
    Init,
    /// Read config and run with the configured name
    Run,
    /// Print the config path and current name
    Status,
}

#[derive(Deserialize, Debug, Default, Clone)]
struct Config {
    #[serde(default)]
    name: String,
}

const DEFAULT_NAME: &str = "cli_tool";
const CONFIG_DIR: &str = ".cli_tool";
const CONFIG_FILE: &str = "config.toml";

fn default_config_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join(CONFIG_DIR)
            .join(CONFIG_FILE)
    })
}

fn config_path(cli: &Cli) -> PathBuf {
    if let Some(p) = &cli.config {
        return p.clone();
    }
    default_config_path().unwrap_or_else(|| PathBuf::from(CONFIG_FILE))
}

fn load_config(path: &Path, verbose: bool) -> Result<Config, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str::<Config>(&text)
            .map_err(|e| format!("Failed to parse config {}: {}", path.display(), e)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if verbose {
                eprintln!("[verbose] config not found at {}", path.display());
            }
            Err(format!(
                "Config not found. Run `cli_tool init` first. (looked at {})",
                path.display()
            ))
        }
        Err(e) => Err(format!("Failed to read config {}: {}", path.display(), e)),
    }
}

fn write_default_config(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config dir {}: {}", parent.display(), e))?;
    }
    let body = "name = \"my-project\"\n";
    std::fs::write(path, body)
        .map_err(|e| format!("Failed to write config {}: {}", path.display(), e))
}

fn run_init(cli: &Cli) -> ExitCode {
    let path = config_path(cli);
    match std::fs::metadata(&path) {
        Ok(_) => {
            eprintln!(
                "Config already exists at {}. Refusing to overwrite.",
                path.display()
            );
            return ExitCode::from(1);
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            eprintln!("Failed to inspect config {}: {}", path.display(), e);
            return ExitCode::from(1);
        }
    }
    if let Err(msg) = write_default_config(&path) {
        eprintln!("{}", msg);
        return ExitCode::from(1);
    }
    println!("Created config at {}", path.display());
    ExitCode::SUCCESS
}

fn run_status(cli: &Cli) -> ExitCode {
    let path = config_path(cli);
    println!("config: {}", path.display());
    match load_config(&path, cli.verbose) {
        Ok(cfg) => {
            let name = if cfg.name.is_empty() {
                DEFAULT_NAME
            } else {
                cfg.name.as_str()
            };
            println!("name: {}", name);
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("{}", msg);
            ExitCode::from(1)
        }
    }
}

fn run_run(cli: &Cli) -> ExitCode {
    let path = config_path(cli);
    match load_config(&path, cli.verbose) {
        Ok(cfg) => {
            let name = if cfg.name.is_empty() {
                DEFAULT_NAME
            } else {
                cfg.name.as_str()
            };
            println!("Running {}...", name);
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("{}", msg);
            ExitCode::from(1)
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init => run_init(&cli),
        Commands::Run => run_run(&cli),
        Commands::Status => run_status(&cli),
    }
}
