use super::{
    bash_permission_review_data, classification_data, command_classifier::classify_command,
    kill_process_tree, preview_text, sanitize_agent_runtime_env, shell_output_artifact_path,
    should_write_shell_output_artifact, BashExecutionBackend,
};
use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::{oneshot, Mutex, RwLock};
use uuid::Uuid;

const BACKGROUND_BUFFER_LIMIT_CHARS: usize = 200_000;
const DEFAULT_READ_PREVIEW_CHARS: usize = 4_000;

static BACKGROUND_SHELLS: Lazy<BackgroundShellManager> = Lazy::new(BackgroundShellManager::new);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundShellStatus {
    Running,
    Completed,
    Failed,
    TimedOut,
    Cancelled,
}

impl BackgroundShellStatus {
    fn as_str(self) -> &'static str {
        match self {
            BackgroundShellStatus::Running => "running",
            BackgroundShellStatus::Completed => "completed",
            BackgroundShellStatus::Failed => "failed",
            BackgroundShellStatus::TimedOut => "timed_out",
            BackgroundShellStatus::Cancelled => "cancelled",
        }
    }

    fn is_terminal(self) -> bool {
        !matches!(self, BackgroundShellStatus::Running)
    }
}

#[derive(Debug)]
struct BackgroundShellState {
    command: String,
    working_dir: PathBuf,
    backend: BashExecutionBackend,
    timeout_secs: u64,
    started_at: Instant,
    started_at_wall: SystemTime,
    completed_at: Option<Instant>,
    completed_at_wall: Option<SystemTime>,
    status: BackgroundShellStatus,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    stdout_truncated: bool,
    stderr_truncated: bool,
}

impl BackgroundShellState {
    fn append_stdout(&mut self, text: &str) {
        if append_bounded(&mut self.stdout, text) {
            self.stdout_truncated = true;
        }
    }

    fn append_stderr(&mut self, text: &str) {
        if append_bounded(&mut self.stderr, text) {
            self.stderr_truncated = true;
        }
    }

    fn mark_done(&mut self, status: BackgroundShellStatus, exit_code: Option<i32>) {
        self.status = status;
        self.exit_code = exit_code;
        self.completed_at = Some(Instant::now());
        self.completed_at_wall = Some(SystemTime::now());
    }
}

#[derive(Clone)]
struct BackgroundShellRecord {
    state: Arc<Mutex<BackgroundShellState>>,
    cancel_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

struct BackgroundShellManager {
    processes: RwLock<HashMap<String, BackgroundShellRecord>>,
}

#[derive(Debug, Clone)]
pub(super) struct BackgroundShellSnapshot {
    handle: String,
    command: String,
    working_dir: PathBuf,
    backend: BashExecutionBackend,
    timeout_secs: u64,
    status: BackgroundShellStatus,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    stdout_truncated: bool,
    stderr_truncated: bool,
    duration_ms: u64,
    started_at_ms: u128,
    ended_at_ms: Option<u128>,
}

impl BackgroundShellSnapshot {
    fn truncated(&self) -> bool {
        self.stdout_truncated || self.stderr_truncated
    }

    fn timed_out(&self) -> bool {
        self.status == BackgroundShellStatus::TimedOut
    }

    fn cancelled(&self) -> bool {
        self.status == BackgroundShellStatus::Cancelled
    }

    fn combined_output(&self) -> String {
        let mut output = String::new();
        if !self.stdout.is_empty() {
            output.push_str(&self.stdout);
        }
        if !self.stderr.is_empty() {
            if !output.is_empty() {
                output.push_str("\n\n[stderr]:\n");
            } else {
                output.push_str("[stderr]:\n");
            }
            output.push_str(&self.stderr);
        }
        output
    }

    fn data(&self, max_chars: usize, output_path: Option<String>) -> serde_json::Value {
        let (stdout_preview, stdout_truncated) = preview_text(&self.stdout, max_chars);
        let (stderr_preview, stderr_truncated) = preview_text(&self.stderr, max_chars);
        let output_path_for_task = output_path.clone();
        let output = self.combined_output();
        let output_persisted = output_path_for_task.is_some();
        let output_available = output_persisted || !output.trim().is_empty();
        json!({
            "shell_background": {
                "handle": self.handle,
                "command": self.command,
                "cwd": self.working_dir.display().to_string(),
                "backend": self.backend.as_str(),
                "timeout_secs": self.timeout_secs,
                "status": self.status.as_str(),
                "running": self.status == BackgroundShellStatus::Running,
                "exit_code": self.exit_code,
                "duration_ms": self.duration_ms,
                "timed_out": self.timed_out(),
                "cancelled": self.cancelled(),
                "truncated": self.truncated() || stdout_truncated || stderr_truncated,
                "output_path": output_path,
                "output_persisted": output_persisted,
                "output_bytes": output.len(),
                "stdout_bytes": self.stdout.len(),
                "stderr_bytes": self.stderr.len(),
                "stdout_preview": stdout_preview,
                "stderr_preview": stderr_preview,
                "classification": classification_data(&self.command),
            },
            "terminal_task": {
                "task_id": self.handle,
                "handle": self.handle,
                "command": self.command,
                "cwd": self.working_dir.display().to_string(),
                "status": self.status.as_str(),
                "started_at_ms": self.started_at_ms,
                "ended_at_ms": self.ended_at_ms,
                "duration_ms": self.duration_ms,
                "exit_code": self.exit_code,
                "output_path": output_path_for_task,
                "output_available": output_available,
                "output_persisted": output_persisted,
                "output_bytes": output.len(),
                "stdout_bytes": self.stdout.len(),
                "stderr_bytes": self.stderr.len(),
                "read_tool": "bash_output",
                "cancel_tool": "bash_cancel",
                "cancel_handle": if self.status == BackgroundShellStatus::Running {
                    serde_json::Value::String(self.handle.clone())
                } else {
                    serde_json::Value::Null
                },
                "terminal_kind": "background_shell"
            }
        })
    }
}

fn background_handle_param(params: &serde_json::Value) -> &str {
    params["handle"]
        .as_str()
        .or_else(|| params["task_id"].as_str())
        .unwrap_or("")
}

impl BackgroundShellManager {
    fn new() -> Self {
        Self {
            processes: RwLock::new(HashMap::new()),
        }
    }

    async fn insert(
        &self,
        handle: String,
        state: Arc<Mutex<BackgroundShellState>>,
        cancel_tx: oneshot::Sender<()>,
    ) {
        self.processes.write().await.insert(
            handle,
            BackgroundShellRecord {
                state,
                cancel_tx: Arc::new(Mutex::new(Some(cancel_tx))),
            },
        );
    }

    async fn get(&self, handle: &str) -> Option<BackgroundShellRecord> {
        self.processes.read().await.get(handle).cloned()
    }

    async fn snapshot(&self, handle: &str) -> Option<BackgroundShellSnapshot> {
        let record = self.get(handle).await?;
        let state = record.state.lock().await;
        Some(snapshot_from_state(handle, &state))
    }

    async fn list(&self) -> Vec<BackgroundShellSnapshot> {
        let records = self
            .processes
            .read()
            .await
            .iter()
            .map(|(handle, record)| (handle.clone(), record.clone()))
            .collect::<Vec<_>>();
        let mut snapshots = Vec::with_capacity(records.len());
        for (handle, record) in records {
            let state = record.state.lock().await;
            snapshots.push(snapshot_from_state(&handle, &state));
        }
        snapshots.sort_by(|a, b| a.handle.cmp(&b.handle));
        snapshots
    }

    async fn cancel(&self, handle: &str) -> Result<BackgroundShellSnapshot, String> {
        let record = self
            .get(handle)
            .await
            .ok_or_else(|| format!("background shell handle not found: {handle}"))?;

        if record.state.lock().await.status.is_terminal() {
            return self
                .snapshot(handle)
                .await
                .ok_or_else(|| format!("background shell handle not found: {handle}"));
        }

        if let Some(tx) = record.cancel_tx.lock().await.take() {
            let _ = tx.send(());
        }

        for _ in 0..20 {
            if let Some(snapshot) = self.snapshot(handle).await {
                if snapshot.status.is_terminal() {
                    return Ok(snapshot);
                }
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        self.snapshot(handle)
            .await
            .ok_or_else(|| format!("background shell handle not found: {handle}"))
    }
}

pub(super) async fn start_background_shell(
    command: &str,
    actual_command: &str,
    working_dir: &Path,
    backend: BashExecutionBackend,
    timeout_secs: u64,
) -> Result<BackgroundShellSnapshot, String> {
    let mut cmd = Command::new("bash");
    cmd.arg("-c")
        .arg(actual_command)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    sanitize_agent_runtime_env(&mut cmd);

    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let mut child = cmd
        .spawn()
        .map_err(|err| format!("Failed to spawn background command: {err}"))?;
    let child_pid = child.id().map(|id| id as i32);
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let handle = format!("shell_{}", Uuid::new_v4().simple());
    let state = Arc::new(Mutex::new(BackgroundShellState {
        command: command.to_string(),
        working_dir: working_dir.to_path_buf(),
        backend,
        timeout_secs,
        started_at: Instant::now(),
        started_at_wall: SystemTime::now(),
        completed_at: None,
        completed_at_wall: None,
        status: BackgroundShellStatus::Running,
        exit_code: None,
        stdout: String::new(),
        stderr: String::new(),
        stdout_truncated: false,
        stderr_truncated: false,
    }));
    let (cancel_tx, cancel_rx) = oneshot::channel();

    BACKGROUND_SHELLS
        .insert(handle.clone(), state.clone(), cancel_tx)
        .await;

    if let Some(stdout) = stdout {
        tokio::spawn(read_stream(stdout, state.clone(), StreamKind::Stdout));
    }
    if let Some(stderr) = stderr {
        tokio::spawn(read_stream(stderr, state.clone(), StreamKind::Stderr));
    }

    tokio::spawn(supervise_child(
        child,
        child_pid,
        state.clone(),
        cancel_rx,
        timeout_secs,
    ));

    BACKGROUND_SHELLS
        .snapshot(&handle)
        .await
        .ok_or_else(|| "background shell failed to register".to_string())
}

async fn read_background_shell(handle: &str) -> Result<BackgroundShellSnapshot, String> {
    BACKGROUND_SHELLS
        .snapshot(handle)
        .await
        .ok_or_else(|| format!("background shell handle not found: {handle}"))
}

async fn cancel_background_shell(handle: &str) -> Result<BackgroundShellSnapshot, String> {
    BACKGROUND_SHELLS.cancel(handle).await
}

async fn list_background_shells() -> Vec<BackgroundShellSnapshot> {
    BACKGROUND_SHELLS.list().await
}

async fn supervise_child(
    mut child: tokio::process::Child,
    child_pid: Option<i32>,
    state: Arc<Mutex<BackgroundShellState>>,
    cancel_rx: oneshot::Receiver<()>,
    timeout_secs: u64,
) {
    tokio::pin!(cancel_rx);
    tokio::select! {
        status = child.wait() => {
            let exit_code = status.ok().and_then(|status| status.code()).unwrap_or(-1);
            let final_status = if exit_code == 0 {
                BackgroundShellStatus::Completed
            } else {
                BackgroundShellStatus::Failed
            };
            state.lock().await.mark_done(final_status, Some(exit_code));
        }
        _ = &mut cancel_rx => {
            kill_process_tree(child_pid);
            let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
            state.lock().await.mark_done(BackgroundShellStatus::Cancelled, None);
        }
        _ = tokio::time::sleep(Duration::from_secs(timeout_secs)), if timeout_secs > 0 => {
            kill_process_tree(child_pid);
            let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
            state.lock().await.mark_done(BackgroundShellStatus::TimedOut, None);
        }
    }
}

enum StreamKind {
    Stdout,
    Stderr,
}

async fn read_stream<R>(mut reader: R, state: Arc<Mutex<BackgroundShellState>>, kind: StreamKind)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                let text = String::from_utf8_lossy(&buf[..n]);
                let mut state = state.lock().await;
                match kind {
                    StreamKind::Stdout => state.append_stdout(&text),
                    StreamKind::Stderr => state.append_stderr(&text),
                }
            }
            Err(err) => {
                let mut state = state.lock().await;
                state.append_stderr(&format!("\n[background output read error: {err}]\n"));
                break;
            }
        }
    }
}

fn snapshot_from_state(handle: &str, state: &BackgroundShellState) -> BackgroundShellSnapshot {
    let end = state.completed_at.unwrap_or_else(Instant::now);
    BackgroundShellSnapshot {
        handle: handle.to_string(),
        command: state.command.clone(),
        working_dir: state.working_dir.clone(),
        backend: state.backend,
        timeout_secs: state.timeout_secs,
        status: state.status,
        exit_code: state.exit_code,
        stdout: state.stdout.clone(),
        stderr: state.stderr.clone(),
        stdout_truncated: state.stdout_truncated,
        stderr_truncated: state.stderr_truncated,
        duration_ms: end.duration_since(state.started_at).as_millis() as u64,
        started_at_ms: system_time_millis(state.started_at_wall),
        ended_at_ms: state.completed_at_wall.map(system_time_millis),
    }
}

fn system_time_millis(time: SystemTime) -> u128 {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn append_bounded(target: &mut String, text: &str) -> bool {
    let current = target.chars().count();
    if current >= BACKGROUND_BUFFER_LIMIT_CHARS {
        return !text.is_empty();
    }
    let remaining = BACKGROUND_BUFFER_LIMIT_CHARS - current;
    let incoming = text.chars().count();
    if incoming <= remaining {
        target.push_str(text);
        false
    } else {
        target.extend(text.chars().take(remaining));
        true
    }
}

fn background_content(snapshot: &BackgroundShellSnapshot, max_chars: usize) -> String {
    let output = snapshot.combined_output();
    let (preview, truncated) = preview_text(&output, max_chars);
    let mut lines = vec![format!(
        "Background shell {} is {}.",
        snapshot.handle,
        snapshot.status.as_str()
    )];
    if let Some(exit_code) = snapshot.exit_code {
        lines.push(format!("Exit code: {exit_code}."));
    }
    if !preview.trim().is_empty() {
        lines.push(String::new());
        lines.push(preview);
    }
    if truncated || snapshot.truncated() {
        lines.push(String::new());
        lines.push(
            "[Output truncated; call bash_output again for the current bounded buffer.]"
                .to_string(),
        );
    }
    lines.join("\n")
}

fn background_output_artifact_path(
    snapshot: &BackgroundShellSnapshot,
    context: &ToolContext,
    max_chars: usize,
) -> Option<String> {
    let output = snapshot.combined_output();
    let (_, preview_truncated) = preview_text(&output, max_chars);
    if snapshot.truncated() || should_write_shell_output_artifact(&output, preview_truncated) {
        shell_output_artifact_path(context, &snapshot.working_dir, &snapshot.command, &output)
    } else {
        None
    }
}

pub(super) fn background_started_content(snapshot: &BackgroundShellSnapshot) -> String {
    format!(
        "Started background shell command.\nHandle: {}\nStatus: {}.\nUse bash_output with this handle to read output; use bash_cancel to stop it.",
        snapshot.handle,
        snapshot.status.as_str()
    )
}

pub(super) fn background_shell_result_data(
    snapshot: &BackgroundShellSnapshot,
) -> serde_json::Value {
    let classification = classification_data(&snapshot.command);
    let command_classification = classify_command(&snapshot.command);
    let permission_review = bash_permission_review_data(
        &snapshot.command,
        &command_classification,
        snapshot.backend,
        "background",
        false,
    );
    json!({
        "command_classification": classification.clone(),
        "permission_review": permission_review,
        "shell_result": {
            "handle": snapshot.handle,
            "command": snapshot.command,
            "cwd": snapshot.working_dir.display().to_string(),
            "exit_code": snapshot.exit_code,
            "stdout_preview": "",
            "stderr_preview": "",
            "output_path": serde_json::Value::Null,
            "duration_ms": snapshot.duration_ms,
            "timed_out": false,
            "truncated": false,
            "classification": classification,
            "evidence_status": "running",
            "status": snapshot.status.as_str(),
            "background": true,
        },
        "shell_background": snapshot.data(DEFAULT_READ_PREVIEW_CHARS, None)["shell_background"].clone(),
        "terminal_task": snapshot.data(DEFAULT_READ_PREVIEW_CHARS, None)["terminal_task"].clone(),
        "execution": {
            "exit_code": serde_json::Value::Null,
            "backend": snapshot.backend.as_str(),
            "background": true,
            "truncated": false,
        }
    })
}

pub struct BashOutputTool;

#[async_trait]
impl Tool for BashOutputTool {
    fn name(&self) -> &str {
        "bash_output"
    }

    fn description(&self) -> &str {
        "Read output from a background bash task returned by bash mode=background."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "handle": {
                    "type": "string",
                    "description": "Background shell handle returned by bash"
                },
                "task_id": {
                    "type": "string",
                    "description": "Terminal task id returned by bash; accepted as an alias for handle"
                },
                "max_chars": {
                    "type": "integer",
                    "description": "Maximum output preview characters (default: 4000)",
                    "default": 4000
                }
            },
            "required": []
        })
    }

    fn search_hint(&self) -> Option<&'static str> {
        Some("read background shell output")
    }

    fn strict_schema(&self) -> bool {
        true
    }

    fn operation_kind(&self, _params: &serde_json::Value) -> ToolOperationKind {
        ToolOperationKind::Read
    }

    fn is_read_only(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _params: &serde_json::Value) -> bool {
        true
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let handle = background_handle_param(&params);
        if handle.trim().is_empty() {
            return ToolResult::error("handle or task_id cannot be empty");
        }
        let max_chars = params["max_chars"]
            .as_u64()
            .unwrap_or(DEFAULT_READ_PREVIEW_CHARS as u64)
            .clamp(200, 20_000) as usize;

        match read_background_shell(handle).await {
            Ok(snapshot) => {
                let output_path = background_output_artifact_path(&snapshot, &context, max_chars);
                ToolResult::success_with_data(
                    background_content(&snapshot, max_chars),
                    snapshot.data(max_chars, output_path),
                )
            }
            Err(err) => ToolResult::error(err),
        }
    }
}

pub struct BashCancelTool;

#[async_trait]
impl Tool for BashCancelTool {
    fn name(&self) -> &str {
        "bash_cancel"
    }

    fn description(&self) -> &str {
        "Cancel a background bash task by handle or task_id."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "handle": {
                    "type": "string",
                    "description": "Background shell handle returned by bash"
                },
                "task_id": {
                    "type": "string",
                    "description": "Terminal task id returned by bash; accepted as an alias for handle"
                }
            },
            "required": []
        })
    }

    fn search_hint(&self) -> Option<&'static str> {
        Some("stop background shell command")
    }

    fn strict_schema(&self) -> bool {
        true
    }

    fn operation_kind(&self, _params: &serde_json::Value) -> ToolOperationKind {
        ToolOperationKind::Task
    }

    fn is_read_only(&self, _params: &serde_json::Value) -> bool {
        false
    }

    fn is_concurrency_safe(&self, _params: &serde_json::Value) -> bool {
        false
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let handle = background_handle_param(&params);
        if handle.trim().is_empty() {
            return ToolResult::error("handle or task_id cannot be empty");
        }

        match cancel_background_shell(handle).await {
            Ok(snapshot) => {
                let output_path = background_output_artifact_path(
                    &snapshot,
                    &context,
                    DEFAULT_READ_PREVIEW_CHARS,
                );
                ToolResult::success_with_data(
                    format!(
                        "Background shell {} is {}.",
                        snapshot.handle,
                        snapshot.status.as_str()
                    ),
                    snapshot.data(DEFAULT_READ_PREVIEW_CHARS, output_path),
                )
            }
            Err(err) => ToolResult::error(err),
        }
    }
}

pub struct BashTasksTool;

#[async_trait]
impl Tool for BashTasksTool {
    fn name(&self) -> &str {
        "bash_tasks"
    }

    fn description(&self) -> &str {
        "List background bash handles and current status."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    fn search_hint(&self) -> Option<&'static str> {
        Some("list background shell commands")
    }

    fn strict_schema(&self) -> bool {
        true
    }

    fn operation_kind(&self, _params: &serde_json::Value) -> ToolOperationKind {
        ToolOperationKind::List
    }

    fn is_read_only(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _params: &serde_json::Value) -> bool {
        true
    }

    async fn execute(&self, _params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let snapshots = list_background_shells().await;
        if snapshots.is_empty() {
            return ToolResult::success_with_data(
                "No background shell tasks.".to_string(),
                json!({ "shell_background_tasks": [], "terminal_tasks": [] }),
            );
        }

        let lines = snapshots
            .iter()
            .map(|snapshot| {
                let exit = snapshot
                    .exit_code
                    .map(|code| format!(" exit={code}"))
                    .unwrap_or_default();
                format!(
                    "{} {}{} · {}",
                    snapshot.handle,
                    snapshot.status.as_str(),
                    exit,
                    snapshot.command
                )
            })
            .collect::<Vec<_>>();
        let tasks = snapshots
            .iter()
            .map(|snapshot| {
                snapshot.data(DEFAULT_READ_PREVIEW_CHARS, None)["shell_background"].clone()
            })
            .collect::<Vec<_>>();
        let terminal_tasks = snapshots
            .iter()
            .map(|snapshot| {
                snapshot.data(DEFAULT_READ_PREVIEW_CHARS, None)["terminal_task"].clone()
            })
            .collect::<Vec<_>>();
        ToolResult::success_with_data(
            format!(
                "Background shell tasks ({}):\n{}",
                lines.len(),
                lines.join("\n")
            ),
            json!({
                "shell_background_tasks": tasks,
                "terminal_tasks": terminal_tasks
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn background_shell_can_be_read_and_cancelled() {
        let dir = tempdir().expect("temp dir");
        let snapshot = start_background_shell(
            "printf ready; sleep 5",
            "printf ready; sleep 5",
            dir.path(),
            BashExecutionBackend::Local,
            30,
        )
        .await
        .expect("start background shell");

        tokio::time::sleep(Duration::from_millis(100)).await;
        let read = read_background_shell(&snapshot.handle)
            .await
            .expect("read background shell");
        assert_eq!(read.status, BackgroundShellStatus::Running);
        assert!(read.stdout.contains("ready"));

        let cancelled = cancel_background_shell(&snapshot.handle)
            .await
            .expect("cancel background shell");
        assert_eq!(cancelled.status, BackgroundShellStatus::Cancelled);
    }

    #[tokio::test]
    async fn bash_output_tool_reports_completed_output() {
        let dir = tempdir().expect("temp dir");
        let snapshot = start_background_shell(
            "printf done",
            "printf done",
            dir.path(),
            BashExecutionBackend::Local,
            30,
        )
        .await
        .expect("start background shell");

        for _ in 0..20 {
            let read = read_background_shell(&snapshot.handle)
                .await
                .expect("read background shell");
            if read.status.is_terminal() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        let tool = BashOutputTool;
        let result = tool
            .execute(
                json!({"task_id": snapshot.handle, "max_chars": 1000}),
                ToolContext::new(dir.path(), "test-background-output"),
            )
            .await;

        assert!(result.success, "output tool failed: {:?}", result.error);
        assert!(result.content.contains("done"));
        assert_eq!(
            result.data.as_ref().unwrap()["shell_background"]["status"],
            "completed"
        );
        let terminal_task = &result.data.as_ref().unwrap()["terminal_task"];
        assert_eq!(terminal_task["task_id"], snapshot.handle);
        assert_eq!(terminal_task["status"], "completed");
        assert_eq!(terminal_task["read_tool"], "bash_output");
        assert_eq!(terminal_task["cancel_handle"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn bash_output_tool_writes_artifact_for_long_output() {
        let dir = tempdir().expect("temp dir");
        let snapshot = start_background_shell(
            "printf '%12050s' x",
            "printf '%12050s' x",
            dir.path(),
            BashExecutionBackend::Local,
            30,
        )
        .await
        .expect("start background shell");

        for _ in 0..20 {
            let read = read_background_shell(&snapshot.handle)
                .await
                .expect("read background shell");
            if read.status.is_terminal() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        let result = BashOutputTool
            .execute(
                json!({"handle": snapshot.handle, "max_chars": 1000}),
                ToolContext::new(dir.path(), "test-background-artifact"),
            )
            .await;

        assert!(result.success, "output tool failed: {:?}", result.error);
        let output_path = result.data.as_ref().unwrap()["shell_background"]["output_path"]
            .as_str()
            .expect("long background output should be stored");
        assert!(output_path.starts_with(".priority-agent/tool-results/"));
        assert!(dir.path().join(output_path).exists());
        assert_eq!(
            result.data.as_ref().unwrap()["terminal_task"]["output_path"],
            output_path
        );
        assert_eq!(
            result.data.as_ref().unwrap()["terminal_task"]["output_available"],
            true
        );
        assert_eq!(
            result.data.as_ref().unwrap()["terminal_task"]["output_persisted"],
            true
        );
    }

    #[tokio::test]
    async fn bash_tasks_tool_lists_background_shells() {
        let dir = tempdir().expect("temp dir");
        let snapshot = start_background_shell(
            "printf listed; sleep 5",
            "printf listed; sleep 5",
            dir.path(),
            BashExecutionBackend::Local,
            30,
        )
        .await
        .expect("start background shell");

        let result = BashTasksTool
            .execute(
                json!({}),
                ToolContext::new(dir.path(), "test-background-list"),
            )
            .await;

        assert!(result.success, "tasks tool failed: {:?}", result.error);
        assert!(result.content.contains(&snapshot.handle));
        let tasks = result.data.as_ref().unwrap()["shell_background_tasks"]
            .as_array()
            .expect("tasks array");
        assert!(tasks
            .iter()
            .any(|task| task["handle"].as_str() == Some(snapshot.handle.as_str())));
        let terminal_tasks = result.data.as_ref().unwrap()["terminal_tasks"]
            .as_array()
            .expect("terminal tasks array");
        let terminal_task = terminal_tasks
            .iter()
            .find(|task| task["task_id"].as_str() == Some(snapshot.handle.as_str()))
            .expect("terminal task for background shell");
        assert_eq!(terminal_task["status"], "running");
        assert_eq!(
            terminal_task["cancel_handle"].as_str(),
            Some(snapshot.handle.as_str())
        );
        assert_eq!(terminal_task["read_tool"], "bash_output");

        let _ = cancel_background_shell(&snapshot.handle).await;
    }

    #[tokio::test]
    async fn bash_cancel_tool_accepts_task_id_alias() {
        let dir = tempdir().expect("temp dir");
        let snapshot = start_background_shell(
            "printf cancel-alias; sleep 5",
            "printf cancel-alias; sleep 5",
            dir.path(),
            BashExecutionBackend::Local,
            30,
        )
        .await
        .expect("start background shell");

        let result = BashCancelTool
            .execute(
                json!({"task_id": snapshot.handle}),
                ToolContext::new(dir.path(), "test-background-cancel-alias"),
            )
            .await;

        assert!(result.success, "cancel failed: {:?}", result.error);
        assert_eq!(
            result.data.as_ref().unwrap()["shell_background"]["status"],
            "cancelled"
        );
    }
}
