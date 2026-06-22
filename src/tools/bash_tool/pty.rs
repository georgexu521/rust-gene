//! Bash tool support module.
//!
//! Separates process execution, background handling, PTY behavior, and command classification from the tool entrypoint.

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

const MAX_PTY_OUTPUT_BYTES: usize = 1_000_000;

#[derive(Debug)]
pub struct PtyRunOutput {
    pub output: String,
    pub exit_code: i32,
    pub timed_out: bool,
}

pub async fn run_pty_shell(
    actual_command: String,
    working_dir: PathBuf,
    timeout_secs: u64,
) -> Result<PtyRunOutput, String> {
    tokio::task::spawn_blocking(move || {
        run_pty_shell_blocking(actual_command, working_dir, timeout_secs)
    })
    .await
    .map_err(|err| format!("PTY task failed: {err}"))?
}

fn run_pty_shell_blocking(
    actual_command: String,
    working_dir: PathBuf,
    timeout_secs: u64,
) -> Result<PtyRunOutput, String> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|err| format!("Failed to open PTY: {err}"))?;

    let mut command = CommandBuilder::new("bash");
    // Match the foreground bash path: avoid login-shell startup files in PTY
    // mode, because user shell initialization can block short command tests.
    command.args(["-c", actual_command.as_str()]);
    command.cwd(working_dir.as_os_str());
    command.env("TERM", "xterm-256color");
    command.env("PRIORITY_AGENT_TERMINAL", "pty");
    sanitize_agent_runtime_env(&mut command);

    let mut child = pair
        .slave
        .spawn_command(command)
        .map_err(|err| format!("Failed to spawn PTY command: {err}"))?;
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|err| format!("Failed to read PTY output: {err}"))?;
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let deadline = Instant::now() + Duration::from_secs(timeout_secs.max(1));
    let mut output = Vec::new();
    let mut output_truncated = false;
    let mut timed_out = false;
    let mut exit_code = -1;

    loop {
        drain_pty_chunks(&rx, &mut output, &mut output_truncated);
        match child
            .try_wait()
            .map_err(|err| format!("Failed to poll PTY command: {err}"))?
        {
            Some(status) => {
                exit_code = status.exit_code() as i32;
                break;
            }
            None if Instant::now() >= deadline => {
                timed_out = true;
                let _ = child.kill();
                for _ in 0..50 {
                    match child
                        .try_wait()
                        .map_err(|err| format!("Failed to poll killed PTY command: {err}"))?
                    {
                        Some(status) => {
                            exit_code = status.exit_code() as i32;
                            break;
                        }
                        None => std::thread::sleep(Duration::from_millis(20)),
                    }
                }
                break;
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }

    drain_pty_chunks_until_quiet(&rx, &mut output, &mut output_truncated);
    if output_truncated {
        output.extend_from_slice(b"\n[PTY output truncated by runner at 1000000 bytes]\n");
    }

    Ok(PtyRunOutput {
        output: String::from_utf8_lossy(&output).to_string(),
        exit_code,
        timed_out,
    })
}

fn drain_pty_chunks(rx: &Receiver<Vec<u8>>, output: &mut Vec<u8>, truncated: &mut bool) {
    while let Ok(chunk) = rx.try_recv() {
        append_bounded(output, &chunk, truncated);
    }
}

fn drain_pty_chunks_until_quiet(
    rx: &Receiver<Vec<u8>>,
    output: &mut Vec<u8>,
    truncated: &mut bool,
) {
    let quiet_deadline = Instant::now() + Duration::from_millis(200);
    while Instant::now() < quiet_deadline {
        match rx.recv_timeout(Duration::from_millis(20)) {
            Ok(chunk) => append_bounded(output, &chunk, truncated),
            Err(mpsc::RecvTimeoutError::Timeout) => break,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    drain_pty_chunks(rx, output, truncated);
}

fn append_bounded(output: &mut Vec<u8>, chunk: &[u8], truncated: &mut bool) {
    if output.len() >= MAX_PTY_OUTPUT_BYTES {
        *truncated = true;
        return;
    }
    let remaining = MAX_PTY_OUTPUT_BYTES - output.len();
    if chunk.len() > remaining {
        output.extend_from_slice(&chunk[..remaining]);
        *truncated = true;
    } else {
        output.extend_from_slice(chunk);
    }
}

fn sanitize_agent_runtime_env(command: &mut CommandBuilder) {
    for key in [
        "PRIORITY_AGENT_A2A_TRANSCRIPT_PATH",
        "PRIORITY_AGENT_AUTO_REVIEW",
        "PRIORITY_AGENT_AUTO_TEST",
        "PRIORITY_AGENT_BASH_BACKEND",
        "PRIORITY_AGENT_BASH_EXTERNAL_ALLOWLIST",
        "PRIORITY_AGENT_BASH_EXTERNAL_CMD",
        "PRIORITY_AGENT_BASH_EXTERNAL_FALLBACK",
        "PRIORITY_AGENT_BASH_EXTERNAL_WRAPPER_ALLOWLIST",
        "PRIORITY_AGENT_BASH_SANDBOX_CMD",
        "PRIORITY_AGENT_BASH_SANDBOX_FALLBACK",
        "PRIORITY_AGENT_BASH_TIMEOUT_FLOOR_SECS",
        "PRIORITY_AGENT_CLOSEOUT_VISIBILITY",
        "PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE",
        "PRIORITY_AGENT_EVAL_EVENTS",
        "PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED",
        "PRIORITY_AGENT_LLM_MEMORY_EXTRACTION",
        "PRIORITY_AGENT_ROUTE_SCOPED_TOOLS",
        "PRIORITY_AGENT_TOOL_PROFILE",
        "PRIORITY_AGENT_WORKFLOW_CONTRACT",
        "PRIORITY_AGENT_WORKFLOW_ENABLED",
    ] {
        command.env_remove(key);
    }
}
