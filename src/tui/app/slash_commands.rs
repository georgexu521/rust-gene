use super::*;

impl TuiApp {
    /// 处理斜杠命令
    pub(super) async fn handle_slash_command(&mut self, input: &str) {
        let parts: Vec<&str> = input.trim().splitn(2, ' ').collect();
        let cmd = parts[0].to_lowercase();
        let args = parts.get(1).unwrap_or(&"");

        use crate::tui::slash_handler as slash;

        let response = match cmd.as_str() {
            "/help" | "/h" => {
                if args.trim() == "maturity" {
                    return self.add_system_message(self.command_registry.maturity_report());
                }
                let mut help = self.command_registry.help_text();
                help.push_str("\n\nSession Commands:\n");
                help.push_str("  /sessions    - List recent sessions\n");
                help.push_str("  /session     - Show current session or restore by number/ID\n");
                help.push_str("  /new         - Start a new session\n");
                help.push_str("  /export      - Export current session to JSON\n");
                help.push_str("  /search      - Search through all sessions\n");
                help.push_str("  /stats       - Show session statistics\n");
                help.push_str("\nSettings:\n");
                help.push_str("  /settings    - Open settings interface\n");
                help.push_str("  /permissions - View/update permission mode and policy rules\n");
                help.push_str("  /mcp         - Manage MCP server approvals\n");
                help.push_str("  /voice       - Check voice TTS/STT status\n");
                help.push_str("  /telemetry   - View telemetry status\n");
                help.push_str("  /onboarding  - Restart the onboarding guide\n");
                help.push_str("\nThe agent has 30+ tools (file, bash, web, github, memory, cron, swarm, MCP, skills, project).\nJust ask naturally - the agent will use the right tools.");
                help
            }
            "/clear" => {
                if let Some(ref engine) = self.streaming_engine {
                    engine.clear_history().await;
                }
                self.messages.clear();
                self.clear_tool_transcript();
                "Conversation history cleared.".to_string()
            }
            "/memory" => {
                let query = args.trim();
                let maintain = query == "--maintain";
                let doctor = matches!(
                    query,
                    "--doctor" | "doctor" | "doctor json" | "doctor --json" | "--doctor json"
                );
                let doctor_json =
                    matches!(query, "doctor json" | "doctor --json" | "--doctor json");
                let (memory_action, memory_arg) = query
                    .split_once(' ')
                    .map(|(action, rest)| (action, rest.trim()))
                    .unwrap_or((query, ""));
                if memory_action == "status" {
                    let write_policy = std::env::var("PRIORITY_AGENT_AUTO_MEMORY_WRITE")
                        .unwrap_or_else(|_| "review_only".to_string());
                    let active_memory = std::env::var("PRIORITY_AGENT_ACTIVE_MEMORY")
                        .map(|v| v == "1")
                        .unwrap_or(false);
                    let use_status = if self.memory_use { "on" } else { "off" };
                    let generate_status = if self.memory_generate { "on" } else { "off" };
                    let recall_status = &self.memory_recall_mode;
                    let active_status = if active_memory { "on" } else { "off" };

                    let use_explanation = if self.memory_use {
                        "Memory is being loaded into context. Pinned items, project notes, and user preferences are available."
                    } else {
                        "Memory is disabled. No memory items will be loaded into context."
                    };
                    let generate_explanation = if self.memory_generate {
                        "The agent will propose memory updates during closeout. Proposals go to review first."
                    } else {
                        "Memory generation is off. The agent will not propose memory updates."
                    };
                    let recall_explanation = match recall_status.as_str() {
                        "off" => "Active recall is disabled. Memory is only loaded from pinned and static sources.",
                        "strict" => "Strict recall: only high-confidence, directly relevant memories are retrieved.",
                        "balanced" => "Balanced recall: relevant memories are retrieved with moderate filtering.",
                        "preference-only" => "Only explicit user preferences are recalled, not project facts.",
                        _ => "Unknown recall mode.",
                    };
                    let write_explanation = match write_policy.as_str() {
                        "review_only" => "All memory proposals require manual review before persistence.",
                        "narrow" => "Only explicit user preference statements are auto-persisted during closeout.",
                        "legacy" => "Legacy auto-write: memory proposals are auto-persisted without review.",
                        _ => &write_policy,
                    };
                    let active_explanation = if active_memory {
                        "Active memory is enabled. A background FTS worker may retrieve additional context for interactive sessions."
                    } else {
                        "Active memory is off. Only pinned and recall-based memory is used."
                    };

                    if memory_arg.contains("--json") {
                        return self.add_system_message(
                            serde_json::json!({
                                "use": use_status,
                                "generate": generate_status,
                                "recall": recall_status,
                                "write_policy": write_policy,
                                "active": active_status,
                                "explanations": {
                                    "use": use_explanation,
                                    "generate": generate_explanation,
                                    "recall": recall_explanation,
                                    "write_policy": write_explanation,
                                    "active": active_explanation,
                                }
                            })
                            .to_string(),
                        );
                    }

                    return self.add_system_message(format!(
                        "Memory Status\n\n\
                         Stable Prefix:\n\
                         - project memory: loaded into system prompt\n\
                         - user memory: loaded if available\n\
                         - accepted facts: indexed for recall\n\n\
                         Turn-Tail Updates:\n\
                         - proposals pending: checked during closeout\n\
                         - new memories: apply on next session reload\n\n\
                         On-Demand Reads:\n\
                         - retrieval policy: {recall_status}\n\
                         - active memory: {active_status}\n\n\
                         Controls:\n\
                         - use: {use_status} (load memory into context)\n\
                         - generate: {generate_status} (propose memory updates)\n\
                         - recall: {recall_status} (active retrieval mode)\n\
                         - write-policy: {write_policy} (auto-persist policy)\n\
                         - active: {active_status} (background FTS worker)\n\n\
                         Commands:\n\
                         - /memory control use on|off\n\
                         - /memory control generate on|off\n\
                         - /memory control recall off|strict|balanced|preference-only\n\
                         - /memory status (this view)\n\
                         - /memory status --json (machine-readable)\n\
                         - /memory doctor (detailed diagnostics)\n\n\
                         Memory Model:\n\
                         - Stable prefix: project/user memory docs + accepted durable facts\n\
                         - Turn-tail updates: notes about memory changes this session\n\
                         - On-demand reads: agent reads source files when index says relevant"
                    ));
                }
                if memory_action == "control" {
                    let mut parts = memory_arg.split_whitespace();
                    let Some(control) = parts.next() else {
                        return self.add_system_message(format!(
                            "Memory controls\n- use: {}\n- generate: {}\n- recall: {}\n- write-policy: {}\n\nUsage:\n- /memory control use on|off\n- /memory control generate on|off\n- /memory control recall off|strict|balanced|preference-only\n- /memory control write-policy review_only|narrow|legacy",
                            if self.memory_use { "on" } else { "off" },
                            if self.memory_generate { "on" } else { "off" },
                            self.memory_recall_mode,
                            std::env::var("PRIORITY_AGENT_AUTO_MEMORY_WRITE").unwrap_or_else(|_| "review_only".to_string())
                        ));
                    };
                    let Some(value) = parts.next() else {
                        return self.add_system_message(
                            "Usage: /memory control use on|off\n       /memory control generate on|off\n       /memory control recall off|strict|balanced|preference-only\n       /memory control write-policy review_only|narrow|legacy"
                                .to_string(),
                        );
                    };
                    match control {
                        "use" => {
                            let Some(enabled) = parse_on_off(value) else {
                                return self.add_system_message(
                                    "Usage: /memory control use on|off".to_string(),
                                );
                            };
                            self.memory_use = enabled;
                            if let Some(ref engine) = self.streaming_engine {
                                engine.set_memory_use(enabled);
                            }
                        }
                        "generate" => {
                            let Some(enabled) = parse_on_off(value) else {
                                return self.add_system_message(
                                    "Usage: /memory control generate on|off".to_string(),
                                );
                            };
                            self.memory_generate = enabled;
                            if let Some(ref engine) = self.streaming_engine {
                                engine.set_memory_generate(enabled);
                            }
                        }
                        "recall" | "active_recall" => {
                            let mode = value.to_ascii_lowercase();
                            if !matches!(
                                mode.as_str(),
                                "off" | "strict" | "balanced" | "preference-only"
                            ) {
                                return self.add_system_message(
                                    "Usage: /memory control recall off|strict|balanced|preference-only"
                                        .to_string(),
                                );
                            }
                            self.memory_recall_mode = mode;
                            if let Some(ref engine) = self.streaming_engine {
                                engine.set_memory_recall_mode(self.memory_recall_mode.clone());
                            }
                        }
                        "write-policy" | "write_policy" => {
                            let policy = value.to_ascii_lowercase();
                            if !matches!(policy.as_str(), "review_only" | "narrow" | "legacy") {
                                return self.add_system_message(
                                    "Usage: /memory control write-policy review_only|narrow|legacy"
                                        .to_string(),
                                );
                            }
                            std::env::set_var("PRIORITY_AGENT_AUTO_MEMORY_WRITE", &policy);
                        }
                        _ => {
                            return self.add_system_message(
                                "Usage: /memory control use on|off\n       /memory control generate on|off\n       /memory control recall off|strict|balanced|preference-only\n       /memory control write-policy review_only|narrow|legacy"
                                    .to_string(),
                            );
                        }
                    }
                    let write_policy = std::env::var("PRIORITY_AGENT_AUTO_MEMORY_WRITE")
                        .unwrap_or_else(|_| "review_only".to_string());
                    return self.add_system_message(format!(
                        "Memory controls\n- use: {}\n- generate: {}\n- recall: {}\n- write-policy: {}",
                        if self.memory_use { "on" } else { "off" },
                        if self.memory_generate { "on" } else { "off" },
                        self.memory_recall_mode,
                        write_policy
                    ));
                }
                if memory_action == "review" {
                    let report = slash::memory_review_report(self).await;
                    self.add_system_message(report);
                    return;
                }
                if memory_action == "files" {
                    self.add_system_message(slash::memory_files_report());
                    return;
                }
                let latest_user_message = self
                    .messages
                    .iter()
                    .rev()
                    .find(|m| m.role == MessageRole::User)
                    .map(|m| m.content.as_str())
                    .unwrap_or("");

                let memory_manager = if let Some(ref engine) = self.streaming_engine {
                    engine.memory_manager_or_init()
                } else {
                    None
                };

                if let Some(memory_manager) = memory_manager {
                    let mem = memory_manager.lock().await;
                    if maintain {
                        let report = mem.maintain_memory();
                        report.format()
                    } else if doctor {
                        let summary = mem.memory_summary();
                        let decisions = mem.memory_decision_counts();
                        let flushes = mem.memory_flush_summary();
                        let calibration = crate::memory::run_memory_calibration_samples();
                        let eval_suite = crate::memory::run_memory_eval_suite();
                        let calibration_passed =
                            calibration.iter().filter(|result| result.passed).count();
                        let conflicts = mem.memory_conflicts(8);
                        if doctor_json {
                            let snapshot = mem.memory_snapshot_report();
                            let proposal_queue = memory_proposal_queue_json();
                            serde_json::json!({
                                "summary": {
                                    "project_memory_chars": summary.project_memory_chars,
                                    "project_memory_files": summary.project_memory_files,
                                    "project_memory_file_chars": summary.project_memory_file_chars,
                                    "user_memory_chars": summary.user_memory_chars,
                                    "session_memory_items": summary.session_memory_items,
                                    "has_frozen_snapshot": summary.has_frozen_snapshot,
                                },
                                "snapshot": snapshot,
                                "proposal_queue": proposal_queue,
                                "decisions": decisions,
                                "flushes": flushes,
                                "quality_gates": {
                                    "accept_threshold": 0.65,
                                    "propose_threshold": 0.45,
                                    "explicit_override_threshold": 0.60,
                                    "hard_stops": ["unsafe_content", "secret_like_content", "duplicate_memory"],
                                },
                                "calibration": {
                                    "passed": calibration_passed,
                                    "total": calibration.len(),
                                    "results": calibration,
                                },
                                "eval_suite": eval_suite,
                                "conflicts": conflicts,
                            })
                            .to_string()
                        } else {
                            let snapshot = mem.memory_snapshot_report();
                            let proposal_queue = format_memory_proposal_queue();
                            format!(
                                "# Memory Doctor\n\n{}\n\n{}\n\n{}\n\nDecisions:\n  Accepted: {}\n  Proposed: {}\n  Rejected: {}\n  Blocked: {}\n\n{}\n\nQuality gates:\n  accept>=0.65 · propose>=0.45 · explicit>=0.60 with safety/duplicate hard stops\n\nCalibration: {}/{} passed\nMemory evals: {}/{} passed",
                                summary.format(),
                                format_memory_snapshot_report(&snapshot),
                                proposal_queue,
                                decisions.accepted,
                                decisions.proposed,
                                decisions.rejected,
                                decisions.blocked,
                                flushes.format(),
                                calibration_passed,
                                calibration.len(),
                                eval_suite.passed,
                                eval_suite.total
                            )
                        }
                    } else if memory_action == "snapshot" {
                        format_memory_snapshot_report(&mem.memory_snapshot_report())
                    } else if memory_action == "eval" {
                        crate::memory::run_memory_eval_suite().format()
                    } else if memory_action == "records" {
                        format_memory_records(&mem.memory_records(), memory_arg)
                    } else if memory_action == "migrate" {
                        format_memory_migration_command(&mem, memory_arg)
                    } else if memory_action == "repair-proposals" {
                        let limit = memory_arg.parse::<usize>().ok().unwrap_or(20).clamp(1, 200);
                        let created = mem.upsert_projection_repair_proposals(limit);
                        format!(
                            "Memory repair proposal scan complete\n- projection drift proposals: {}\n- review: /memory-proposals list --source repair",
                            created
                        )
                    } else if memory_action == "conflicts" {
                        let conflicts = mem.memory_conflicts(20);
                        if conflicts.is_empty() {
                            "Memory conflicts: none".to_string()
                        } else {
                            format!("Memory Conflicts\n{}", conflicts.join("\n"))
                        }
                    } else if memory_action == "review" {
                        let review = mem.memory_review_report(8);
                        let decisions = mem.memory_decision_counts();
                        let flushes = mem.memory_flush_summary();
                        let conflicts = mem.memory_conflicts(8);
                        format!(
                            "Memory Review\n\n{}\n\nDecisions: {} accepted · {} proposed · {} rejected · {} blocked\n{}\n\nConflicts:\n{}",
                            review.format(),
                            decisions.accepted,
                            decisions.proposed,
                            decisions.rejected,
                            decisions.blocked,
                            flushes.format(),
                            if conflicts.is_empty() {
                                "none".to_string()
                            } else {
                                conflicts.join("\n")
                            }
                        )
                    } else if memory_action == "search" {
                        let search_query = if memory_arg.is_empty() {
                            latest_user_message
                        } else {
                            memory_arg
                        };
                        match mem.preview_retrieval_context(
                            search_query,
                            8,
                            crate::engine::intent_router::RetrievalPolicy::Memory,
                        ) {
                            Some(ctx) => {
                                if let Err(error) =
                                    crate::tools::memory_tool::record_last_memory_retrieval_trace(
                                        &ctx,
                                    )
                                {
                                    warn!("failed to write last memory retrieval trace: {}", error);
                                }
                                format_memory_retrieval_context(&ctx)
                            }
                            None => format!("No memory retrieval hits for '{}'.", search_query),
                        }
                    } else if matches!(memory_action, "explain" | "why") {
                        if let Some((search_query, selector, last_turn)) =
                            parse_memory_why_args(memory_arg, latest_user_message)
                        {
                            match mem.preview_retrieval_context(
                                search_query,
                                20,
                                crate::engine::intent_router::RetrievalPolicy::Memory,
                            ) {
                                Some(ctx) => {
                                    if let Err(error) =
                                        crate::tools::memory_tool::record_last_memory_retrieval_trace(
                                            &ctx,
                                        )
                                    {
                                        warn!("failed to write last memory retrieval trace: {}", error);
                                    }
                                    let prefix = if last_turn {
                                        format!("Last-turn query: {}\n\n", search_query)
                                    } else {
                                        String::new()
                                    };
                                    if let Some(selector) = selector {
                                        format!(
                                            "{}{}",
                                            prefix,
                                            explain_memory_retrieval_item(&ctx, selector)
                                        )
                                    } else {
                                        format!(
                                            "{}{}",
                                            prefix,
                                            format_memory_retrieval_context(&ctx)
                                        )
                                    }
                                }
                                None => format!(
                                    "No memory retrieval context available for '{}'.",
                                    search_query
                                ),
                            }
                        } else {
                            "Usage: /memory why <query> [--last-turn] [--item <retrieval-id-or-source>]"
                                .to_string()
                        }
                    } else {
                        let summary = mem.memory_summary();
                        let project = mem.load_tier(crate::memory::manager::MemoryTier::Project);
                        let user = mem.load_tier(crate::memory::manager::MemoryTier::User);
                        let preview_query = if query.is_empty() {
                            latest_user_message
                        } else {
                            query
                        };
                        let relevant = mem.preview_relevant_memories(preview_query, 5);

                        let mut lines = vec![
                            "# Memory".to_string(),
                            "".to_string(),
                            summary.format(),
                            "".to_string(),
                        ];

                        if !query.is_empty() {
                            let hits = mem.search(query);
                            lines.push("## Search".to_string());
                            if hits.is_empty() {
                                lines.push(format!("No memories matching '{}'.", query));
                            } else {
                                for hit in hits {
                                    let hit = hit.lines().take(4).collect::<Vec<_>>().join(" ");
                                    lines.push(format!(
                                        "- {}",
                                        hit.chars().take(220).collect::<String>()
                                    ));
                                }
                            }
                            lines.push("".to_string());
                        }

                        if !relevant.is_empty() {
                            lines.push("## Relevant Preview".to_string());
                            for item in relevant {
                                let snippet = item
                                    .snippet
                                    .lines()
                                    .map(str::trim)
                                    .filter(|line| !line.is_empty())
                                    .take(2)
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                lines.push(format!(
                                    "- {} (score {}): {}",
                                    item.source,
                                    item.score,
                                    snippet.chars().take(220).collect::<String>()
                                ));
                            }
                            lines.push("".to_string());
                        }

                        if !project.trim().is_empty() {
                            lines.push("## Project Memory Index".to_string());
                            lines.push(project.chars().take(1800).collect());
                            lines.push("".to_string());
                        }
                        if !user.trim().is_empty() {
                            lines.push("## User Preferences".to_string());
                            lines.push(user.chars().take(1000).collect());
                        }

                        if lines.len() <= 4 {
                            "No memory saved yet. Use /save <text> to save.".to_string()
                        } else {
                            lines.join("\n")
                        }
                    }
                } else {
                    let mut mem = crate::memory::MemoryManager::new();
                    mem.freeze_snapshot();
                    if maintain {
                        let report = mem.maintain_memory();
                        report.format()
                    } else if doctor {
                        let summary = mem.memory_summary();
                        let decisions = mem.memory_decision_counts();
                        let flushes = mem.memory_flush_summary();
                        let calibration = crate::memory::run_memory_calibration_samples();
                        let eval_suite = crate::memory::run_memory_eval_suite();
                        let calibration_passed =
                            calibration.iter().filter(|result| result.passed).count();
                        if doctor_json {
                            let snapshot = mem.memory_snapshot_report();
                            let proposal_queue = memory_proposal_queue_json();
                            serde_json::json!({
                                "summary": {
                                    "project_memory_chars": summary.project_memory_chars,
                                    "project_memory_files": summary.project_memory_files,
                                    "project_memory_file_chars": summary.project_memory_file_chars,
                                    "user_memory_chars": summary.user_memory_chars,
                                    "session_memory_items": summary.session_memory_items,
                                    "has_frozen_snapshot": summary.has_frozen_snapshot,
                                },
                                "snapshot": snapshot,
                                "proposal_queue": proposal_queue,
                                "decisions": decisions,
                                "flushes": flushes,
                                "quality_gates": {
                                    "accept_threshold": 0.65,
                                    "propose_threshold": 0.45,
                                    "explicit_override_threshold": 0.60,
                                    "hard_stops": ["unsafe_content", "secret_like_content", "duplicate_memory"],
                                },
                                "calibration": {
                                    "passed": calibration_passed,
                                    "total": calibration.len(),
                                    "results": calibration,
                                },
                                "eval_suite": eval_suite,
                                "conflicts": mem.memory_conflicts(8),
                            })
                            .to_string()
                        } else {
                            let snapshot = mem.memory_snapshot_report();
                            let proposal_queue = format_memory_proposal_queue();
                            format!(
                                "# Memory Doctor\n\n{}\n\n{}\n\n{}\n\nDecisions:\n  Accepted: {}\n  Proposed: {}\n  Rejected: {}\n  Blocked: {}\n\n{}\n\nQuality gates:\n  accept>=0.65 · propose>=0.45 · explicit>=0.60 with safety/duplicate hard stops\n\nCalibration: {}/{} passed\nMemory evals: {}/{} passed",
                                summary.format(),
                                format_memory_snapshot_report(&snapshot),
                                proposal_queue,
                                decisions.accepted,
                                decisions.proposed,
                                decisions.rejected,
                                decisions.blocked,
                                flushes.format(),
                                calibration_passed,
                                calibration.len(),
                                eval_suite.passed,
                                eval_suite.total
                            )
                        }
                    } else if memory_action == "snapshot" {
                        format_memory_snapshot_report(&mem.memory_snapshot_report())
                    } else if memory_action == "eval" {
                        crate::memory::run_memory_eval_suite().format()
                    } else if memory_action == "records" {
                        format_memory_records(&mem.memory_records(), memory_arg)
                    } else if memory_action == "migrate" {
                        format_memory_migration_command(&mem, memory_arg)
                    } else if memory_action == "repair-proposals" {
                        let limit = memory_arg.parse::<usize>().ok().unwrap_or(20).clamp(1, 200);
                        let created = mem.upsert_projection_repair_proposals(limit);
                        format!(
                            "Memory repair proposal scan complete\n- projection drift proposals: {}\n- review: /memory-proposals list --source repair",
                            created
                        )
                    } else {
                        let summary = mem.memory_summary();
                        let project = mem.load_tier(crate::memory::manager::MemoryTier::Project);
                        if project.trim().is_empty() {
                            "No memory saved yet. Use /save <text> to save.".to_string()
                        } else {
                            format!("# Memory\n\n{}\n\n{}", summary.format(), project)
                        }
                    }
                }
            }
            "/save" => {
                if args.is_empty() {
                    "Usage: /save <text> | /save --topic <name> <text> | /save --user <text>"
                        .to_string()
                } else {
                    let (save_target, save_topic, save_content) = parse_memory_save_args(args);
                    if save_content.trim().is_empty() {
                        "Usage: /save <text> | /save --topic <name> <text> | /save --user <text>"
                            .to_string()
                    } else {
                        let memory_manager = if let Some(ref engine) = self.streaming_engine {
                            engine.memory_manager_or_init()
                        } else {
                            None
                        };

                        if let Some(memory_manager) = memory_manager {
                            let mem = memory_manager.lock().await;
                            let outcome = match save_target {
                                MemorySaveTarget::User => {
                                    mem.add_learning_async(save_content, "preference").await
                                }
                                MemorySaveTarget::Topic => {
                                    mem.add_topic_learning_async(
                                        save_content,
                                        "note",
                                        save_topic.unwrap_or("notes"),
                                    )
                                    .await
                                }
                                MemorySaveTarget::Auto => {
                                    mem.add_auto_learning_async(save_content, "note").await
                                }
                            };
                            format_memory_write_outcome(save_content, &outcome)
                        } else {
                            let mem = crate::memory::MemoryManager::new();
                            let outcome = match save_target {
                                MemorySaveTarget::User => {
                                    mem.add_learning_async(save_content, "preference").await
                                }
                                MemorySaveTarget::Topic => {
                                    mem.add_topic_learning_async(
                                        save_content,
                                        "note",
                                        save_topic.unwrap_or("notes"),
                                    )
                                    .await
                                }
                                MemorySaveTarget::Auto => {
                                    mem.add_auto_learning_async(save_content, "note").await
                                }
                            };
                            format_memory_write_outcome(save_content, &outcome)
                        }
                    }
                }
            }
            "/model" => {
                let args = args.trim();
                if let Some(model) = args
                    .strip_prefix("set ")
                    .or_else(|| args.strip_prefix("switch "))
                    .map(str::trim)
                    .filter(|m| !m.is_empty())
                {
                    if let Some(engine) = &self.streaming_engine {
                        engine.set_model(model.to_string());
                    }
                    if let Ok(mut config) = crate::services::config::AppConfig::load() {
                        config.api.model = model.to_string();
                        let _ = config.save();
                    }
                    self.model_notice = Some(format!("Model switched to {}", model));
                    format!("Model switched to {}. Next request will use it.", model)
                } else if args == "list" {
                    let lines = self
                        .model_choices()
                        .into_iter()
                        .map(|choice| {
                            format!(
                                "{} {} ({})",
                                if choice.active { "*" } else { "-" },
                                choice.model,
                                choice.note
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("Models for {}:\n{}", self.current_provider_label(), lines)
                } else if self.streaming_engine.is_some() {
                    let model = self.current_model_label();
                    let provider = self.current_provider_label();
                    let base = self.current_provider_base_url();
                    format!(
                        "Model: {} (via {})\nBase URL: {}\n\nUse Ctrl+M for the model picker, /model list, or /model set <name>.",
                        model, provider, base
                    )
                } else {
                    "Model: unavailable (no engine connected)".to_string()
                }
            }
            "/provider" => {
                let args = args.trim();
                if let Some(provider) = args
                    .strip_prefix("set ")
                    .or_else(|| args.strip_prefix("switch "))
                    .map(str::trim)
                    .filter(|p| !p.is_empty())
                {
                    self.switch_provider_by_name(provider)
                } else if args == "list" {
                    let lines = self
                        .provider_choices()
                        .into_iter()
                        .map(|choice| {
                            format!(
                                "{} {:<10} {:<12} {:<20} {}{}",
                                if choice.active { "*" } else { "-" },
                                choice.name,
                                choice.provider_type,
                                choice.model,
                                choice.note,
                                if choice.base_url.is_empty() {
                                    String::new()
                                } else {
                                    format!(" - {}", choice.base_url)
                                }
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("Providers:\n{}", lines)
                } else if self.streaming_engine.is_some() {
                    format!(
                        "Provider: {}\nModel: {}\nBase URL: {}\n\nUse Ctrl+L for the provider picker, /provider list, or /provider switch <name>.",
                        self.current_provider_label(),
                        self.current_model_label(),
                        self.current_provider_base_url()
                    )
                } else {
                    "Provider: unavailable (no engine connected)".to_string()
                }
            }
            "/status" => slash::handle_status(self).await,
            "/panel" | "/panels" | "/runtime" => slash::handle_panel(self, args).await,
            "/tool-output" | "/tool" => {
                let args = args.trim();
                if args.is_empty() || args == "latest" {
                    if self.open_tool_viewer() {
                        String::new()
                    } else {
                        "No tool output to view yet.".to_string()
                    }
                } else if args == "list" {
                    let lines = self.tool_output_index_lines();
                    if lines.is_empty() {
                        "No tool output to view yet.".to_string()
                    } else {
                        format!("Tool outputs:\n{}", lines.join("\n"))
                    }
                } else if self.open_tool_viewer_for(args) {
                    String::new()
                } else {
                    format!("Tool output '{}' not found. Use /tool-output list.", args)
                }
            }
            "/statusbar" => {
                let args = args.trim();
                if args.is_empty() {
                    format!(
                        "Status bar density: {}\nOptions: compact, normal, debug\nShortcut: Ctrl+Shift+S cycles density.",
                        self.status_bar_density.name()
                    )
                } else if let Some(density) = StatusBarDensity::parse(args) {
                    self.set_status_bar_density(density);
                    format!("Status bar density: {}", density.name())
                } else {
                    "Usage: /statusbar [compact|normal|debug]".to_string()
                }
            }
            "/resume" => slash::handle_resume(self, args).await,
            "/rewind" => slash::handle_rewind(self, args).await,
            // Phase 10 Batch 1: Session & Control Commands
            "/session" => slash::handle_session_cmd(self, args).await,
            "/undo" => slash::handle_undo(self, args),
            "/revert" => slash::handle_revert_turn(self).await,
            "/changes" => slash::handle_changes(self).await,
            "/validate" => {
                if let Some(sid) = self.session_manager.current_session_id() {
                    let sid = sid.to_string();
                    let mgr = crate::engine::checkpoint::get_checkpoint_manager(&sid).await;
                    let cp = mgr.lock().await;
                    let changes = cp.list_file_changes();
                    let rounds = cp.list_file_change_rounds();
                    let mut lines = vec![
                        "Validation Summary".to_string(),
                        "==================".to_string(),
                        String::new(),
                        format!("File changes: {}", changes.len()),
                        format!("Tool rounds: {}", rounds.len()),
                        String::new(),
                    ];
                    if changes.is_empty() {
                        lines.push("No file changes to validate.".to_string());
                    } else {
                        lines.push("Changed files:".to_string());
                        for c in changes.iter().rev().take(10) {
                            lines.push(format!(
                                "  {} ({}, {}B)",
                                c.path, c.tool_name, c.bytes_written
                            ));
                        }
                        lines.push(String::new());
                        lines.push(
                            "Run 'cargo test' or 'npm test' to execute the test suite.".to_string(),
                        );
                        lines.push("Use /changes for a turn-by-turn breakdown.".to_string());
                    }
                    lines.join("\n")
                } else {
                    "No active session.".to_string()
                }
            }
            "/diagnostic" | "/diagnostics" => slash::handle_diagnostic(self).await,
            "/redo" => slash::handle_redo(self, args),
            "/retry" => slash::handle_retry(self, args).await,
            "/stop" => slash::handle_stop(self, args),
            "/reload" => slash::handle_reload(self, args).await,
            "/share" => slash::handle_share(self, args),
            "/cost" | "/token" => slash::handle_token(self).await,
            "/diff" => slash::handle_diff(self, args).await,
            "/quit" | "/exit" | "/q" => {
                if let Some(ref engine) = self.streaming_engine {
                    engine
                        .flush_memory_for_current_history(crate::memory::MemoryFlushReason::Exit)
                        .await;
                }
                "Use Ctrl+C to exit".to_string()
            }
            "/sessions" => slash::handle_sessions(self),
            "/new" => slash::handle_new(self).await,
            "/stats" => slash::handle_stats(self),
            "/checkpoints" => slash::handle_checkpoints(self).await,
            "/restore" | "/r" => slash::handle_restore(self, args).await,
            "/batch" => slash::handle_batch(self, args).await,
            "/settings" => {
                let config = crate::services::config::AppConfig::load().unwrap_or_default();
                self.settings_state = Some(crate::tui::components::settings::SettingsState::new(
                    config,
                    self.keybindings.clone(),
                ));
                self.mode = AppMode::Settings;
                "Entering settings mode...".to_string()
            }
            "/tools" => {
                let registry = crate::tools::ToolRegistry::default_registry();
                let context = self.build_tool_context().await;
                let mut available = Vec::new();
                let mut unavailable = Vec::new();
                for tool in registry.iter_tools() {
                    if tool.is_available(&context) {
                        available.push(tool.name().to_string());
                    } else {
                        unavailable.push(format!(
                            "{} ({})",
                            tool.name(),
                            tool.unavailable_reason(&context)
                                .unwrap_or_else(|| "unavailable".to_string())
                        ));
                    }
                }
                available.sort();
                unavailable.sort();
                let unavailable_line = if unavailable.is_empty() {
                    String::new()
                } else {
                    format!(
                        "\n\nUnavailable in this session ({}):\n{}",
                        unavailable.len(),
                        unavailable.join(", ")
                    )
                };
                format!(
                    "Available tools ({}):\n{}{}",
                    available.len(),
                    available.join(", "),
                    unavailable_line
                )
            }
            "/tasks" => slash::handle_tasks(self).await,
            "/agents" => slash::handle_agents(self, args).await,
            "/doctor" => slash::handle_doctor(self, args).await,
            "/audit" => slash::handle_audit(self, args).await,
            "/permissions" | "/perm" => slash::handle_permissions(self, args),
            "/commit" => slash::handle_commit(self).await,
            "/commit-push-pr" => slash::handle_commit_push_pr(self, args).await,
            "/review-pr" => slash::handle_review_pr(self, args).await,
            "/review" => slash::handle_review(self).await,
            "/security-review" => slash::handle_security_review(self).await,
            "/explain" => slash::handle_explain(self, args).await,
            "/fix" => slash::handle_fix(self).await,
            "/simplify" => slash::handle_simplify(self, args).await,
            "/karpathy" => slash::handle_karpathy(self, args).await,
            "/verify" => slash::handle_verify(self).await,
            "/debug" => slash::handle_debug(self).await,
            "/stuck" => slash::handle_stuck(self).await,
            "/remember" => slash::handle_remember(self, args).await,
            "/keybindings" => slash::handle_keybindings(self, args),
            "/mcp" => slash::handle_mcp(self, args).await,
            "/voice" => slash::handle_voice(),
            "/telemetry" => slash::handle_telemetry(),
            "/lsp" => slash::handle_lsp(self, args),
            "/npm" => slash::handle_npm(self, args).await,
            // Phase 10 Batch 2: hooks, profiling, prompt, migrate, focus, pause, install, skeleton, branch, color
            "/hooks" => slash::handle_hooks(self),
            "/profiling" => slash::handle_profiling(self),
            "/prompt" => slash::handle_prompt(self, args).await,
            "/migrate" => slash::handle_migrate(self, args).await,
            "/focus" => slash::handle_focus(self, args),
            "/pause" => slash::handle_pause(self, args),
            "/install" => slash::handle_install(self, args).await,
            "/skeleton" => slash::handle_skeleton(self, args),
            "/branch" => slash::handle_branch(self, args).await,
            "/color" => slash::handle_color(self, args),
            // Phase 10 Batch 3: webhook, wizard, workspace, slack, stealth, shadow, reject, subscribe, slots, ticker
            "/webhook" => slash::handle_webhook(self, args).await,
            "/wizard" => slash::handle_wizard(self),
            "/workspace" => slash::handle_workspace(self, args),
            "/slack" => slash::handle_slack(self, args).await,
            "/stealth" => slash::handle_stealth(self, args),
            "/shadow" => slash::handle_shadow(self, args),
            "/reject" => slash::handle_reject(self, args),
            "/subscribe" => slash::handle_subscribe(self, args),
            "/slots" => slash::handle_slots(self, args),
            "/ticker" => slash::handle_ticker(self, args),
            // Phase 10 Batch 4: config, copy, desktop, chrome, effort, preamble, untrap, verbose, write
            "/config" => slash::handle_config(self, args),
            "/copy" => slash::handle_copy(self, args).await,
            "/desktop" => slash::handle_desktop(self, args),
            "/chrome" => slash::handle_chrome(self, args),
            "/effort" => slash::handle_effort(self, args),
            "/preamble" => slash::handle_preamble(self, args),
            "/untrap" => slash::handle_untrap(self, args),
            "/verbose" => slash::handle_verbose(self, args),
            "/write" => slash::handle_write(self, args).await,
            "/vim" => slash::handle_vim(self),
            "/onboarding" | "/onboard" => slash::handle_onboarding(self),
            "/skip" => slash::handle_skip(self),
            // Phase 9 Task 3: New high-value commands
            "/btw" => slash::handle_btw(self, args).await,
            "/context" => slash::handle_context(self).await,
            "/git" => slash::handle_git(self, args).await,
            "/history" => slash::handle_history(self, args),
            "/mode" => slash::handle_mode(self, args),
            "/package" => slash::handle_package(self, args).await,
            // Phase 9 Task 1: Advanced Agent Types
            "/teammate" => slash::handle_teammate(self, args).await,
            "/critic" => slash::handle_critic(self, args).await,
            "/assistant" => slash::handle_assistant(self, args).await,
            "/remote" => slash::handle_remote(self, args).await,
            "/dream" => slash::handle_dream(self, args).await,
            "/custom" => slash::handle_custom(self, args).await,
            "/orchestrate" => slash::handle_orchestrate(self, args).await,
            // Phase 10 Extended: More commands
            "/rollback" => slash::handle_rollback(self, args).await,
            "/project" => slash::handle_project(self, args),
            "/backend" => slash::handle_backend(self, args),
            "/sandbox" => slash::handle_sandbox(self, args),
            "/env" => slash::handle_env(self, args),
            "/cache" => slash::handle_cache(self, args).await,
            "/benchmark" => slash::handle_benchmark(self, args).await,
            "/test" => slash::handle_test(self, args).await,
            "/trace" => slash::handle_trace(self, args),
            "/eval" => slash::handle_eval(self, args),
            "/resource" => slash::handle_resource(self),
            // Phase 10 Extended 2: More commands
            "/init" => slash::handle_init(self, args),
            "/login" => slash::handle_login(self, args),
            "/logout" => slash::handle_logout(self, args),
            "/key" => slash::handle_key(self, args),
            "/health" => slash::handle_health(self),
            "/ping" => slash::handle_ping(self),
            "/uptime" => slash::handle_uptime(self),
            "/version" => slash::handle_version(self),
            "/about" => slash::handle_about(self),
            // Phase 10 Extended 3: Session management and utility commands
            "/reset" => slash::handle_reset(self, args),
            "/export" => slash::handle_export_data(self, args).await,
            "/import" => slash::handle_import(self, args).await,
            "/save-session" => slash::handle_save_session(self),
            "/load-session" => slash::handle_load_session(self, args).await,
            "/merge" => slash::handle_merge(self, args).await,
            "/cleanup" => slash::handle_cleanup(self, args),
            "/compact" => slash::handle_compact(self).await,
            "/snippet" => slash::handle_snippet(self, args),
            "/bookmark" => slash::handle_bookmark(self, args).await,
            "/tag" => slash::handle_tag(self, args),
            "/search" => slash::handle_search(self, args),
            "/filter" => slash::handle_filter(self, args),
            // Phase 10 Final: Complete commands
            "/profile" => slash::handle_profile(self, args),
            "/theme" => slash::handle_theme(self, args),
            "/shortcuts" => slash::handle_shortcuts(self),
            "/quick" => slash::handle_quick(self),
            "/active-task" | "/progress" => slash::handle_active_task(self),
            "/goal" => slash::handle_goal(self, args),
            "/learn" => slash::handle_learn(self, args),
            "/experience" => slash::handle_experience(self, args),
            "/memory-proposals" | "/memory-proposal" => slash::handle_memory_proposals(self, args),
            "/evolution" => slash::handle_evolution(self, args),
            "/improvements" => slash::handle_improvements(self, args),
            "/skill-proposals" => slash::handle_skill_proposals(self, args),
            "/recover" => slash::handle_recover(self, args),
            "/feedback" => slash::handle_feedback(self, args),
            _ => {
                if let Some(invocation) = self.skill_runtime.invocation(&cmd, args) {
                    let skill_version = self
                        .skill_runtime
                        .get(&cmd)
                        .map(|skill| skill.meta.version.clone())
                        .unwrap_or_else(|| "unknown".to_string());
                    self.apply_skill_invocation_policy(&invocation);
                    let mut notice = format!("Skill /{} applied", invocation.name);
                    if !invocation.allowed_tools.is_empty() {
                        notice.push_str(&format!(
                            " · allowed tools: {}",
                            invocation.allowed_tools.join(", ")
                        ));
                    }
                    if !invocation.disallowed_tools.is_empty() {
                        notice.push_str(&format!(
                            " · denied tools: {}",
                            invocation.disallowed_tools.join(", ")
                        ));
                    }
                    if let Some(model) = &invocation.model {
                        notice.push_str(&format!(" · preferred model: {}", model));
                    }
                    if let Some(effort) = &invocation.effort {
                        notice.push_str(&format!(" · effort: {}", effort));
                    }
                    if let Some(context) = &invocation.context {
                        notice.push_str(&format!(" · context: {}", context));
                    }
                    self.add_system_message(notice);
                    self.record_skill_invocation_usage(&invocation.name, &skill_version);
                    self.send_message(invocation.prompt).await;
                    String::new()
                } else {
                    format!(
                        "Unknown command: {}. Type /help for available commands.",
                        cmd
                    )
                }
            }
        };

        self.add_system_message(response);
    }

    fn record_skill_invocation_usage(&mut self, skill_name: &str, skill_version: &str) {
        let event = crate::engine::skill_evolution::SkillUsageEvent {
            skill_name: skill_name.to_string(),
            skill_version: skill_version.to_string(),
            provisional: true,
            success: false,
            acceptance_passed: None,
            tests_passed: None,
            user_satisfaction: None,
            duration_ms: None,
            tool_calls: 0,
            risk_penalty: 0.05,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let store = crate::engine::skill_evolution::SkillProposalStore::default();
        if let Err(e) = store.record_usage(&event) {
            warn!("Failed to record skill usage event: {}", e);
        }
        if let Ok(payload) = serde_json::to_value(&event) {
            let _ = self.session_manager.add_learning_event(
                "skill_usage",
                "skill_runtime",
                &format!("Skill /{} invoked", skill_name),
                0.75,
                &payload,
            );
        }
        self.pending_skill_invocations.push(PendingSkillInvocation {
            name: skill_name.to_string(),
            version: skill_version.to_string(),
            started_at: std::time::Instant::now(),
        });
    }

    pub(super) fn record_pending_skill_outcomes(&mut self, assistant_response: &str) {
        if self.pending_skill_invocations.is_empty() {
            return;
        }
        let failed_tool = self
            .tool_runs_snapshot
            .iter()
            .any(|run| matches!(run.status, ToolRunStatus::Failed | ToolRunStatus::TimedOut));
        let stream_error = assistant_response.contains("[Error:");
        let has_response = !assistant_response.trim().is_empty();
        let trace = self
            .streaming_engine
            .as_ref()
            .and_then(|engine| engine.trace_store().latest())
            .or_else(|| self.session_manager.latest_trace().ok().flatten());
        let attribution =
            skill_outcome_attribution(trace.as_ref(), has_response, stream_error, failed_tool);
        let tool_calls = self.tool_runs_snapshot.len();
        let store = crate::engine::skill_evolution::SkillProposalStore::default();
        for pending in self.pending_skill_invocations.drain(..) {
            let event = crate::engine::skill_evolution::SkillUsageEvent {
                skill_name: pending.name.clone(),
                skill_version: pending.version.clone(),
                provisional: false,
                success: attribution.success,
                acceptance_passed: attribution.acceptance_passed,
                tests_passed: attribution.tests_passed,
                user_satisfaction: attribution.user_satisfaction,
                duration_ms: Some(
                    pending
                        .started_at
                        .elapsed()
                        .as_millis()
                        .min(u128::from(u64::MAX)) as u64,
                ),
                tool_calls,
                risk_penalty: attribution.risk_penalty,
                created_at: chrono::Utc::now().to_rfc3339(),
            };
            if let Err(e) = store.record_usage(&event) {
                warn!("Failed to record skill outcome event: {}", e);
            }
            if let Ok(payload) = serde_json::to_value(&event) {
                let _ = self.session_manager.add_learning_event(
                    "skill_usage",
                    "skill_runtime",
                    &format!(
                        "Skill /{} outcome inferred from {}: {}",
                        pending.name,
                        attribution.source,
                        if attribution.success {
                            "success"
                        } else {
                            "fail"
                        }
                    ),
                    f64::from(attribution.confidence),
                    &payload,
                );
            }
        }
    }

    /// 恢复会话
    pub(crate) async fn restore_session(&mut self, session_id: &str) -> String {
        if let Some(ref engine) = self.streaming_engine {
            engine
                .flush_memory_for_current_history(crate::memory::MemoryFlushReason::ResumeSwitch)
                .await;
        }
        match self.session_manager.switch_to_session(session_id) {
            Ok(messages) => {
                // 清空当前消息
                self.messages.clear();
                self.clear_tool_transcript();

                // 加载会话消息到 UI
                for msg in messages {
                    self.messages.push(msg);
                }

                // 同步恢复引擎的对话历史
                if let Some(ref engine) = self.streaming_engine {
                    match self.session_manager.load_api_messages(session_id) {
                        Ok(api_messages) => {
                            engine.set_history(api_messages).await;
                            engine.set_session_id(session_id.to_string());
                        }
                        Err(e) => {
                            warn!("Failed to restore engine history: {}", e);
                        }
                    }
                }

                let mut lines = vec![format!(
                    "Restored session {} ({} messages). Previous conversation loaded.",
                    &session_id[..8.min(session_id.len())],
                    self.messages.len()
                )];
                if let Ok(preview) = self.session_manager.recent_preview_lines(session_id, 4) {
                    if !preview.is_empty() {
                        lines.push("Recent context:".to_string());
                        lines.extend(preview);
                    }
                }
                lines.join("\n")
            }
            Err(e) => format!("Failed to restore session: {}", e),
        }
    }
}
