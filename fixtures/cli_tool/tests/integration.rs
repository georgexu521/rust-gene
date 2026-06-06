use std::process::Command;

use tempfile::TempDir;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cli_tool"))
}

#[test]
fn init_creates_default_config_in_tempdir() {
    let tmp = TempDir::new().expect("tempdir");
    let config_path = tmp.path().join("config.toml");

    let out = bin()
        .args(["init", "--config", config_path.to_str().unwrap()])
        .output()
        .expect("run init");
    assert!(out.status.success(), "init failed: {:?}", out);
    assert!(config_path.exists(), "config file should exist");

    let body = std::fs::read_to_string(&config_path).unwrap();
    assert!(body.contains("name = \"my-project\""));
}

#[test]
fn init_then_status_reports_name_from_config() {
    let tmp = TempDir::new().expect("tempdir");
    let config_path = tmp.path().join("config.toml");

    let out = bin()
        .args(["init", "--config", config_path.to_str().unwrap()])
        .output()
        .expect("run init");
    assert!(out.status.success());

    let out = bin()
        .args(["status", "--config", config_path.to_str().unwrap()])
        .output()
        .expect("run status");
    assert!(out.status.success(), "status failed: {:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("name: my-project"),
        "expected 'name: my-project' in stdout, got: {}",
        stdout
    );
    assert!(
        stdout.contains(&config_path.display().to_string()),
        "expected config path in stdout, got: {}",
        stdout
    );
}

#[test]
fn run_prints_running_with_configured_name() {
    let tmp = TempDir::new().expect("tempdir");
    let config_path = tmp.path().join("config.toml");
    std::fs::write(&config_path, "name = \"demo-app\"\n").unwrap();

    let out = bin()
        .args(["run", "--config", config_path.to_str().unwrap()])
        .output()
        .expect("run");
    assert!(out.status.success(), "run failed: {:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Running demo-app"),
        "expected 'Running demo-app...' in stdout, got: {}",
        stdout
    );
}

#[test]
fn run_without_config_exits_nonzero_with_helpful_message() {
    let tmp = TempDir::new().expect("tempdir");
    let missing = tmp.path().join("nope.toml");

    let out = bin()
        .args(["run", "--config", missing.to_str().unwrap()])
        .output()
        .expect("run");
    assert!(!out.status.success(), "expected non-zero exit without config");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Config not found"),
        "expected helpful error, got: {}",
        stderr
    );
    assert!(
        stderr.contains("init"),
        "expected message to point at `init`, got: {}",
        stderr
    );
}

#[test]
fn verbose_flag_is_accepted() {
    let tmp = TempDir::new().expect("tempdir");
    let config_path = tmp.path().join("config.toml");
    std::fs::write(&config_path, "name = \"loud\"\n").unwrap();

    let out = bin()
        .args(["-v", "status", "--config", config_path.to_str().unwrap()])
        .output()
        .expect("run status");
    assert!(out.status.success(), "status -v failed: {:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("name: loud"));
}
