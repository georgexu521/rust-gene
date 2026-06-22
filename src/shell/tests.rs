use super::*;
use crate::shell::surface::TestSurface;
use crate::shell::test_support::{test_cli_host, test_engine};

#[test]
fn parse_lab_background_hybrid_args_supports_numeric_prefixes_and_instructions() {
    let (steps, interval, instructions) =
        parse_lab_background_hybrid_args("7 250 refine professor plan").unwrap();
    assert_eq!(steps, 7);
    assert_eq!(interval, 250);
    assert_eq!(instructions, "refine professor plan");

    let (steps, interval, instructions) =
        parse_lab_background_hybrid_args("refine professor plan").unwrap();
    assert_eq!(steps, crate::lab::scheduler::default_background_max_steps());
    assert_eq!(
        interval,
        crate::lab::scheduler::default_background_interval_ms()
    );
    assert_eq!(instructions, "refine professor plan");

    let (steps, interval, instructions) =
        parse_lab_background_hybrid_args("3 refine professor plan").unwrap();
    assert_eq!(steps, 3);
    assert_eq!(
        interval,
        crate::lab::scheduler::default_background_interval_ms()
    );
    assert_eq!(instructions, "refine professor plan");
}

#[test]
fn lab_welcome_hint_reflects_intake_proposal_and_active_run() {
    let temp = tempfile::tempdir().unwrap();

    let empty = lab_welcome_hint(temp.path());
    assert!(empty.contains("Professor intake ready"));
    assert!(empty.contains("/lab propose <idea>"));

    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build Lab Mode", None).unwrap();
    let proposed = lab_welcome_hint(temp.path());
    assert!(proposed.contains(&proposal.proposal_id));
    assert!(proposed.contains("approve with /lab approve"));

    let run = store.approve_proposal(&proposal.proposal_id).unwrap();
    let active = lab_welcome_hint(temp.path());
    assert!(active.contains(&run.lab_run_id));
    assert!(active.contains("stage=professor_discussion"));
    assert!(active.contains("/lab dashboard"));
    assert!(active.contains("/lab recovery"));

    let paused = store.pause_latest_run_for_shutdown().unwrap().unwrap();
    let recover = lab_welcome_hint(temp.path());
    assert!(recover.contains(&paused.lab_run_id));
    assert!(recover.contains("Recover LabRun"));
    assert!(recover.contains("/lab resume"));
    assert!(recover.contains("/lab recovery"));
}

#[tokio::test]
async fn handle_help_command_prints_commands() {
    let engine = test_engine();
    let mut host = test_cli_host(engine.clone());
    let mut surface = TestSurface::new();
    let mut attachments = AttachmentManager::new();

    let consumed =
        handle_local_command(&mut host, &engine, "/help", &mut surface, &mut attachments)
            .await
            .unwrap();

    assert!(consumed, "/help should be consumed");
    assert!(attachments.is_empty());
}

#[tokio::test]
async fn handle_unknown_slash_command_is_consumed() {
    let engine = test_engine();
    let mut host = test_cli_host(engine.clone());
    let mut surface = TestSurface::new();
    let mut attachments = AttachmentManager::new();

    let consumed = handle_local_command(
        &mut host,
        &engine,
        "/notacommand",
        &mut surface,
        &mut attachments,
    )
    .await
    .unwrap();

    assert!(consumed, "unknown slash commands are still consumed");
}

#[tokio::test]
async fn handle_plain_message_is_not_consumed() {
    let engine = test_engine();
    let mut host = test_cli_host(engine.clone());
    let mut surface = TestSurface::new();
    let mut attachments = AttachmentManager::new();

    let consumed =
        handle_local_command(&mut host, &engine, "hello", &mut surface, &mut attachments)
            .await
            .unwrap();

    assert!(!consumed, "plain messages should not be consumed");
}

#[tokio::test]
async fn handle_exit_command_is_consumed() {
    let engine = test_engine();
    let mut host = test_cli_host(engine.clone());
    let mut surface = TestSurface::new();
    let mut attachments = AttachmentManager::new();

    let consumed =
        handle_local_command(&mut host, &engine, "/exit", &mut surface, &mut attachments)
            .await
            .unwrap();

    assert!(consumed, "/exit should be consumed");
}

#[tokio::test]
async fn handle_new_command_creates_session() {
    let engine = test_engine();
    let mut host = test_cli_host(engine.clone());
    let mut surface = TestSurface::new();
    let mut attachments = AttachmentManager::new();

    let consumed = handle_local_command(&mut host, &engine, "/new", &mut surface, &mut attachments)
        .await
        .unwrap();

    assert!(consumed);
    let sid = host.session_manager.current_session_id();
    assert!(sid.is_some(), "/new should create a session");
    assert_eq!(sid, engine.current_session_id().as_deref());
}

#[tokio::test]
async fn handle_clear_command_is_consumed() {
    let engine = test_engine();
    let mut host = test_cli_host(engine.clone());
    let mut surface = TestSurface::new();
    let mut attachments = AttachmentManager::new();

    let consumed =
        handle_local_command(&mut host, &engine, "/clear", &mut surface, &mut attachments)
            .await
            .unwrap();

    assert!(consumed, "/clear should be consumed");
}

#[tokio::test]
async fn handle_model_command_is_consumed() {
    let engine = test_engine();
    let mut host = test_cli_host(engine.clone());
    let mut surface = TestSurface::new();
    let mut attachments = AttachmentManager::new();

    let consumed =
        handle_local_command(&mut host, &engine, "/model", &mut surface, &mut attachments)
            .await
            .unwrap();

    assert!(consumed, "/model should be consumed");
}

#[tokio::test]
async fn handle_status_command_is_consumed() {
    let engine = test_engine();
    let mut host = test_cli_host(engine.clone());
    let mut surface = TestSurface::new();
    let mut attachments = AttachmentManager::new();

    let consumed = handle_local_command(
        &mut host,
        &engine,
        "/status",
        &mut surface,
        &mut attachments,
    )
    .await
    .unwrap();

    assert!(consumed, "/status should be consumed");
}

#[tokio::test]
async fn handle_attach_and_detach_commands() {
    let engine = test_engine();
    let mut host = test_cli_host(engine.clone());
    let mut surface = TestSurface::new();
    let mut attachments = AttachmentManager::new();

    let file_path = std::env::current_dir().unwrap().join("Cargo.toml");
    let cmd = format!("/attach {}", file_path.display());
    let consumed = handle_local_command(&mut host, &engine, &cmd, &mut surface, &mut attachments)
        .await
        .unwrap();
    assert!(consumed, "/attach should be consumed");
    assert_eq!(attachments.count(), 1);

    let consumed = handle_local_command(
        &mut host,
        &engine,
        "/detach all",
        &mut surface,
        &mut attachments,
    )
    .await
    .unwrap();
    assert!(consumed, "/detach all should be consumed");
    assert!(attachments.is_empty());
}
