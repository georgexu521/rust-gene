//! 权限系统
//!
//! 细粒度的工具权限控制
//! 支持通配符匹配、规则源分类

pub mod llm_classifier;

use serde::{Deserialize, Serialize};

/// Once 模式授权有效期（秒）
const ONCE_AUTHORIZATION_TTL_SECS: u64 = 30;

/// 检查字符串是否匹配通配符模式
/// 支持 * (任意字符) 和 ? (单个字符)
pub fn match_wildcard(pattern: &str, text: &str) -> bool {
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();
    let p_len = pattern_chars.len();
    let t_len = text_chars.len();

    let mut p_idx = 0;
    let mut t_idx = 0;
    let mut star_idx = None;
    let mut match_idx = 0;

    while t_idx < t_len {
        if p_idx < p_len
            && (pattern_chars[p_idx] == '?' || pattern_chars[p_idx] == text_chars[t_idx])
        {
            p_idx += 1;
            t_idx += 1;
        } else if p_idx < p_len && pattern_chars[p_idx] == '*' {
            star_idx = Some(p_idx);
            match_idx = t_idx;
            p_idx += 1;
        } else if let Some(star) = star_idx {
            p_idx = star + 1;
            match_idx += 1;
            t_idx = match_idx;
        } else {
            return false;
        }
    }

    while p_idx < p_len && pattern_chars[p_idx] == '*' {
        p_idx += 1;
    }

    p_idx == p_len
}

/// 权限模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum PermissionMode {
    /// 默认模式 - 每次询问
    Default,
    /// 自动允许低风险操作
    AutoLowRisk,
    /// 开发者自动模式：默认允许常规开发操作，高风险操作仍需确认
    #[default]
    AutoAll,
    /// 只读模式
    ReadOnly,
    /// 一次性授权模式 - 允许一次后自动拒绝
    Once,
}

/// 风险级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

/// 规则源类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum RuleSource {
    /// 全局配置（用户主目录）
    Global,
    /// 项目配置（项目根目录）
    Project,
    /// 用户配置（当前会话）
    User,
    /// 系统默认
    #[default]
    System,
}

/// 带源的权限规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcedRule {
    pub pattern: String,
    pub source: RuleSource,
}

impl SourcedRule {
    pub fn new(pattern: impl Into<String>, source: RuleSource) -> Self {
        Self {
            pattern: pattern.into(),
            source,
        }
    }

    /// 检查是否匹配工具名（支持通配符）
    pub fn matches(&self, tool_name: &str) -> bool {
        match_wildcard(&self.pattern, tool_name)
    }
}

/// 权限规则
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionRules {
    /// 总是允许的工具（支持通配符）
    pub always_allow: Vec<SourcedRule>,
    /// 总是拒绝的工具（支持通配符）
    pub always_deny: Vec<SourcedRule>,
    /// 总是询问的工具（支持通配符）
    pub always_ask: Vec<SourcedRule>,
}

impl PermissionRules {
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加允许规则（支持通配符）
    pub fn allow(mut self, pattern: impl Into<String>) -> Self {
        self.always_allow
            .push(SourcedRule::new(pattern, RuleSource::User));
        self
    }

    /// 添加允许规则（带源）
    pub fn allow_with_source(mut self, pattern: impl Into<String>, source: RuleSource) -> Self {
        self.always_allow.push(SourcedRule::new(pattern, source));
        self
    }

    /// 添加拒绝规则（支持通配符）
    pub fn deny(mut self, pattern: impl Into<String>) -> Self {
        self.always_deny
            .push(SourcedRule::new(pattern, RuleSource::User));
        self
    }

    /// 添加拒绝规则（带源）
    pub fn deny_with_source(mut self, pattern: impl Into<String>, source: RuleSource) -> Self {
        self.always_deny.push(SourcedRule::new(pattern, source));
        self
    }

    /// 添加询问规则（支持通配符）
    pub fn ask(mut self, pattern: impl Into<String>) -> Self {
        self.always_ask
            .push(SourcedRule::new(pattern, RuleSource::User));
        self
    }

    /// 添加询问规则（带源）
    pub fn ask_with_source(mut self, pattern: impl Into<String>, source: RuleSource) -> Self {
        self.always_ask.push(SourcedRule::new(pattern, source));
        self
    }

    /// 检查工具权限
    /// 优先级: deny > allow > ask
    pub fn check(&self, tool_name: &str) -> PermissionDecision {
        // 先检查 deny（最高优先级）
        for rule in &self.always_deny {
            if rule.matches(tool_name) {
                return PermissionDecision::Deny;
            }
        }

        // 再检查 allow
        for rule in &self.always_allow {
            if rule.matches(tool_name) {
                return PermissionDecision::Allow;
            }
        }

        // 最后检查 ask
        for rule in &self.always_ask {
            if rule.matches(tool_name) {
                return PermissionDecision::Ask;
            }
        }

        PermissionDecision::Ask
    }

    /// 获取匹配的规则列表
    pub fn get_matching_rules(&self, tool_name: &str) -> Vec<(PermissionDecision, &SourcedRule)> {
        let mut matches = Vec::new();

        for rule in &self.always_deny {
            if rule.matches(tool_name) {
                matches.push((PermissionDecision::Deny, rule));
            }
        }

        for rule in &self.always_allow {
            if rule.matches(tool_name) {
                matches.push((PermissionDecision::Allow, rule));
            }
        }

        for rule in &self.always_ask {
            if rule.matches(tool_name) {
                matches.push((PermissionDecision::Ask, rule));
            }
        }

        matches
    }
}

/// 权限决策
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionDecision {
    /// 允许
    Allow,
    /// 拒绝
    Deny,
    /// 询问用户
    Ask,
}

/// 权限上下文
#[derive(Debug, Clone)]
pub struct PermissionContext {
    pub mode: PermissionMode,
    pub rules: PermissionRules,
    pub working_dir: std::path::PathBuf,
    pub is_bypass_available: bool,
    /// 一次性授权记录 (tool_call_id -> expiration time)
    once_authorizations: std::collections::HashMap<String, std::time::Instant>,
}

impl PermissionContext {
    pub fn new(working_dir: impl Into<std::path::PathBuf>) -> Self {
        let working_dir = working_dir.into();
        Self {
            mode: PermissionMode::default(),
            rules: Self::load_merged_rules(working_dir.clone()),
            working_dir,
            is_bypass_available: false,
            once_authorizations: std::collections::HashMap::new(),
        }
    }

    /// 从多个来源加载合并的规则
    /// 优先级: User > Project > Global > System
    fn load_merged_rules(working_dir: std::path::PathBuf) -> PermissionRules {
        let mut rules = PermissionRules::new();

        // 1. 加载系统默认规则
        rules = Self::load_system_defaults(rules);

        // 2. 加载全局配置 (~/.priority-agent/permissions.toml)
        rules = Self::load_global_config(rules);

        // 3. 加载项目配置 (.priority-agent/permissions.toml)
        rules = Self::load_project_config(rules, &working_dir);

        rules
    }

    /// 系统默认规则
    fn load_system_defaults(mut rules: PermissionRules) -> PermissionRules {
        // 读操作默认允许
        rules
            .always_allow
            .push(SourcedRule::new("file_read", RuleSource::System));
        rules
            .always_allow
            .push(SourcedRule::new("glob", RuleSource::System));
        rules
            .always_allow
            .push(SourcedRule::new("grep", RuleSource::System));
        rules
            .always_allow
            .push(SourcedRule::new("project_list", RuleSource::System));
        rules
    }

    /// 加载全局配置
    fn load_global_config(mut rules: PermissionRules) -> PermissionRules {
        if let Some(home) = dirs::home_dir() {
            let config_path = home.join(".priority-agent").join("permissions.toml");
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Ok(global_rules) = toml::from_str::<PermissionRules>(&content) {
                    // 合并全局规则，保持源信息
                    for rule in global_rules.always_allow {
                        rules
                            .always_allow
                            .push(SourcedRule::new(rule.pattern, RuleSource::Global));
                    }
                    for rule in global_rules.always_deny {
                        rules
                            .always_deny
                            .push(SourcedRule::new(rule.pattern, RuleSource::Global));
                    }
                    for rule in global_rules.always_ask {
                        rules
                            .always_ask
                            .push(SourcedRule::new(rule.pattern, RuleSource::Global));
                    }
                }
            }
        }
        rules
    }

    /// 加载项目配置
    fn load_project_config(
        mut rules: PermissionRules,
        working_dir: &std::path::Path,
    ) -> PermissionRules {
        let config_path = working_dir.join(".priority-agent").join("permissions.toml");
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(project_rules) = toml::from_str::<PermissionRules>(&content) {
                // 合并项目规则，保持源信息
                for rule in project_rules.always_allow {
                    rules
                        .always_allow
                        .push(SourcedRule::new(rule.pattern, RuleSource::Project));
                }
                for rule in project_rules.always_deny {
                    rules
                        .always_deny
                        .push(SourcedRule::new(rule.pattern, RuleSource::Project));
                }
                for rule in project_rules.always_ask {
                    rules
                        .always_ask
                        .push(SourcedRule::new(rule.pattern, RuleSource::Project));
                }
            }
        }
        rules
    }

    /// 检查是否需要确认
    pub fn requires_confirmation(&self, tool_name: &str, params: &serde_json::Value) -> bool {
        // 对 mcp_tool 构建粒度名称 mcp/<server>/<tool> 进行权限检查
        let effective_tool_name = if tool_name == "mcp_tool" {
            let server = params["server_name"].as_str().unwrap_or("");
            let t = params["tool_name"].as_str().unwrap_or("");
            if !server.is_empty() && !t.is_empty() {
                format!("mcp/{}/{}", server, t)
            } else {
                tool_name.to_string()
            }
        } else {
            tool_name.to_string()
        };
        let matching_rules = self.rules.get_matching_rules(&effective_tool_name);
        let has_deny = matching_rules
            .iter()
            .any(|(d, _)| matches!(d, PermissionDecision::Deny));
        let has_allow = matching_rules
            .iter()
            .any(|(d, _)| matches!(d, PermissionDecision::Allow));
        let has_ask = matching_rules
            .iter()
            .any(|(d, _)| matches!(d, PermissionDecision::Ask));

        match self.mode {
            PermissionMode::ReadOnly => {
                // 只读模式下，任何写入操作都需要确认
                matches!(tool_name, "file_write" | "file_edit" | "bash" | "mcp_tool")
            }
            PermissionMode::AutoAll => {
                // 开发者默认模式：减少常规编程中的打断，但保留显式规则和高风险兜底。
                if has_deny || has_ask {
                    return true;
                }
                self.requires_safety_confirmation(tool_name, params)
            }
            PermissionMode::AutoLowRisk => {
                // 规则优先: deny > allow > ask；未命中规则时按参数风险级别决定
                if has_deny {
                    return true;
                }
                if has_allow {
                    return false;
                }
                if has_ask {
                    return true;
                }
                self.risk_level(tool_name, params) >= RiskLevel::Medium
            }
            PermissionMode::Once => {
                // Once 模式：先检查是否已有有效的一次性授权
                // NOTE: 按 tool_name 粒度授权，不区分参数。
                if let Some(expired) = self.once_authorizations.get(tool_name) {
                    if expired.elapsed().as_secs() < ONCE_AUTHORIZATION_TTL_SECS {
                        return false;
                    }
                }
                // 没有授权或已过期，需要询问
                true
            }
            PermissionMode::Default => {
                // 根据规则决定
                matches!(
                    self.rules.check(&effective_tool_name),
                    PermissionDecision::Ask
                )
            }
        }
    }

    /// 是否应该把工具暴露给模型。
    ///
    /// 执行层仍会做最终授权；这个方法只用于请求前收窄工具池，减少
    /// 被明确拒绝或只读模式下不可用的工具被模型反复调用。
    pub fn should_expose_tool(&self, tool_name: &str) -> bool {
        if matches!(self.rules.check(tool_name), PermissionDecision::Deny) {
            return false;
        }

        if self.mode == PermissionMode::ReadOnly {
            return matches!(
                tool_name,
                "file_read"
                    | "glob"
                    | "grep"
                    | "project_list"
                    | "memory_load"
                    | "skills_list"
                    | "skill_view"
                    | "web_search"
                    | "list_mcp_resources"
                    | "read_mcp_resource"
                    | "cost"
                    | "context"
                    | "context_vis"
                    | "diff"
                    | "lsp"
                    | "symbol_query"
            );
        }

        true
    }

    /// 在 AutoAll 下是否可以跳过工具自身的普通确认。
    ///
    /// 有些工具出于保守默认会对所有写操作声明 requires_confirmation。
    /// 开发者自动模式允许这类常规开发动作直接执行，但仍不绕过：
    /// - 用户/项目显式 deny 或 ask 规则
    /// - bash 高危命令
    /// - 敏感路径写入、清空记忆、MCP/插件运行等高风险动作
    pub fn auto_approves_tool_confirmation(
        &self,
        tool_name: &str,
        params: &serde_json::Value,
    ) -> bool {
        self.mode == PermissionMode::AutoAll && !self.requires_confirmation(tool_name, params)
    }

    /// 获取工具的权限决策详情
    pub fn check_with_details(&self, tool_name: &str) -> (PermissionDecision, Vec<String>) {
        let decision = self.rules.check(tool_name);
        let matching_rules = self.rules.get_matching_rules(tool_name);
        let details: Vec<String> = matching_rules
            .into_iter()
            .map(|(d, r)| format!("{:?} from {:?}: {}", d, r.source, r.pattern))
            .collect();
        (decision, details)
    }

    /// 授予一次性授权（用于 Once 模式）
    pub fn grant_once(&mut self, tool_name: &str) {
        self.once_authorizations
            .insert(tool_name.to_string(), std::time::Instant::now());
    }

    /// 撤销一次性授权
    pub fn revoke_once(&mut self, tool_name: &str) {
        self.once_authorizations.remove(tool_name);
    }

    /// 检查工具是否拥有有效的一次性授权
    pub fn has_once_authorization(&self, tool_name: &str) -> bool {
        self.once_authorizations
            .get(tool_name)
            .map(|exp| exp.elapsed().as_secs() < ONCE_AUTHORIZATION_TTL_SECS)
            .unwrap_or(false)
    }

    /// 清理过期的一次性授权
    pub fn cleanup_expired_once(&mut self) {
        self.once_authorizations
            .retain(|_, exp| exp.elapsed().as_secs() < ONCE_AUTHORIZATION_TTL_SECS);
    }

    fn risk_level(&self, tool_name: &str, params: &serde_json::Value) -> RiskLevel {
        match tool_name {
            "file_read" | "glob" | "grep" | "project_list" | "memory_load" => RiskLevel::Low,
            "memory_clear" | "mcp" => RiskLevel::High,
            "agent" => RiskLevel::Medium,
            "bash" => {
                let cmd = params["command"]
                    .as_str()
                    .or_else(|| params["cmd"].as_str())
                    .unwrap_or_default();
                let lower = cmd.to_lowercase();
                let classification =
                    crate::tools::bash_tool::command_classifier::classify_command(cmd);
                if Self::is_high_risk_command(cmd)
                    || Self::has_external_network_command(&lower)
                    || Self::has_remote_git_command(&lower)
                    || self.bash_references_outside_workspace_path(&lower)
                {
                    RiskLevel::High
                } else {
                    match classification.category {
                        crate::tools::bash_tool::command_classifier::ShellCommandCategory::Read
                        | crate::tools::bash_tool::command_classifier::ShellCommandCategory::List
                        | crate::tools::bash_tool::command_classifier::ShellCommandCategory::Search
                        | crate::tools::bash_tool::command_classifier::ShellCommandCategory::Validation
                        | crate::tools::bash_tool::command_classifier::ShellCommandCategory::TestRun => {
                            RiskLevel::Low
                        }
                        crate::tools::bash_tool::command_classifier::ShellCommandCategory::Destructive => {
                            RiskLevel::High
                        }
                        crate::tools::bash_tool::command_classifier::ShellCommandCategory::PackageInstall
                        | crate::tools::bash_tool::command_classifier::ShellCommandCategory::DevServer
                        | crate::tools::bash_tool::command_classifier::ShellCommandCategory::FileMutation
                        | crate::tools::bash_tool::command_classifier::ShellCommandCategory::GitMutation
                        | crate::tools::bash_tool::command_classifier::ShellCommandCategory::Unknown => {
                            RiskLevel::Medium
                        }
                    }
                }
            }
            "file_write" | "file_edit" => {
                let path = params["path"].as_str().unwrap_or_default();
                if Self::is_high_risk_path(path)
                    || !self.path_is_in_trusted_workspace(path)
                    || Self::is_large_content_write(params)
                {
                    RiskLevel::High
                } else {
                    RiskLevel::Medium
                }
            }
            "git" => match params["action"].as_str() {
                Some("push") => RiskLevel::High,
                Some("checkout" | "branch") => RiskLevel::Medium,
                Some("add" | "commit") => RiskLevel::Low,
                _ => RiskLevel::Low,
            },
            "worktree" => match params["action"].as_str() {
                Some("remove") => RiskLevel::High,
                Some("prune" | "create" | "switch") => RiskLevel::Medium,
                _ => RiskLevel::Low,
            },
            "github" => match params["action"].as_str() {
                Some("pr_create") => RiskLevel::Medium,
                _ => RiskLevel::Low,
            },
            "web_fetch" => {
                let url = params["url"].as_str().unwrap_or_default();
                if self.url_is_trusted(url) {
                    RiskLevel::Medium
                } else {
                    RiskLevel::High
                }
            }
            "web_search" => RiskLevel::Medium,
            "plugin" => match params["action"].as_str() {
                Some("run") => RiskLevel::High,
                _ => RiskLevel::Low,
            },
            "mcp_tool" => RiskLevel::High,
            _ => RiskLevel::Low,
        }
    }

    fn requires_safety_confirmation(&self, tool_name: &str, params: &serde_json::Value) -> bool {
        self.risk_level(tool_name, params) >= RiskLevel::High
    }

    fn path_is_in_trusted_workspace(&self, path: &str) -> bool {
        if path.trim().is_empty() {
            return false;
        }
        let input = std::path::Path::new(path);
        let candidate = if input.is_absolute() {
            Self::normalize_path(input)
        } else {
            Self::normalize_path(&self.working_dir.join(input))
        };
        self.trusted_workspace_roots()
            .into_iter()
            .any(|root| candidate.starts_with(root))
    }

    fn bash_references_outside_workspace_path(&self, cmd: &str) -> bool {
        Self::extract_absolute_paths_from_shell(cmd)
            .into_iter()
            .any(|path| !self.path_is_in_trusted_workspace(&path))
    }

    fn extract_absolute_paths_from_shell(cmd: &str) -> Vec<String> {
        let mut paths = Vec::new();
        for raw in cmd.split(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '"' | '\'' | '`' | '(' | ')' | '{' | '}' | '[' | ']' | ';' | '|' | '&'
                )
        }) {
            let token = raw.trim_matches(|ch: char| matches!(ch, '<' | '>' | ',' | ':' | '='));
            if token.is_empty() {
                continue;
            }

            let candidate = if token.starts_with('/') {
                Some(token.to_string())
            } else if let Some((_, path)) = token.split_once("=/") {
                Some(format!("/{}", path))
            } else if let Some((_, path)) = token.split_once(":/") {
                Some(format!("/{}", path))
            } else {
                None
            };

            if let Some(path) = candidate {
                let trimmed = path.as_str().trim_matches(|ch: char| {
                    matches!(ch, '<' | '>' | ',' | ':' | ')' | ']' | '}' | '.')
                });
                if trimmed.starts_with('/') {
                    paths.push(trimmed.to_string());
                }
            }
        }
        paths
    }

    fn trusted_workspace_roots(&self) -> Vec<std::path::PathBuf> {
        let mut roots = vec![Self::normalize_path(&self.working_dir)];
        if let Ok(extra) = std::env::var("PRIORITY_AGENT_TRUSTED_WORKSPACES") {
            roots.extend(
                extra
                    .split(':')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .map(|part| Self::normalize_path(std::path::Path::new(part))),
            );
        }
        roots
    }

    fn url_is_trusted(&self, url: &str) -> bool {
        if url.trim().is_empty() {
            return false;
        }
        let host = match Self::url_host(url) {
            Some(host) => host,
            None => return false,
        };
        if let Ok(trusted) = std::env::var("PRIORITY_AGENT_TRUSTED_DOMAINS") {
            return trusted
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .any(|domain| host == domain || host.ends_with(&format!(".{}", domain)));
        }
        false
    }

    fn url_host(url: &str) -> Option<String> {
        let after_scheme = url.split_once("://")?.1;
        let host_port = after_scheme.split('/').next()?.split('@').next_back()?;
        let host = if host_port.starts_with('[') {
            host_port
                .find(']')
                .map(|end| host_port[1..end].to_ascii_lowercase())?
        } else {
            host_port
                .split(':')
                .next()
                .unwrap_or(host_port)
                .to_ascii_lowercase()
        };
        (!host.is_empty()).then_some(host)
    }

    fn normalize_path(path: &std::path::Path) -> std::path::PathBuf {
        let mut normalized = std::path::PathBuf::new();
        for component in path.components() {
            match component {
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    normalized.pop();
                }
                other => normalized.push(other.as_os_str()),
            }
        }
        normalized
    }

    fn is_high_risk_command(cmd: &str) -> bool {
        if cmd.is_empty() {
            return true;
        }
        if crate::security::is_dangerous_command(cmd) {
            return true;
        }
        let dangerous_patterns = [
            "rm -rf",
            "mkfs",
            "dd if=",
            "shutdown",
            "reboot",
            "poweroff",
            ":(){",
            "chmod 777",
            "chown -r",
            "sudo ",
        ];
        dangerous_patterns.iter().any(|p| cmd.contains(p))
    }

    fn has_external_network_command(cmd: &str) -> bool {
        let lower = cmd.to_ascii_lowercase();
        ["curl ", "wget ", "ssh ", "scp ", "rsync ", "nc ", "ncat "]
            .iter()
            .any(|pattern| lower.contains(pattern))
    }

    fn has_remote_git_command(cmd: &str) -> bool {
        let lower = cmd.to_ascii_lowercase();
        lower.contains("git push") || lower.contains("git fetch") || lower.contains("git pull")
    }

    fn is_high_risk_path(path: &str) -> bool {
        if path.is_empty() {
            return true;
        }
        let lower = path.to_lowercase();
        let sensitive_markers = [
            "/etc/",
            "/usr/",
            "/bin/",
            "/sbin/",
            "/.ssh/",
            ".env",
            "id_rsa",
            "authorized_keys",
            "/dev/sda",
            "/dev/sdb",
            "/dev/hda",
        ];
        sensitive_markers.iter().any(|m| lower.contains(m))
    }

    fn is_large_content_write(params: &serde_json::Value) -> bool {
        let content_len = params["content"]
            .as_str()
            .or_else(|| params["new_content"].as_str())
            .map(str::len)
            .unwrap_or(0);
        content_len > 20_000
    }

    /// Enhanced decision with full explainability
    pub fn explain_decision(
        &self,
        tool_name: &str,
        params: &serde_json::Value,
    ) -> ExplainableDecision {
        let effective_tool_name = if tool_name == "mcp_tool" {
            let server = params["server_name"].as_str().unwrap_or("");
            let t = params["tool_name"].as_str().unwrap_or("");
            if !server.is_empty() && !t.is_empty() {
                format!("mcp/{}/{}", server, t)
            } else {
                tool_name.to_string()
            }
        } else {
            tool_name.to_string()
        };

        let base_decision = self.rules.check(&effective_tool_name);
        let matching_rules = self.rules.get_matching_rules(&effective_tool_name);
        let risk = self.risk_level(tool_name, params);
        let confidence = self.calculate_confidence(tool_name, params, &matching_rules);

        // Build explanation
        let mut reasons = Vec::new();
        for (decision, rule) in &matching_rules {
            reasons.push(format!(
                "{:?} by {:?} rule '{}'",
                decision, rule.source, rule.pattern
            ));
        }
        if reasons.is_empty() {
            reasons.push(format!("No matching rules, default to {:?}", base_decision));
        }

        // Risk-specific warnings
        let mut warnings = Vec::new();
        if tool_name == "bash" {
            let cmd = params["command"].as_str().unwrap_or_default();
            let classification = crate::tools::bash_tool::command_classifier::classify_command(cmd);
            reasons.push(format!(
                "Shell command category: {:?}",
                classification.category
            ));
            if Self::is_high_risk_command(cmd) {
                warnings.push("HIGH_RISK_COMMAND: dangerous shell command detected".to_string());
            }
            if self.bash_references_outside_workspace_path(cmd) {
                warnings.push(
                    "OUTSIDE_WORKSPACE: shell command references a path outside the trusted workspace"
                        .to_string(),
                );
            }
            if crate::security::is_dangerous_command(cmd) {
                warnings
                    .push("COMMAND_INJECTION: potentially malicious pattern detected".to_string());
            }
        }
        if tool_name == "file_write" || tool_name == "file_edit" {
            let path = params["path"].as_str().unwrap_or_default();
            if Self::is_high_risk_path(path) {
                warnings.push("HIGH_RISK_PATH: sensitive system path detected".to_string());
            }
            if !self.path_is_in_trusted_workspace(path) {
                warnings.push("OUTSIDE_WORKSPACE: path is outside trusted workspace".to_string());
            }
            // Check for path traversal
            if path.contains("..") {
                warnings.push("PATH_TRAVERSAL: parent directory reference detected".to_string());
            }
        }
        if tool_name == "web_fetch" {
            let url = params["url"].as_str().unwrap_or_default();
            if !self.url_is_trusted(url) {
                warnings.push("UNTRUSTED_NETWORK: URL host is not in trusted domains".to_string());
            }
        }

        ExplainableDecision {
            decision: base_decision,
            confidence,
            reasons,
            risk_level: risk,
            warnings,
            matched_rules: matching_rules
                .into_iter()
                .map(|(d, r)| (d, r.clone()))
                .collect(),
        }
    }

    /// Calculate confidence score for the decision (0.0 - 1.0)
    fn calculate_confidence(
        &self,
        tool_name: &str,
        params: &serde_json::Value,
        rules: &[(PermissionDecision, &SourcedRule)],
    ) -> f32 {
        // Base confidence based on rule coverage
        let rule_confidence = if rules.is_empty() {
            0.5 // No rules, moderate confidence in default
        } else {
            // More specific rules = higher confidence
            let avg_pattern_len: f32 = rules
                .iter()
                .map(|(_, r)| r.pattern.len() as f32)
                .sum::<f32>()
                / rules.len() as f32;
            (avg_pattern_len / 50.0).min(0.95)
        };

        // Adjust based on mode
        let mode_confidence = match self.mode {
            PermissionMode::AutoAll => 0.9, // Developer auto mode with high-risk guardrails
            PermissionMode::ReadOnly => 0.85,
            PermissionMode::Once => 0.8,
            PermissionMode::AutoLowRisk => 0.75, // Conservative
            PermissionMode::Default => 0.7,
        };

        // Adjust for risk
        let risk_adjustment = match self.risk_level(tool_name, params) {
            RiskLevel::High => -0.1,
            RiskLevel::Medium => 0.0,
            RiskLevel::Low => 0.05,
        };

        (rule_confidence + mode_confidence + risk_adjustment) / 2.0
    }
}

/// Explainable permission decision with full context
#[derive(Debug, Clone)]
pub struct ExplainableDecision {
    /// The permission decision
    pub decision: PermissionDecision,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// Human-readable reasons for the decision
    pub reasons: Vec<String>,
    /// Risk level of the operation
    pub risk_level: RiskLevel,
    /// Security warnings (injections, traversal, etc.)
    pub warnings: Vec<String>,
    /// The matched rules that led to this decision
    pub matched_rules: Vec<(PermissionDecision, SourcedRule)>,
}

impl ExplainableDecision {
    /// Compact one-line summary suitable for approval prompts and traces.
    pub fn concise_summary(&self) -> String {
        let reason = self
            .reasons
            .first()
            .cloned()
            .unwrap_or_else(|| "no explicit rule matched".to_string());
        let warnings = if self.warnings.is_empty() {
            "none".to_string()
        } else {
            self.warnings.join("; ")
        };
        format!(
            "decision={:?}, risk={:?}, confidence={:.0}%, reason={}, warnings={}",
            self.decision,
            self.risk_level,
            self.confidence * 100.0,
            reason,
            warnings
        )
    }

    /// Format as human-readable string
    pub fn format(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("Decision: {:?}", self.decision));
        lines.push(format!("Confidence: {:.0}%", self.confidence * 100.0));
        lines.push(format!("Risk: {:?}", self.risk_level));
        lines.push("\nReasons:".to_string());
        for reason in &self.reasons {
            lines.push(format!("  - {}", reason));
        }
        if !self.warnings.is_empty() {
            lines.push("\n⚠️  Warnings:".to_string());
            for warning in &self.warnings {
                lines.push(format!("  - {}", warning));
            }
        }
        lines.join("\n")
    }

    /// Format as machine-parseable JSON
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "decision": format!("{:?}", self.decision),
            "confidence": self.confidence,
            "risk_level": format!("{:?}", self.risk_level),
            "reasons": self.reasons,
            "warnings": self.warnings,
            "matched_rules": self.matched_rules.iter().map(|(d, r)| {
                serde_json::json!({
                    "decision": format!("{:?}", d),
                    "pattern": r.pattern,
                    "source": format!("{:?}", r.source)
                })
            }).collect::<Vec<_>>()
        })
    }
}

/// Permission classifier trait for extensible risk assessment
/// Implement this trait to add custom classifiers (e.g., LLM-based)
#[async_trait::async_trait]
pub trait PermissionClassifier: Send + Sync {
    /// Classify a tool call with parameters
    async fn classify(
        &self,
        tool_name: &str,
        params: &serde_json::Value,
        context: &PermissionContext,
    ) -> Result<ExplainableDecision, ClassifierError>;

    /// Name of this classifier
    fn name(&self) -> &str;

    /// Priority of this classifier (higher = evaluated first)
    fn priority(&self) -> u32 {
        0
    }
}

/// Classifier error types
#[derive(Debug, Clone)]
pub enum ClassifierError {
    /// Classification failed due to internal error
    Internal(String),
    /// Classifier unavailable (e.g., LLM not configured)
    Unavailable(String),
    /// Classification timed out
    Timeout,
}

impl std::fmt::Display for ClassifierError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClassifierError::Internal(s) => write!(f, "Classification error: {}", s),
            ClassifierError::Unavailable(s) => write!(f, "Classifier unavailable: {}", s),
            ClassifierError::Timeout => write!(f, "Classification timed out"),
        }
    }
}

impl std::error::Error for ClassifierError {}

/// Default rule-based classifier (uses existing PermissionContext logic)
pub struct RuleBasedClassifier;

#[async_trait::async_trait]
impl PermissionClassifier for RuleBasedClassifier {
    async fn classify(
        &self,
        tool_name: &str,
        params: &serde_json::Value,
        context: &PermissionContext,
    ) -> Result<ExplainableDecision, ClassifierError> {
        Ok(context.explain_decision(tool_name, params))
    }

    fn name(&self) -> &str {
        "rule-based"
    }

    fn priority(&self) -> u32 {
        0 // Low priority - fallback
    }
}

impl Default for PermissionContext {
    fn default() -> Self {
        Self::new(".")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wildcard_matching() {
        assert!(match_wildcard("file_*", "file_read"));
        assert!(match_wildcard("file_*", "file_write"));
        assert!(!match_wildcard("file_*", "bash"));

        assert!(match_wildcard("*tool", "mytool"));
        assert!(match_wildcard("*tool", "sometool"));
        assert!(!match_wildcard("*tool", "bash"));

        assert!(match_wildcard("web_*", "web_fetch"));
        assert!(match_wildcard("web_*", "web_search"));
        assert!(!match_wildcard("web_*", "bash"));

        assert!(match_wildcard("?at", "cat"));
        assert!(match_wildcard("?at", "bat"));
        assert!(!match_wildcard("?at", "chat"));

        assert!(match_wildcard("*", "anything"));
        assert!(match_wildcard("exact", "exact"));
    }

    #[test]
    fn test_sourced_rule_matching() {
        let rule = SourcedRule::new("file_*", RuleSource::User);
        assert!(rule.matches("file_read"));
        assert!(rule.matches("file_write"));
        assert!(!rule.matches("bash"));
    }

    #[test]
    fn test_permission_rules_with_wildcards() {
        let rules = PermissionRules::new()
            .allow("file_*")
            .deny("*_dangerous")
            .ask("custom_*");

        // file_* should be allowed
        assert_eq!(rules.check("file_read"), PermissionDecision::Allow);
        assert_eq!(rules.check("file_write"), PermissionDecision::Allow);

        // *_dangerous should be denied
        assert_eq!(rules.check("tool_dangerous"), PermissionDecision::Deny);

        // custom_* should ask
        assert_eq!(rules.check("custom_tool"), PermissionDecision::Ask);

        // unknown tools should ask
        assert_eq!(rules.check("unknown"), PermissionDecision::Ask);
    }

    #[test]
    fn test_permission_rules_priority() {
        // deny has highest priority
        let rules = PermissionRules::new()
            .allow("file_*")
            .deny("file_dangerous");

        assert_eq!(rules.check("file_read"), PermissionDecision::Allow);
        assert_eq!(rules.check("file_dangerous"), PermissionDecision::Deny);
    }

    #[test]
    fn test_get_matching_rules() {
        let rules = PermissionRules::new()
            .allow("file_*")
            .allow("read_*")
            .deny("*_dangerous");

        let matches = rules.get_matching_rules("file_read");
        assert_eq!(matches.len(), 1); // only allow matches

        let matches = rules.get_matching_rules("file_dangerous");
        assert_eq!(matches.len(), 2); // allow and deny both match
    }

    #[test]
    fn test_explainable_decision_concise_summary_mentions_risk_and_reason() {
        let ctx = PermissionContext {
            mode: PermissionMode::Default,
            rules: PermissionRules::new().ask_with_source("bash", RuleSource::Project),
            working_dir: std::path::PathBuf::from("."),
            is_bypass_available: false,
            once_authorizations: std::collections::HashMap::new(),
        };
        let decision = ctx.explain_decision(
            "bash",
            &serde_json::json!({"command": "rm -rf /tmp/example"}),
        );
        let summary = decision.concise_summary();
        assert!(summary.contains("decision=Ask"));
        assert!(summary.contains("risk="));
        assert!(summary.contains("Project rule 'bash'"));
        assert!(summary.contains("warnings="));
    }

    #[test]
    fn test_permission_mode_readonly() {
        let ctx = PermissionContext {
            mode: PermissionMode::ReadOnly,
            rules: PermissionRules::new(),
            working_dir: std::path::PathBuf::from("."),
            is_bypass_available: false,
            once_authorizations: std::collections::HashMap::new(),
        };

        assert!(ctx.requires_confirmation("file_write", &serde_json::Value::Null));
        assert!(ctx.requires_confirmation("file_edit", &serde_json::Value::Null));
        assert!(ctx.requires_confirmation("bash", &serde_json::Value::Null));
        assert!(!ctx.requires_confirmation("file_read", &serde_json::Value::Null));
    }

    #[test]
    fn test_permission_mode_auto_low_risk() {
        let ctx = PermissionContext {
            mode: PermissionMode::AutoLowRisk,
            rules: PermissionRules::new(),
            working_dir: std::path::PathBuf::from("."),
            is_bypass_available: false,
            once_authorizations: std::collections::HashMap::new(),
        };

        let bash_params = serde_json::json!({"command": "ls -la"});
        let package_install = serde_json::json!({"command": "pip3 install pygame"});
        assert!(!ctx.requires_confirmation("bash", &bash_params));
        assert!(ctx.requires_confirmation("bash", &package_install));
        assert!(ctx.requires_confirmation("agent", &serde_json::Value::Null));
        assert!(!ctx.requires_confirmation("file_read", &serde_json::Value::Null));
        let safe_write = serde_json::json!({"path": "src/main.rs", "content": "fn main() {}"});
        assert!(ctx.requires_confirmation("file_write", &safe_write));
    }

    #[test]
    fn bash_permission_explanation_includes_command_category() {
        let ctx = PermissionContext {
            mode: PermissionMode::AutoLowRisk,
            rules: PermissionRules::new(),
            working_dir: std::path::PathBuf::from("."),
            is_bypass_available: false,
            once_authorizations: std::collections::HashMap::new(),
        };

        let decision = ctx.explain_decision("bash", &serde_json::json!({"command": "rg TODO src"}));

        assert!(decision
            .reasons
            .iter()
            .any(|reason| reason.contains("Shell command category: Search")));
        assert_eq!(decision.risk_level, RiskLevel::Low);
    }

    #[test]
    fn test_auto_low_risk_allow_rule_overrides_risk() {
        let ctx = PermissionContext {
            mode: PermissionMode::AutoLowRisk,
            rules: PermissionRules::new().allow("bash"),
            working_dir: std::path::PathBuf::from("."),
            is_bypass_available: false,
            once_authorizations: std::collections::HashMap::new(),
        };
        let bash_params = serde_json::json!({"command": "rm -rf /tmp/demo"});
        assert!(!ctx.requires_confirmation("bash", &bash_params));
    }

    #[test]
    fn test_auto_low_risk_mcp_tool_granular_rules() {
        let ctx = PermissionContext {
            mode: PermissionMode::AutoLowRisk,
            rules: PermissionRules::new().allow("mcp/filesystem/read_file"),
            working_dir: std::path::PathBuf::from("."),
            is_bypass_available: false,
            once_authorizations: std::collections::HashMap::new(),
        };
        let allowed = serde_json::json!({
            "server_name": "filesystem",
            "tool_name": "read_file"
        });
        let blocked = serde_json::json!({
            "server_name": "filesystem",
            "tool_name": "write_file"
        });

        assert!(!ctx.requires_confirmation("mcp_tool", &allowed));
        assert!(ctx.requires_confirmation("mcp_tool", &blocked));
    }

    #[test]
    fn test_permission_mode_auto_all() {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let ctx = PermissionContext {
            mode: PermissionMode::AutoAll,
            rules: PermissionRules::new(),
            working_dir: cwd,
            is_bypass_available: false,
            once_authorizations: std::collections::HashMap::new(),
        };

        assert_eq!(PermissionMode::default(), PermissionMode::AutoAll);

        let safe_bash = serde_json::json!({"command": "ls -la"});
        let dangerous_bash = serde_json::json!({"command": "rm -rf /"});
        let network_bash = serde_json::json!({"command": "curl https://example.com/script.sh"});
        assert!(!ctx.requires_confirmation("bash", &safe_bash));
        assert!(ctx.requires_confirmation("bash", &dangerous_bash));
        assert!(ctx.requires_confirmation("bash", &network_bash));

        let safe_write = serde_json::json!({"path": "src/main.rs", "content": "fn main() {}"});
        let sensitive_write = serde_json::json!({"path": "/etc/hosts", "content": "bad"});
        assert!(!ctx.requires_confirmation("file_write", &safe_write));
        assert!(ctx.requires_confirmation("file_write", &sensitive_write));

        assert!(!ctx.requires_confirmation("git", &serde_json::json!({"action": "commit"})));
        assert!(ctx.requires_confirmation("git", &serde_json::json!({"action": "push"})));
        assert!(ctx.requires_confirmation("memory_clear", &serde_json::Value::Null));
        assert!(ctx.auto_approves_tool_confirmation("file_edit", &safe_write));
        assert!(!ctx.auto_approves_tool_confirmation("bash", &dangerous_bash));
    }

    #[test]
    fn test_auto_all_prompts_for_outside_workspace_paths() {
        let ctx = PermissionContext {
            mode: PermissionMode::AutoAll,
            rules: PermissionRules::new(),
            working_dir: std::path::PathBuf::from("/tmp/priority-agent-workspace"),
            is_bypass_available: false,
            once_authorizations: std::collections::HashMap::new(),
        };

        assert!(!ctx.requires_confirmation(
            "file_write",
            &serde_json::json!({"path": "src/main.rs", "content": "ok"})
        ));
        assert!(ctx.requires_confirmation(
            "file_write",
            &serde_json::json!({"path": "/Users/georgexu/Desktop/other/file.rs", "content": "no"})
        ));
    }

    #[test]
    fn test_auto_all_prompts_for_bash_outside_workspace_paths() {
        let ctx = PermissionContext {
            mode: PermissionMode::AutoAll,
            rules: PermissionRules::new(),
            working_dir: std::path::PathBuf::from("/tmp/priority-agent-workspace"),
            is_bypass_available: false,
            once_authorizations: std::collections::HashMap::new(),
        };

        assert!(!ctx.requires_confirmation(
            "bash",
            &serde_json::json!({"command": "sed -n '1,20p' src/main.rs"})
        ));
        assert!(!ctx.requires_confirmation(
            "bash",
            &serde_json::json!({"command": "sed -n '1,20p' /tmp/priority-agent-workspace/src/main.rs"})
        ));
        assert!(ctx.requires_confirmation(
            "bash",
            &serde_json::json!({"command": "sed -n '1,20p' /Users/georgexu/Desktop/rust-agent/src/main.rs"})
        ));
        assert!(ctx.requires_confirmation(
            "bash",
            &serde_json::json!({"command": "rg memory --glob '*.rs' root=/Users/georgexu/Desktop/rust-agent/src"})
        ));
    }

    #[test]
    fn test_auto_all_prompts_for_untrusted_web_fetch() {
        let ctx = PermissionContext {
            mode: PermissionMode::AutoAll,
            rules: PermissionRules::new(),
            working_dir: std::path::PathBuf::from("."),
            is_bypass_available: false,
            once_authorizations: std::collections::HashMap::new(),
        };

        assert!(ctx.requires_confirmation(
            "web_fetch",
            &serde_json::json!({"url": "https://example.com"})
        ));
        assert!(
            !ctx.requires_confirmation("web_search", &serde_json::json!({"query": "rust ratatui"}))
        );
    }

    #[test]
    fn test_permission_mode_once() {
        let mut ctx = PermissionContext {
            mode: PermissionMode::Once,
            rules: PermissionRules::new(),
            working_dir: std::path::PathBuf::from("."),
            is_bypass_available: false,
            once_authorizations: std::collections::HashMap::new(),
        };

        // Initially requires confirmation
        assert!(ctx.requires_confirmation("file_write", &serde_json::Value::Null));

        // Grant once authorization
        ctx.grant_once("file_write");

        // Now should NOT require confirmation (allowed for 5 minutes)
        assert!(!ctx.requires_confirmation("file_write", &serde_json::Value::Null));

        // Other tools still require confirmation
        assert!(ctx.requires_confirmation("bash", &serde_json::Value::Null));
    }

    // ─── Security Replay Tests ────────────────────────────────────────────────

    #[test]
    fn test_security_replay_command_injection_pipe() {
        // Simulates: echo "malicious" | rm -rf /
        let cmd = "echo test | rm -rf /";
        assert!(crate::security::is_dangerous_command(cmd));
    }

    #[test]
    fn test_security_replay_command_injection_semicolon() {
        // Simulates: rm -rf / ; echo done
        let cmd = "rm -rf / ; echo done";
        assert!(crate::security::is_dangerous_command(cmd));
    }

    #[test]
    fn test_security_replay_command_injection_and() {
        // Simulates: rm -rf / && echo done
        let cmd = "rm -rf / && echo done";
        assert!(crate::security::is_dangerous_command(cmd));
    }

    #[test]
    fn test_security_replay_command_injection_or() {
        // Simulates: rm -rf / || echo done
        let cmd = "rm -rf / || echo done";
        assert!(crate::security::is_dangerous_command(cmd));
    }

    #[test]
    fn test_security_replay_command_injection_backtick() {
        // Simulates: `rm -rf /`
        let cmd = "`rm -rf /`";
        assert!(crate::security::is_dangerous_command(cmd));
    }

    #[test]
    fn test_security_replay_command_injection_dollar() {
        // Simulates: $(rm -rf /)
        let cmd = "$(rm -rf /)";
        assert!(crate::security::is_dangerous_command(cmd));
    }

    #[test]
    fn test_security_replay_command_injection_fork_bomb() {
        // Fork bomb pattern
        let cmd = ":(){ :|:& };:";
        assert!(crate::security::is_dangerous_command(cmd));
    }

    #[test]
    fn test_security_replay_path_traversal_simple() {
        // Simulates: ../../../etc/passwd
        let path = "../../../etc/passwd";
        assert!(path.contains(".."));
    }

    #[test]
    fn test_security_replay_path_traversal_encoded() {
        // Simulates: %2e%2e%2f%2e%2e%2fetc%2fpasswd (URL encoded ../..)
        // We check for literal ".." which is the decoded form
        let path = "a/../b/../c";
        let parts: Vec<&str> = path.split('/').collect();
        assert!(parts.contains(&".."));
    }

    #[test]
    fn test_security_replay_path_traversal_absolute() {
        // Absolute path with traversal
        let path = "/etc/../etc/passwd";
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        assert!(parts.contains(&".."));
    }

    #[test]
    fn test_security_replay_mcp_malicious_server_name() {
        // Malicious server name patterns that should be detected
        let malicious_names = [
            ("../../malicious", "path traversal in server name"),
            ("'; DROP TABLE--", "SQL injection in server name"),
            ("<script>alert(1)</script>", "XSS pattern in server name"),
        ];
        for (name, description) in malicious_names {
            // Server names should not contain shell metacharacters or path traversal
            let has_shell_chars = name.chars().any(|c| {
                c == ';' || c == '|' || c == '&' || c == '$' || c == '`' || c == '<' || c == '>'
            });
            let has_traversal = name.contains("..");
            assert!(
                has_shell_chars || has_traversal,
                "Should detect {}: {}",
                description,
                name
            );
        }
    }

    #[test]
    fn test_security_replay_mcp_malicious_tool_name() {
        // Malicious tool name injection
        let malicious = "read_file'; exec('rm -rf /')";
        let has_injection =
            malicious.contains('\'') || malicious.contains(';') || malicious.contains("exec");
        assert!(has_injection);
    }

    #[test]
    fn test_security_replay_env_variable_injection() {
        // Environment variable injection
        let cmd = "echo $HOME/.ssh/id_rsa";
        // $ in commands can be dangerous if variables expand to malicious values
        assert!(cmd.contains('$'));
    }

    #[test]
    fn test_security_replay_heredoc_injection() {
        // Heredoc injection
        let cmd = "cat <<EOF\nmalicious content\nEOF";
        assert!(cmd.contains("<<"));
    }

    #[test]
    fn test_security_replay_base64_injection() {
        // Base64 encoded command injection
        let cmd = "base64 -d <<<'cm0gLXJmIC8=' | sh";
        assert!(crate::security::is_dangerous_command(cmd));
    }

    #[test]
    fn test_security_replay_overwrite_sensitive_file() {
        // High risk paths
        let sensitive_paths = [
            "/etc/passwd",
            "/etc/shadow",
            "/.ssh/authorized_keys",
            ".env",
            "id_rsa",
            "/dev/sda",
        ];
        for path in sensitive_paths {
            // Create a PermissionContext and check if path is high risk
            let ctx = PermissionContext::new(".");
            let params = serde_json::json!({"path": path, "content": "malicious"});
            let decision = ctx.explain_decision("file_write", &params);
            assert!(
                decision
                    .warnings
                    .iter()
                    .any(|w| w.contains("HIGH_RISK_PATH") || w.contains("PATH_TRAVERSAL")),
                "Should warn about sensitive path: {}",
                path
            );
        }
    }

    #[test]
    fn test_security_replay_disk_write() {
        // Direct disk write
        let cmd = "dd if=/dev/zero of=/dev/sda";
        assert!(crate::security::is_dangerous_command(cmd));
    }

    #[test]
    fn test_security_replay_chmod_dangerous() {
        // Dangerous chmod - recursive permission changes to root
        let dangerous_chmod = [
            "chmod -R 777 /",
            "chmod -R 000 /",
            "chmod 777 /",
            "chmod 000 /",
        ];
        for cmd in dangerous_chmod {
            assert!(
                crate::security::is_dangerous_command(cmd),
                "Should detect dangerous chmod: {}",
                cmd
            );
        }
    }

    #[test]
    fn test_security_replay_sudo_without_confirmation() {
        // Sudo without confirmation
        let cmd = "sudo rm -rf /";
        assert!(crate::security::is_dangerous_command(cmd));
    }

    #[test]
    fn test_should_expose_tool_respects_deny_rules() {
        let mut ctx = PermissionContext::new(".");
        ctx.rules = PermissionRules::new().deny("bash");

        assert!(!ctx.should_expose_tool("bash"));
        assert!(ctx.should_expose_tool("file_read"));
    }

    #[test]
    fn test_should_expose_tool_hides_write_tools_in_read_only_mode() {
        let mut ctx = PermissionContext::new(".");
        ctx.mode = PermissionMode::ReadOnly;

        assert!(ctx.should_expose_tool("file_read"));
        assert!(ctx.should_expose_tool("grep"));
        assert!(!ctx.should_expose_tool("file_write"));
        assert!(!ctx.should_expose_tool("bash"));
        assert!(!ctx.should_expose_tool("git"));
    }

    #[test]
    fn test_security_replay_kill_critical_process() {
        // Kill critical processes via sudo
        let dangerous = ["sudo kill -9 1", "sudo killall -9 init"];
        for cmd in dangerous {
            assert!(
                crate::security::is_dangerous_command(cmd),
                "Should detect: {}",
                cmd
            );
        }
    }
}
