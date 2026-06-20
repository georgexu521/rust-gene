use crate::tools::ToolContext;
use crate::{
    lab::model::{
        LabProviderCertificationKind, LabProviderCertificationOutcome,
        LabProviderCertificationRecord,
    },
    lab::store::LabStore,
};

const FORCE_UNCERTIFIED_ENV: &str = "PRIORITY_AGENT_LAB_ALLOW_UNCERTIFIED_GRADUATE_PROVIDER";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabGraduateProviderCertification {
    Certified,
    KnownUnsupported,
    Unverified,
}

impl LabGraduateProviderCertification {
    pub fn allows_graduate_execution(self) -> bool {
        !matches!(self, Self::KnownUnsupported)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Certified => "certified",
            Self::KnownUnsupported => "known_unsupported",
            Self::Unverified => "unverified",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabProviderCertificationReport {
    pub provider_id: String,
    pub model: String,
    pub graduate_certification: LabGraduateProviderCertification,
    pub graduate_execution_allowed: bool,
    pub override_enabled: bool,
    pub latest_control_plane_record: Option<LabProviderCertificationRecord>,
    pub latest_graduate_record: Option<LabProviderCertificationRecord>,
    pub control_plane_command: String,
    pub graduate_command: String,
    pub recommendation: String,
}

pub fn graduate_provider_certification(
    provider_id: Option<&str>,
    model: &str,
) -> LabGraduateProviderCertification {
    let provider = provider_id.unwrap_or_default().trim().to_ascii_lowercase();
    let model = model.trim().to_ascii_lowercase();
    let is_deepseek = provider == "deepseek" || model.contains("deepseek");
    if is_deepseek && model.contains("v4-flash") {
        return LabGraduateProviderCertification::KnownUnsupported;
    }
    LabGraduateProviderCertification::Unverified
}

pub fn validate_graduate_provider_for_execution(context: &ToolContext) -> Result<(), String> {
    if allow_uncertified_override() {
        return Ok(());
    }
    let provider_id = provider_id_from_context(context);
    let certification = graduate_provider_certification_with_records(context);
    if certification.allows_graduate_execution() {
        return Ok(());
    }
    Err(format!(
        "Lab graduate provider is not certified for tool-backed code-writing: provider={} model={}. \
         DeepSeek v4 flash generic subagent runs can use tools with tool_choice=auto, but this provider has not passed \
         the formal Lab graduate certification path: isolated graduate task execution, runtime-observed file changes, \
         required validation, and worktree review/merge/cleanup proof. Graduate execution remains blocked before spending \
         another Lab agent run. Use `scripts/lab-live-validation.sh --live-control-plane` for DeepSeek control-plane validation, \
         choose a graduate-certified provider, or set {FORCE_UNCERTIFIED_ENV}=1 for an explicit experimental run.",
        provider_id.unwrap_or("unknown"),
        if context.model.trim().is_empty() {
            "unknown"
        } else {
            context.model.trim()
        }
    ))
}

pub fn provider_certification_report(context: &ToolContext) -> LabProviderCertificationReport {
    let provider_id = provider_id_from_context(context)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_string();
    let model = if context.model.trim().is_empty() {
        "unknown".to_string()
    } else {
        context.model.trim().to_string()
    };
    let latest_control_plane_record = latest_provider_record(
        context,
        &provider_id,
        &model,
        LabProviderCertificationKind::ControlPlane,
    );
    let latest_graduate_record = latest_provider_record(
        context,
        &provider_id,
        &model,
        LabProviderCertificationKind::Graduate,
    );
    let graduate_certification = graduate_provider_certification_from_record(
        Some(&provider_id),
        &model,
        latest_graduate_record.as_ref(),
    );
    let override_enabled = allow_uncertified_override();
    let graduate_execution_allowed =
        graduate_certification.allows_graduate_execution() || override_enabled;
    let recommendation = match (graduate_certification, override_enabled) {
        (LabGraduateProviderCertification::KnownUnsupported, false) => {
            "Use --live-control-plane for this provider, choose a graduate-certified provider for code-writing, or set PRIORITY_AGENT_LAB_ALLOW_UNCERTIFIED_GRADUATE_PROVIDER=1 only for an explicit experiment."
                .to_string()
        }
        (LabGraduateProviderCertification::KnownUnsupported, true) => {
            "Override is enabled; graduate execution can run experimentally, but results still require runtime-observed file changes and validation."
                .to_string()
        }
        (LabGraduateProviderCertification::Unverified, _) => {
            "Run --live-graduate to certify tool-backed graduate code-writing before treating this provider as reliable."
                .to_string()
        }
        (LabGraduateProviderCertification::Certified, _) => {
            "Provider is certified for Lab graduate code-writing under the current certification table."
                .to_string()
        }
    };
    LabProviderCertificationReport {
        provider_id,
        model,
        graduate_certification,
        graduate_execution_allowed,
        override_enabled,
        latest_control_plane_record,
        latest_graduate_record,
        control_plane_command: "scripts/lab-live-validation.sh --live-control-plane".to_string(),
        graduate_command: "scripts/lab-live-validation.sh --live-graduate".to_string(),
        recommendation,
    }
}

fn graduate_provider_certification_with_records(
    context: &ToolContext,
) -> LabGraduateProviderCertification {
    let provider_id = provider_id_from_context(context);
    let model = context.model.trim();
    let latest_graduate_record = provider_id.and_then(|provider_id| {
        latest_provider_record(
            context,
            provider_id,
            model,
            LabProviderCertificationKind::Graduate,
        )
    });
    graduate_provider_certification_from_record(provider_id, model, latest_graduate_record.as_ref())
}

fn graduate_provider_certification_from_record(
    provider_id: Option<&str>,
    model: &str,
    latest_graduate_record: Option<&LabProviderCertificationRecord>,
) -> LabGraduateProviderCertification {
    if latest_graduate_record
        .filter(|record| record.outcome == LabProviderCertificationOutcome::Passed)
        .is_some()
    {
        return LabGraduateProviderCertification::Certified;
    }
    graduate_provider_certification(provider_id, model)
}

fn latest_provider_record(
    context: &ToolContext,
    provider_id: &str,
    model: &str,
    kind: LabProviderCertificationKind,
) -> Option<LabProviderCertificationRecord> {
    let model = model.trim();
    if provider_id.trim().is_empty() || model.is_empty() {
        return None;
    }
    LabStore::for_project(&context.working_dir)
        .latest_provider_certification(provider_id, model, kind)
        .ok()
        .flatten()
}

fn provider_id_from_context(context: &ToolContext) -> Option<&str> {
    context
        .metadata
        .get("provider_id")
        .map(String::as_str)
        .or_else(|| context.metadata.get("provider").map(String::as_str))
}

fn allow_uncertified_override() -> bool {
    std::env::var(FORCE_UNCERTIFIED_ENV)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolContext;

    #[test]
    fn deepseek_v4_flash_is_known_unsupported_for_lab_graduate_code_writing() {
        assert_eq!(
            graduate_provider_certification(Some("deepseek"), "deepseek-v4-flash"),
            LabGraduateProviderCertification::KnownUnsupported
        );
    }

    #[test]
    fn unknown_provider_is_unverified_not_blocked_by_certification_table() {
        assert_eq!(
            graduate_provider_certification(Some("future-provider"), "future-model"),
            LabGraduateProviderCertification::Unverified
        );
        assert!(
            graduate_provider_certification(Some("future-provider"), "future-model")
                .allows_graduate_execution()
        );
    }

    #[test]
    fn execution_gate_blocks_deepseek_v4_flash_without_override() {
        let mut context = ToolContext::new(".", "lab-test").with_model("deepseek-v4-flash");
        context
            .metadata
            .insert("provider_id".to_string(), "deepseek".to_string());

        let err = validate_graduate_provider_for_execution(&context)
            .unwrap_err()
            .to_string();

        assert!(err.contains("not certified"));
        assert!(err.contains("generic subagent"));
        assert!(err.contains("formal Lab graduate certification"));
    }

    #[test]
    fn local_graduate_pass_record_certifies_provider_for_execution() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        store
            .record_provider_certification(
                "deepseek",
                "deepseek-v4-flash",
                LabProviderCertificationKind::Graduate,
                LabProviderCertificationOutcome::Passed,
                "target/lab-live-validation/pass/report.md",
                "full live graduate validation passed",
            )
            .unwrap();
        let mut context = ToolContext::new(temp.path(), "lab-test").with_model("deepseek-v4-flash");
        context
            .metadata
            .insert("provider_id".to_string(), "deepseek".to_string());

        let report = provider_certification_report(&context);

        assert_eq!(
            report.graduate_certification,
            LabGraduateProviderCertification::Certified
        );
        assert!(report.graduate_execution_allowed);
        validate_graduate_provider_for_execution(&context).unwrap();
    }

    #[test]
    fn local_graduate_failed_record_does_not_certify_provider() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        store
            .record_provider_certification(
                "deepseek",
                "deepseek-v4-flash",
                LabProviderCertificationKind::Graduate,
                LabProviderCertificationOutcome::Failed,
                "target/lab-live-validation/fail/report.md",
                "full live graduate validation failed",
            )
            .unwrap();
        let mut context = ToolContext::new(temp.path(), "lab-test").with_model("deepseek-v4-flash");
        context
            .metadata
            .insert("provider_id".to_string(), "deepseek".to_string());

        let report = provider_certification_report(&context);

        assert_eq!(
            report.graduate_certification,
            LabGraduateProviderCertification::KnownUnsupported
        );
        assert!(!report.graduate_execution_allowed);
        assert!(validate_graduate_provider_for_execution(&context).is_err());
    }
}
