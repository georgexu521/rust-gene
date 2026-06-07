//! 权限系统
//!
//! 细粒度的工具权限控制
//! 支持通配符匹配、规则源分类

pub mod llm_classifier;

use crate::engine::action_policy::{ActionSideEffectProfile, WorkspacePathClass};
use crate::services::api::ToolCall;
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

pub(crate) fn permission_match_keys(tool_name: &str, params: &serde_json::Value) -> Vec<String> {
    let mut keys = Vec::new();
    if is_edit_family_tool(tool_name, params) {
        keys.push("edit".to_string());
    }
    if tool_name == "mcp_tool" {
        let server = params["server_name"].as_str().unwrap_or("");
        let tool = params["tool_name"].as_str().unwrap_or("");
        if !server.is_empty() && !tool.is_empty() {
            keys.push(format!("mcp/{}/{}", server, tool));
        }
    }
    if tool_name == "bash" {
        if let Some(command) = params["command"]
            .as_str()
            .or_else(|| params["cmd"].as_str())
            .map(|command| {
                crate::tools::bash_tool::command_classifier::classify_command(command)
                    .normalized_command
            })
            .filter(|command| !command.trim().is_empty())
        {
            keys.push(format!("bash:{}", command.trim()));
        }
    }
    if !keys.iter().any(|key| key == tool_name) {
        keys.push(tool_name.to_string());
    }
    keys
}

fn is_edit_family_tool(tool_name: &str, params: &serde_json::Value) -> bool {
    matches!(tool_name, "file_write" | "file_edit" | "file_patch")
        || (tool_name == "format" && params["action"].as_str().unwrap_or("format") == "format")
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
        let match_keys = permission_match_keys(tool_name, params);
        let matching_rules = self.matching_rules_for_keys(&match_keys);
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
                // 只读模式下，任何写入/修改/执行操作都需要确认
                matches!(
                    tool_name,
                    "file_write"
                        | "file_edit"
                        | "file_patch"
                        | "bash"
                        | "powershell"
                        | "mcp_tool"
                        | "format"
                        | "notebook"
                        | "skill_manage"
                        | "install_dependencies"
                        | "remote_dev"
                        | "worktree"
                        | "plugin_manage"
                        | "plugin_tool"
                        | "memory_save"
                        | "memory_clear"
                        | "memory_tool"
                        | "rewind"
                        | "rewind_tool"
                        | "start_dev_server"
                        | "send_message"
                        | "agent"
                        | "agent_tool"
                )
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
                    self.rule_decision_for_keys(&match_keys),
                    PermissionDecision::Ask
                )
            }
        }
    }

    fn matching_rules_for_keys(&self, keys: &[String]) -> Vec<(PermissionDecision, &SourcedRule)> {
        let mut matches = Vec::new();
        for key in keys {
            matches.extend(self.rules.get_matching_rules(key));
        }
        matches
    }

    fn rule_decision_for_keys(&self, keys: &[String]) -> PermissionDecision {
        let matching_rules = self.matching_rules_for_keys(keys);
        if matching_rules
            .iter()
            .any(|(decision, _)| matches!(decision, PermissionDecision::Deny))
        {
            return PermissionDecision::Deny;
        }
        if matching_rules
            .iter()
            .any(|(decision, _)| matches!(decision, PermissionDecision::Allow))
        {
            return PermissionDecision::Allow;
        }
        if matching_rules
            .iter()
            .any(|(decision, _)| matches!(decision, PermissionDecision::Ask))
        {
            return PermissionDecision::Ask;
        }
        PermissionDecision::Ask
    }

    /// 是否应该把工具暴露给模型。
    ///
    /// 执行层仍会做最终授权；这个方法只用于请求前收窄工具池，减少
    /// 被明确拒绝或只读模式下不可用的工具被模型反复调用。
    pub fn should_expose_tool(&self, tool_name: &str) -> bool {
        let match_keys = permission_match_keys(tool_name, &serde_json::Value::Null);
        if matches!(
            self.rule_decision_for_keys(&match_keys),
            PermissionDecision::Deny
        ) {
            return false;
        }

        if self.mode == PermissionMode::ReadOnly {
            return matches!(
                tool_name,
                "file_read"
                    | "glob"
                    | "grep"
                    | "bash_output"
                    | "bash_tasks"
                    | "project_list"
                    | "memory_load"
                    | "skills_list"
                    | "skill_list"
                    | "skill_view"
                    | "web_search"
                    | "web_fetch"
                    | "list_mcp_resources"
                    | "read_mcp_resource"
                    | "cost"
                    | "context"
                    | "context_visualization"
                    | "diff"
                    | "symbol_query"
                    | "git_status"
                    | "git_diff"
                    | "datetime"
                    | "json_query"
                    | "calculate"
                    | "tool_search"
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
        let boundary = self.boundary_profile(tool_name, params);
        match tool_name {
            "file_read" | "glob" | "grep" | "bash_output" | "bash_tasks" | "project_list"
            | "memory_load" | "run_tests" | "git_status" | "git_diff" | "list_mcp_resources"
            | "read_mcp_resource" => RiskLevel::Low,
            "memory_clear" | "mcp" | "mcp_auth" | "install_dependencies" | "rewind" => {
                RiskLevel::High
            }
            "agent" => RiskLevel::Medium,
            "start_dev_server" => RiskLevel::Medium,
            "bash_cancel" => RiskLevel::Medium,
            "powershell" => {
                if params["action"].as_str() == Some("execute") {
                    RiskLevel::High
                } else {
                    RiskLevel::Low
                }
            }
            "bash" => {
                let cmd = params["command"]
                    .as_str()
                    .or_else(|| params["cmd"].as_str())
                    .unwrap_or_default();
                let classification =
                    crate::tools::bash_tool::command_classifier::classify_command(cmd);
                if Self::is_high_risk_command(cmd)
                    || classification.network_access
                    || classification.command_plan.fail_closed
                    || Self::boundary_has_high_risk_path(&boundary)
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
                        | crate::tools::bash_tool::command_classifier::ShellCommandCategory::Interactive
                        | crate::tools::bash_tool::command_classifier::ShellCommandCategory::FileMutation
                        | crate::tools::bash_tool::command_classifier::ShellCommandCategory::GitMutation
                        | crate::tools::bash_tool::command_classifier::ShellCommandCategory::Unknown => {
                            RiskLevel::Medium
                        }
                    }
                }
            }
            "file_write" | "file_edit" | "file_patch" => {
                if Self::boundary_has_high_risk_path(&boundary)
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
            "format" => match params["action"].as_str() {
                Some("format") => RiskLevel::High,
                _ => RiskLevel::Low,
            },
            "notebook" => match params["action"].as_str() {
                Some("edit_cell" | "insert_cell" | "delete_cell") => RiskLevel::High,
                _ => RiskLevel::Low,
            },
            "config" => match params["action"].as_str() {
                Some("set") => RiskLevel::High,
                _ => RiskLevel::Low,
            },
            "skill_manage" => match params["action"].as_str() {
                Some("create" | "patch" | "delete") => RiskLevel::High,
                _ => RiskLevel::Low,
            },
            "task_create" | "task_update" | "task_stop" | "todo_write" => RiskLevel::Medium,
            "task_output" => match params["action"].as_str() {
                Some("append") => RiskLevel::Medium,
                _ => RiskLevel::Low,
            },
            "cron" => match params["action"].as_str() {
                Some("create" | "remove" | "run") => RiskLevel::High,
                Some("pause" | "resume") => RiskLevel::Medium,
                _ => RiskLevel::Low,
            },
            "swarm" => match params["action"].as_str() {
                Some("spawn" | "execute" | "clear") => RiskLevel::Medium,
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
            "plugin" | "plugin_manage" | "plugin_runtime" => match params["action"].as_str() {
                Some("run") => RiskLevel::High,
                _ => RiskLevel::High,
            },
            "remote_trigger" => match params["action"].as_str() {
                Some("run") => RiskLevel::High,
                Some("create" | "sync") => RiskLevel::Medium,
                _ => RiskLevel::Low,
            },
            "remote_dev" => match params["action"].as_str() {
                Some("exec") => RiskLevel::High,
                Some("create" | "remove") => RiskLevel::Medium,
                _ => RiskLevel::Low,
            },
            "desktop" | "browser" => RiskLevel::Medium,
            "send_message" | "share" | "copy" => RiskLevel::Medium,
            "mcp_tool" => RiskLevel::High,
            _ => RiskLevel::Low,
        }
    }

    fn requires_safety_confirmation(&self, tool_name: &str, params: &serde_json::Value) -> bool {
        self.risk_level(tool_name, params) >= RiskLevel::High
    }

    fn boundary_profile(
        &self,
        tool_name: &str,
        params: &serde_json::Value,
    ) -> ActionSideEffectProfile {
        let tool_call = ToolCall {
            id: "permission_review".to_string(),
            name: tool_name.to_string(),
            arguments: params.clone(),
        };
        ActionSideEffectProfile::from_tool_call(&tool_call, None, &self.working_dir)
    }

    fn boundary_has_high_risk_path(boundary: &ActionSideEffectProfile) -> bool {
        boundary.has_external_or_sensitive_path()
            || boundary.paths.iter().any(|path| {
                matches!(
                    path.class,
                    WorkspacePathClass::Dependency | WorkspacePathClass::Generated
                )
            })
    }

    fn push_warning(warnings: &mut Vec<String>, warning: String) {
        if !warnings.iter().any(|existing| existing == &warning) {
            warnings.push(warning);
        }
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
        let match_keys = permission_match_keys(tool_name, params);
        let base_decision = self.rule_decision_for_keys(&match_keys);
        let matching_rules = self.matching_rules_for_keys(&match_keys);
        let risk = self.risk_level(tool_name, params);
        let confidence = self.calculate_confidence(tool_name, params, &matching_rules);
        let boundary = self.boundary_profile(tool_name, params);

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
        reasons.push(format!("Boundary profile: {}", boundary.summary));
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
            if classification.network_access {
                warnings.push("NETWORK_ACCESS: shell command may access the network".to_string());
            }
            if classification.command_plan.fail_closed {
                warnings.push(format!(
                    "SHELL_STRUCTURE_REVIEW: command requires explicit review ({})",
                    classification.command_plan.fail_closed_reasons.join(", ")
                ));
            }
            if classification.risky_shell_wrapper {
                warnings.push(
                    "RISKY_SHELL_WRAPPER: shell wrapper contains risky or compound behavior"
                        .to_string(),
                );
            }
            if classification.expected_silent_output {
                warnings.push(
                    "EXPECTED_SILENT_OUTPUT: success may produce little or no terminal output"
                        .to_string(),
                );
            }
            if crate::security::is_dangerous_command(cmd) {
                warnings
                    .push("COMMAND_INJECTION: potentially malicious pattern detected".to_string());
            }
        }
        if tool_name == "file_write" || tool_name == "file_edit" || tool_name == "file_patch" {
            let path = params["path"].as_str().unwrap_or_default();
            // Check for path traversal
            if path.contains("..") {
                warnings.push("PATH_TRAVERSAL: parent directory reference detected".to_string());
            }
        }
        for warning in boundary.boundary_warnings() {
            Self::push_warning(&mut warnings, warning);
        }
        if tool_name == "web_fetch" {
            let url = params["url"].as_str().unwrap_or_default();
            if !self.url_is_trusted(url) {
                warnings.push("UNTRUSTED_NETWORK: URL host is not in trusted domains".to_string());
            }
        }
        if tool_name == "install_dependencies" {
            let manager = params["manager"].as_str().unwrap_or("unknown");
            reasons.push(format!("Package manager install: {}", manager));
            warnings.push(
                "PACKAGE_INSTALL: dependency installation can download and execute external content"
                    .to_string(),
            );
        }
        if tool_name == "start_dev_server" {
            let command = params["command"].as_str().unwrap_or_default();
            reasons.push(format!("Local dev server command: {}", command));
            warnings.push(
                "LOCALHOST_SERVER: starts a long-running local process that may expose a port"
                    .to_string(),
            );
        }
        if tool_name == "mcp_auth" {
            warnings.push(
                "AUTH_FLOW: MCP authentication can grant external service access".to_string(),
            );
        }
        if matches!(tool_name, "plugin" | "plugin_manage" | "plugin_runtime") {
            warnings.push(
                "PLUGIN_SIDE_EFFECT: plugin actions can mutate local or external runtime state"
                    .to_string(),
            );
        }
        if tool_name == "format" && params["action"].as_str() == Some("format") {
            warnings.push(
                "FORMAT_MUTATION: formatter can rewrite files and should have checkpoint context"
                    .to_string(),
            );
        }
        if tool_name == "config" && params["action"].as_str() == Some("set") {
            warnings.push(
                "CONFIG_MUTATION: config set changes persistent agent configuration".to_string(),
            );
        }
        if tool_name == "skill_manage"
            && matches!(
                params["action"].as_str(),
                Some("create" | "patch" | "delete")
            )
        {
            warnings.push("SKILL_MUTATION: skill changes alter future agent behavior".to_string());
        }
        if tool_name == "notebook"
            && matches!(
                params["action"].as_str(),
                Some("edit_cell" | "insert_cell" | "delete_cell")
            )
        {
            warnings.push(
                "NOTEBOOK_MUTATION: notebook cell changes can rewrite code or outputs".to_string(),
            );
        }
        if tool_name == "rewind" {
            warnings.push(
                "REWIND_MUTATION: rewind restores previous file state and changes the workspace"
                    .to_string(),
            );
        }
        if tool_name == "remote_trigger" {
            let facts =
                crate::tools::remote_trigger_tool::remote_trigger_permission_metadata(params);
            if let Some(summary) = facts["permission_summary"].as_str() {
                reasons.push(format!("Remote bridge facts: {}", summary));
            }
            match facts["remote_effect"].as_str().unwrap_or_default() {
                "remote_execution" => warnings.push(
                    "REMOTE_EXECUTION: bridge trigger can execute work outside the local process"
                        .to_string(),
                ),
                "remote_read_and_local_cursor_write" => warnings.push(
                    "LOCAL_CURSOR_WRITE: sync reads remote state and updates the local replay cursor"
                        .to_string(),
                ),
                "remote_session_create" => warnings.push(
                    "REMOTE_SESSION_CREATE: prompt content will be sent to the configured bridge"
                        .to_string(),
                ),
                _ => {}
            }
            if !facts["bridge_url_configured"].as_bool().unwrap_or(false) {
                warnings.push(
                    "BRIDGE_NOT_CONFIGURED: bridge URL is missing, execution will fail before contacting a remote"
                        .to_string(),
                );
            }
        }
        if tool_name == "remote_dev" {
            let facts = crate::tools::remote_dev_tool::remote_dev_permission_metadata(params);
            if let Some(summary) = facts["permission_summary"].as_str() {
                reasons.push(format!("Remote dev facts: {}", summary));
            }
            match facts["remote_effect"].as_str().unwrap_or_default() {
                "remote_ssh_execution" => warnings.push(
                    "REMOTE_COMMAND: SSH command can mutate remote files/processes outside the local workspace"
                        .to_string(),
                ),
                "local_remote_session_create" => warnings.push(
                    "REMOTE_SESSION_CONFIG: stores host/user/key metadata for future remote use"
                        .to_string(),
                ),
                "local_remote_session_delete" => warnings.push(
                    "REMOTE_SESSION_DELETE: removes a saved remote session configuration"
                        .to_string(),
                ),
                _ => {}
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
            rule_views: Vec::new(),
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
    /// Product-facing rule views for display.
    pub rule_views: Vec<PermissionRuleView>,
}

/// Product-facing view of a single permission rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRuleView {
    pub scope: String,
    pub matcher_key: String,
    pub effect: String,
    pub source: String,
    pub expires: Option<String>,
    pub risk_reason: Option<String>,
}

impl PermissionRuleView {
    pub fn from_sourced(rule: &SourcedRule, effect: &str) -> Self {
        let scope = match rule.source {
            RuleSource::Global => "global",
            RuleSource::Project => "project",
            RuleSource::User => "session",
            RuleSource::System => "system",
        };
        Self {
            scope: scope.to_string(),
            matcher_key: rule.pattern.clone(),
            effect: effect.to_string(),
            source: format!("{:?}", rule.source),
            expires: None,
            risk_reason: None,
        }
    }

    pub fn format(&self) -> String {
        let mut parts = vec![
            format!("  scope: {}", self.scope),
            format!("  matcher: {}", self.matcher_key),
            format!("  effect: {}", self.effect),
            format!("  source: {}", self.source),
        ];
        if let Some(exp) = &self.expires {
            parts.push(format!("  expires: {exp}"));
        }
        if let Some(reason) = &self.risk_reason {
            parts.push(format!("  risk: {reason}"));
        }
        parts.join("\n")
    }
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
        if !self.matched_rules.is_empty() {
            lines.push("\nMatched Rules:".to_string());
            for (decision, rule) in &self.matched_rules {
                lines.push(format!(
                    "  {:?} | pattern=\"{}\" | source={:?}",
                    decision, rule.pattern, rule.source
                ));
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
mod tests;
