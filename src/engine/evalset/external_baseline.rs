use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalReportBundle {
    pub generated_at: String,
    pub sets: usize,
    pub scenarios: usize,
    pub passed: usize,
    pub failed: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<EvalBaselineSummary>,
    pub reports: Vec<EvalReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalBaselineSummary {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    pub scenarios: usize,
    pub passed: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalExternalBaselineSet {
    pub provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default)]
    pub scenarios: Vec<EvalExternalBaselineScenario>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalExternalBaselineScenario {
    pub id: String,
    pub outcome: EvalExternalBaselineOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair_turns: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_passed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_evidence_backed: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalExternalBaselineOutcome {
    Pass,
    Fail,
    Blocked,
    NotRun,
}

impl EvalExternalBaselineOutcome {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Blocked => "blocked",
            Self::NotRun => "not_run",
        }
    }
}

impl EvalReportBundle {
    pub fn from_reports(reports: &[EvalReport]) -> Self {
        Self {
            generated_at: chrono::Utc::now().to_rfc3339(),
            sets: reports.len(),
            scenarios: reports.iter().map(|r| r.total).sum(),
            passed: reports.iter().map(|r| r.passed).sum(),
            failed: reports.iter().map(|r| r.failed).sum(),
            baseline: None,
            reports: reports.to_vec(),
        }
    }
}

pub fn format_reports_json(reports: &[EvalReport]) -> Result<String> {
    serde_json::to_string_pretty(&EvalReportBundle::from_reports(reports))
        .context("failed to serialize eval report bundle")
}

pub fn safe_eval_report_label(label: &str) -> String {
    let safe_label = label
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if safe_label.is_empty() {
        "all".to_string()
    } else {
        safe_label
    }
}

pub fn write_reports_json(
    reports: &[EvalReport],
    dir: impl AsRef<Path>,
    label: &str,
) -> Result<PathBuf> {
    let dir = dir.as_ref();
    fs::create_dir_all(dir)
        .with_context(|| format!("failed to create eval report dir {}", dir.display()))?;
    let safe_label = safe_eval_report_label(label);
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let path = dir.join(format!("eval-{}-{}.json", timestamp, safe_label));
    let json = format_reports_json(reports)?;
    fs::write(&path, json)
        .with_context(|| format!("failed to write eval report {}", path.display()))?;
    Ok(path)
}

pub fn load_external_baseline(path: impl AsRef<Path>) -> Result<EvalExternalBaselineSet> {
    let path = path.as_ref();
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read external baseline {}", path.display()))?;
    serde_yaml::from_str(&content)
        .with_context(|| format!("failed to parse external baseline {}", path.display()))
}

pub fn load_external_baseline_artifact(
    path: impl AsRef<Path>,
    provider: &str,
    model: Option<&str>,
) -> Result<EvalExternalBaselineSet> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).with_context(|| {
        format!(
            "failed to read external baseline artifact {}",
            path.display()
        )
    })?;
    if is_evalset_file(path) {
        if let Ok(mut baseline) = serde_yaml::from_str::<EvalExternalBaselineSet>(&content) {
            if baseline.provider.trim().is_empty() {
                baseline.provider = normalized_external_provider(provider);
            }
            if baseline.model.is_none() {
                baseline.model = normalized_external_model(model);
            }
            if baseline.source.is_none() {
                baseline.source = Some(path.display().to_string());
            }
            return Ok(baseline);
        }
    }

    parse_external_baseline_markdown_artifact(&content, path, provider, model)
}

pub fn external_baseline_template(provider: &str, model: Option<&str>) -> EvalExternalBaselineSet {
    EvalExternalBaselineSet {
        provider: normalized_external_provider(provider),
        generated_at: Some(chrono::Utc::now().to_rfc3339()),
        model: normalized_external_model(model),
        source: Some("TODO: replace with run artifact path or manual baseline notes".to_string()),
        scenarios: crate::engine::scenario_matrix::deterministic_scenarios()
            .iter()
            .map(|scenario| EvalExternalBaselineScenario {
                id: scenario.id.to_string(),
                outcome: EvalExternalBaselineOutcome::NotRun,
                evidence: Some(
                    "TODO: record concrete diff, command, trace, or transcript evidence"
                        .to_string(),
                ),
                notes: Some(scenario.user_task.to_string()),
                tool_calls: None,
                repair_turns: None,
                validation_passed: None,
                final_evidence_backed: None,
            })
            .collect(),
    }
}

pub fn format_external_baseline_template(provider: &str, model: Option<&str>) -> Result<String> {
    serde_yaml::to_string(&external_baseline_template(provider, model))
        .context("failed to serialize external baseline template")
}

pub fn write_external_baseline_template(
    dir: impl AsRef<Path>,
    provider: &str,
    model: Option<&str>,
) -> Result<PathBuf> {
    let dir = dir.as_ref();
    fs::create_dir_all(dir)
        .with_context(|| format!("failed to create external baseline dir {}", dir.display()))?;
    let provider_for_path = if provider.trim().is_empty() {
        "external-agent"
    } else {
        provider
    };
    let safe_provider = safe_eval_report_label(provider_for_path);
    let path = dir.join(format!("baseline-{}.yaml", safe_provider));
    if path.exists() {
        anyhow::bail!(
            "external baseline template already exists at {}; refusing to overwrite",
            path.display()
        );
    }
    let yaml = format_external_baseline_template(provider, model)?;
    fs::write(&path, yaml).with_context(|| {
        format!(
            "failed to write external baseline template {}",
            path.display()
        )
    })?;
    Ok(path)
}

pub fn write_external_baseline_import(
    artifact: impl AsRef<Path>,
    dir: impl AsRef<Path>,
    provider: &str,
    model: Option<&str>,
) -> Result<PathBuf> {
    let artifact = artifact.as_ref();
    let dir = dir.as_ref();
    fs::create_dir_all(dir)
        .with_context(|| format!("failed to create external baseline dir {}", dir.display()))?;
    let baseline = load_external_baseline_artifact(artifact, provider, model)?;
    let safe_provider = safe_eval_report_label(&baseline.provider);
    let path = dir.join(format!("baseline-{}-import.yaml", safe_provider));
    if path.exists() {
        anyhow::bail!(
            "external baseline import already exists at {}; refusing to overwrite",
            path.display()
        );
    }
    let yaml = serde_yaml::to_string(&baseline).context("failed to serialize baseline import")?;
    fs::write(&path, yaml).with_context(|| {
        format!(
            "failed to write external baseline import {}",
            path.display()
        )
    })?;
    Ok(path)
}

pub fn load_external_baselines_from_dir(
    dir: impl AsRef<Path>,
) -> Result<Vec<(PathBuf, EvalExternalBaselineSet)>> {
    let dir = dir.as_ref();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut baselines = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if !is_evalset_file(&path) {
            continue;
        }
        baselines.push((path.clone(), load_external_baseline(&path)?));
    }
    baselines.sort_by(|a, b| a.1.provider.cmp(&b.1.provider).then_with(|| a.0.cmp(&b.0)));
    Ok(baselines)
}

pub fn format_external_baseline_comparison(
    baselines: &[(PathBuf, EvalExternalBaselineSet)],
    provider_filter: Option<&str>,
) -> String {
    let expected = crate::engine::scenario_matrix::deterministic_scenarios()
        .iter()
        .map(|scenario| scenario.id)
        .collect::<Vec<_>>();
    let expected_set = expected.iter().copied().collect::<BTreeSet<_>>();
    let filter = provider_filter.filter(|value| !value.eq_ignore_ascii_case("all"));
    let filtered = baselines
        .iter()
        .filter(|(_, baseline)| {
            filter.is_none_or(|target| baseline.provider.eq_ignore_ascii_case(target))
        })
        .collect::<Vec<_>>();

    if filtered.is_empty() {
        return match filter {
            Some(provider) => format!(
                "External Baseline Comparison\nNo external baseline found for provider '{}'. Add YAML or JSON files under evalsets/external_baselines/.",
                provider
            ),
            None => "External Baseline Comparison\nNo external baselines found. Add YAML or JSON files under evalsets/external_baselines/.".to_string(),
        };
    }

    let mut lines = vec![
        "External Baseline Comparison".to_string(),
        format!(
            "Expected scenarios: {}  Providers: {}",
            expected.len(),
            filtered.len()
        ),
    ];

    for (path, baseline) in filtered {
        let records = baseline
            .scenarios
            .iter()
            .filter(|record| expected_set.contains(record.id.as_str()))
            .collect::<Vec<_>>();
        let recorded_ids = records
            .iter()
            .map(|record| record.id.as_str())
            .collect::<BTreeSet<_>>();
        let missing = expected
            .iter()
            .copied()
            .filter(|id| !recorded_ids.contains(id))
            .collect::<Vec<_>>();
        let unknown = baseline
            .scenarios
            .iter()
            .filter(|record| !expected_set.contains(record.id.as_str()))
            .map(|record| record.id.as_str())
            .collect::<Vec<_>>();
        let pass = records
            .iter()
            .filter(|record| record.outcome == EvalExternalBaselineOutcome::Pass)
            .count();
        let fail = records
            .iter()
            .filter(|record| record.outcome == EvalExternalBaselineOutcome::Fail)
            .count();
        let blocked = records
            .iter()
            .filter(|record| record.outcome == EvalExternalBaselineOutcome::Blocked)
            .count();
        let not_run = records
            .iter()
            .filter(|record| record.outcome == EvalExternalBaselineOutcome::NotRun)
            .count();
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown");
        lines.push(format!(
            "\n{} [{}] file={}",
            baseline.provider,
            baseline.model.as_deref().unwrap_or("model unknown"),
            filename
        ));
        if let Some(generated_at) = &baseline.generated_at {
            lines.push(format!("  generated_at={}", generated_at));
        }
        if let Some(source) = &baseline.source {
            lines.push(format!("  source={}", source));
        }
        lines.push(format!(
            "  coverage={}/{} pass={} fail={} blocked={} not_run={}",
            records.len(),
            expected.len(),
            pass,
            fail,
            blocked,
            not_run
        ));
        if !missing.is_empty() {
            lines.push(format!("  missing: {}", missing.join(", ")));
        }
        if !unknown.is_empty() {
            lines.push(format!("  unknown: {}", unknown.join(", ")));
        }
        for id in &expected {
            if let Some(record) = records.iter().find(|record| record.id == *id) {
                let mut detail = format!("  - {}: {}", id, record.outcome.label());
                if let Some(validation) = record.validation_passed {
                    detail.push_str(&format!(" validation={}", validation));
                }
                if let Some(evidence_backed) = record.final_evidence_backed {
                    detail.push_str(&format!(" evidence_backed={}", evidence_backed));
                }
                if let Some(tool_calls) = record.tool_calls {
                    detail.push_str(&format!(" tool_calls={}", tool_calls));
                }
                if let Some(repair_turns) = record.repair_turns {
                    detail.push_str(&format!(" repair_turns={}", repair_turns));
                }
                if let Some(evidence) = &record.evidence {
                    detail.push_str(&format!(" evidence={}", evidence));
                }
                lines.push(detail);
            }
        }
    }

    lines.join("\n")
}

pub fn format_external_baseline_validation(
    baselines: &[(PathBuf, EvalExternalBaselineSet)],
    provider_filter: Option<&str>,
) -> String {
    let expected = crate::engine::scenario_matrix::deterministic_scenarios()
        .iter()
        .map(|scenario| scenario.id)
        .collect::<Vec<_>>();
    let expected_set = expected.iter().copied().collect::<BTreeSet<_>>();
    let filter = provider_filter.filter(|value| !value.eq_ignore_ascii_case("all"));
    let filtered = baselines
        .iter()
        .filter(|(_, baseline)| {
            filter.is_none_or(|target| baseline.provider.eq_ignore_ascii_case(target))
        })
        .collect::<Vec<_>>();

    if filtered.is_empty() {
        return match filter {
            Some(provider) => format!(
                "External Baseline Validation\nNo external baseline found for provider '{}'. Add YAML or JSON files under evalsets/external_baselines/.",
                provider
            ),
            None => "External Baseline Validation\nNo external baselines found. Add YAML or JSON files under evalsets/external_baselines/.".to_string(),
        };
    }

    let mut lines = vec![
        "External Baseline Validation".to_string(),
        format!(
            "Expected scenarios: {}  Providers: {}",
            expected.len(),
            filtered.len()
        ),
    ];

    for (path, baseline) in filtered {
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown");
        let records = baseline
            .scenarios
            .iter()
            .filter(|record| expected_set.contains(record.id.as_str()))
            .collect::<Vec<_>>();
        let id_counts =
            baseline
                .scenarios
                .iter()
                .fold(BTreeMap::<&str, usize>::new(), |mut counts, record| {
                    *counts.entry(record.id.as_str()).or_default() += 1;
                    counts
                });
        let recorded_ids = records
            .iter()
            .map(|record| record.id.as_str())
            .collect::<BTreeSet<_>>();
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        for id in &expected {
            if !recorded_ids.contains(id) {
                errors.push(format!("missing required scenario {}", id));
            }
        }
        for (id, count) in id_counts {
            if count > 1 {
                errors.push(format!("duplicate scenario {} appears {} times", id, count));
            }
            if !expected_set.contains(id) {
                warnings.push(format!("unknown scenario {} is ignored by comparison", id));
            }
        }
        for record in &records {
            if record.outcome == EvalExternalBaselineOutcome::NotRun {
                warnings.push(format!("{} is not_run", record.id));
            }
            if matches!(
                record.outcome,
                EvalExternalBaselineOutcome::Pass
                    | EvalExternalBaselineOutcome::Fail
                    | EvalExternalBaselineOutcome::Blocked
            ) && !has_meaningful_external_evidence(record.evidence.as_deref())
            {
                warnings.push(format!("{} is missing concrete evidence", record.id));
            }
            if record.outcome == EvalExternalBaselineOutcome::Pass {
                if record.validation_passed != Some(true) {
                    errors.push(format!("{} pass is missing validation=true", record.id));
                }
                if record.final_evidence_backed != Some(true) {
                    errors.push(format!(
                        "{} pass is missing final_evidence_backed=true",
                        record.id
                    ));
                }
            }
            if record.outcome == EvalExternalBaselineOutcome::Fail
                && record.validation_passed.is_none()
            {
                warnings.push(format!(
                    "{} fail should record validation_passed=false when applicable",
                    record.id
                ));
            }
        }

        lines.push(format!(
            "\n{} [{}] file={}",
            baseline.provider,
            baseline.model.as_deref().unwrap_or("model unknown"),
            filename
        ));
        lines.push(format!(
            "  status={} coverage={}/{} errors={} warnings={}",
            if errors.is_empty() {
                "valid"
            } else {
                "invalid"
            },
            records.len(),
            expected.len(),
            errors.len(),
            warnings.len()
        ));
        for error in errors {
            lines.push(format!("  error: {}", error));
        }
        for warning in warnings {
            lines.push(format!("  warn: {}", warning));
        }
    }

    lines.join("\n")
}

pub fn format_external_parity_report(
    baselines: &[(PathBuf, EvalExternalBaselineSet)],
    provider_filter: Option<&str>,
) -> String {
    let scenarios = crate::engine::scenario_matrix::deterministic_scenarios();
    let filter = provider_filter.filter(|value| !value.eq_ignore_ascii_case("all"));
    let providers = baselines
        .iter()
        .filter(|(_, baseline)| {
            filter.is_none_or(|target| baseline.provider.eq_ignore_ascii_case(target))
        })
        .collect::<Vec<_>>();

    if providers.is_empty() {
        return match filter {
            Some(provider) => format!(
                "Phase 12 Parity Report\nNo external baseline found for provider '{}'. Add YAML or JSON files under evalsets/external_baselines/.",
                provider
            ),
            None => "Phase 12 Parity Report\nNo external baselines found. Local replay fixtures are ready, but external Claude/Codex rows have not been imported yet.".to_string(),
        };
    }

    let local_ready = scenarios
        .iter()
        .filter(|scenario| {
            scenario.status == crate::engine::scenario_matrix::ReplayStatus::ReplayFixtureReady
        })
        .count();
    let mut provider_pass = BTreeMap::<&str, usize>::new();
    let mut provider_fail = BTreeMap::<&str, usize>::new();
    let mut provider_blocked = BTreeMap::<&str, usize>::new();
    let mut provider_not_run = BTreeMap::<&str, usize>::new();

    for (_, baseline) in &providers {
        for scenario in scenarios {
            let outcome = baseline
                .scenarios
                .iter()
                .find(|record| record.id == scenario.id)
                .map(|record| record.outcome)
                .unwrap_or(EvalExternalBaselineOutcome::NotRun);
            match outcome {
                EvalExternalBaselineOutcome::Pass => {
                    *provider_pass.entry(baseline.provider.as_str()).or_default() += 1;
                }
                EvalExternalBaselineOutcome::Fail => {
                    *provider_fail.entry(baseline.provider.as_str()).or_default() += 1;
                }
                EvalExternalBaselineOutcome::Blocked => {
                    *provider_blocked
                        .entry(baseline.provider.as_str())
                        .or_default() += 1;
                }
                EvalExternalBaselineOutcome::NotRun => {
                    *provider_not_run
                        .entry(baseline.provider.as_str())
                        .or_default() += 1;
                }
            }
        }
    }

    let mut lines = vec![
        "Phase 12 Parity Report".to_string(),
        format!(
            "Local replay-ready: {}/{}  External providers: {}",
            local_ready,
            scenarios.len(),
            providers.len()
        ),
    ];
    for (_, baseline) in &providers {
        lines.push(format!(
            "- {} [{}]: pass={} fail={} blocked={} not_run={}",
            baseline.provider,
            baseline.model.as_deref().unwrap_or("model unknown"),
            provider_pass
                .get(baseline.provider.as_str())
                .copied()
                .unwrap_or(0),
            provider_fail
                .get(baseline.provider.as_str())
                .copied()
                .unwrap_or(0),
            provider_blocked
                .get(baseline.provider.as_str())
                .copied()
                .unwrap_or(0),
            provider_not_run
                .get(baseline.provider.as_str())
                .copied()
                .unwrap_or(0)
        ));
    }

    for scenario in scenarios {
        lines.push(format!("\n{} [{}]", scenario.id, scenario.status.label()));
        lines.push(format!("  task: {}", scenario.user_task));
        for (_, baseline) in &providers {
            let record = baseline
                .scenarios
                .iter()
                .find(|record| record.id == scenario.id);
            let detail = match record {
                Some(record) => format_parity_provider_detail(&baseline.provider, record),
                None => format!("{}=missing gap=external_missing", baseline.provider),
            };
            lines.push(format!("  {}", detail));
        }
    }

    lines.join("\n")
}

pub fn write_external_parity_report(
    baselines: &[(PathBuf, EvalExternalBaselineSet)],
    provider_filter: Option<&str>,
    dir: impl AsRef<Path>,
) -> Result<PathBuf> {
    let dir = dir.as_ref();
    fs::create_dir_all(dir)
        .with_context(|| format!("failed to create parity report dir {}", dir.display()))?;
    let label = provider_filter
        .filter(|value| !value.eq_ignore_ascii_case("all"))
        .unwrap_or("all");
    let safe_label = safe_eval_report_label(label);
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let path = dir.join(format!("parity-{}-{}.txt", timestamp, safe_label));
    let report = format_external_parity_report(baselines, provider_filter);
    fs::write(&path, report)
        .with_context(|| format!("failed to write parity report {}", path.display()))?;
    Ok(path)
}

fn format_parity_provider_detail(provider: &str, record: &EvalExternalBaselineScenario) -> String {
    let gap = match record.outcome {
        EvalExternalBaselineOutcome::Pass
            if record.validation_passed == Some(true)
                && record.final_evidence_backed == Some(true)
                && has_meaningful_external_evidence(record.evidence.as_deref()) =>
        {
            "none"
        }
        EvalExternalBaselineOutcome::Pass => "evidence_incomplete",
        EvalExternalBaselineOutcome::Fail => "external_failed",
        EvalExternalBaselineOutcome::Blocked => "external_blocked",
        EvalExternalBaselineOutcome::NotRun => "external_not_run",
    };
    let mut detail = format!("{}={} gap={}", provider, record.outcome.label(), gap);
    if let Some(validation) = record.validation_passed {
        detail.push_str(&format!(" validation={}", validation));
    }
    if let Some(evidence_backed) = record.final_evidence_backed {
        detail.push_str(&format!(" evidence_backed={}", evidence_backed));
    }
    if let Some(tool_calls) = record.tool_calls {
        detail.push_str(&format!(" tool_calls={}", tool_calls));
    }
    if let Some(repair_turns) = record.repair_turns {
        detail.push_str(&format!(" repair_turns={}", repair_turns));
    }
    if let Some(evidence) = record
        .evidence
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        detail.push_str(&format!(" evidence={}", evidence));
    }
    detail
}

fn normalized_external_provider(provider: &str) -> String {
    let provider = provider.trim();
    if provider.is_empty() {
        "external-agent".to_string()
    } else {
        provider.to_string()
    }
}

fn normalized_external_model(model: Option<&str>) -> Option<String> {
    model
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn parse_external_baseline_markdown_artifact(
    content: &str,
    path: &Path,
    provider: &str,
    model: Option<&str>,
) -> Result<EvalExternalBaselineSet> {
    let expected = crate::engine::scenario_matrix::deterministic_scenarios()
        .iter()
        .map(|scenario| scenario.id)
        .collect::<BTreeSet<_>>();
    let mut scenarios = Vec::new();
    let mut header: Option<Vec<String>> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
            continue;
        }
        let cells = trimmed
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim().to_string())
            .collect::<Vec<_>>();
        if cells.iter().all(|cell| {
            cell.chars()
                .all(|ch| ch == '-' || ch == ':' || ch.is_whitespace())
        }) {
            continue;
        }
        if header.is_none() {
            header = Some(
                cells
                    .iter()
                    .map(|cell| normalize_table_header(cell))
                    .collect(),
            );
            continue;
        }
        let Some(headers) = &header else {
            continue;
        };
        let Some(id) = table_cell(headers, &cells, &["id", "scenario", "scenario_id"]) else {
            continue;
        };
        if !expected.contains(id) {
            continue;
        }
        let outcome = table_cell(headers, &cells, &["outcome", "status", "result"])
            .and_then(parse_external_baseline_outcome)
            .unwrap_or(EvalExternalBaselineOutcome::NotRun);
        scenarios.push(EvalExternalBaselineScenario {
            id: id.to_string(),
            outcome,
            evidence: table_cell(headers, &cells, &["evidence", "artifact", "proof"])
                .map(str::to_string),
            notes: table_cell(headers, &cells, &["notes", "note", "summary"]).map(str::to_string),
            tool_calls: table_cell(headers, &cells, &["tool_calls", "tools"])
                .and_then(|value| value.parse::<usize>().ok()),
            repair_turns: table_cell(headers, &cells, &["repair_turns", "repairs"])
                .and_then(|value| value.parse::<usize>().ok()),
            validation_passed: table_cell(headers, &cells, &["validation_passed", "validation"])
                .and_then(parse_bool_cell),
            final_evidence_backed: table_cell(
                headers,
                &cells,
                &["final_evidence_backed", "evidence_backed"],
            )
            .and_then(parse_bool_cell),
        });
    }

    if scenarios.is_empty() {
        anyhow::bail!(
            "no Phase 12 scenario rows found in {}; expected a markdown table with id/scenario and outcome/result columns",
            path.display()
        );
    }

    Ok(EvalExternalBaselineSet {
        provider: normalized_external_provider(provider),
        generated_at: Some(chrono::Utc::now().to_rfc3339()),
        model: normalized_external_model(model),
        source: Some(path.display().to_string()),
        scenarios,
    })
}

fn normalize_table_header(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

fn table_cell<'a>(headers: &[String], cells: &'a [String], names: &[&str]) -> Option<&'a str> {
    headers
        .iter()
        .position(|header| names.iter().any(|name| header == name))
        .and_then(|index| cells.get(index))
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "-")
}

fn parse_external_baseline_outcome(value: &str) -> Option<EvalExternalBaselineOutcome> {
    match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "pass" | "passed" | "ok" | "success" => Some(EvalExternalBaselineOutcome::Pass),
        "fail" | "failed" | "failure" => Some(EvalExternalBaselineOutcome::Fail),
        "blocked" | "block" => Some(EvalExternalBaselineOutcome::Blocked),
        "not_run" | "notrun" | "skip" | "skipped" | "todo" => {
            Some(EvalExternalBaselineOutcome::NotRun)
        }
        _ => None,
    }
}

fn parse_bool_cell(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "y" | "1" | "pass" | "passed" => Some(true),
        "false" | "no" | "n" | "0" | "fail" | "failed" => Some(false),
        _ => None,
    }
}

fn has_meaningful_external_evidence(value: Option<&str>) -> bool {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    let lower = value.to_ascii_lowercase();
    !(lower.starts_with("todo") || lower == "-" || lower == "n/a")
}
