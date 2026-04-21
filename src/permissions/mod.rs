//! 权限系统
//!
//! 细粒度的工具权限控制
//! 支持通配符匹配、规则源分类

use serde::{Deserialize, Serialize};

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
    #[default]
    Default,
    /// 自动允许低风险操作
    AutoLowRisk,
    /// 自动允许所有（危险）
    AutoAll,
    /// 只读模式
    ReadOnly,
    /// 一次性授权模式 - 允许一次后自动拒绝
    Once,
}

/// 风险级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RiskLevel {
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
            PermissionMode::AutoAll => false,
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
                Self::risk_level(tool_name, params) >= RiskLevel::Medium
            }
            PermissionMode::Once => {
                // Once 模式：先检查是否已有有效的一次性授权
                if let Some(expired) = self.once_authorizations.get(tool_name) {
                    if expired.elapsed().as_secs() < 300 {
                        // 5分钟内有效，直接拒绝
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
            .map(|exp| exp.elapsed().as_secs() < 300)
            .unwrap_or(false)
    }

    /// 清理过期的一次性授权
    pub fn cleanup_expired_once(&mut self) {
        self.once_authorizations
            .retain(|_, exp| exp.elapsed().as_secs() < 300);
    }

    fn risk_level(tool_name: &str, params: &serde_json::Value) -> RiskLevel {
        match tool_name {
            "file_read" | "glob" | "grep" | "project_list" | "memory_load" => RiskLevel::Low,
            "memory_clear" | "agent" | "mcp" => RiskLevel::High,
            "bash" => {
                let cmd = params["command"]
                    .as_str()
                    .or_else(|| params["cmd"].as_str())
                    .unwrap_or_default()
                    .to_lowercase();
                if Self::is_high_risk_command(&cmd) {
                    RiskLevel::High
                } else {
                    RiskLevel::Medium
                }
            }
            "file_write" | "file_edit" => {
                let path = params["path"].as_str().unwrap_or_default();
                if Self::is_high_risk_path(path) || Self::is_large_content_write(params) {
                    RiskLevel::High
                } else {
                    RiskLevel::Medium
                }
            }
            "mcp_tool" => RiskLevel::High,
            _ => RiskLevel::Low,
        }
    }

    fn is_high_risk_command(cmd: &str) -> bool {
        if cmd.is_empty() {
            return true;
        }
        if crate::tools::bash_tool::is_dangerous_command(cmd) {
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
        assert!(ctx.requires_confirmation("bash", &bash_params));
        assert!(ctx.requires_confirmation("agent", &serde_json::Value::Null));
        assert!(!ctx.requires_confirmation("file_read", &serde_json::Value::Null));
        let safe_write = serde_json::json!({"path": "src/main.rs", "content": "fn main() {}"});
        assert!(ctx.requires_confirmation("file_write", &safe_write));
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
        let ctx = PermissionContext {
            mode: PermissionMode::AutoAll,
            rules: PermissionRules::new(),
            working_dir: std::path::PathBuf::from("."),
            is_bypass_available: false,
            once_authorizations: std::collections::HashMap::new(),
        };

        assert!(!ctx.requires_confirmation("bash", &serde_json::Value::Null));
        assert!(!ctx.requires_confirmation("file_write", &serde_json::Value::Null));
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
}
