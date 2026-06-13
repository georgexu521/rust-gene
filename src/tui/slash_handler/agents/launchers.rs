use super::*;

/// /teammate - 启动协作队友 Agent
pub async fn handle_teammate(app: &mut TuiApp, args: &str) -> String {
    match app.bundled_skills.get("teammate") {
        Some(skill) => {
            let started = std::time::Instant::now();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_teammate",
                    true,
                    started.elapsed().as_millis() as u64,
                    None,
                );
            }

            let domain = if args.is_empty() {
                "general software development tasks".to_string()
            } else {
                args.to_string()
            };

            let prompt = format!(
                "{}\n\n## Your Focus\n\nYou are collaborating on: {}\n\nBegin by introducing yourself and asking what specific task you'd like to work on together.",
                skill.content, domain
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'teammate' not found.".to_string(),
    }
}
/// /critic - 启动批评型 Agent 审查代码
pub async fn handle_critic(app: &mut TuiApp, args: &str) -> String {
    match app.bundled_skills.get("critic") {
        Some(skill) => {
            let started = std::time::Instant::now();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_critic",
                    true,
                    started.elapsed().as_millis() as u64,
                    None,
                );
            }

            let tool = crate::tools::GitTool;
            let params = serde_json::json!({ "action": "diff" });
            let result = tool.execute(params, app.build_tool_context().await).await;
            let diff = if result.success {
                result.content
            } else {
                result
                    .error
                    .unwrap_or_else(|| "No changes to review.".to_string())
            };

            let scope = if args.is_empty() {
                "all code in the diff".to_string()
            } else {
                args.to_string()
            };

            let prompt = format!(
                "{}\n\n## Review Scope\n\nPlease critically review: {}\n\n## Changes\n\n```diff\n{}\n```",
                skill.content, scope, diff
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'critic' not found.".to_string(),
    }
}
/// /assistant - 启动领域专家 Agent
pub async fn handle_assistant(app: &mut TuiApp, args: &str) -> String {
    match app.bundled_skills.get("assistant") {
        Some(skill) => {
            let started = std::time::Instant::now();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_assistant",
                    true,
                    started.elapsed().as_millis() as u64,
                    None,
                );
            }

            let parts: Vec<&str> = args.splitn(2, ':').collect();
            let domain = parts.first().unwrap_or(&"general");
            let task = parts.get(1).map(|s| s.trim()).unwrap_or("");

            let domain_intro = match *domain {
                "code_review" => {
                    "You are an expert code analyst. Provide deep insights into code structure, patterns, and potential issues."
                }
                "security" => {
                    "You are a security expert. Focus on vulnerabilities, injection risks, authentication issues, and secure coding practices."
                }
                "data" => {
                    "You are a data engineering expert. Focus on data pipelines, transformations, storage, and processing efficiency."
                }
                "infrastructure" => {
                    "You are an infrastructure expert. Focus on DevOps, deployment, CI/CD, and infrastructure as code."
                }
                "testing" => {
                    "You are a testing expert. Focus on test strategy, coverage, edge cases, and quality assurance."
                }
                _ => "You are a helpful specialized assistant.",
            };

            let prompt = if task.is_empty() {
                format!(
                    "{}\n\n## Domain\n\n{}\n\nWhat would you like expert assistance with?",
                    skill.content, domain_intro
                )
            } else {
                format!(
                    "{}\n\n## Domain\n\n{}\n\n## Task\n\n{}",
                    skill.content, domain_intro, task
                )
            };
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'assistant' not found.".to_string(),
    }
}
/// /remote - 启动远程专家 Agent
pub async fn handle_remote(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if matches!(args, "status" | "bridge" | "runtime" | "panel") {
        return crate::tui::runtime_panels::render_runtime_panel(
            app,
            crate::tui::runtime_panels::RuntimePanelKind::Bridge,
        )
        .await;
    }

    match app.bundled_skills.get("remote") {
        Some(skill) => {
            let started = std::time::Instant::now();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_remote",
                    true,
                    started.elapsed().as_millis() as u64,
                    None,
                );
            }

            let bridge = crate::bridge::runtime_snapshot();
            let bridge_url = bridge
                .bridge_url
                .as_deref()
                .unwrap_or("not configured")
                .to_string();
            let bridge_source = bridge.bridge_url_source.as_deref().unwrap_or("none");

            let prompt = format!(
                "{}\n\n## Bridge Configuration\n\nBridge URL: {}\nBridge source: {}\nAuth token configured: {}\nTenant: {}\n\nTo inspect bridge state, run `/remote status`.\n\n## Your Task\n\n{}",
                skill.content,
                bridge_url,
                bridge_source,
                bridge.auth_token_configured,
                bridge.tenant_id.as_deref().unwrap_or("none"),
                if args.is_empty() {
                    "What remote task would you like to execute?"
                } else {
                    args
                }
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'remote' not found.".to_string(),
    }
}
/// /dream - 启动梦境任务 Agent（后台探索性分析）
pub async fn handle_dream(app: &mut TuiApp, args: &str) -> String {
    match app.bundled_skills.get("dream") {
        Some(skill) => {
            let started = std::time::Instant::now();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_dream",
                    true,
                    started.elapsed().as_millis() as u64,
                    None,
                );
            }

            let prompt = format!(
                "{}\n\n## Dream Task\n\n{}",
                skill.content,
                if args.is_empty() {
                    "What would you like me to explore in the background?"
                } else {
                    args
                }
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'dream' not found. The dream skill is not yet loaded.".to_string(),
    }
}
/// /custom - Create a custom agent
pub async fn handle_custom(app: &mut TuiApp, args: &str) -> String {
    match app.bundled_skills.get("custom") {
        Some(skill) => {
            let started = std::time::Instant::now();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_custom",
                    true,
                    started.elapsed().as_millis() as u64,
                    None,
                );
            }

            let prompt = format!(
                "{}\n\n## Custom Agent Request\n\n{}",
                skill.content,
                if args.is_empty() {
                    "Describe the custom agent you want to create:"
                } else {
                    args
                }
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'custom' not found.".to_string(),
    }
}
/// /orchestrate - Multi-agent coordination
pub async fn handle_orchestrate(app: &mut TuiApp, args: &str) -> String {
    match app.bundled_skills.get("orchestrate") {
        Some(skill) => {
            let started = std::time::Instant::now();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_orchestrate",
                    true,
                    started.elapsed().as_millis() as u64,
                    None,
                );
            }

            let prompt = format!(
                "{}\n\n## Orchestration Task\n\n{}",
                skill.content,
                if args.is_empty() {
                    "What complex task would you like me to coordinate?"
                } else {
                    args
                }
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'orchestrate' not found.".to_string(),
    }
}
/// /token - 显示 token 使用情况
pub async fn handle_token(app: &TuiApp) -> String {
    if let Some(ref engine) = app.streaming_engine {
        let tracker = engine.cost_tracker().lock().await;
        let report = tracker.generate_report();
        format!("Token Usage:\n{}", report)
    } else {
        "Engine not initialized.".to_string()
    }
}
