use super::*;
use crate::services::api::{ChatRequest, ChatResponse, LlmProvider, ToolCall, Usage};
use async_openai::types::ChatCompletionResponseStream;
use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;

fn lab_command_git(cwd: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("failed to run git {}: {}", args.join(" "), err));
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn init_lab_command_git_repo(path: &Path) {
    lab_command_git(path, &["init", "-q"]);
    lab_command_git(path, &["config", "user.email", "lab@example.test"]);
    lab_command_git(path, &["config", "user.name", "Lab Test"]);
    std::fs::write(path.join("hello.txt"), "base\n").expect("seed repo file");
    lab_command_git(path, &["add", "hello.txt"]);
    lab_command_git(path, &["commit", "-q", "-m", "initial"]);
}

fn drive_lab_command_to_user_report(path: &Path) {
    for stage in [
        "professor_discussion",
        "postdoc_plan",
        "graduate_work",
        "postdoc_review",
        "professor_review",
    ] {
        let planned = handle_lab_command(
            path,
            Some("session".to_string()),
            &format!("plan explicit artifact for {stage}"),
        );
        assert!(
            planned.contains("Gate satisfied"),
            "plan failed at {stage}: {planned}"
        );
        let advanced = handle_lab_command(path, Some("session".to_string()), "advance");
        assert!(
            advanced.contains("Advanced LabRun") || advanced.contains("needs user review"),
            "advance failed at {stage}: {advanced}"
        );
    }
}

struct ProposalProvider {
    response: String,
}

#[async_trait]
impl LlmProvider for ProposalProvider {
    async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
        Ok(ChatResponse {
            content: self.response.clone(),
            tool_calls: None::<Vec<ToolCall>>,
            usage: Some(Usage {
                prompt_tokens: 12,
                completion_tokens: 8,
                total_tokens: 20,
                reasoning_tokens: None,
                cached_tokens: None,
                cache_write_tokens: None,
            }),
            tool_call_repair: None,
            finish_reason: Some("stop".to_string()),
        })
    }

    async fn chat_stream(
        &self,
        _request: ChatRequest,
    ) -> anyhow::Result<ChatCompletionResponseStream> {
        unimplemented!("not needed for Lab command tests")
    }

    fn base_url(&self) -> &str {
        "mock://proposal-provider"
    }

    fn default_model(&self) -> &str {
        "mock-proposal"
    }
}

struct SequenceCommandProvider {
    responses: parking_lot::Mutex<std::collections::VecDeque<String>>,
}

#[async_trait]
impl LlmProvider for SequenceCommandProvider {
    async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
        let response = self
            .responses
            .lock()
            .pop_front()
            .unwrap_or_else(|| r#"{"decision":"accept","note":"ok"}"#.to_string());
        Ok(ChatResponse {
            content: response,
            tool_calls: None::<Vec<ToolCall>>,
            usage: Some(Usage {
                prompt_tokens: 10,
                completion_tokens: 6,
                total_tokens: 16,
                reasoning_tokens: None,
                cached_tokens: None,
                cache_write_tokens: None,
            }),
            tool_call_repair: None,
            finish_reason: Some("stop".to_string()),
        })
    }

    async fn chat_stream(
        &self,
        _request: ChatRequest,
    ) -> anyhow::Result<ChatCompletionResponseStream> {
        unimplemented!("not needed for Lab command tests")
    }

    fn base_url(&self) -> &str {
        "mock://sequence-command-provider"
    }

    fn default_model(&self) -> &str {
        "mock-sequence"
    }
}

struct ToolProbeProvider;

#[async_trait]
impl LlmProvider for ToolProbeProvider {
    async fn chat(&self, request: ChatRequest) -> anyhow::Result<ChatResponse> {
        let tool_name = request
            .tools
            .as_ref()
            .and_then(|tools| tools.first())
            .map(|tool| tool.name.clone())
            .unwrap_or_else(|| "missing_tool".to_string());
        Ok(ChatResponse {
            content: String::new(),
            tool_calls: Some(vec![ToolCall {
                id: "call_probe".to_string(),
                name: tool_name,
                arguments: serde_json::json!({"message": "tool probe ok"}),
            }]),
            usage: Some(Usage {
                prompt_tokens: 10,
                completion_tokens: 4,
                total_tokens: 14,
                reasoning_tokens: None,
                cached_tokens: None,
                cache_write_tokens: None,
            }),
            tool_call_repair: None,
            finish_reason: Some("tool_calls".to_string()),
        })
    }

    async fn chat_stream(
        &self,
        _request: ChatRequest,
    ) -> anyhow::Result<ChatCompletionResponseStream> {
        anyhow::bail!("streaming not supported in ToolProbeProvider")
    }

    fn base_url(&self) -> &str {
        "mock://tool-probe"
    }

    fn default_model(&self) -> &str {
        "mock-tool-probe"
    }
}

#[test]
fn daemon_launchd_plist_renders_worker_entrypoint() {
    let plist = render_launchd_plist(
        "com.priority-agent.lab.demo&one",
        Path::new("/tmp/priority-agent"),
        Path::new("/tmp/project <root>"),
        Path::new("/tmp/lab/out.log"),
        Path::new("/tmp/lab/err.log"),
    );

    assert!(plist.contains("<string>com.priority-agent.lab.demo&amp;one</string>"));
    assert!(plist.contains("<string>/tmp/priority-agent</string>"));
    assert!(plist.contains("<string>lab-daemon</string>"));
    assert!(plist.contains("<key>WorkingDirectory</key>"));
    assert!(plist.contains("<string>/tmp/project &lt;root&gt;</string>"));
    assert!(plist.contains("<key>RunAtLoad</key>"));
    assert!(plist.contains("<key>KeepAlive</key>"));
    assert!(plist.contains("<string>/tmp/lab/out.log</string>"));
    assert!(plist.contains("<string>/tmp/lab/err.log</string>"));
}

#[test]
fn daemon_enable_accepts_hybrid_cycles_mode() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let enabled = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "daemon enable hybrid-cycles 4 6 500 continue bounded cycles",
    );
    assert!(enabled.contains("Enabled Lab daemon policy"));
    assert!(enabled.contains("Mode: HybridCycles"));
    assert!(enabled.contains("Max steps: 4"));
    assert!(enabled.contains("Max steps per cycle: 6"));
    assert!(enabled.contains("Interval ms: 500"));

    let status = handle_lab_command(temp.path(), Some("session".to_string()), "daemon status");
    assert!(status.contains("Mode: HybridCycles"));
    assert!(status.contains("Max steps per cycle: 6"));
    assert!(status.contains("Instructions: continue bounded cycles"));
}

#[test]
fn daemon_health_reports_policy_scheduler_and_start_errors() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let enabled = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "daemon enable strict 3 250",
    );
    assert!(enabled.contains("Enabled Lab daemon policy"));

    let health = handle_lab_command(temp.path(), Some("session".to_string()), "daemon health");

    assert!(health.contains("Lab daemon health: enabled_not_started"));
    assert!(health.contains("Policy: enabled=true mode=Strict"));
    assert!(health.contains("Scheduler: running_in_process=false persisted=none"));
    assert!(health.contains("Last start error: none"));
    assert!(health.contains("LaunchAgent exists: false"));

    let store = LabStore::for_project(temp.path());
    store
        .record_daemon_start_result(None, Some("provider unavailable"))
        .unwrap();
    let unhealthy = handle_lab_command(temp.path(), Some("session".to_string()), "daemon health");

    assert!(unhealthy.contains("Lab daemon health: unhealthy_start_error"));
    assert!(unhealthy.contains("Last start error: provider unavailable"));
}

#[test]
fn daemon_service_status_reports_install_plan() {
    let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
    let temp = tempfile::tempdir().unwrap();
    let launch_agents = tempfile::tempdir().unwrap();
    env.set(
        "PRIORITY_AGENT_LAUNCH_AGENTS_DIR",
        launch_agents.path().to_str().unwrap(),
    );

    let status = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "daemon service status com.example.Lab Service",
    );

    assert!(status.contains("Lab daemon service status."));
    assert!(status.contains("Label: com.example.lab-service"));
    assert!(status.contains("Generated exists: false"));
    assert!(status.contains("Installed exists: false"));
    assert!(status.contains("Bootstrap command: launchctl bootstrap gui/$(id -u)"));
    assert!(status.contains(
        "Kickstart command: launchctl kickstart -k gui/$(id -u)/com.example.lab-service"
    ));
    assert!(status.contains("Health command: /lab daemon health"));
    assert!(status.contains(&launch_agents.path().display().to_string()));
}

#[test]
fn daemon_service_install_and_uninstall_manage_launchagent_plist() {
    let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
    let temp = tempfile::tempdir().unwrap();
    let launch_agents = tempfile::tempdir().unwrap();
    env.set(
        "PRIORITY_AGENT_LAUNCH_AGENTS_DIR",
        launch_agents.path().to_str().unwrap(),
    );

    let install = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "daemon service install com.example.lab.demo",
    );

    let installed = launch_agents.path().join("com.example.lab.demo.plist");
    assert!(install.contains("Installed Lab daemon LaunchAgent plist."));
    assert!(install.contains("Generated exists: true"));
    assert!(install.contains("Installed exists: true"));
    assert!(installed.exists());
    let installed_plist = fs::read_to_string(&installed).unwrap();
    assert!(installed_plist.contains("<string>com.example.lab.demo</string>"));
    assert!(installed_plist.contains("<string>lab-daemon</string>"));

    let uninstall = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "daemon service uninstall com.example.lab.demo",
    );

    assert!(uninstall.contains("Uninstalled Lab daemon LaunchAgent plist."));
    assert!(uninstall.contains("Removed: true"));
    assert!(uninstall.contains("Installed exists: false"));
    assert!(!installed.exists());
}

#[test]
#[cfg(unix)]
fn daemon_service_load_unload_and_restart_call_launchctl() {
    use std::os::unix::fs::PermissionsExt;

    let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
    let temp = tempfile::tempdir().unwrap();
    let launch_agents = tempfile::tempdir().unwrap();
    let bin_dir = tempfile::tempdir().unwrap();
    let fake_launchctl = bin_dir.path().join("launchctl");
    let launchctl_log = bin_dir.path().join("launchctl.log");
    fs::write(
        &fake_launchctl,
        r#"#!/bin/sh
printf '%s' "$1" >> "$PRIORITY_AGENT_FAKE_LAUNCHCTL_LOG"
shift
for arg in "$@"; do
  printf '|%s' "$arg" >> "$PRIORITY_AGENT_FAKE_LAUNCHCTL_LOG"
done
printf '\n' >> "$PRIORITY_AGENT_FAKE_LAUNCHCTL_LOG"
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_launchctl).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_launchctl, permissions).unwrap();
    env.set(
        "PRIORITY_AGENT_LAUNCH_AGENTS_DIR",
        launch_agents.path().to_str().unwrap(),
    );
    env.set(
        "PRIORITY_AGENT_LAUNCHCTL_BIN",
        fake_launchctl.to_str().unwrap(),
    );
    env.set("PRIORITY_AGENT_LAUNCHCTL_DOMAIN", "gui/test");
    env.set(
        "PRIORITY_AGENT_FAKE_LAUNCHCTL_LOG",
        launchctl_log.to_str().unwrap(),
    );

    let load = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "daemon service load com.example.lab.demo",
    );
    let restart = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "daemon service restart com.example.lab.demo",
    );
    let unload = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "daemon service unload com.example.lab.demo",
    );

    assert!(load.contains("Loaded Lab daemon service."));
    assert!(restart.contains("Restarted Lab daemon service."));
    assert!(unload.contains("Unloaded Lab daemon service."));
    let installed = launch_agents.path().join("com.example.lab.demo.plist");
    assert!(installed.exists());
    let log = fs::read_to_string(launchctl_log).unwrap();
    assert!(log.contains(&format!("bootstrap|gui/test|{}", installed.display())));
    assert!(log.contains("kickstart|-k|gui/test/com.example.lab.demo"));
    assert!(log.contains("bootout|gui/test/com.example.lab.demo"));
}

#[test]
#[cfg(unix)]
fn daemon_service_supervise_skips_disabled_and_repairs_missing_service() {
    use std::os::unix::fs::PermissionsExt;

    let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
    let temp = tempfile::tempdir().unwrap();
    let launch_agents = tempfile::tempdir().unwrap();
    let bin_dir = tempfile::tempdir().unwrap();
    let fake_launchctl = bin_dir.path().join("launchctl");
    let launchctl_log = bin_dir.path().join("launchctl.log");
    fs::write(
        &fake_launchctl,
        r#"#!/bin/sh
printf '%s' "$1" >> "$PRIORITY_AGENT_FAKE_LAUNCHCTL_LOG"
shift
for arg in "$@"; do
  printf '|%s' "$arg" >> "$PRIORITY_AGENT_FAKE_LAUNCHCTL_LOG"
done
printf '\n' >> "$PRIORITY_AGENT_FAKE_LAUNCHCTL_LOG"
if [ "$1" = "gui/test/com.example.lab.demo" ]; then
  printf 'missing service\n' >&2
  exit 113
fi
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_launchctl).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_launchctl, permissions).unwrap();
    env.set(
        "PRIORITY_AGENT_LAUNCH_AGENTS_DIR",
        launch_agents.path().to_str().unwrap(),
    );
    env.set(
        "PRIORITY_AGENT_LAUNCHCTL_BIN",
        fake_launchctl.to_str().unwrap(),
    );
    env.set("PRIORITY_AGENT_LAUNCHCTL_DOMAIN", "gui/test");
    env.set(
        "PRIORITY_AGENT_FAKE_LAUNCHCTL_LOG",
        launchctl_log.to_str().unwrap(),
    );

    let skipped = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "daemon service supervise com.example.lab.demo",
    );

    assert!(skipped.contains("supervision skipped: no daemon policy"));

    let enabled = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "daemon enable strict 3 250",
    );
    assert!(enabled.contains("Enabled Lab daemon policy"));

    let repaired = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "daemon service supervise com.example.lab.demo",
    );

    let installed = launch_agents.path().join("com.example.lab.demo.plist");
    assert!(repaired.contains("supervision repaired missing service"));
    assert!(repaired.contains("Exit status: 113"));
    assert!(repaired.contains("Repair:"));
    assert!(installed.exists());
    let log = fs::read_to_string(launchctl_log).unwrap();
    assert!(log.contains("print|gui/test/com.example.lab.demo"));
    assert!(log.contains(&format!("bootstrap|gui/test|{}", installed.display())));
}

#[test]
fn start_drafts_proposal_without_creating_run() {
    let temp = tempfile::tempdir().unwrap();
    let output = handle_lab_command(temp.path(), Some("session".to_string()), "start Build it");

    assert!(output.contains("Lab proposal drafted"));
    assert!(temp.path().join(".priority-agent/lab/proposals").exists());
    assert!(!temp.path().join(".priority-agent/lab/runs").exists());
}

#[tokio::test]
async fn proposal_llm_command_structures_intake_without_creating_run() {
    let temp = tempfile::tempdir().unwrap();
    let provider = Arc::new(ProposalProvider {
        response: serde_json::json!({
            "problem_statement": "Need a formal LabRun intake.",
            "desired_outcome": "A proposal that can be approved explicitly.",
            "scope": ["proposal drafting", "approval boundary"],
            "non_goals": ["auto-approve"],
            "constraints": ["do not mutate code before approval"],
            "risks": ["unclear scope"],
            "success_criteria": ["structured proposal exists"],
            "recommended_mode": "labrun",
            "professor_rationale": "This should use LabRun because it spans planning and implementation."
        })
        .to_string(),
    });
    let context = ToolContext::new(temp.path(), "lab-command-test")
        .with_llm_provider(provider)
        .with_model("mock-proposal".to_string());

    let output = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "propose llm Build a safer Lab Mode",
        context,
    )
    .await;

    assert!(output.contains("Professor drafted Lab proposal:"));
    assert!(output.contains("Recommended mode: Labrun"));
    assert!(output.contains("Formal approval is required"));
    let store = LabStore::for_project(temp.path());
    let proposal = store.latest_proposal().unwrap().unwrap();
    assert_eq!(proposal.problem_statement, "Need a formal LabRun intake.");
    assert_eq!(
        proposal.success_criteria,
        vec!["structured proposal exists".to_string()]
    );
    assert!(store.latest_run().unwrap().is_none());
}

#[tokio::test]
async fn meeting_llm_command_writes_provider_meeting_summary() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let provider = Arc::new(ProposalProvider {
        response: serde_json::json!({
            "professor_view": "Keep the project scope narrow.",
            "postdoc_view": "One implementation blocker needs repair.",
            "decision": "revise_plan",
            "next_actions": ["revise the next postdoc slice"],
            "evidence_ids": []
        })
        .to_string(),
    });
    let context = ToolContext::new(temp.path(), "lab-command-test")
        .with_llm_provider(provider)
        .with_model("mock-meeting".to_string());

    let output = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "meeting llm discuss blocker",
        context,
    )
    .await;

    assert!(output.contains("Provider Lab meeting summary created:"));
    assert!(output.contains("This meeting is read-only and does not mutate code."));
    assert!(output.contains("Usage recorded: true"));
    let store = LabStore::for_project(temp.path());
    let run = store.latest_run().unwrap().unwrap();
    assert_eq!(run.meeting_ids.len(), 1);
    let artifact = store
        .load_stage_artifact(&run.lab_run_id, run.artifact_ids.last().unwrap())
        .unwrap();
    match artifact {
        StageArtifact::LabMeetingSummary(envelope) => {
            assert_eq!(envelope.body.topic, "discuss blocker");
            assert_eq!(envelope.body.decision, "revise_plan");
            assert_eq!(
                envelope.validation_status.as_deref(),
                Some("read_only_provider_summary")
            );
        }
        other => panic!(
            "expected LabMeetingSummary, got {:?}",
            other.artifact_type()
        ),
    }
}

#[test]
fn provider_command_without_context_points_to_provider_shell() {
    let temp = tempfile::tempdir().unwrap();

    let output = handle_lab_command(temp.path(), Some("session".to_string()), "provider");

    assert!(output.contains("requires the Lab Mode shell provider"));
    assert!(output.contains("--with-provider"));
}

#[test]
fn provider_compare_without_context_points_to_provider_shell() {
    let temp = tempfile::tempdir().unwrap();

    let output = handle_lab_command(temp.path(), Some("session".to_string()), "provider compare");

    assert!(output.contains("provider compare requires the Lab Mode shell provider"));
    assert!(output.contains("--with-provider"));
}

#[test]
fn provider_tool_diagnostics_without_context_points_to_provider_shell() {
    let temp = tempfile::tempdir().unwrap();

    let output = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "provider diagnose-tools",
    );

    assert!(output.contains("provider diagnose-tools requires the Lab Mode shell provider"));
    assert!(output.contains("--with-provider"));
}

#[test]
fn meeting_llm_without_context_points_to_provider_shell() {
    let temp = tempfile::tempdir().unwrap();

    let output = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "meeting llm validate repair plan",
    );

    assert!(
        output.contains("meeting llm validate repair plan requires the Lab Mode shell provider")
    );
    assert!(output.contains("--with-provider"));
}

#[test]
fn provider_run_commands_without_context_point_to_provider_shell() {
    let temp = tempfile::tempdir().unwrap();

    let step = handle_lab_command(temp.path(), Some("session".to_string()), "step llm focus");
    let run_llm = handle_lab_command(temp.path(), Some("session".to_string()), "run llm 2 focus");
    let run_hybrid = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "run hybrid 2 focus",
    );
    let run_hybrid_cycles = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "run hybrid-cycles 2 1 focus",
    );

    assert!(step.contains("step llm"));
    assert!(step.contains("--with-provider"));
    assert!(run_llm.contains("run <llm|hybrid|hybrid-cycles>"));
    assert!(run_llm.contains("--with-provider"));
    assert!(run_hybrid.contains("run <llm|hybrid|hybrid-cycles>"));
    assert!(run_hybrid.contains("--with-provider"));
    assert!(run_hybrid_cycles.contains("run <llm|hybrid|hybrid-cycles>"));
    assert!(run_hybrid_cycles.contains("--with-provider"));
}

#[tokio::test]
async fn step_llm_command_runs_provider_stage_step() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let provider = Arc::new(SequenceCommandProvider {
        responses: parking_lot::Mutex::new(std::collections::VecDeque::from([
            serde_json::json!({
                "professor_plan": {
                    "problem_statement": "Build LabRun",
                    "strategic_direction": "Keep gates strict.",
                    "success_criteria": ["advance"],
                    "constraints": ["no overclaiming"],
                    "risks": ["weak evidence"],
                    "handoff_to_postdoc": "Plan the implementation."
                }
            })
            .to_string(),
            r#"{"decision":"accept","note":"ready"}"#.to_string(),
        ])),
    });
    let context = ToolContext::new(temp.path(), "lab-step-llm-command")
        .with_llm_provider(provider)
        .with_model("mock-sequence".to_string());

    let output = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "step llm advance professor plan",
        context,
    )
    .await;

    assert!(output.contains("Provider Lab step:"));
    assert!(output.contains("From: professor_discussion"));
    assert!(output.contains("To: postdoc_plan"));
    assert!(output.contains("Advanced: true"));
    let saved = LabStore::for_project(temp.path())
        .latest_run()
        .unwrap()
        .unwrap();
    assert_eq!(saved.current_stage, "postdoc_plan");
}

#[tokio::test]
async fn run_llm_command_reaches_graduate_boundary() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    assert!(handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}")
    )
    .contains("LabRun created"));
    let provider = Arc::new(SequenceCommandProvider {
        responses: parking_lot::Mutex::new(std::collections::VecDeque::from([
            serde_json::json!({
                "professor_plan": {
                    "problem_statement": "Build LabRun",
                    "strategic_direction": "Keep runtime gates strict.",
                    "success_criteria": ["reach graduate boundary"],
                    "constraints": ["no hidden mutation"],
                    "risks": ["missing task scope"],
                    "handoff_to_postdoc": "Create a scoped plan."
                }
            })
            .to_string(),
            r#"{"decision":"accept","note":"ready for postdoc"}"#.to_string(),
            serde_json::json!({
                "postdoc_plan": {
                    "implementation_summary": "Prepare one implementation slice.",
                    "slices": ["runtime command route"],
                    "files_expected": ["src/lab/commands.rs"],
                    "validation_plan": ["cargo check -q"],
                    "graduate_handoff": "Implement the command route."
                }
            })
            .to_string(),
            r#"{"decision":"accept","note":"ready for graduate"}"#.to_string(),
        ])),
    });
    let context = ToolContext::new(temp.path(), "lab-run-llm-command")
        .with_llm_provider(provider)
        .with_model("mock-sequence".to_string());

    let output = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "run llm 5 command routing",
        context,
    )
    .await;

    assert!(output.contains("Provider Lab run: 2 step(s)"));
    assert!(output.contains("Stop reason: GraduateBoundary"));
    let saved = LabStore::for_project(temp.path())
        .latest_run()
        .unwrap()
        .unwrap();
    assert_eq!(saved.current_stage, "graduate_work");
}

#[tokio::test]
async fn run_hybrid_command_enters_strict_graduate_scheduler_boundary() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    assert!(handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}")
    )
    .contains("LabRun created"));
    let provider = Arc::new(SequenceCommandProvider {
        responses: parking_lot::Mutex::new(std::collections::VecDeque::from([
            serde_json::json!({
                "professor_plan": {
                    "problem_statement": "Build LabRun",
                    "strategic_direction": "Keep runtime gates strict.",
                    "success_criteria": ["hit strict scheduler"],
                    "constraints": ["no provider-only graduate work"],
                    "risks": ["weak tool evidence"],
                    "handoff_to_postdoc": "Create a plan with no scoped graduate work."
                }
            })
            .to_string(),
            r#"{"decision":"accept","note":"ready"}"#.to_string(),
            serde_json::json!({
                "postdoc_plan": {
                    "implementation_summary": "Reach the strict scheduler boundary.",
                    "slices": ["boundary"],
                    "files_expected": [],
                    "validation_plan": ["cargo check -q"],
                    "graduate_handoff": "No scoped graduate task is available."
                }
            })
            .to_string(),
            r#"{"decision":"accept","note":"ready"}"#.to_string(),
        ])),
    });
    let context = ToolContext::new(temp.path(), "lab-run-hybrid-command")
        .with_llm_provider(provider)
        .with_model("mock-sequence".to_string());

    let output = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "run hybrid 5 command routing",
        context,
    )
    .await;

    assert!(output.contains("Hybrid Lab run:"));
    assert!(output.contains("Stop reason: SchedulerStopped(Blocked)"));
    assert!(output.contains("scheduler Blocked"));
    let saved = LabStore::for_project(temp.path())
        .latest_run()
        .unwrap()
        .unwrap();
    assert_eq!(saved.current_stage, "graduate_work");
}

#[tokio::test]
async fn run_hybrid_cycles_command_stops_at_professor_gate_without_explicit_review() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = store
        .create_graduate_task(
            &run.lab_run_id,
            "Complete previous cycle",
            "Provide accepted graduate evidence.",
            vec!["src/lab/commands.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    orchestrator
        .create_graduate_result_for_task_latest(
            &task.task_id,
            "Previous cycle implementation complete.",
            vec!["src/lab/commands.rs".to_string()],
            vec!["cargo check -q passed".to_string()],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
    let mut saved = store.load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "postdoc_review".to_string();
    saved.internal_owner = LabRole::Postdoc;
    store.save_run(&saved).unwrap();
    orchestrator
        .create_postdoc_integration_summary_for_latest(None)
        .unwrap();
    let mut saved = store.load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "professor_review".to_string();
    saved.internal_owner = LabRole::Professor;
    store.save_run(&saved).unwrap();

    let provider = Arc::new(SequenceCommandProvider {
        responses: parking_lot::Mutex::new(std::collections::VecDeque::from([
            serde_json::json!({
                "professor_plan": {
                    "problem_statement": "Continue LabRun",
                    "strategic_direction": "Start the next bounded cycle.",
                    "success_criteria": ["next cycle starts"],
                    "constraints": ["bounded only"],
                    "risks": ["unbounded autonomy"],
                    "handoff_to_postdoc": "Prepare the next implementation plan."
                }
            })
            .to_string(),
            r#"{"decision":"accept","note":"next cycle ready"}"#.to_string(),
        ])),
    });
    let context = ToolContext::new(temp.path(), "lab-run-hybrid-cycles-command")
        .with_llm_provider(provider)
        .with_model("mock-sequence".to_string());

    let output = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "run hybrid-cycles 2 1 continue bounded cycle",
        context,
    )
    .await;

    assert!(output.contains("Hybrid Lab cycle run: 1 cycle(s)"));
    assert!(output.contains("Final stage: professor_review"));
    assert!(output.contains("Stop reason: Stopped(DeterministicGateBlocked)"));
    assert!(output.contains("continued_to_next_cycle=false"));
    let saved = store.latest_run().unwrap().unwrap();
    assert_eq!(saved.cycle_count, 0);
    assert_eq!(saved.current_stage, "professor_review");
}

#[tokio::test]
async fn run_hybrid_cycles_command_stops_when_cycle_token_budget_is_exceeded() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let mut run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    run.cost_policy.max_cycle_tokens = 10;
    store.save_run(&run).unwrap();
    store
        .record_cost_usage(
            &run.lab_run_id,
            LabRole::Professor,
            "mock-sequence",
            LabCostTokens {
                prompt_tokens: 12,
                completion_tokens: 2,
                reasoning_tokens: 0,
                cached_tokens: 0,
                cache_write_tokens: 0,
                cycle_id: Some("0".to_string()),
                meeting_id: None,
            },
            0.0,
            Some("budget test"),
        )
        .unwrap();
    let provider = Arc::new(SequenceCommandProvider {
        responses: parking_lot::Mutex::new(std::collections::VecDeque::from([
            r#"{"decision":"accept","note":"should not be called"}"#.to_string(),
        ])),
    });
    let context = ToolContext::new(temp.path(), "lab-run-hybrid-cycle-budget")
        .with_llm_provider(provider.clone())
        .with_model("mock-sequence".to_string());

    let output = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "run hybrid-cycles 1 5 budget check",
        context,
    )
    .await;

    assert!(output.contains("Hybrid Lab cycle run: 0 cycle(s)"));
    assert!(output.contains("CostBudgetExceeded"));
    assert!(output.contains("total_tokens: 14"));
    assert_eq!(provider.responses.lock().len(), 1);
    let saved = store.latest_run().unwrap().unwrap();
    assert_eq!(saved.current_stage, "professor_discussion");
}

#[tokio::test]
async fn run_hybrid_cycles_command_does_not_compress_blocked_professor_gate() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = store
        .create_graduate_task(
            &run.lab_run_id,
            "Complete previous cycle",
            "Provide accepted graduate evidence.",
            vec!["src/lab/commands.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    orchestrator
        .create_graduate_result_for_task_latest(
            &task.task_id,
            "Previous cycle implementation complete.",
            vec!["src/lab/commands.rs".to_string()],
            vec!["cargo check -q passed".to_string()],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
    let mut saved = store.load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "postdoc_review".to_string();
    saved.internal_owner = LabRole::Postdoc;
    saved.cost_policy.professor_context_budget = 10;
    saved.cost_policy.postdoc_context_budget = 10;
    saved.cost_policy.auto_compress_after_cycle = true;
    store.save_run(&saved).unwrap();
    orchestrator
        .create_postdoc_integration_summary_for_latest(None)
        .unwrap();
    let mut saved = store.load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "professor_review".to_string();
    saved.internal_owner = LabRole::Professor;
    saved.cost_policy.professor_context_budget = 10;
    saved.cost_policy.postdoc_context_budget = 10;
    saved.cost_policy.auto_compress_after_cycle = true;
    store.save_run(&saved).unwrap();
    let provider = Arc::new(SequenceCommandProvider {
        responses: parking_lot::Mutex::new(std::collections::VecDeque::new()),
    });
    let context = ToolContext::new(temp.path(), "lab-run-hybrid-cycle-compress")
        .with_llm_provider(provider)
        .with_model("mock-sequence".to_string());

    let output = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "run hybrid-cycles 1 1 compression check",
        context,
    )
    .await;

    assert!(output.contains("Hybrid Lab cycle run: 1 cycle(s)"));
    assert!(output.contains("Stop reason: Stopped(DeterministicGateBlocked)"));
    assert!(output.contains("compression_artifacts=none"));
    let saved = store.latest_run().unwrap().unwrap();
    assert!(!store
        .list_stage_artifacts(&saved.lab_run_id)
        .unwrap()
        .iter()
        .any(|artifact| matches!(artifact, StageArtifact::CompressionSummary(_))));
}

#[tokio::test]
async fn provider_command_reports_provider_neutral_graduate_diagnostics() {
    let temp = tempfile::tempdir().unwrap();
    let mut context =
        ToolContext::new(temp.path(), "lab-provider-test").with_model("deepseek-v4-flash");
    context
        .metadata
        .insert("provider_id".to_string(), "deepseek".to_string());

    let output = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "provider",
        context,
    )
    .await;

    assert!(output.contains("Lab provider diagnostics:"));
    assert!(output.contains("Provider: deepseek"));
    assert!(output.contains("Model: deepseek-v4-flash"));
    assert!(output.contains("Graduate diagnostic status: unverified"));
    assert!(output.contains("Graduate dispatch policy: provider_neutral_task_evidence"));
    assert!(output.contains("scripts/lab-live-validation.sh --live-control-plane"));
    assert!(output.contains("scripts/lab-live-validation.sh --live-graduate"));
    assert!(output.contains("Latest graduate record: none"));
}

#[tokio::test]
async fn provider_record_command_certifies_graduate_provider_locally() {
    let temp = tempfile::tempdir().unwrap();
    let mut context =
        ToolContext::new(temp.path(), "lab-provider-test").with_model("deepseek-v4-flash");
    context
        .metadata
        .insert("provider_id".to_string(), "deepseek".to_string());

    let recorded = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "provider record graduate passed target/lab-live-validation/pass/report.md full live graduate validation passed",
        context.clone(),
    )
    .await;

    assert!(recorded.contains("Recorded provider diagnostic:"));
    assert!(recorded.contains("Kind: graduate"));
    assert!(recorded.contains("Outcome: passed"));

    let output = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "provider",
        context,
    )
    .await;

    assert!(output.contains("Graduate diagnostic status: certified"));
    assert!(output.contains("Graduate dispatch policy: provider_neutral_task_evidence"));
    assert!(output.contains("Latest graduate record: graduate passed"));
    assert!(output.contains("target/lab-live-validation/pass/report.md"));
    let store = LabStore::for_project(temp.path());
    let latest = store
        .latest_provider_certification(
            "deepseek",
            "deepseek-v4-flash",
            LabProviderCertificationKind::Graduate,
        )
        .unwrap()
        .unwrap();
    assert_eq!(latest.outcome, LabProviderCertificationOutcome::Passed);
}

#[tokio::test]
async fn provider_compare_recovers_generic_foreground_from_durable_sink() {
    let temp = tempfile::tempdir().unwrap();
    let session_id = "lab-provider-command";
    let task_id = "provider-compare-generic";
    let agent_id = "agent_generic";
    let store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
    store
        .create_session(session_id, "Lab provider command", "mock", Some("/repo"))
        .unwrap();
    let worktree = temp.path().join("generic-worktree");
    std::fs::create_dir_all(&worktree).unwrap();
    std::fs::write(
        worktree.join("lab-provider-compare-generic.txt"),
        "generic subagent tool smoke\n",
    )
    .unwrap();
    let artifact_id = store
        .add_agent_artifact(
            session_id,
            agent_id,
            Some("implementer"),
            "Specialist",
            "completed",
            "Provider comparison generic implementer smoke",
            "completed generic compare",
            &serde_json::json!({
                "completion_sink": "agent_manager",
                "tools_used": ["file_write", "file_read", "bash"],
                "confidence": 1.0,
                "has_conflict": false
            }),
        )
        .unwrap();
    store
        .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
            session_id: session_id.to_string(),
            task_id: task_id.to_string(),
            agent_id: agent_id.to_string(),
            profile: Some("implementer".to_string()),
            role: "Specialist".to_string(),
            status: "completed".to_string(),
            description: "Provider comparison generic implementer smoke".to_string(),
            transcript_path: None,
            tool_ids_in_progress: Vec::new(),
            permission_requests: Vec::new(),
            result_artifact_id: Some(artifact_id),
            cleanup_hooks: Vec::new(),
            payload: serde_json::json!({
                "allowed_tools": ["file_read", "file_write", "file_edit", "bash", "diff"],
                "context_mode": "isolated_worktree_fork",
                "isolated_worktree": {
                    "path": worktree.to_string_lossy(),
                    "branch": "codex/agent-generic"
                },
                "tools_used": ["file_write", "file_read", "bash"],
                "completion_sink": "agent_manager"
            }),
        })
        .unwrap();

    let recovered = recover_provider_compare_durable_subagent(
        "Generic subagent",
        &store,
        session_id,
        task_id,
        "lab-provider-compare-generic.txt",
        "Timeout waiting for agent agent_generic result after 90s",
    )
    .await
    .expect("durable compare state should recover");

    assert!(recovered.success);
    assert!(recovered.used_mutating_tool);
    assert!(recovered
        .summary
        .contains("recovered_from_durable_sink: true"));
    assert!(recovered.summary.contains("hard_file_proof: true"));
    assert!(recovered.summary.contains("completion_sink: agent_manager"));
}

#[tokio::test]
async fn provider_failed_record_is_visible_but_does_not_certify() {
    let temp = tempfile::tempdir().unwrap();
    let mut context =
        ToolContext::new(temp.path(), "lab-provider-test").with_model("deepseek-v4-flash");
    context
        .metadata
        .insert("provider_id".to_string(), "deepseek".to_string());

    let recorded = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "provider record graduate failed target/lab-live-validation/fail/report.md full live graduate validation failed",
        context.clone(),
    )
    .await;

    assert!(recorded.contains("Recorded provider diagnostic:"));
    assert!(recorded.contains("Kind: graduate"));
    assert!(recorded.contains("Outcome: failed"));

    let output = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "provider",
        context,
    )
    .await;

    assert!(output.contains("Graduate diagnostic status: unverified"));
    assert!(output.contains("Graduate dispatch policy: provider_neutral_task_evidence"));
    assert!(output.contains("Latest graduate record: graduate failed"));
    assert!(output.contains("target/lab-live-validation/fail/report.md"));
}

#[tokio::test]
async fn provider_compare_reports_generic_and_lab_paths() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "propose Compare provider paths",
    );
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let mut context =
        ToolContext::new(temp.path(), "lab-provider-compare-test").with_model("deepseek-v4-flash");
    context
        .metadata
        .insert("provider_id".to_string(), "deepseek".to_string());

    let output = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "provider compare",
        context,
    )
    .await;

    assert!(output.contains("Provider subagent comparison:"));
    assert!(output.contains("Provider: deepseek"));
    assert!(output.contains("Generic subagent:"));
    assert!(output.contains("AgentManager not available"));
    assert!(output.contains("Lab graduate:"));
    assert!(output.contains("status: Failed"));
    assert!(output.contains("Conclusion:"));
}

#[test]
fn provider_compare_does_not_treat_denied_tool_attempt_as_mutation_proof() {
    assert!(!hard_subagent_mutation_proof(
        true,
        false,
        "file_write returns Permission denied: 'file_write' requires user confirmation"
    ));
    assert!(!hard_subagent_mutation_proof(
        true,
        true,
        "Action rejected before execution: checkpoint_required"
    ));
    assert!(hard_subagent_mutation_proof(
        true,
        true,
        "Created lab-provider-compare-background.txt"
    ));
}

#[test]
fn lab_graduate_provider_compare_reports_durable_subagent_proof() {
    let temp = tempfile::tempdir().unwrap();
    let worktree = temp.path().join("lab-graduate-worktree");
    std::fs::create_dir_all(&worktree).unwrap();
    std::fs::write(
        worktree.join("lab-provider-compare-lab.txt"),
        "lab graduate tool smoke\n",
    )
    .unwrap();
    let session_store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
    session_store
        .create_session("lab-test", "lab durable proof", "test-model", None)
        .unwrap();
    let artifact_id = session_store
        .add_agent_artifact(
            "lab-test",
            "agent_lab",
            Some("lab-graduate"),
            "implementation",
            "completed",
            "lab graduate durable proof",
            "Created lab-provider-compare-lab.txt",
            &serde_json::json!({"completion_sink": "agent_manager"}),
        )
        .unwrap();
    session_store
        .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
            session_id: "lab-test".to_string(),
            task_id: "lab-graduate-gradtask_compare".to_string(),
            agent_id: "agent_lab".to_string(),
            profile: Some("lab-graduate".to_string()),
            role: "implementation".to_string(),
            status: "completed".to_string(),
            description: "lab graduate durable proof".to_string(),
            transcript_path: None,
            tool_ids_in_progress: Vec::new(),
            permission_requests: Vec::new(),
            result_artifact_id: Some(artifact_id),
            cleanup_hooks: Vec::new(),
            payload: serde_json::json!({
                "completion_sink": "agent_manager",
                "context_mode": "isolated_worktree_fork",
                "tools_used": ["file_write", "bash"],
                "isolated_worktree": {
                    "path": worktree.to_string_lossy().to_string(),
                    "branch": "codex/lab-graduate-proof"
                }
            }),
        })
        .unwrap();
    let context = ToolContext::new(temp.path(), "lab-test").with_session_store(session_store);

    let (lines, hard_proof) = lab_graduate_durable_smoke_details(
        &context,
        "lab-graduate-gradtask_compare",
        "lab-provider-compare-lab.txt",
    );
    let rendered = lines.join("\n");

    assert!(hard_proof);
    assert!(rendered.contains("durable_state: present"));
    assert!(rendered.contains("durable_profile: lab-graduate"));
    assert!(rendered.contains("durable_context_mode: isolated_worktree_fork"));
    assert!(rendered.contains("tools_used: file_write,bash"));
    assert!(rendered.contains("hard_file_proof: true"));
    assert!(rendered.contains("permission_denied: false"));
}

#[tokio::test]
async fn provider_tool_diagnostics_reports_request_and_response_tool_calls() {
    let temp = tempfile::tempdir().unwrap();
    let mut context = ToolContext::new(temp.path(), "lab-provider-tool-diagnostics-test")
        .with_llm_provider(Arc::new(ToolProbeProvider))
        .with_model("mock-tool-probe".to_string());
    context
        .metadata
        .insert("provider_id".to_string(), "mock".to_string());

    let output = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "provider diagnose-tools",
        context,
    )
    .await;

    assert!(output.contains("Provider tool-call diagnostics:"));
    assert!(output.contains("Provider: mock"));
    assert!(output.contains("Probe: minimal_auto"));
    assert!(output.contains("Probe: minimal_required"));
    assert!(output.contains("Probe: minimal_forced"));
    assert!(output.contains("Probe: runtime_file_write_auto"));
    assert!(output.contains("Probe: runtime_file_write_bash_auto"));
    assert!(output.contains("Probe: runtime_subagent_allowed_auto"));
    assert!(output.contains("request_tools_count: 1"));
    assert!(output.contains("request_tools: lab_provider_echo"));
    assert!(output.contains("request_tools: file_write"));
    assert!(output.contains("request_tools: file_write,bash"));
    assert!(output.contains("request_tools: file_write,file_edit,bash,diff"));
    assert!(output.contains("response_tool_calls_count: 1"));
    assert!(output.contains("response_tool_calls: lab_provider_echo"));
    assert!(output.contains("response_tool_calls: file_write"));
    assert!(output.contains("finish_reason: tool_calls"));
}

#[test]
fn advance_requires_gate_satisfaction() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let blocked = handle_lab_command(temp.path(), Some("session".to_string()), "advance");
    assert!(blocked.contains("Failed to advance LabRun"));
    assert!(blocked.contains("artifact_id"));

    let gate = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "gate satisfy artifact_professor_plan_001 not_verified",
    );
    assert!(gate.contains("Failed to satisfy artifact gate"));
    assert!(gate.contains("missing or malformed artifact"));

    let planned = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "plan Professor direction",
    );
    assert!(planned.contains("Created ProfessorPlan artifact"));
    assert!(planned.contains("Gate satisfied"));

    let advanced = handle_lab_command(temp.path(), Some("session".to_string()), "advance");
    assert!(advanced.contains("postdoc_plan"));
}

#[test]
fn plan_command_creates_artifact_and_allows_advance() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let planned = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "plan Professor direction",
    );
    assert!(planned.contains("Created ProfessorPlan artifact"));
    assert!(planned.contains("Gate satisfied"));
    assert!(planned.contains("Report: "));

    let advanced = handle_lab_command(temp.path(), Some("session".to_string()), "advance");
    assert!(advanced.contains("postdoc_plan"));
    assert!(temp.path().join(".priority-agent/lab/runs").exists());
}

#[test]
fn report_command_shows_latest_generated_report() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let planned = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "plan Professor direction",
    );
    assert!(planned.contains("Report: "));

    let report = handle_lab_command(temp.path(), Some("session".to_string()), "report");
    let list = handle_lab_command(temp.path(), Some("session".to_string()), "report list");

    assert!(report.contains("Lab report:"));
    assert!(report.contains("Artifact: artifact_professorplan_"));
    assert!(report.contains("Path:"));
    assert!(report.contains("Preview:"));
    assert!(list.contains("Lab reports:"));
    assert!(list.contains("artifact_professorplan_"));
}

#[test]
fn review_command_summarizes_current_review_state() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let planned = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "plan Professor direction",
    );
    assert!(planned.contains("Created ProfessorPlan artifact"));

    let review = handle_lab_command(temp.path(), Some("session".to_string()), "review");

    assert!(review.contains("Lab review:"));
    assert!(review.contains("Run: status=Active"));
    assert!(review.contains("Artifacts: 1 latest=artifact_professorplan_"));
    assert!(review.contains("Reports: 1 latest="));
    assert!(review.contains("Current gate: stage=professor_discussion"));
    assert!(review.contains("satisfied=true"));
    assert!(review.contains("Graduate worktree proof: none"));
    assert!(review.contains("Graduate workspace snapshots: none"));
    assert!(
        review.contains("Provider artifact review: /lab review artifact artifact_professorplan_")
    );
    assert!(!review.contains("planned for a later orchestration slice"));
}

#[test]
fn artifact_revise_blocks_advance_until_acceptance() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let planned = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "plan Professor direction",
    );
    let artifact_id = planned
        .lines()
        .find_map(|line| line.strip_prefix("Created ProfessorPlan artifact: "))
        .unwrap()
        .to_string();

    let revised = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("revise {artifact_id} needs clearer constraints"),
    );
    assert!(revised.contains("Revision requested"));
    assert!(revised.contains("needs_revision"));
    let blocked = handle_lab_command(temp.path(), Some("session".to_string()), "advance");
    assert!(blocked.contains("Failed to advance LabRun"));
    assert!(blocked.contains("blocked") || blocked.contains("needs revision"));

    let accepted = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("accept {artifact_id} revised offline"),
    );
    assert!(accepted.contains("Accepted artifact"));
    assert!(accepted.contains("accepted"));
    let advanced = handle_lab_command(temp.path(), Some("session".to_string()), "advance");
    assert!(advanced.contains("postdoc_plan"));
}

#[test]
fn cost_command_records_and_summarizes_lab_usage() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let recorded = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "cost record professor test-model 1000 200 50 700 120 0.0123 draft",
    );
    assert!(recorded.contains("Recorded Lab cost usage"));
    assert!(recorded.contains("cached=700"));
    assert!(recorded.contains("cache_write=120"));
    assert!(recorded.contains("miss=300"));

    let summary = handle_lab_command(temp.path(), Some("session".to_string()), "cost");
    assert!(summary.contains("Requests: 1"));
    assert!(summary.contains("total=1250"));
    assert!(summary.contains("hit_rate=70.0%"));
    assert!(summary.contains("Professor"));
}

#[test]
fn closeout_command_marks_latest_run_verified_completed() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let output = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "closeout verified validation passed",
    );

    assert!(output.contains("LabRun closeout recorded"));
    assert!(output.contains("Status: Completed"));
    assert!(output.contains("CompletedVerified"));
    let store = LabStore::for_project(temp.path());
    let saved = store.latest_run().unwrap().unwrap();
    assert_eq!(saved.status, crate::lab::model::LabRunStatus::Completed);
    assert_eq!(
        saved.closeout_status,
        Some(LabCloseoutStatus::CompletedVerified)
    );
    assert!(!store.root().join("active_lease.json").exists());
}

#[test]
fn auto_closeout_command_uses_final_professor_gate() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    drive_lab_command_to_user_report(temp.path());

    let output = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "closeout auto final report shown",
    );

    assert!(output.contains("LabRun closeout recorded from final evidence"));
    assert!(output.contains("Status: Completed"));
    assert!(output.contains("CompletedNotVerified"));
    let store = LabStore::for_project(temp.path());
    let saved = store.latest_run().unwrap().unwrap();
    assert_eq!(saved.status, crate::lab::model::LabRunStatus::Completed);
    assert_eq!(
        saved.closeout_status,
        Some(LabCloseoutStatus::CompletedNotVerified)
    );
}

#[test]
fn continue_command_starts_next_cycle_from_user_report() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    drive_lab_command_to_user_report(temp.path());

    let output = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "continue next cycle approved",
    );

    assert!(output.contains("Continued LabRun"));
    assert!(output.contains("cycle 1"));
    assert!(output.contains("professor_discussion"));
    let store = LabStore::for_project(temp.path());
    let saved = store.latest_run().unwrap().unwrap();
    assert_eq!(saved.status, crate::lab::model::LabRunStatus::Active);
    assert_eq!(saved.current_stage, "professor_discussion");
    assert_eq!(saved.cycle_count, 1);
}

#[test]
fn intervene_command_pauses_run_without_creating_graduate_task() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let output = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "intervene Reconsider whether this is still in scope",
    );

    assert!(output.contains("LabRun intervention queued"));
    assert!(output.contains("Run status: NeedsUser"));
    let store = LabStore::for_project(temp.path());
    let saved = store.latest_run().unwrap().unwrap();
    assert_eq!(saved.status, crate::lab::model::LabRunStatus::NeedsUser);
    assert_eq!(store.latest_graduate_tasks().unwrap().len(), 0);
    assert!(!store.root().join("active_lease.json").exists());
}

#[test]
fn recovery_command_shows_paused_run_options_without_resuming() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let paused = handle_lab_command(temp.path(), Some("session".to_string()), "pause user");
    assert!(paused.contains("Paused LabRun"));

    let recovery = handle_lab_command(temp.path(), Some("session".to_string()), "recovery");

    assert!(recovery.contains("Lab recovery:"));
    assert!(recovery.contains("Recovery: available"));
    assert!(recovery.contains("Resume cursor:"));
    assert!(recovery.contains("Continue: /lab resume"));
    assert!(recovery.contains("Inspect: /lab dashboard"));
    assert!(recovery.contains("Keep paused: no action"));
    assert!(recovery.contains("Close/cancel: /lab close"));
    let store = LabStore::for_project(temp.path());
    let saved = store.latest_run().unwrap().unwrap();
    assert_eq!(saved.status, crate::lab::model::LabRunStatus::Paused);
    assert!(!store.root().join("active_lease.json").exists());
}

#[test]
fn open_command_switches_active_labrun_without_resuming() {
    let temp = tempfile::tempdir().unwrap();
    let first = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "propose First run",
    );
    let first_proposal_id = first
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let first_approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {first_proposal_id}"),
    );
    assert!(first_approved.contains("LabRun created"));
    let store = LabStore::for_project(temp.path());
    let first_run_id = store.latest_run().unwrap().unwrap().lab_run_id;
    let paused_first = handle_lab_command(temp.path(), Some("session".to_string()), "pause user");
    assert!(paused_first.contains("Paused LabRun"));

    let second = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "propose Second run",
    );
    let second_proposal_id = second
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let second_approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {second_proposal_id}"),
    );
    assert!(second_approved.contains("LabRun created"));
    let paused_second = handle_lab_command(temp.path(), Some("session".to_string()), "pause user");
    assert!(paused_second.contains("Paused LabRun"));

    let opened = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("open {first_run_id}"),
    );

    assert!(opened.contains("Opened LabRun"));
    assert!(opened.contains("for inspection"));
    let latest = store.latest_run().unwrap().unwrap();
    assert_eq!(latest.lab_run_id, first_run_id);
    assert_eq!(latest.status, crate::lab::model::LabRunStatus::Paused);
    assert!(!store.root().join("active_lease.json").exists());
}

#[test]
fn runs_command_lists_recent_lab_runs() {
    let temp = tempfile::tempdir().unwrap();
    for goal in ["First run", "Second run"] {
        let proposal = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("propose {goal}"),
        );
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let paused = handle_lab_command(temp.path(), Some("session".to_string()), "pause user");
        assert!(paused.contains("Paused LabRun"));
    }

    let runs = handle_lab_command(temp.path(), Some("session".to_string()), "runs");

    assert!(runs.contains("Lab runs:"));
    assert!(runs.contains("Total: 2"));
    assert!(runs.contains("Index:"));
    assert!(runs.contains("runs_index.json"));
    assert!(runs.contains("Open one with /lab open <lab_run_id>"));
    assert!(runs.matches("status=Paused").count() >= 2);
    assert!(runs.contains("tasks=0 artifacts=0"));
    assert!(runs.contains("* labrun_"));
}

#[test]
fn status_reports_file_and_sqlite_index_summaries() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let runs = handle_lab_command(temp.path(), Some("session".to_string()), "runs");
    assert!(runs.contains("Lab runs:"));

    let status = handle_lab_command(temp.path(), Some("session".to_string()), "status");

    assert!(status.contains("Latest LabRun:"));
    assert!(status.contains("Index:"));
    assert!(status.contains("runs_index.json"));
    assert!(status.contains("latest=matched"));
    assert!(status.contains("SQLite index:"));
    assert!(status.contains("lab_index.sqlite3"));
    assert!(status.contains("runs=1"));
}

#[test]
fn messages_command_lists_professor_side_channel_inbox() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let queued = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "professor This needs a tighter product boundary",
    );
    assert!(queued.contains("Message queued for professor"));
    let inbox = handle_lab_command(temp.path(), Some("session".to_string()), "messages");

    assert!(inbox.contains("Professor side-channel inbox"));
    assert!(inbox.contains("Messages: 1"));
    assert!(inbox.contains("Concern/Queued/normal"));
    assert!(inbox.contains("tighter product boundary"));
}

#[test]
fn messages_command_updates_professor_side_channel_status() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let queued = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "professor Convert this into a group meeting",
    );
    let message_id = queued
        .lines()
        .find_map(|line| line.strip_prefix("Message queued for professor: "))
        .unwrap()
        .to_string();

    let updated = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("messages meeting {message_id} schedule it"),
    );
    let inbox = handle_lab_command(temp.path(), Some("session".to_string()), "messages");

    assert!(updated.contains("Professor side-channel message updated"));
    assert!(updated.contains("ConvertedToMeeting"));
    assert!(inbox.contains("Concern/ConvertedToMeeting/normal"));
}

#[test]
fn messages_decision_renders_professor_steering_state_without_applying() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let queued = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "professor Schedule a product review meeting",
    );
    let message_id = queued
        .lines()
        .find_map(|line| line.strip_prefix("Message queued for professor: "))
        .unwrap()
        .to_string();
    let converted = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("messages meeting {message_id}"),
    );
    assert!(converted.contains("ConvertedToMeeting"));

    let decision = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "messages decision latest",
    );

    assert!(decision.contains("Professor steering decision:"));
    assert!(decision.contains("Decision: open_lab_meeting"));
    assert!(decision.contains("Status: ConvertedToMeeting"));
    assert!(decision.contains("Next action: Apply with /lab messages apply"));
    assert!(decision.contains("Report: "));
    assert!(decision.contains("product review meeting"));
    let store = LabStore::for_project(temp.path());
    let run = store.latest_run().unwrap().unwrap();
    let messages = store.list_sponsor_messages(&run.lab_run_id).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].status, SponsorMessageStatus::ConvertedToMeeting);
    let artifacts = store.list_stage_artifacts(&run.lab_run_id).unwrap();
    let steering = artifacts
        .iter()
        .find_map(|artifact| match artifact {
            StageArtifact::ProfessorSteeringDecision(decision) => Some(decision),
            _ => None,
        })
        .expect("professor steering decision artifact");
    assert_eq!(steering.body.source_message_id, message_id);
    assert_eq!(steering.body.decision, "open_lab_meeting");
    assert_eq!(
        steering.validation_status.as_deref(),
        Some("decision_recorded_not_applied")
    );
    let reports = store
        .list_stage_artifact_report_paths(&run.lab_run_id)
        .unwrap();
    assert!(reports
        .iter()
        .any(|(artifact_id, path)| { artifact_id == &steering.artifact_id && path.exists() }));
}

#[test]
fn messages_apply_meeting_creates_report_and_marks_message_applied() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let queued = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "professor Schedule a lab meeting about scope",
    );
    let message_id = queued
        .lines()
        .find_map(|line| line.strip_prefix("Message queued for professor: "))
        .unwrap()
        .to_string();
    let converted = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("messages meeting {message_id}"),
    );
    assert!(converted.contains("ConvertedToMeeting"));

    let applied = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("messages apply {message_id} scope meeting"),
    );
    let inbox = handle_lab_command(temp.path(), Some("session".to_string()), "messages");

    assert!(applied.contains("applied as meeting"));
    assert!(applied.contains("Report:"));
    assert!(inbox.contains("Concern/Applied/normal"));
}

#[test]
fn messages_apply_task_creates_blocked_graduate_task() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let queued = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "professor Turn this concern into a scoped implementation task",
    );
    let message_id = queued
        .lines()
        .find_map(|line| line.strip_prefix("Message queued for professor: "))
        .unwrap()
        .to_string();
    let converted = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("messages task {message_id}"),
    );
    assert!(converted.contains("ConvertedToTask"));

    let applied = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("messages apply {message_id} implementation task"),
    );

    assert!(applied.contains("applied as blocked graduate task"));
    let store = LabStore::for_project(temp.path());
    let tasks = store.latest_graduate_tasks().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].status, crate::lab::model::LabTaskStatus::Blocked);
    assert!(tasks[0]
        .blocker
        .as_deref()
        .unwrap_or_default()
        .contains("allowed_scope"));
    let inbox = handle_lab_command(temp.path(), Some("session".to_string()), "messages");
    assert!(inbox.contains("Concern/Applied/normal"));
}

#[test]
fn context_command_renders_packet_fingerprints() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let context = handle_lab_command(temp.path(), Some("session".to_string()), "context postdoc");

    assert!(context.contains("Lab context packet"));
    assert!(context.contains("Role: Postdoc"));
    assert!(context.contains("Stable prefix: hash="));
    assert!(context.contains("Dynamic tail: hash="));
    assert!(context.contains("L0 role-profile-and-project-charter"));
    assert!(context.contains("L3 cost-and-cache-summary"));
    assert!(context.contains("L5 validation-retry-history"));
    assert!(context.contains("L6 artifact-and-gate-evidence-refs"));
}

#[test]
fn next_command_recommends_proposal_approval() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();

    let next = handle_lab_command(temp.path(), Some("session".to_string()), "next");

    assert!(next.contains(&format!("Lab next: /lab approve {proposal_id}")));
    assert!(next.contains("State: proposal_awaiting_approval"));
    assert!(next.contains("Owner: Professor"));
}

#[test]
fn next_command_recommends_current_gate_artifact() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let next = handle_lab_command(temp.path(), Some("session".to_string()), "next");

    assert!(next.contains("Lab next: /lab plan <note>"));
    assert!(next.contains("State: gate_required"));
    assert!(next.contains("Stage: professor_discussion"));
    assert!(next.contains("Gate: stage=professor_discussion artifact_type=ProfessorPlan"));
}

#[test]
fn next_command_recommends_queued_graduate_task() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store
        .create_proposal("Build LabRun", Some("session".to_string()))
        .unwrap();
    let run = LabOrchestrator::for_project(temp.path())
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = store
        .create_graduate_task(
            &run.lab_run_id,
            "Fix lab model",
            "Update the LabRun model.",
            vec!["src/lab/model.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    let mut saved = store.load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "graduate_work".to_string();
    saved.internal_owner = LabRole::Graduate;
    store.save_run(&saved).unwrap();

    let next = handle_lab_command(temp.path(), Some("session".to_string()), "next");

    assert!(next.contains(&format!("Lab next: /lab task run {}", task.task_id)));
    assert!(next.contains("State: queued_graduate_task"));
    assert!(next.contains("Tasks: open=1 blocked=0"));
    assert!(next.contains(&format!("Task: {}", task.task_id)));
}

#[test]
fn next_command_recommends_blocked_graduate_task_revision() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store
        .create_proposal("Build LabRun", Some("session".to_string()))
        .unwrap();
    let run = LabOrchestrator::for_project(temp.path())
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = store
        .create_graduate_task(
            &run.lab_run_id,
            "Fix lab model",
            "Update the LabRun model.",
            vec!["src/lab/model.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    store
        .block_graduate_task(&run.lab_run_id, &task.task_id, "validation failed")
        .unwrap();
    let mut saved = store.load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "graduate_work".to_string();
    saved.internal_owner = LabRole::Graduate;
    store.save_run(&saved).unwrap();

    let next = handle_lab_command(temp.path(), Some("session".to_string()), "next");

    assert!(next.contains("State: blocked_graduate_task"));
    assert!(next.contains(&format!("Lab next: /lab task revise {}", task.task_id)));
    assert!(next.contains("Tasks: open=1 blocked=1"));
    assert!(next.contains("Blocker: validation failed"));
}

#[test]
fn next_json_command_is_machine_readable() {
    let temp = tempfile::tempdir().unwrap();
    handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");

    let json = handle_lab_command(temp.path(), Some("session".to_string()), "next --json");
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(value["state"], "proposal_awaiting_approval");
    assert!(value["recommended_command"]
        .as_str()
        .unwrap()
        .starts_with("/lab approve labproposal_"));
    assert_eq!(value["open_task_count"], 0);
}

#[test]
fn dashboard_command_renders_status_panel_summary() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let dashboard = handle_lab_command(temp.path(), Some("session".to_string()), "dashboard");

    assert!(dashboard.contains("Lab dashboard:"));
    assert!(dashboard.contains("Run: status=Active"));
    assert!(dashboard.contains("Tasks: total=0 open=0 blocked=0"));
    assert!(dashboard.contains("Validation retries: total=0 escalated=0"));
    assert!(dashboard.contains("Cost: requests=0"));
    assert!(dashboard.contains("Runtime escalation signals: suggested_meeting=false"));
    assert!(dashboard.contains("Scheduler:"));
    assert!(dashboard.contains("Indexed dashboard: missing"));
    assert!(dashboard.contains("Graduate worktree proof: none"));
    assert!(dashboard.contains("Graduate workspace snapshots: none"));
}

#[test]
fn proof_command_rolls_up_persisted_lab_evidence() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let planned = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "plan initial professor direction",
    );
    assert!(planned.contains("Gate satisfied"));

    let proof = handle_lab_command(temp.path(), Some("session".to_string()), "proof");

    assert!(proof.contains("Lab proof:"));
    assert!(proof.contains("Run: status=Active"));
    assert!(proof.contains("Next: /lab advance"));
    assert!(proof.contains("professor_discussion artifact=artifact_professorplan_"));
    assert!(proof.contains("Recent artifacts:"));
    assert!(proof.contains("ProfessorPlan artifact_professorplan_"));
}

#[test]
fn trace_command_shows_recent_lab_events() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let trace = handle_lab_command(temp.path(), Some("session".to_string()), "trace 3");

    assert!(trace.contains("Lab trace:"));
    assert!(trace.contains("Events:"));
    assert!(trace.contains("labrun_created") || trace.contains("artifact_gate_written"));
}

#[test]
fn review_and_dashboard_render_graduate_workspace_snapshots() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let store = LabStore::for_project(temp.path());
    let run = store.latest_run().unwrap().unwrap();
    store
        .record_run_event(
            &run.lab_run_id,
            "lab_graduate_workspace_snapshot",
            serde_json::json!({
                "task_id": "gradtask_snapshot",
                "dispatch_id": "graddispatch_snapshot",
                "phase": "before",
                "dirty_path_count": 2,
                "dirty_paths": ["preexisting-user-change.txt", "src/lib.rs"],
                "changed_path_count": 0,
                "changed_paths": [],
            }),
        )
        .unwrap();
    store
        .record_run_event(
            &run.lab_run_id,
            "lab_graduate_workspace_snapshot",
            serde_json::json!({
                "task_id": "gradtask_snapshot",
                "dispatch_id": "graddispatch_snapshot",
                "phase": "after",
                "dirty_path_count": 3,
                "dirty_paths": ["preexisting-user-change.txt", "src/lib.rs", "src/lab/model.rs"],
                "changed_path_count": 1,
                "changed_paths": ["src/lab/model.rs"],
            }),
        )
        .unwrap();

    let review = handle_lab_command(temp.path(), Some("session".to_string()), "review");
    assert!(review.contains("Graduate workspace snapshots:"));
    assert!(review.contains("before task=gradtask_snapshot"));
    assert!(review.contains("dirty=2 [preexisting-user-change.txt,src/lib.rs]"));
    assert!(review.contains("after task=gradtask_snapshot"));
    assert!(review.contains("changed=1 [src/lab/model.rs]"));

    let dashboard = handle_lab_command(temp.path(), Some("session".to_string()), "dashboard");
    assert!(dashboard.contains("Graduate workspace snapshots:"));
    assert!(dashboard.contains("after task=gradtask_snapshot"));
    assert!(dashboard.contains("changed=1 [src/lab/model.rs]"));
}

#[test]
fn dashboard_consumes_sqlite_index_for_professor_postdoc_state() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let planned = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "plan Professor plan",
    );
    assert!(planned.contains("Created ProfessorPlan artifact"));
    let advanced = handle_lab_command(temp.path(), Some("session".to_string()), "advance");
    assert!(advanced.contains("postdoc_plan"));
    let postdoc = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "plan Postdoc plan",
    );
    assert!(postdoc.contains("Created PostdocPlan artifact"));
    let runs = handle_lab_command(temp.path(), Some("session".to_string()), "runs");
    assert!(runs.contains("Lab runs:"));

    let dashboard = handle_lab_command(temp.path(), Some("session".to_string()), "dashboard");

    assert!(dashboard.contains("Indexed dashboard: sqlite="));
    assert!(dashboard.contains("lab_index.sqlite3"));
    assert!(dashboard.contains("ProfessorPlan:"));
    assert!(dashboard.contains("PostdocPlan:"));
    assert!(dashboard.contains("artifacts=2"));
}

#[test]
fn evidence_command_records_refs_only_index() {
    let temp = tempfile::tempdir().unwrap();
    let evidence_path = temp.path().join("proof.log");
    std::fs::write(&evidence_path, "proof").unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let recorded = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!(
            "evidence add file {} cargo check passed",
            evidence_path.display()
        ),
    );
    assert!(recorded.contains("Recorded Lab evidence ref"));
    assert!(recorded.contains("kind=File"));

    let listed = handle_lab_command(temp.path(), Some("session".to_string()), "evidence list");
    assert!(listed.contains("Lab evidence refs: 1"));
    assert!(listed.contains("cargo check passed"));

    let context = handle_lab_command(temp.path(), Some("session".to_string()), "context");
    assert!(context.contains("L4 refs-only-evidence-index"));
}

#[test]
fn task_command_manages_graduate_task_lifecycle() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let created = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "task create Implement task queue | src/lab/model.rs,src/lab/store.rs | cargo check -q | Add graduate task persistence and tests",
    );
    assert!(created.contains("Created graduate task"));
    assert!(created.contains("Status: Queued"));
    let task_id = created
        .lines()
        .find_map(|line| line.strip_prefix("Created graduate task: "))
        .unwrap()
        .to_string();

    let listed = handle_lab_command(temp.path(), Some("session".to_string()), "task list");
    assert!(listed.contains("Graduate tasks: 1 total, 1 open"));
    assert!(listed.contains("Implement task queue"));

    let envelope = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("task envelope {task_id}"),
    );
    assert!(envelope.contains("Graduate task envelope"));
    assert!(envelope.contains("\"profile\": \"lab-graduate\""));
    assert!(envelope.contains("\"context_mode\": \"isolated_worktree_fork\""));
    assert!(envelope.contains("GraduateResult"));

    let dispatch = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("task dispatch {task_id}"),
    );
    assert!(dispatch.contains("Prepared graduate dispatch"));
    assert!(dispatch.contains("Status: Prepared"));
    assert!(dispatch.contains("Dispatch: "));

    let started = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("task start {task_id}"),
    );
    assert!(started.contains("Status: InProgress"));

    let completed = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!(
            "task result {task_id} | src/lab/model.rs | cargo check -q | | labevidence_001 | Implemented task queue"
        ),
    );
    assert!(completed.contains("Created graduate result artifact"));
    assert!(completed.contains("Report: "));
    assert!(completed.contains("Gate status: satisfied"));

    let listed = handle_lab_command(temp.path(), Some("session".to_string()), "tasks");
    assert!(listed.contains("Graduate tasks: 1 total, 0 open"));
    assert!(listed.contains("Completed"));
}

#[test]
fn task_bind_json_command_binds_agent_contract_output() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let created = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "task create Bind graduate JSON | src/lab/model.rs | cargo check -q | Verify structured graduate output binding",
    );
    let task_id = created
        .lines()
        .find_map(|line| line.strip_prefix("Created graduate task: "))
        .unwrap()
        .to_string();
    let json_path = temp.path().join("graduate-result.json");
    std::fs::write(
        &json_path,
        serde_json::json!({
            "result": serde_json::json!({
                "graduate_result": {
                    "summary": "Bound structured graduate output.",
                    "changed_files": ["src/lab/model.rs"],
                    "validation_results": ["cargo check -q passed"],
                    "blockers": [],
                    "evidence_ids": ["labevidence_bind_json"]
                }
            })
            .to_string()
        })
        .to_string(),
    )
    .unwrap();

    let bound = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("task bind-json {task_id} {}", json_path.display()),
    );

    assert!(bound.contains("Bound graduate agent JSON result"));
    assert!(bound.contains("Gate status: satisfied"));
    let listed = handle_lab_command(temp.path(), Some("session".to_string()), "tasks");
    assert!(listed.contains("Graduate tasks: 1 total, 0 open"));
    assert!(listed.contains("Completed"));
}

#[tokio::test]
async fn task_sync_command_binds_completed_durable_graduate_result() {
    let temp = tempfile::tempdir().unwrap();
    init_lab_command_git_repo(temp.path());
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build it", None)
        .unwrap();
    let mut run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    run.current_stage = "graduate_work".to_string();
    run.internal_owner = LabRole::Graduate;
    orchestrator.store().save_run(&run).unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Sync durable result",
            "Update scoped file.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["test -f src/lab/orchestrator.rs".to_string()],
        )
        .unwrap();
    let dispatch = build_graduate_task_dispatch(&task).unwrap();
    let record = orchestrator
        .store()
        .record_graduate_dispatch(&run.lab_run_id, &task.task_id, dispatch)
        .unwrap();
    orchestrator
        .store()
        .start_graduate_task(&run.lab_run_id, &task.task_id)
        .unwrap();

    let worktree = temp.path().join("graduate-sync-worktree");
    std::fs::create_dir_all(worktree.join("src/lab")).unwrap();
    lab_command_git(&worktree, &["init", "-q"]);
    lab_command_git(&worktree, &["config", "user.email", "lab@example.test"]);
    lab_command_git(&worktree, &["config", "user.name", "Lab Test"]);
    std::fs::write(
        worktree.join("src/lab/orchestrator.rs"),
        "durable graduate command sync\n",
    )
    .unwrap();

    let session_store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
    session_store
        .create_session("lab-test", "lab command sync", "test-model", None)
        .unwrap();
    let agent_task_id = crate::lab::delegation::graduate_agent_task_id(&task);
    let agent_artifact_id = session_store
        .add_agent_artifact(
            "lab-test",
            "agent_sync",
            Some("lab-graduate"),
            "implementation",
            "completed",
            "graduate durable sync result",
            r#"{"graduate_result":{"summary":"Synced command result.","changed_files":["src/lab/orchestrator.rs"],"validation_results":["claimed validation"],"blockers":[],"evidence_ids":[]}}"#,
            &serde_json::json!({"completion_sink": "agent_manager"}),
        )
        .unwrap();
    session_store
        .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
            session_id: "lab-test".to_string(),
            task_id: agent_task_id.clone(),
            agent_id: "agent_sync".to_string(),
            profile: Some("lab-graduate".to_string()),
            role: "implementation".to_string(),
            status: "completed".to_string(),
            description: "graduate durable sync result".to_string(),
            transcript_path: None,
            tool_ids_in_progress: Vec::new(),
            permission_requests: Vec::new(),
            result_artifact_id: Some(agent_artifact_id),
            cleanup_hooks: vec!["worktree_cleanup".to_string()],
            payload: serde_json::json!({
                "completion_sink": "agent_manager",
                "tools_used": ["file_write", "bash"],
                "isolated_worktree": {
                    "path": worktree.to_string_lossy().to_string(),
                    "branch": "codex/graduate-sync"
                }
            }),
        })
        .unwrap();

    let output = handle_lab_command_with_context(
        temp.path(),
        Some("lab-test".to_string()),
        &format!("task sync {}", task.task_id),
        ToolContext::new(temp.path(), "lab-test").with_session_store(session_store),
    )
    .await;

    assert!(output.contains("Synced graduate durable subagent result"));
    assert!(output.contains("Gate status: satisfied"));
    let saved_task = orchestrator
        .store()
        .load_graduate_task(&run.lab_run_id, &task.task_id)
        .unwrap();
    assert_eq!(
        saved_task.status,
        crate::lab::model::LabTaskStatus::Completed
    );
    assert!(saved_task
        .evidence_ids
        .contains(&format!("agent_task:{agent_task_id}")));
    let saved_dispatch = orchestrator
        .store()
        .load_graduate_dispatch(&run.lab_run_id, &record.dispatch_id)
        .unwrap();
    assert_eq!(
        saved_dispatch.status,
        crate::lab::model::GraduateDispatchStatus::Succeeded
    );
    assert_eq!(saved_dispatch.agent_id.as_deref(), Some("agent_sync"));
}

#[tokio::test]
async fn task_run_command_uses_runtime_context_and_records_failure() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let created = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "task create Implement task queue | src/lab/model.rs | cargo check -q | Add graduate task persistence and tests",
    );
    let task_id = created
        .lines()
        .find_map(|line| line.strip_prefix("Created graduate task: "))
        .unwrap()
        .to_string();

    let output = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        &format!("task run {task_id}"),
        ToolContext::new(temp.path(), "lab-test"),
    )
    .await;

    assert!(output.contains("Graduate task run dispatched"));
    assert!(output.contains("Status: Failed"));
    assert!(output.contains("AgentManager not available"));
    let store = LabStore::for_project(temp.path());
    let run = store.latest_run().unwrap().unwrap();
    let task = store.load_graduate_task(&run.lab_run_id, &task_id).unwrap();
    assert_eq!(task.status, crate::lab::model::LabTaskStatus::Blocked);
}

#[tokio::test]
async fn task_worktree_command_falls_back_to_durable_task_id() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let created = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "task create Fix lab model | src/lab/model.rs | cargo check -q | update model",
    );
    let task_id = created
        .lines()
        .find_map(|line| line.strip_prefix("Created graduate task: "))
        .unwrap()
        .to_string();
    let dispatch = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("task dispatch {task_id}"),
    );
    assert!(dispatch.contains("Prepared graduate dispatch"));

    let output = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        &format!("task worktree review {task_id}"),
        ToolContext::new(temp.path(), "lab-test"),
    )
    .await;

    assert!(output.contains("via task_id"));
    assert!(output.contains("lab-graduate-"));
    assert!(output.contains("Worktree manager not available"));
}

#[tokio::test]
async fn task_worktree_command_reviews_durable_task_id_worktree() {
    let temp = tempfile::tempdir().unwrap();
    init_lab_command_git_repo(temp.path());
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let created = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "task create Fix hello | hello.txt | test -f hello.txt | update hello",
    );
    let task_id = created
        .lines()
        .find_map(|line| line.strip_prefix("Created graduate task: "))
        .unwrap()
        .to_string();
    let dispatch = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("task dispatch {task_id}"),
    );
    assert!(dispatch.contains("Prepared graduate dispatch"));

    let lab_store = LabStore::for_project(temp.path());
    let run = lab_store.latest_run().unwrap().unwrap();
    let dispatch = lab_store
        .list_graduate_dispatches(&run.lab_run_id)
        .unwrap()
        .into_iter()
        .find(|dispatch| dispatch.task_id == task_id)
        .unwrap();
    assert!(dispatch.agent_id.is_none());
    let durable_task_id = dispatch.agent_tool_params["task_id"]
        .as_str()
        .unwrap()
        .to_string();

    let manager = Arc::new(crate::engine::worktree::WorktreeManager::for_root(
        temp.path().to_path_buf(),
    ));
    let branch = "codex/lab-command-durable-review";
    let worktree_path = manager
        .create("lab-command-durable-review", Some(branch))
        .await
        .unwrap();
    std::fs::write(worktree_path.join("hello.txt"), "agent edit\n").unwrap();

    let session_store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
    session_store
        .create_session(
            "lab-test",
            "lab command durable worktree",
            "test-model",
            None,
        )
        .unwrap();
    session_store
        .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
            session_id: "lab-test".to_string(),
            task_id: durable_task_id.clone(),
            agent_id: "agent_runtime_1".to_string(),
            profile: Some("lab-graduate".to_string()),
            role: "implementation".to_string(),
            status: "completed".to_string(),
            description: "durable graduate worktree".to_string(),
            transcript_path: None,
            tool_ids_in_progress: Vec::new(),
            permission_requests: Vec::new(),
            result_artifact_id: None,
            cleanup_hooks: vec!["worktree_cleanup".to_string()],
            payload: serde_json::json!({
                "isolated_worktree": {
                    "path": worktree_path.to_string_lossy().to_string(),
                    "branch": branch
                }
            }),
        })
        .unwrap();

    let output = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        &format!("task worktree review {task_id}"),
        ToolContext::new(temp.path(), "lab-test")
            .with_session_store(session_store)
            .with_worktree_manager(manager),
    )
    .await;

    assert!(output.contains("Lab graduate worktree review succeeded"));
    assert!(output.contains("via task_id"));
    assert!(output.contains(&durable_task_id));
    assert!(output.contains("Agent worktree review: agent_runtime_1"));
    assert!(output.contains("hello.txt"));
}

#[tokio::test]
async fn task_worktree_command_merges_and_cleans_durable_task_id_worktree() {
    let temp = tempfile::tempdir().unwrap();
    init_lab_command_git_repo(temp.path());
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let created = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "task create Merge hello | hello.txt | test -f hello.txt | merge hello edit",
    );
    let task_id = created
        .lines()
        .find_map(|line| line.strip_prefix("Created graduate task: "))
        .unwrap()
        .to_string();
    let dispatch = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("task dispatch {task_id}"),
    );
    assert!(dispatch.contains("Prepared graduate dispatch"));

    let lab_store = LabStore::for_project(temp.path());
    let run = lab_store.latest_run().unwrap().unwrap();
    let dispatch = lab_store
        .list_graduate_dispatches(&run.lab_run_id)
        .unwrap()
        .into_iter()
        .find(|dispatch| dispatch.task_id == task_id)
        .unwrap();
    let durable_task_id = dispatch.agent_tool_params["task_id"]
        .as_str()
        .unwrap()
        .to_string();

    let manager = Arc::new(crate::engine::worktree::WorktreeManager::for_root(
        temp.path().to_path_buf(),
    ));
    let branch = "codex/lab-command-durable-merge";
    let worktree_path = manager
        .create("lab-command-durable-merge", Some(branch))
        .await
        .unwrap();
    std::fs::write(worktree_path.join("hello.txt"), "agent merged edit\n").unwrap();

    let session_store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
    session_store
        .create_session("lab-test", "lab command durable merge", "test-model", None)
        .unwrap();
    session_store
        .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
            session_id: "lab-test".to_string(),
            task_id: durable_task_id.clone(),
            agent_id: "agent_runtime_2".to_string(),
            profile: Some("lab-graduate".to_string()),
            role: "implementation".to_string(),
            status: "completed".to_string(),
            description: "durable graduate merge worktree".to_string(),
            transcript_path: None,
            tool_ids_in_progress: Vec::new(),
            permission_requests: Vec::new(),
            result_artifact_id: None,
            cleanup_hooks: vec!["worktree_cleanup".to_string()],
            payload: serde_json::json!({
                "isolated_worktree": {
                    "path": worktree_path.to_string_lossy().to_string(),
                    "branch": branch
                }
            }),
        })
        .unwrap();

    let context = ToolContext::new(temp.path(), "lab-test")
        .with_session_store(session_store)
        .with_worktree_manager(manager);
    let merge = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        &format!("task worktree merge {task_id}"),
        context.clone(),
    )
    .await;

    assert!(merge.contains("Lab graduate worktree merge succeeded"));
    assert!(merge.contains("via task_id"));
    assert!(merge.contains(&durable_task_id));
    assert_eq!(
        std::fs::read_to_string(temp.path().join("hello.txt")).unwrap(),
        "agent merged edit\n"
    );
    assert!(
        worktree_path.exists(),
        "merge should not remove dirty graduate worktree without cleanup"
    );

    let cleanup = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        &format!("task worktree cleanup {task_id} force"),
        context,
    )
    .await;

    assert!(cleanup.contains("Lab graduate worktree cleanup succeeded"));
    assert!(cleanup.contains("via task_id"));
    let cleaned_dispatch = lab_store
        .load_graduate_dispatch(&run.lab_run_id, &dispatch.dispatch_id)
        .unwrap();
    assert_eq!(
        cleaned_dispatch.cleanup_status,
        GraduateCleanupStatus::CleanupDone
    );
    assert!(cleaned_dispatch
        .cleanup_message
        .as_deref()
        .unwrap_or_default()
        .contains("cleanup succeeded"));
    assert!(
        !worktree_path.exists(),
        "force cleanup should remove the graduate worktree"
    );

    let events = std::fs::read_to_string(
        lab_store
            .root()
            .join("runs")
            .join(&run.lab_run_id)
            .join("events.jsonl"),
    )
    .unwrap();
    assert!(events.contains("\"event_type\":\"lab_graduate_worktree_action\""));
    assert!(events.contains("\"agent_ref_kind\":\"task_id\""));
    assert!(events.contains(&durable_task_id));
    assert!(events.contains("\"result_data\""));
    assert!(events.contains("\"merge_kind\":\"tracked_diff\""));
    assert!(events.contains("\"result_content_preview\""));

    let review = handle_lab_command(temp.path(), Some("session".to_string()), "review");
    assert!(review.contains("Graduate cleanup states:"));
    assert!(review.contains("cleanup_done"));
    assert!(review.contains("Graduate worktree proof:"));
    assert!(review.contains("agent_merge"));
    assert!(review.contains("agent_cleanup"));
    assert!(review.contains("ref=task_id:lab-graduate-"));
    assert!(review.contains("merge_kind=tracked_diff"));

    let dashboard = handle_lab_command(temp.path(), Some("session".to_string()), "dashboard");
    assert!(dashboard.contains("Graduate cleanup states:"));
    assert!(dashboard.contains("cleanup_done"));
    assert!(dashboard.contains("Graduate worktree proof:"));
    assert!(dashboard.contains("agent_merge"));
    assert!(dashboard.contains("ref=task_id:lab-graduate-"));
    assert!(dashboard.contains("merge_kind=tracked_diff"));
    let recovery = handle_lab_command(temp.path(), Some("session".to_string()), "recovery");
    assert!(recovery.contains("Graduate cleanup states:"));
    assert!(recovery.contains("cleanup_done"));
}

#[tokio::test]
async fn step_command_blocks_graduate_stage_without_task() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let store = LabStore::for_project(temp.path());
    let mut run = store.latest_run().unwrap().unwrap();
    run.current_stage = "graduate_work".to_string();
    run.internal_owner = LabRole::Graduate;
    store.save_run(&run).unwrap();

    let output = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "step",
        ToolContext::new(temp.path(), "lab-test"),
    )
    .await;

    assert!(output.contains("Lab scheduler step: Blocked"));
    assert!(output.contains("requires a queued GraduateTask"));
}

#[tokio::test]
async fn run_command_stops_when_scheduler_blocks() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let output = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "run 5",
        ToolContext::new(temp.path(), "lab-test"),
    )
    .await;

    assert!(output.contains("Blocked"));
    assert!(output.contains("Scheduler blocked at professor_discussion"));
}

#[tokio::test]
async fn background_command_starts_reports_and_stops_scheduler() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let started = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "background start 3 100",
        ToolContext::new(temp.path(), "lab-test"),
    )
    .await;
    assert!(started.contains("Started Lab background scheduler"));

    let status = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "background status",
        ToolContext::new(temp.path(), "lab-test"),
    )
    .await;
    assert!(status.contains("Running in process: true"));

    let stopped = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "background stop",
        ToolContext::new(temp.path(), "lab-test"),
    )
    .await;
    assert!(stopped.contains("Stopped Lab background scheduler"));
}

#[tokio::test]
async fn background_hybrid_command_requires_provider_context() {
    let temp = tempfile::tempdir().unwrap();
    let output = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "background hybrid 3 100 focus",
        ToolContext::new(temp.path(), "lab-test"),
    )
    .await;

    assert!(output.contains("requires an active Lab Mode provider"));
}

#[tokio::test]
async fn background_hybrid_command_starts_reports_and_stops_scheduler() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let provider = Arc::new(SequenceCommandProvider {
        responses: parking_lot::Mutex::new(std::collections::VecDeque::from([
            serde_json::json!({
                "professor_plan": {
                    "problem_statement": "Build LabRun",
                    "strategic_direction": "Keep background hybrid bounded.",
                    "success_criteria": ["hybrid background starts"],
                    "constraints": ["do not bypass runtime gates"],
                    "risks": ["weak provider evidence"],
                    "handoff_to_postdoc": "Create a small plan."
                }
            })
            .to_string(),
            r#"{"decision":"accept","note":"ready"}"#.to_string(),
        ])),
    });
    let context = ToolContext::new(temp.path(), "lab-background-hybrid-command")
        .with_llm_provider(provider)
        .with_model("mock-sequence".to_string());

    let started = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "background hybrid 3 100 background focus",
        context.clone(),
    )
    .await;
    assert!(started.contains("Started Lab hybrid background scheduler"));

    let status = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "background status",
        context.clone(),
    )
    .await;
    assert!(status.contains("Running in process: true"));
    assert!(status.contains("Persisted status: Running"));

    let stopped = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "background stop",
        context,
    )
    .await;
    assert!(stopped.contains("Stopped Lab background scheduler"));
}

#[tokio::test]
async fn background_hybrid_cycles_command_requires_provider_context() {
    let temp = tempfile::tempdir().unwrap();
    let output = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "background hybrid-cycles 2 5 100 focus",
        ToolContext::new(temp.path(), "lab-test"),
    )
    .await;

    assert!(output.contains("requires an active Lab Mode provider"));
}

#[tokio::test]
async fn background_hybrid_cycles_command_starts_reports_and_stops_scheduler() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let provider = Arc::new(SequenceCommandProvider {
        responses: parking_lot::Mutex::new(std::collections::VecDeque::from([
            serde_json::json!({
                "professor_plan": {
                    "problem_statement": "Build LabRun",
                    "strategic_direction": "Keep background cycles bounded.",
                    "success_criteria": ["hybrid-cycle background starts"],
                    "constraints": ["do not bypass runtime gates"],
                    "risks": ["weak provider evidence"],
                    "handoff_to_postdoc": "Create a small plan."
                }
            })
            .to_string(),
            r#"{"decision":"accept","note":"ready"}"#.to_string(),
        ])),
    });
    let context = ToolContext::new(temp.path(), "lab-background-hybrid-cycles-command")
        .with_llm_provider(provider)
        .with_model("mock-sequence".to_string());

    let started = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "background hybrid-cycles 2 5 100 background cycles",
        context.clone(),
    )
    .await;
    assert!(started.contains("Started Lab hybrid-cycle background scheduler"));
    assert!(started.contains("Max cycles: 2"));

    let status = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "background status",
        context.clone(),
    )
    .await;
    assert!(status.contains("Running in process: true"));
    assert!(status.contains("Persisted status: Running"));

    let stopped = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "background stop",
        context,
    )
    .await;
    assert!(stopped.contains("Stopped Lab background scheduler"));
}

#[tokio::test]
async fn background_start_refuses_missing_active_lease() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let store = LabStore::for_project(temp.path());
    let run = store.latest_run().unwrap().unwrap();
    std::fs::remove_file(store.root().join("active_lease.json")).unwrap();

    let output = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "background start 3 100",
        ToolContext::new(temp.path(), "lab-test"),
    )
    .await;

    assert!(output.contains("Failed to start Lab background scheduler"));
    assert!(output.contains("active lease is missing"));
    assert!(store
        .load_scheduler_state(&run.lab_run_id)
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn background_recover_marks_interrupted_scheduler_resumable() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let store = LabStore::for_project(temp.path());
    let run = store.latest_run().unwrap().unwrap();
    let now = chrono::Utc::now();
    store
        .write_scheduler_state(&crate::lab::model::LabSchedulerState {
            schema_version: crate::lab::model::LAB_SCHEMA_VERSION,
            lab_run_id: run.lab_run_id.clone(),
            status: crate::lab::model::LabSchedulerStatus::Running,
            updated_at: now,
            started_at: Some(now),
            stopped_at: None,
            max_steps: 10,
            steps_completed: 2,
            interval_ms: 250,
            last_action: None,
            last_message: None,
            stop_reason: None,
        })
        .unwrap();

    let recovered = handle_lab_command_with_context(
        temp.path(),
        Some("session".to_string()),
        "background recover",
        ToolContext::new(temp.path(), "lab-test"),
    )
    .await;

    assert!(recovered.contains("Recovered interrupted Lab background scheduler"));
    assert!(recovered.contains("Status: PausedRestart"));
    assert!(recovered.contains("Stop reason: process_restart"));
}

#[test]
fn cycle_summary_command_writes_artifact_and_report() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let output = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "cycle summary Finished initial planning slice",
    );

    assert!(output.contains("Created cycle summary"));
    assert!(output.contains("Artifact: "));
    assert!(output.contains("Report: "));
    let status = handle_lab_command(temp.path(), Some("session".to_string()), "status");
    assert!(status.contains("Cycles: 1"));
}

#[test]
fn meeting_recommend_command_reports_no_signal_by_default() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let output = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "meeting recommend",
    );

    assert!(output.contains("Suggested meeting: false"));
    assert!(output.contains("Signals: none"));
}

#[test]
fn meeting_open_refuses_without_recommendation_signal() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let output = handle_lab_command(temp.path(), Some("session".to_string()), "meeting open");

    assert!(output.contains("No runtime escalation signal is open"));
    assert!(output.contains("Use /lab meeting <topic>"));
    let store = LabStore::for_project(temp.path());
    let run = store.latest_run().unwrap().unwrap();
    assert!(run.meeting_ids.is_empty());
}

#[test]
fn meeting_open_creates_read_only_report_from_recommendation_signal() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let queued = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "professor Turn this concern into a scoped implementation task",
    );
    let message_id = queued
        .lines()
        .find_map(|line| line.strip_prefix("Message queued for professor: "))
        .unwrap()
        .to_string();
    let converted = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("messages task {message_id}"),
    );
    assert!(converted.contains("ConvertedToTask"));
    let applied = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("messages apply {message_id} implementation task"),
    );
    assert!(applied.contains("applied as blocked graduate task"));
    let recommendation = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "meeting recommend",
    );
    assert!(recommendation.contains("Suggested meeting: true"));
    assert!(recommendation.contains("Open meeting with /lab meeting open"));

    let opened = handle_lab_command(temp.path(), Some("session".to_string()), "meeting open");

    assert!(opened.contains("Lab meeting opened from runtime escalation signal"));
    assert!(opened.contains("This meeting is read-only and does not mutate code."));
    assert!(opened.contains("Topic: resolve 1 blocked graduate task(s)"));
    assert!(opened.contains("Request: "));
    assert!(opened.contains("Request report: "));
    assert!(opened.contains("Artifact: "));
    assert!(opened.contains("Report: "));
    let store = LabStore::for_project(temp.path());
    let run = store.latest_run().unwrap().unwrap();
    assert_eq!(run.meeting_ids.len(), 1);
    let artifacts = store.list_stage_artifacts(&run.lab_run_id).unwrap();
    assert!(artifacts.iter().any(|artifact| matches!(
        artifact,
        StageArtifact::LabMeetingRequest(request)
            if request.body.reason == "runtime_escalation_signals_present"
                && request.body.topic.starts_with("resolve 1 blocked graduate task")
    )));
    assert!(artifacts
        .iter()
        .any(|artifact| matches!(artifact, StageArtifact::LabMeetingSummary(_))));
    assert!(store.root().join("active_lease.json").exists());
}

#[test]
fn blocker_report_command_writes_artifact_and_report() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let created = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "task create Fix lab model | src/lab/model.rs | cargo check -q | update model",
    );
    let task_id = created
        .lines()
        .find_map(|line| line.strip_prefix("Created graduate task: "))
        .unwrap()
        .to_string();
    let blocked = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("task block {task_id} validation failed"),
    );
    assert!(blocked.contains("Blocked graduate task"));

    let output = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "blocker report Need professor decision",
    );

    assert!(output.contains("Lab blocker report created"));
    assert!(output.contains("Artifact: "));
    assert!(output.contains("Report: "));

    let escalated =
        handle_lab_command(temp.path(), Some("session".to_string()), "blocker escalate");
    assert!(escalated.contains("Escalated Lab blocker to professor review"));
    assert!(escalated.contains("Stage: professor_review"));
}

#[test]
fn task_revise_command_requeues_blocked_task() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let created = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "task create Fix lab model | | cargo check -q | update model",
    );
    let task_id = created
        .lines()
        .find_map(|line| line.strip_prefix("Created graduate task: "))
        .unwrap()
        .to_string();
    let blocked = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("task block {task_id} missing scope"),
    );
    assert!(blocked.contains("Blocked graduate task"));

    let revised = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!(
            "task revise {task_id} | src/lab/commands.rs | cargo check -q --tests | narrow command repair"
        ),
    );

    assert!(revised.contains("Revised graduate task"));
    assert!(revised.contains("Status: Queued"));
    assert!(revised.contains("src/lab/commands.rs"));
    assert!(revised.contains("cargo check -q --tests"));
    assert!(revised.contains("Blocker: none"));
}

#[test]
fn integrate_command_writes_postdoc_summary() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", Some("session".to_string()))
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update lab commands.",
            vec!["src/lab/commands.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    orchestrator
        .create_graduate_result_for_task_latest(
            &task.task_id,
            "Implemented command path.",
            vec!["src/lab/commands.rs".to_string()],
            vec!["cargo check -q passed".to_string()],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
    let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "postdoc_review".to_string();
    saved.internal_owner = crate::lab::model::LabRole::Postdoc;
    orchestrator.store().save_run(&saved).unwrap();

    let output = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "integrate Ready for professor review",
    );

    assert!(output.contains("Created postdoc integration summary"));
    assert!(output.contains("Gate: postdoc_review (satisfied)"));
    assert!(output.contains("Artifact: "));
    assert!(output.contains("Report: "));
}

#[test]
fn professor_review_command_writes_final_review() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", Some("session".to_string()))
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update lab commands.",
            vec!["src/lab/commands.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    orchestrator
        .create_graduate_result_for_task_latest(
            &task.task_id,
            "Implemented command path.",
            vec!["src/lab/commands.rs".to_string()],
            vec!["cargo check -q passed".to_string()],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
    let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "postdoc_review".to_string();
    saved.internal_owner = crate::lab::model::LabRole::Postdoc;
    orchestrator.store().save_run(&saved).unwrap();
    orchestrator
        .create_postdoc_integration_summary_for_latest(Some("Ready for professor."))
        .unwrap();
    let advanced = orchestrator.advance_latest().unwrap();
    assert_eq!(advanced.current_stage, "professor_review");

    let output = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "professor-review Final professor review",
    );

    assert!(output.contains("Created professor review"));
    assert!(output.contains("Gate: professor_review (blocked)"));
    assert!(output.contains("Artifact: "));
    assert!(output.contains("Report: "));
}

#[test]
fn task_retry_command_creates_repair_task() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let created = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "task create Fix lab model | src/lab/model.rs | cargo check -q | update model",
    );
    let task_id = created
        .lines()
        .find_map(|line| line.strip_prefix("Created graduate task: "))
        .unwrap()
        .to_string();

    let output = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("task retry {task_id} | cargo check failed"),
    );

    assert!(output.contains("Recorded validation retry"));
    assert!(output.contains("Attempt: 1"));
    assert!(output.contains("Repair task: gradtask_"));
    assert!(output.contains("Escalated: false"));

    let blocker_status =
        handle_lab_command(temp.path(), Some("session".to_string()), "blocker status");
    assert!(blocker_status.contains("validation_retries=1"));
    assert!(blocker_status.contains("escalated_retries=0"));
}

#[test]
fn compression_command_records_context_decision() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let output = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "compression professor",
    );

    assert!(output.contains("Lab compression decision"));
    assert!(output.contains("role=Professor"));
    assert!(output.contains("action="));
    assert!(output.contains("stable_hash="));
    assert!(temp.path().join(".priority-agent/lab/runs").exists());
}

#[test]
fn compress_command_writes_summary_when_budget_requires_it() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));
    let store = LabStore::for_project(temp.path());
    let mut run = store.latest_run().unwrap().unwrap();
    run.cost_policy.professor_context_budget = 10;
    store.save_run(&run).unwrap();

    let output = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        "compress professor",
    );

    assert!(output.contains("Created compression summary"));
    assert!(output.contains("Artifact: "));
    assert!(output.contains("Report: "));
}

#[test]
fn tick_command_runs_one_orchestration_step() {
    let temp = tempfile::tempdir().unwrap();
    let proposal = handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
    let proposal_id = proposal
        .lines()
        .find_map(|line| line.strip_prefix("Lab proposal created: "))
        .unwrap()
        .to_string();
    let approved = handle_lab_command(
        temp.path(),
        Some("session".to_string()),
        &format!("approve {proposal_id}"),
    );
    assert!(approved.contains("LabRun created"));

    let output = handle_lab_command(temp.path(), Some("session".to_string()), "tick");

    assert!(output.contains("Lab tick: Blocked"));
    assert!(output.contains("Stage: professor_discussion -> professor_discussion"));
}
