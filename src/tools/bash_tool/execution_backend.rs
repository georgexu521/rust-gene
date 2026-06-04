use tokio::process::Command;
use tracing::warn;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BashExecutionBackend {
    Local,
    Restricted,
    External,
}

impl BashExecutionBackend {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            BashExecutionBackend::Local => "local",
            BashExecutionBackend::Restricted => "restricted",
            BashExecutionBackend::External => "external",
        }
    }
}

pub(super) fn parse_backend(value: &str) -> Option<BashExecutionBackend> {
    match value.trim().to_ascii_lowercase().as_str() {
        "local" => Some(BashExecutionBackend::Local),
        "restricted" | "sandbox" | "soft_sandbox" => Some(BashExecutionBackend::Restricted),
        "external" => Some(BashExecutionBackend::External),
        _ => None,
    }
}

pub(super) fn default_backend() -> BashExecutionBackend {
    match std::env::var("PRIORITY_AGENT_BASH_BACKEND") {
        Ok(raw) => {
            let trimmed = raw.trim();
            match parse_backend(trimmed) {
                Some(backend) => backend,
                None => {
                    warn!(
                        "Invalid PRIORITY_AGENT_BASH_BACKEND='{}', expected 'local'/'restricted'/'external'. Falling back to 'local'.",
                        trimmed
                    );
                    BashExecutionBackend::Local
                }
            }
        }
        Err(_) => BashExecutionBackend::Local,
    }
}

pub(super) fn effective_timeout_secs(requested: Option<u64>) -> u64 {
    let requested = requested.unwrap_or(60).min(3600);
    let floor = std::env::var("PRIORITY_AGENT_BASH_TIMEOUT_FLOOR_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(0)
        .min(3600);
    requested.max(floor).min(3600)
}

pub(super) fn sanitize_agent_runtime_env(cmd: &mut Command) {
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
        cmd.env_remove(key);
    }
}

pub(super) fn restricted_command(command: &str) -> String {
    // 受限后端说明：
    // - 仅应用软资源限制和最小化环境变量
    // - 不是容器/命名空间级别隔离
    format!(
        "ulimit -n 64; ulimit -u 32; ulimit -t 60; \
         export PATH=/usr/bin:/bin; \
         unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY ALL_PROXY all_proxy; \
         {}",
        command
    )
}

pub(super) fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

pub(super) fn external_wrapper_template() -> Option<String> {
    std::env::var("PRIORITY_AGENT_BASH_EXTERNAL_CMD")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            std::env::var("PRIORITY_AGENT_BASH_SANDBOX_CMD")
                .ok()
                .filter(|s| !s.trim().is_empty())
        })
}

pub(super) fn external_wrapper_allowlist() -> Option<Vec<String>> {
    let value = std::env::var("PRIORITY_AGENT_BASH_EXTERNAL_ALLOWLIST")
        .ok()
        .or_else(|| std::env::var("PRIORITY_AGENT_BASH_EXTERNAL_WRAPPER_ALLOWLIST").ok())?;
    let items: Vec<String> = value
        .split(|c: char| c == ',' || c == ';' || c.is_ascii_whitespace())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect();
    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

pub(super) fn external_fallback_backend() -> Option<BashExecutionBackend> {
    let value = std::env::var("PRIORITY_AGENT_BASH_EXTERNAL_FALLBACK")
        .ok()
        .or_else(|| std::env::var("PRIORITY_AGENT_BASH_SANDBOX_FALLBACK").ok())?;
    match value.trim().to_ascii_lowercase().as_str() {
        "none" | "deny" => None,
        other => parse_backend(other).filter(|b| *b != BashExecutionBackend::External),
    }
}

pub(super) fn first_shell_token(s: &str) -> Option<String> {
    s.split_whitespace().next().map(ToString::to_string)
}

pub(super) fn short_command_summary(command: &str) -> String {
    let mut chars = command.chars();
    let summary: String = chars.by_ref().take(120).collect();
    if chars.next().is_some() {
        format!("{summary}...")
    } else {
        summary
    }
}

fn validate_external_wrapper(template: &str) -> Result<(), String> {
    let allowlist = match external_wrapper_allowlist() {
        Some(v) => v,
        None => return Ok(()),
    };
    let wrapper = first_shell_token(template)
        .ok_or_else(|| "external wrapper template is empty".to_string())?;
    let allowed = allowlist.iter().any(|x| x == &wrapper);
    if allowed {
        Ok(())
    } else {
        Err(format!(
            "external wrapper '{}' is not in PRIORITY_AGENT_BASH_EXTERNAL_ALLOWLIST",
            wrapper
        ))
    }
}

pub(super) fn external_command_with_template(template: &str, command: &str) -> String {
    let quoted = shell_single_quote(command);
    if template.contains("{command}") {
        template.replace("{command}", &quoted)
    } else {
        format!("{} -- bash -lc {}", template, quoted)
    }
}

pub(super) fn external_command(command: &str) -> Result<String, String> {
    let template = external_wrapper_template().ok_or_else(|| {
        "external backend requires PRIORITY_AGENT_BASH_EXTERNAL_CMD (or PRIORITY_AGENT_BASH_SANDBOX_CMD)".to_string()
    })?;
    validate_external_wrapper(&template)?;
    Ok(external_command_with_template(&template, command))
}
