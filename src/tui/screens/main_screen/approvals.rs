use super::*;
use ratatui::style::Color;

/// 渲染权限审批弹窗
pub fn render_permission_approval(
    f: &mut Frame,
    req: &crate::engine::conversation_loop::ToolApprovalRequest,
    area: Rect,
    theme: &crate::tui::theme::Theme,
) {
    let popup_area = centered_rect(76, 64, area);
    let tokens = &theme.tokens;
    let review = req.human_review_request();
    let permission_review = req.permission_review();
    let goal_drift_approval =
        review.kind == crate::engine::human_review::HumanReviewKind::GoalDrift;
    let reflection_gate =
        review.kind == crate::engine::human_review::HumanReviewKind::ReflectionGate;
    let risk = review.risk.as_str();
    let risk_reason = review.reason.as_str();
    let rule_pattern = permission_review.rule_pattern.as_str();
    let risk_color = match risk {
        "high" => tokens.tone.err,
        "medium" => tokens.tone.warn,
        _ => tokens.tone.ok,
    };
    let label_color = tokens.fg.faint;
    let value_color = tokens.fg.body;

    let block = Block::default()
        .title(format!(" {} ", review.title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(risk_color))
        .style(Style::default().bg(tokens.surface.bg));

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Subject ", Style::default().fg(label_color)),
            Span::styled(
                review.subject.clone(),
                Style::default()
                    .fg(tokens.tone.brand)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  Risk ", Style::default().fg(label_color)),
            Span::styled(
                risk,
                Style::default().fg(risk_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Scope   ", Style::default().fg(label_color)),
            Span::styled(
                permission_scope_label(&req.tool_call.name, &req.tool_call.arguments),
                Style::default().fg(value_color),
            ),
        ]),
        Line::from(vec![
            Span::styled("Rule    ", Style::default().fg(label_color)),
            Span::styled(
                rule_pattern,
                Style::default()
                    .fg(tokens.tone.brand)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Why     ", Style::default().fg(label_color)),
            Span::styled(risk_reason, Style::default().fg(value_color)),
        ]),
        Line::from(""),
    ];

    if goal_drift_approval {
        lines.push(Line::from(vec![
            Span::styled("Goal    ", Style::default().fg(label_color)),
            Span::styled(
                "drift check requires approval",
                Style::default()
                    .fg(tokens.tone.warn)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));
    }

    if reflection_gate {
        lines.push(Line::from(vec![
            Span::styled("Gate    ", Style::default().fg(label_color)),
            Span::styled(
                "unresolved reflection findings",
                Style::default()
                    .fg(tokens.tone.err)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));
    }

    if let Some(summary) = permission_preview(&req.tool_call.name, &req.tool_call.arguments) {
        lines.push(Line::from(vec![
            Span::styled("Preview ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled("  ", Style::default()),
            Span::styled(summary.0, Style::default().fg(Color::White)),
        ]));
        for line in summary.1.lines().take(6) {
            lines.push(Line::from(Span::styled(
                format!("  {}", line),
                Style::default().fg(value_color),
            )));
        }
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        "Reason",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    for line in req.prompt.lines().take(4) {
        lines.push(Line::from(Span::styled(
            format!("  {}", line),
            Style::default().fg(tokens.fg.body),
        )));
    }

    if let Ok(args) = serde_json::to_string_pretty(&req.tool_call.arguments) {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Arguments",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        for line in args.lines().take(8) {
            lines.push(Line::from(Span::styled(
                format!("  {}", line),
                Style::default().fg(tokens.fg.faint),
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Decision",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled(
            "y",
            Style::default()
                .fg(tokens.tone.ok)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" allow once  ", Style::default().fg(value_color)),
        Span::styled(
            "s",
            Style::default()
                .fg(tokens.tone.ok)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" allow session  ", Style::default().fg(value_color)),
        Span::styled(
            "n",
            Style::default()
                .fg(tokens.tone.err)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" deny  ", Style::default().fg(value_color)),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "p",
            Style::default()
                .fg(tokens.tone.ok)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" allow project  ", Style::default().fg(value_color)),
        Span::styled(
            "a",
            Style::default()
                .fg(tokens.tone.ok)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" allow global  ", Style::default().fg(value_color)),
        Span::styled(
            "x",
            Style::default()
                .fg(tokens.tone.err)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" deny global", Style::default().fg(value_color)),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "esc",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " cancel without saving a rule",
            Style::default().fg(Color::Gray),
        ),
    ]));

    let has_diff_preview = matches!(
        req.tool_call.name.as_str(),
        "file_write" | "file_edit" | "bash"
    );
    if has_diff_preview {
        lines.push(Line::from(vec![
            Span::styled(
                "d",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" preview diff/output", Style::default().fg(Color::Gray)),
        ]));
    }

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: true }).block(block);

    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
}

fn permission_scope_label(tool_name: &str, args: &serde_json::Value) -> String {
    match tool_name {
        "bash" => args
            .get("command")
            .and_then(serde_json::Value::as_str)
            .map(|cmd| format!("shell: {}", compact_permission_line(cmd, 76)))
            .unwrap_or_else(|| "shell command".to_string()),
        "file_write" | "file_edit" | "file_read" => args
            .get("path")
            .and_then(serde_json::Value::as_str)
            .map(|path| format!("file: {}", path))
            .unwrap_or_else(|| "file operation".to_string()),
        "mcp_tool" => {
            let server = args
                .get("server_name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("server");
            let tool = args
                .get("tool_name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("tool");
            format!("mcp: {} / {}", server, tool)
        }
        _ => "tool call".to_string(),
    }
}

pub(super) fn permission_preview(
    tool_name: &str,
    args: &serde_json::Value,
) -> Option<(&'static str, String)> {
    let name = tool_name.to_ascii_lowercase();
    if name.contains("bash") {
        return bash_permission_preview(args);
    }
    if name.contains("file") || name.contains("format") {
        let path = args
            .get("path")
            .or_else(|| args.get("file_path"))
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown path)");
        let action = if name.contains("write") {
            "Write"
        } else if name.contains("edit") {
            "Edit"
        } else {
            "File"
        };
        return Some((action, path.to_string()));
    }
    if name.contains("web") {
        return args
            .get("url")
            .or_else(|| args.get("query"))
            .and_then(|v| v.as_str())
            .map(|target| ("Network", target.to_string()));
    }
    if name.contains("mcp") {
        let server = args
            .get("server_name")
            .and_then(|v| v.as_str())
            .unwrap_or("server");
        let tool = args
            .get("tool_name")
            .and_then(|v| v.as_str())
            .unwrap_or("tool");
        return Some(("MCP", format!("{} / {}", server, tool)));
    }
    None
}

fn bash_permission_preview(args: &serde_json::Value) -> Option<(&'static str, String)> {
    let cmd = args.get("command").and_then(|v| v.as_str())?;
    let classification = crate::tools::bash_tool::command_classifier::classify_command(cmd);
    let mut lines = vec![format!("$ {}", compact_permission_line(cmd, 96))];
    lines.push(format!(
        "category={} kind={}",
        serde_enum_label(classification.category),
        serde_enum_label(classification.command_kind)
    ));
    if let Some(family) = classification.validation_family {
        lines.push(format!("validation={}", serde_enum_label(family)));
    }
    let mut flags = Vec::new();
    if classification.network_access {
        flags.push("network");
    }
    if classification.external_path_access {
        flags.push("external-path");
    }
    if classification.requires_pty() {
        flags.push("pty-required");
    }
    if classification.expected_silent_output {
        flags.push("silent-on-success");
    }
    if classification.risky_shell_wrapper {
        flags.push("risky-wrapper");
    }
    if !flags.is_empty() {
        lines.push(format!("flags={}", flags.join(",")));
    }
    if !classification.path_patterns.is_empty() {
        lines.push(format!(
            "paths={}",
            classification
                .path_patterns
                .iter()
                .take(4)
                .cloned()
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    lines.push(format!(
        "rule={}",
        crate::tui::app::permission_rule_pattern("bash", args)
    ));
    Some(("Command", lines.join("\n")))
}

fn serde_enum_label<T>(value: T) -> String
where
    T: serde::Serialize + std::fmt::Debug,
{
    serde_json::to_value(&value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{:?}", value))
}

fn compact_permission_line(text: &str, max_chars: usize) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    if line.chars().count() <= max_chars {
        line.to_string()
    } else {
        format!(
            "{}…",
            line.chars()
                .take(max_chars.saturating_sub(1))
                .collect::<String>()
        )
    }
}

/// 渲染计划审批弹窗
pub fn render_plan_approval(
    f: &mut Frame,
    plan: &crate::engine::plan_mode::Plan,
    area: Rect,
    theme: &crate::tui::theme::Theme,
) {
    let popup_area = centered_rect(70, 70, area);
    let tokens = &theme.tokens;
    let review = plan.human_review_request();
    let risk_color = match review.risk {
        crate::engine::human_review::HumanReviewRisk::High => tokens.tone.err,
        crate::engine::human_review::HumanReviewRisk::Medium => tokens.tone.warn,
        crate::engine::human_review::HumanReviewRisk::Low => tokens.tone.ok,
    };

    let block = Block::default()
        .title(format!(" Plan Approval: {} ", plan.title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(risk_color))
        .style(Style::default().bg(tokens.surface.bg));

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Goal: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(plan.goal.clone(), Style::default().fg(tokens.fg.body)),
        ]),
        Line::from(vec![
            Span::styled(
                "Complexity: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                plan.estimated_complexity.clone(),
                Style::default().fg(tokens.tone.brand),
            ),
        ]),
        Line::from(vec![
            Span::styled("Review: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(review.risk.as_str(), Style::default().fg(risk_color)),
            Span::styled(" · ", Style::default().fg(tokens.fg.faint)),
            Span::styled(review.reason, Style::default().fg(tokens.fg.sub)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            format!("Steps ({}):", plan.steps.len()),
            Style::default()
                .fg(tokens.fg.body)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(Span::styled(
            "────────────────────────────────────────",
            Style::default().fg(tokens.fg.faint),
        )),
    ];

    for (i, step) in plan.steps.iter().enumerate() {
        let (status_icon, icon_color) = match step.status {
            crate::engine::plan_mode::StepStatus::Pending => ("○", tokens.fg.faint),
            crate::engine::plan_mode::StepStatus::InProgress => ("●", tokens.tone.brand),
            crate::engine::plan_mode::StepStatus::Completed => ("✓", tokens.tone.ok),
            crate::engine::plan_mode::StepStatus::Skipped => ("·", tokens.fg.faint),
            crate::engine::plan_mode::StepStatus::Failed(_) => ("✗", tokens.tone.err),
        };
        let tool_info = step
            .tool
            .as_deref()
            .map(|t| format!(" (via {})", t))
            .unwrap_or_default();
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {} {}. ", status_icon, i + 1),
                Style::default().fg(icon_color),
            ),
            Span::styled(
                step.description.clone(),
                Style::default().fg(tokens.fg.body),
            ),
            Span::styled(tool_info, Style::default().fg(tokens.fg.faint)),
        ]));
    }

    lines.push(Line::from(Span::styled(
        "────────────────────────────────────────",
        Style::default().fg(tokens.fg.faint),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "y",
            Style::default()
                .fg(tokens.tone.ok)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" = Approve  ", Style::default().fg(tokens.fg.faint)),
        Span::styled(
            "n",
            Style::default()
                .fg(tokens.tone.err)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" = Reject  ", Style::default().fg(tokens.fg.faint)),
        Span::styled(
            "m",
            Style::default()
                .fg(tokens.tone.warn)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" = Modify", Style::default().fg(tokens.fg.faint)),
    ]));

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: true }).block(block);

    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
}

/// 渲染问答用户弹窗
pub fn render_ask_user(f: &mut Frame, question: &str, options: &[String], area: Rect) {
    let popup_area = centered_rect(70, 50, area);

    let block = Block::default()
        .title(" Question from Agent ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(Color::Black));

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Q: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(question.to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
    ];

    if !options.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "Options:",
            Style::default().add_modifier(Modifier::BOLD),
        )]));
        for (i, opt) in options.iter().enumerate() {
            lines.push(Line::from(vec![
                Span::styled(format!("  {}. ", i + 1), Style::default().fg(Color::Cyan)),
                Span::styled(opt.clone(), Style::default().fg(Color::White)),
            ]));
        }
        lines.push(Line::from(""));
    }

    lines.push(Line::from(vec![
        Span::styled(
            "Enter",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" = Submit answer  ", Style::default().fg(Color::Gray)),
        Span::styled(
            "Esc",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" = Cancel", Style::default().fg(Color::Gray)),
    ]));

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: true }).block(block);

    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
}

/// 渲染 Onboarding 引导弹窗
pub fn render_onboarding(
    f: &mut Frame,
    state: &crate::onboarding::OnboardingState,
    area: Rect,
    theme: &crate::tui::theme::Theme,
) {
    let popup_area = centered_rect(80, 75, area);
    let step = state.step;

    let block = Block::default()
        .title(format!(
            " Onboarding ({}/{}) — {} ",
            step.index() + 1,
            crate::onboarding::OnboardingStep::total_steps(),
            step.title()
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(theme.tokens.surface.bg));

    let mut lines = vec![Line::from("")];

    // 步骤内容
    for line in step.content().lines() {
        if line.trim().is_empty() {
            lines.push(Line::from(""));
        } else if line.starts_with("- ") {
            lines.push(Line::from(vec![
                Span::styled("  • ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    line.strip_prefix("- ").unwrap_or(line).to_string(),
                    Style::default().fg(theme.tokens.fg.body),
                ),
            ]));
        } else if line.ends_with(':') && !line.contains(" ") {
            lines.push(Line::from(vec![Span::styled(
                line.to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]));
        } else if line.starts_with("Welcome") || line.starts_with("You're all set") {
            lines.push(Line::from(vec![Span::styled(
                line.to_string(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )]));
        } else {
            lines.push(Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(theme.tokens.fg.body),
            )));
        }
    }

    // 底部导航提示
    lines.push(Line::from(""));
    lines.push(Line::from(""));

    let nav_spans = if step.index() == 0 {
        vec![
            Span::styled(
                "Enter/→",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" = Next  ", Style::default().fg(Color::Gray)),
            Span::styled(
                "Esc",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" = Skip", Style::default().fg(Color::Gray)),
        ]
    } else if step == crate::onboarding::OnboardingStep::Done {
        vec![
            Span::styled(
                "←",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" = Back  ", Style::default().fg(Color::Gray)),
            Span::styled(
                "Enter",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" = Finish", Style::default().fg(Color::Gray)),
        ]
    } else {
        vec![
            Span::styled(
                "←",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" = Back  ", Style::default().fg(Color::Gray)),
            Span::styled(
                "Enter/→",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" = Next  ", Style::default().fg(Color::Gray)),
            Span::styled(
                "Esc",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" = Skip", Style::default().fg(Color::Gray)),
        ]
    };

    lines.push(Line::from(nav_spans));

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: true }).block(block);

    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
}
