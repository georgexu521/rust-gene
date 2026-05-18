use crate::engine::intent_router::{IntentRoute, RiskLevel, WorkflowKind};
use crate::engine::task_context::TaskContextBundle;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RiskSignalLevel {
    Ordinary,
    Elevated,
    High,
}

impl RiskSignalLevel {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Ordinary => "ordinary",
            Self::Elevated => "elevated",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RiskSignalAssessment {
    pub(super) level: RiskSignalLevel,
    pub(super) reasons: Vec<String>,
    pub(super) entry_contract: bool,
}

impl RiskSignalAssessment {
    pub(super) fn contract_reason(&self) -> String {
        if self.reasons.is_empty() {
            return "risk signal requested workflow contract".to_string();
        }
        let mut reason = self.reasons.iter().take(3).cloned().collect::<Vec<_>>();
        if self.reasons.len() > reason.len() {
            reason.push(format!("+{} more", self.reasons.len() - reason.len()));
        }
        reason.join("; ")
    }
}

pub(super) struct RiskSignalInput<'a> {
    pub(super) route: &'a IntentRoute,
    pub(super) task_bundle: &'a TaskContextBundle,
    pub(super) required_validation_commands: &'a [String],
}

pub(super) struct RuntimeRiskSignalInput<'a> {
    pub(super) failed_validation_commands: &'a [String],
    pub(super) failed_tool_evidence: &'a [String],
    pub(super) syntax_error: bool,
}

pub(super) struct RiskSignalController;

impl RiskSignalController {
    pub(super) fn assess_turn_entry(input: RiskSignalInput<'_>) -> RiskSignalAssessment {
        let mut builder = RiskSignalBuilder::default();
        let prompt = input.task_bundle.prompt_preview.to_ascii_lowercase();
        let implementation_intent =
            is_implementation_route(input.route) || has_mutation_intent(&prompt);
        let referenced_paths = referenced_paths(input.task_bundle);
        let all_low_risk_paths = !referenced_paths.is_empty()
            && referenced_paths.iter().all(|path| is_low_risk_path(path));

        if matches!(input.route.risk, RiskLevel::High) {
            builder.high("route risk is high");
        }
        if matches!(input.route.workflow, WorkflowKind::BugFix) && !all_low_risk_paths {
            builder.high("bug-fix workflow");
        }

        if !input.required_validation_commands.is_empty() {
            builder.high("required validation commands present");
        }
        if input.required_validation_commands.len() >= 4 {
            builder.high("complex required-validation surface");
        }
        if input
            .required_validation_commands
            .iter()
            .any(|command| is_broad_validation_command(command))
        {
            builder.high("broad validation command requested");
        }

        if input.task_bundle.acceptance_checks.len() >= 3 {
            builder.high("multiple acceptance checks requested");
        } else if !input.task_bundle.acceptance_checks.is_empty() {
            builder.elevated("acceptance checks requested");
        }
        if implementation_intent && has_acceptance_language(&prompt) {
            builder.high("acceptance assertions requested");
        }

        if !all_low_risk_paths {
            if let Some(path) = referenced_paths
                .iter()
                .find(|path| is_core_runtime_path(path))
            {
                builder.high(format!("core runtime path referenced: {}", path.display()));
            }
            if referenced_paths.len() >= 3 {
                builder.high(format!(
                    "multi-file change surface: {} files",
                    referenced_paths.len()
                ));
            }
            let module_count = module_count(&referenced_paths);
            if module_count >= 2 {
                builder.high(format!(
                    "cross-module change surface: {} modules",
                    module_count
                ));
            }
        }

        if implementation_intent {
            if let Some(keyword) = runtime_keyword(&prompt) {
                builder.high(format!("runtime risk keyword in request: {}", keyword));
            }
            if has_public_api_language(&prompt) {
                builder.high("public API or schema surface requested");
            }
        }

        builder.finish()
    }

    pub(super) fn assess_runtime_failure(
        input: RuntimeRiskSignalInput<'_>,
    ) -> Option<RiskSignalAssessment> {
        let mut builder = RiskSignalBuilder::default();
        if !input.failed_validation_commands.is_empty() {
            builder.high("validation failure observed");
        }
        if !input.failed_tool_evidence.is_empty() {
            builder.high("tool failure observed");
        }
        if input.syntax_error {
            builder.high("syntax error observed");
        }
        let assessment = builder.finish();
        if assessment.level == RiskSignalLevel::Ordinary {
            None
        } else {
            Some(RiskSignalAssessment {
                entry_contract: false,
                ..assessment
            })
        }
    }

    pub(super) fn apply_to_task_bundle(
        assessment: &RiskSignalAssessment,
        task_bundle: &mut TaskContextBundle,
    ) {
        if assessment.level == RiskSignalLevel::Ordinary {
            return;
        }
        task_bundle.add_risk(format!("risk_signal={}", assessment.level.label()));
        for reason in assessment.reasons.iter().take(3) {
            task_bundle.add_risk(format!("risk_signal: {}", reason));
        }
    }
}

#[derive(Default)]
struct RiskSignalBuilder {
    high_reasons: Vec<String>,
    elevated_reasons: Vec<String>,
}

impl RiskSignalBuilder {
    fn high(&mut self, reason: impl Into<String>) {
        push_unique(&mut self.high_reasons, reason.into());
    }

    fn elevated(&mut self, reason: impl Into<String>) {
        push_unique(&mut self.elevated_reasons, reason.into());
    }

    fn finish(self) -> RiskSignalAssessment {
        if !self.high_reasons.is_empty() {
            return RiskSignalAssessment {
                level: RiskSignalLevel::High,
                entry_contract: true,
                reasons: self.high_reasons,
            };
        }
        if !self.elevated_reasons.is_empty() {
            return RiskSignalAssessment {
                level: RiskSignalLevel::Elevated,
                entry_contract: false,
                reasons: self.elevated_reasons,
            };
        }
        RiskSignalAssessment {
            level: RiskSignalLevel::Ordinary,
            entry_contract: false,
            reasons: vec!["ordinary change surface".to_string()],
        }
    }
}

fn push_unique(items: &mut Vec<String>, value: String) {
    if !value.trim().is_empty() && !items.contains(&value) {
        items.push(value);
    }
}

fn is_implementation_route(route: &IntentRoute) -> bool {
    matches!(
        route.workflow,
        WorkflowKind::CodeChange | WorkflowKind::BugFix
    )
}

fn has_mutation_intent(text: &str) -> bool {
    contains_any(
        text,
        &[
            "add",
            "change",
            "edit",
            "fix",
            "implement",
            "remove",
            "refactor",
            "update",
            "wire",
            "修改",
            "修复",
            "实现",
            "新增",
            "删除",
            "重构",
            "接入",
            "改",
            "拆",
            "补",
            "优化",
        ],
    )
}

fn has_acceptance_language(text: &str) -> bool {
    contains_any(
        text,
        &[
            "acceptance",
            "assertion",
            "assertions",
            "must pass",
            "required command",
            "required commands",
            "验收",
            "断言",
            "必须通过",
        ],
    )
}

fn has_public_api_language(text: &str) -> bool {
    contains_any(
        text,
        &[
            "public api",
            "schema",
            "migration",
            "contract",
            "breaking",
            "compatibility",
            "公共 api",
            "公共接口",
            "接口",
            "模式",
            "迁移",
            "兼容",
        ],
    )
}

fn runtime_keyword(text: &str) -> Option<&'static str> {
    [
        "runtime",
        "permission",
        "permissions",
        "provider",
        "memory",
        "tool execution",
        "tool registry",
        "tool contract",
        "git",
        "config",
        "configuration",
        "auth",
        "mcp",
        "权限",
        "供应商",
        "模型提供",
        "记忆",
        "工具执行",
        "配置",
    ]
    .into_iter()
    .find(|keyword| text.contains(keyword))
}

fn is_broad_validation_command(command: &str) -> bool {
    let command = command.to_ascii_lowercase();
    contains_any(
        &command,
        &[
            "cargo test -q",
            "cargo test --all",
            "cargo test --workspace",
            "cargo clippy --all-features",
            "cargo check --features",
            "npm test",
            "pnpm test",
            "pytest",
            "--all-features",
        ],
    )
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn referenced_paths(task_bundle: &TaskContextBundle) -> Vec<PathBuf> {
    let mut paths = task_bundle.relevant_files.to_vec();
    paths.extend(extract_paths_from_text(&task_bundle.prompt_preview));
    paths.sort();
    paths.dedup();
    paths
}

fn extract_paths_from_text(text: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut token = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | '\\') {
            token.push(ch);
        } else {
            maybe_push_path(&mut paths, &token);
            token.clear();
        }
    }
    maybe_push_path(&mut paths, &token);
    paths.sort();
    paths.dedup();
    paths
}

fn maybe_push_path(paths: &mut Vec<PathBuf>, raw: &str) {
    let value = raw
        .trim_matches(|ch: char| {
            matches!(
                ch,
                '.' | ',' | ';' | ':' | ')' | '(' | ']' | '[' | '}' | '{' | '"' | '\''
            )
        })
        .trim();
    if value.is_empty()
        || value.starts_with("http")
        || value == "."
        || value == ".."
        || !is_probable_path(value)
    {
        return;
    }
    paths.push(PathBuf::from(value));
}

fn is_probable_path(value: &str) -> bool {
    if matches!(
        value,
        "Cargo.toml" | "Cargo.lock" | "package.json" | "tsconfig.json"
    ) {
        return true;
    }
    if value.contains('/') || value.contains('\\') {
        return true;
    }
    Path::new(value)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext,
                "rs" | "py"
                    | "ts"
                    | "tsx"
                    | "js"
                    | "jsx"
                    | "json"
                    | "toml"
                    | "yaml"
                    | "yml"
                    | "sh"
                    | "md"
                    | "css"
                    | "scss"
                    | "sql"
                    | "proto"
                    | "graphql"
                    | "lock"
            )
        })
        .unwrap_or(false)
}

fn is_core_runtime_path(path: &Path) -> bool {
    let value = path.to_string_lossy().replace('\\', "/");
    matches!(value.as_str(), "Cargo.toml" | "Cargo.lock")
        || value.starts_with(".github/")
        || value.starts_with("src/engine/conversation_loop/")
        || value.starts_with("src/engine/workflow/")
        || value.starts_with("src/engine/mcp/")
        || value.starts_with("src/tools/")
        || value.starts_with("src/memory/")
        || value.starts_with("src/services/")
        || value.starts_with("src/session_store/")
        || value.starts_with("src/security/")
        || value.starts_with("src/permissions")
        || value.starts_with("src/config")
        || value.starts_with("src/tui/slash_handler/")
        || value.starts_with("scripts/run_live_eval.sh")
        || value.contains("permission")
        || value.contains("provider")
        || value.contains("schema")
        || value.contains("tool_execution")
        || value.contains("workflow_contract")
}

fn is_low_risk_path(path: &Path) -> bool {
    let value = path.to_string_lossy().replace('\\', "/");
    value.starts_with("docs/")
        || value.starts_with("fixtures/")
        || value.contains("/fixtures/")
        || value.starts_with("tests/fixtures/")
        || value.contains("/snapshots/")
        || value.starts_with("assets/")
        || value.starts_with("public/")
        || value.ends_with(".css")
        || value.ends_with(".scss")
        || value.ends_with(".md")
}

fn module_count(paths: &[PathBuf]) -> usize {
    let mut modules = BTreeSet::new();
    for path in paths {
        if let Some(module) = module_key(path) {
            modules.insert(module);
        }
    }
    modules.len()
}

fn module_key(path: &Path) -> Option<String> {
    let parts = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return None;
    }
    if parts[0] == "src" && parts.len() >= 2 {
        return Some(format!("src/{}", parts[1]));
    }
    Some(parts[0].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;

    fn bundle(prompt: &str) -> TaskContextBundle {
        let route = IntentRouter::new().route(prompt);
        TaskContextBundle::new(prompt, ".", route, None)
    }

    #[test]
    fn core_runtime_path_marks_high_risk_entry_contract() {
        let bundle = bundle("修改 src/engine/conversation_loop/workflow_runtime.rs");
        let assessment = RiskSignalController::assess_turn_entry(RiskSignalInput {
            route: &bundle.route,
            task_bundle: &bundle,
            required_validation_commands: &[],
        });

        assert_eq!(assessment.level, RiskSignalLevel::High);
        assert!(assessment.entry_contract);
        assert!(assessment
            .reasons
            .iter()
            .any(|reason| reason.contains("core runtime path")));
    }

    #[test]
    fn multi_module_change_marks_high_risk() {
        let mut bundle = bundle("修改 src/tools/mod.rs 和 src/memory/manager.rs");
        bundle.add_file("src/tools/mod.rs");
        bundle.add_file("src/memory/manager.rs");
        let assessment = RiskSignalController::assess_turn_entry(RiskSignalInput {
            route: &bundle.route,
            task_bundle: &bundle,
            required_validation_commands: &[],
        });

        assert_eq!(assessment.level, RiskSignalLevel::High);
        assert!(assessment
            .reasons
            .iter()
            .any(|reason| reason.contains("cross-module")));
    }

    #[test]
    fn required_validation_marks_high_risk() {
        let bundle = bundle("实现小功能");
        let commands = vec!["cargo test -q".to_string()];
        let assessment = RiskSignalController::assess_turn_entry(RiskSignalInput {
            route: &bundle.route,
            task_bundle: &bundle,
            required_validation_commands: &commands,
        });

        assert_eq!(assessment.level, RiskSignalLevel::High);
        assert!(assessment.entry_contract);
        assert!(assessment
            .reasons
            .contains(&"required validation commands present".to_string()));
    }

    #[test]
    fn ui_copy_fixture_and_style_stay_ordinary() {
        let mut bundle = bundle("调整 fixtures/login.json 和 public/styles.css 的文案");
        bundle.add_file("fixtures/login.json");
        bundle.add_file("public/styles.css");
        let assessment = RiskSignalController::assess_turn_entry(RiskSignalInput {
            route: &bundle.route,
            task_bundle: &bundle,
            required_validation_commands: &[],
        });

        assert_eq!(assessment.level, RiskSignalLevel::Ordinary);
        assert!(!assessment.entry_contract);
    }

    #[test]
    fn runtime_failure_marks_dynamic_high_risk_without_entry_contract() {
        let failed = vec!["cargo test -q".to_string()];
        let assessment = RiskSignalController::assess_runtime_failure(RuntimeRiskSignalInput {
            failed_validation_commands: &failed,
            failed_tool_evidence: &[],
            syntax_error: true,
        })
        .expect("dynamic risk");

        assert_eq!(assessment.level, RiskSignalLevel::High);
        assert!(!assessment.entry_contract);
        assert!(assessment
            .reasons
            .contains(&"validation failure observed".to_string()));
        assert!(assessment
            .reasons
            .contains(&"syntax error observed".to_string()));
    }
}
