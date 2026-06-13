use super::*;

#[test]
fn test_command_lookup() {
    let registry = default_command_registry();
    assert!(registry.get("/help").is_some());
    assert!(registry.get("/h").is_some()); // alias
    assert!(registry.get("/tool-output").is_some());
    assert!(registry.get("/tool").is_some()); // alias
    assert!(registry.get("/panel").is_some());
    assert!(registry.get("/runtime").is_some()); // alias
    assert!(registry.get("/quit").is_some());
    assert!(registry.get("/exit").is_some()); // alias
    assert!(registry.get("/nonexistent").is_none());
}

#[test]
fn test_help_text() {
    let registry = default_command_registry();
    let help = registry.help_text();
    assert!(help.contains("/help"));
    assert!(help.contains("/cost"));
    assert!(help.contains("General:"));
    assert!(help.contains("Memory:"));
    assert!(help.contains("[production]"));
    assert!(help.contains("[usable]"));
    assert!(!help.contains("[placeholder]"));
    assert!(!help.contains("/desktop"));

    let all_help = registry.help_text_all();
    assert!(all_help.contains("[placeholder]"));
    assert!(all_help.contains("/desktop"));
}

#[test]
fn test_command_maturity_labels_are_explicit() {
    let registry = default_command_registry();
    assert_eq!(
        registry.get("/help").map(|cmd| cmd.maturity),
        Some(CommandMaturity::Production)
    );
    assert_eq!(
        registry.get("/agents").map(|cmd| cmd.maturity),
        Some(CommandMaturity::Usable)
    );
    assert_eq!(
        registry.get("/panel").map(|cmd| cmd.maturity),
        Some(CommandMaturity::Usable)
    );
    assert_eq!(
        registry.get("/runtime").map(|cmd| cmd.maturity),
        Some(CommandMaturity::Usable)
    );
    assert_eq!(
        registry.get("/tool-output").map(|cmd| cmd.maturity),
        Some(CommandMaturity::Usable)
    );
    assert_eq!(
        registry.get("/tool").map(|cmd| cmd.maturity),
        Some(CommandMaturity::Usable)
    );
    assert_eq!(
        registry.get("/desktop").map(|cmd| cmd.maturity),
        Some(CommandMaturity::Placeholder)
    );
    assert_eq!(
        registry.get("/desktop").map(|cmd| cmd.placeholder),
        Some(true)
    );
    assert!(!registry
        .maturity_commands(CommandMaturity::Placeholder)
        .is_empty());
}

#[test]
fn test_command_maturity_lists_are_registered_and_disjoint() {
    let registry = default_command_registry();
    let mut listed = HashSet::new();

    for name in USABLE_COMMANDS {
        assert!(
            registry.get(name).is_some(),
            "usable command {name} is registered"
        );
        assert!(
            listed.insert(*name),
            "duplicate command maturity entry {name}"
        );
    }
    for name in PLACEHOLDER_COMMANDS {
        assert!(
            registry.get(name).is_some(),
            "placeholder command {name} is registered"
        );
        assert!(
            listed.insert(*name),
            "duplicate command maturity entry {name}"
        );
    }

    let summary = registry.maturity_summary();
    assert_eq!(
        summary.get(CommandMaturity::Usable.label()).copied(),
        Some(USABLE_COMMANDS.len())
    );
    assert_eq!(
        summary.get(CommandMaturity::Placeholder.label()).copied(),
        Some(PLACEHOLDER_COMMANDS.len())
    );
    assert!(
        summary
            .get(CommandMaturity::Production.label())
            .copied()
            .unwrap_or_default()
            > 0
    );
}

#[test]
fn test_maturity_report_lists_runtime_surfaces() {
    let registry = default_command_registry();
    let report = registry.maturity_report();

    assert!(report.contains("Command maturity:"));
    assert!(report.contains("- usable"));
    assert!(report.contains("/panel"));
    assert!(report.contains("/tool-output"));
    assert!(report.contains("- placeholder"));
    assert!(report.contains("/desktop"));
}

#[test]
fn test_palette_items_filters_and_deduplicates_aliases() {
    let registry = default_command_registry();
    let items = registry.palette_items("help", 20, &[]);
    assert!(items.iter().any(|cmd| cmd.name == "/help"));
    let help_count = items.iter().filter(|cmd| cmd.name == "/help").count();
    assert_eq!(help_count, 1);
}

#[test]
fn test_palette_items_rank_exact_command_above_description_match() {
    let registry = default_command_registry();
    let items = registry.palette_items("model", 20, &[]);
    assert_eq!(items.first().map(|cmd| cmd.name), Some("/model"));
}

#[test]
fn test_palette_items_support_subsequence_query() {
    let registry = default_command_registry();
    let items = registry.palette_items("prv", 20, &[]);
    assert!(items.iter().any(|cmd| cmd.name == "/provider"));
}

#[test]
fn test_palette_items_rank_recent_commands_when_query_empty() {
    let registry = default_command_registry();
    let recent = vec!["/provider".to_string()];
    let items = registry.palette_items("", 20, &recent);
    assert_eq!(items.first().map(|cmd| cmd.name), Some("/provider"));
}

#[test]
fn test_palette_items_show_suggested_commands_first_when_empty() {
    let registry = default_command_registry();
    let items = registry.palette_items("", 20, &[]);
    let names = items.iter().take(4).map(|cmd| cmd.name).collect::<Vec<_>>();
    assert_eq!(names, vec!["/quick", "/doctor", "/permissions", "/session"]);
}

#[test]
fn test_palette_items_hide_placeholder_until_explicit_query() {
    let registry = default_command_registry();
    let default_items = registry.palette_items("", 200, &[]);
    assert!(!default_items.iter().any(|cmd| cmd.name == "/desktop"));

    let explicit_items = registry.palette_items("desktop", 20, &[]);
    assert!(explicit_items.iter().any(|cmd| cmd.name == "/desktop"));
}

#[test]
fn test_command_accept_behavior_inserts_required_args() {
    assert_eq!(
        command_accept_behavior(&CMD_SAVE),
        CommandAcceptBehavior::Insert
    );
    assert_eq!(
        command_accept_behavior(&CMD_STATUS),
        CommandAcceptBehavior::Execute
    );
    assert_eq!(
        command_accept_behavior(&CMD_PROMPT_HISTORY),
        CommandAcceptBehavior::Execute
    );
    assert_eq!(
        command_accept_behavior(&CMD_PROMPT_STASH),
        CommandAcceptBehavior::Execute
    );
    let registry = default_command_registry();
    assert_eq!(
        registry.get("/desktop").map(command_accept_behavior),
        Some(CommandAcceptBehavior::Insert)
    );
}

#[test]
fn test_prompt_composer_commands_are_registered() {
    let registry = default_command_registry();
    let history_items = registry.palette_items("prompt history", 20, &[]);
    assert!(history_items
        .iter()
        .any(|cmd| cmd.name == "/prompt-history"));

    let stash_items = registry.palette_items("stash", 20, &[]);
    assert!(stash_items.iter().any(|cmd| cmd.name == "/prompt-stash"));

    let paste_items = registry.palette_items("paste", 20, &[]);
    assert!(paste_items.iter().any(|cmd| cmd.name == "/paste"));

    let attach_items = registry.palette_items("attach", 20, &[]);
    assert!(attach_items.iter().any(|cmd| cmd.name == "/attach"));

    let jump_items = registry.palette_items("jump", 20, &[]);
    assert!(jump_items.iter().any(|cmd| cmd.name == "/jump"));
}
