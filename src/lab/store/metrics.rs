//! LabRun metrics, meetings, cost, and dashboard summaries.
//!
//! The methods here produce operator-facing summaries without changing stage
//! ownership. They are read-heavy support paths for `/lab dashboard`, meeting
//! diagnostics, and desktop LabRun inspectors.

use super::*;

impl LabStore {
    pub fn record_meeting_request(&self, topic: Option<&str>) -> anyhow::Result<LabRun> {
        let run = self
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for meeting"))?;
        self.append_run_event(
            &run.lab_run_id,
            "lab_meeting_requested",
            serde_json::json!({
                "topic": topic.unwrap_or("").trim(),
                "mutation_allowed": false,
            }),
        )?;
        Ok(run)
    }

    pub fn record_cost_usage(
        &self,
        lab_run_id: &str,
        role: LabRole,
        model: &str,
        tokens: LabCostTokens,
        estimated_cost_usd: f64,
        note: Option<&str>,
    ) -> anyhow::Result<LabCostUsage> {
        let prompt_tokens = tokens.prompt_tokens;
        let cached_tokens = tokens.cached_tokens.min(prompt_tokens);
        let cache_miss_tokens = prompt_tokens.saturating_sub(cached_tokens);
        let usage = LabCostUsage {
            schema_version: LAB_SCHEMA_VERSION,
            usage_id: next_id("labusage"),
            lab_run_id: lab_run_id.to_string(),
            created_at: Utc::now(),
            role,
            cycle_id: tokens.cycle_id,
            meeting_id: tokens.meeting_id,
            model: model.trim().to_string(),
            prompt_tokens,
            completion_tokens: tokens.completion_tokens,
            reasoning_tokens: tokens.reasoning_tokens,
            cached_tokens,
            cache_write_tokens: tokens.cache_write_tokens,
            cache_miss_tokens,
            total_tokens: prompt_tokens
                .saturating_add(tokens.completion_tokens)
                .saturating_add(tokens.reasoning_tokens),
            estimated_cost_usd: estimated_cost_usd.max(0.0),
            note: note
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        };
        let path = self.run_dir(lab_run_id).join("cost_usage.jsonl");
        append_jsonl(&path, &usage)?;
        self.append_run_event(
            lab_run_id,
            "lab_cost_usage_recorded",
            serde_json::json!({
                "usage_id": usage.usage_id,
                "role": format!("{:?}", usage.role),
                "model": usage.model,
                "total_tokens": usage.total_tokens,
                "cached_tokens": usage.cached_tokens,
                "cache_write_tokens": usage.cache_write_tokens,
                "cache_miss_tokens": usage.cache_miss_tokens,
                "estimated_cost_usd": usage.estimated_cost_usd,
            }),
        )?;
        Ok(usage)
    }

    pub fn list_cost_usage(&self, lab_run_id: &str) -> anyhow::Result<Vec<LabCostUsage>> {
        let path = self.run_dir(lab_run_id).join("cost_usage.jsonl");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file =
            fs::File::open(&path).with_context(|| format!("failed to open {}", path.display()))?;
        let reader = std::io::BufReader::new(file);
        let mut usage = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            usage.push(
                serde_json::from_str::<LabCostUsage>(&line)
                    .with_context(|| format!("failed to parse cost usage in {}", path.display()))?,
            );
        }
        Ok(usage)
    }

    pub fn cost_summary(&self, lab_run_id: &str) -> anyhow::Result<LabCostSummary> {
        let mut summary = LabCostSummary::empty(lab_run_id);
        for usage in self.list_cost_usage(lab_run_id)? {
            summary.add_usage(&usage);
        }
        Ok(summary)
    }

    pub fn latest_cost_summary(&self) -> anyhow::Result<Option<LabCostSummary>> {
        let Some(run) = self.latest_run()? else {
            return Ok(None);
        };
        self.cost_summary(&run.lab_run_id).map(Some)
    }

    pub fn record_evidence_ref(
        &self,
        input: LabEvidenceRefInput<'_>,
    ) -> anyhow::Result<LabEvidenceRef> {
        let LabEvidenceRefInput {
            lab_run_id,
            kind,
            role,
            reference,
            summary,
            artifact_id,
            cycle_id,
        } = input;
        let reference = reference.trim();
        if reference.is_empty() {
            return Err(anyhow!("evidence reference cannot be empty"));
        }
        let summary = summary.trim();
        if summary.is_empty() {
            return Err(anyhow!("evidence summary cannot be empty"));
        }

        let evidence = LabEvidenceRef {
            schema_version: LAB_SCHEMA_VERSION,
            evidence_id: next_id("labevidence"),
            lab_run_id: lab_run_id.to_string(),
            created_at: Utc::now(),
            kind,
            role,
            reference: reference.to_string(),
            summary: summary.to_string(),
            artifact_id: artifact_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            cycle_id: cycle_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            metadata_hash: evidence_metadata_hash(reference),
            estimated_summary_tokens: crate::engine::context_compressor::estimate_tokens(summary),
        };
        append_jsonl(
            &self.run_dir(lab_run_id).join("evidence_refs.jsonl"),
            &evidence,
        )?;
        self.append_run_event(
            lab_run_id,
            "lab_evidence_ref_recorded",
            serde_json::json!({
                "evidence_id": evidence.evidence_id,
                "kind": format!("{:?}", evidence.kind),
                "role": format!("{:?}", evidence.role),
                "reference": evidence.reference,
                "metadata_hash": evidence.metadata_hash,
            }),
        )?;
        Ok(evidence)
    }

    pub fn list_evidence_refs(&self, lab_run_id: &str) -> anyhow::Result<Vec<LabEvidenceRef>> {
        let path = self.run_dir(lab_run_id).join("evidence_refs.jsonl");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file =
            fs::File::open(&path).with_context(|| format!("failed to open {}", path.display()))?;
        let reader = std::io::BufReader::new(file);
        let mut evidence = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            evidence.push(
                serde_json::from_str::<LabEvidenceRef>(&line).with_context(|| {
                    format!("failed to parse evidence ref in {}", path.display())
                })?,
            );
        }
        Ok(evidence)
    }

    pub fn latest_evidence_refs(&self) -> anyhow::Result<Vec<LabEvidenceRef>> {
        let Some(run) = self.latest_run()? else {
            return Ok(Vec::new());
        };
        self.list_evidence_refs(&run.lab_run_id)
    }

    pub fn record_provider_certification(
        &self,
        provider_id: &str,
        model: &str,
        kind: LabProviderCertificationKind,
        outcome: LabProviderCertificationOutcome,
        evidence_path: &str,
        summary: &str,
    ) -> anyhow::Result<LabProviderCertificationRecord> {
        let provider_id = provider_id.trim();
        if provider_id.is_empty() {
            return Err(anyhow!("provider_id cannot be empty"));
        }
        let model = model.trim();
        if model.is_empty() {
            return Err(anyhow!("model cannot be empty"));
        }
        let evidence_path = evidence_path.trim();
        if evidence_path.is_empty() {
            return Err(anyhow!(
                "provider certification evidence_path cannot be empty"
            ));
        }
        let summary = summary.trim();
        if summary.is_empty() {
            return Err(anyhow!("provider certification summary cannot be empty"));
        }
        let record = LabProviderCertificationRecord {
            schema_version: LAB_SCHEMA_VERSION,
            record_id: next_id("labprovidercert"),
            provider_id: provider_id.to_string(),
            model: model.to_string(),
            kind,
            outcome,
            recorded_at: Utc::now(),
            evidence_path: evidence_path.to_string(),
            summary: summary.to_string(),
        };
        append_jsonl(&self.provider_certifications_path(), &record)?;
        self.append_project_event(LabEvent {
            schema_version: LAB_SCHEMA_VERSION,
            event_id: next_id("event"),
            lab_run_id: None,
            proposal_id: None,
            event_type: "lab_provider_certification_recorded".to_string(),
            created_at: record.recorded_at,
            payload: serde_json::json!({
                "record_id": record.record_id,
                "provider_id": record.provider_id,
                "model": record.model,
                "kind": record.kind.as_str(),
                "outcome": record.outcome.as_str(),
                "evidence_path": record.evidence_path,
            }),
        })?;
        Ok(record)
    }

    pub fn list_provider_certifications(
        &self,
    ) -> anyhow::Result<Vec<LabProviderCertificationRecord>> {
        let path = self.provider_certifications_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file =
            fs::File::open(&path).with_context(|| format!("failed to open {}", path.display()))?;
        let reader = std::io::BufReader::new(file);
        let mut records = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            records.push(
                serde_json::from_str::<LabProviderCertificationRecord>(&line).with_context(
                    || {
                        format!(
                            "failed to parse provider certification in {}",
                            path.display()
                        )
                    },
                )?,
            );
        }
        Ok(records)
    }

    pub fn latest_provider_certification(
        &self,
        provider_id: &str,
        model: &str,
        kind: LabProviderCertificationKind,
    ) -> anyhow::Result<Option<LabProviderCertificationRecord>> {
        let provider_id = provider_id.trim();
        let model = model.trim();
        Ok(self
            .list_provider_certifications()?
            .into_iter()
            .filter(|record| {
                record.provider_id == provider_id && record.model == model && record.kind == kind
            })
            .max_by_key(|record| record.recorded_at))
    }

    pub fn record_compression_decision(
        &self,
        mut decision: LabCompressionDecision,
    ) -> anyhow::Result<LabCompressionDecision> {
        if decision.decision_id.trim().is_empty() {
            decision.decision_id = next_id("labcompression");
        }
        append_jsonl(
            &self
                .run_dir(&decision.lab_run_id)
                .join("compression_decisions.jsonl"),
            &decision,
        )?;
        self.append_run_event(
            &decision.lab_run_id,
            "lab_compression_decision_recorded",
            serde_json::json!({
                "decision_id": decision.decision_id,
                "role": format!("{:?}", decision.role),
                "action": format!("{:?}", decision.action),
                "packet_tokens": decision.packet_tokens,
                "context_budget_tokens": decision.context_budget_tokens,
                "usage_ratio_percent": decision.usage_ratio_percent,
                "stable_prefix_fingerprint": decision.stable_prefix_fingerprint,
                "dynamic_tail_fingerprint": decision.dynamic_tail_fingerprint,
            }),
        )?;
        Ok(decision)
    }

    pub fn list_compression_decisions(
        &self,
        lab_run_id: &str,
    ) -> anyhow::Result<Vec<LabCompressionDecision>> {
        let path = self.run_dir(lab_run_id).join("compression_decisions.jsonl");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file =
            fs::File::open(&path).with_context(|| format!("failed to open {}", path.display()))?;
        let reader = std::io::BufReader::new(file);
        let mut decisions = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            decisions.push(
                serde_json::from_str::<LabCompressionDecision>(&line).with_context(|| {
                    format!("failed to parse compression decision in {}", path.display())
                })?,
            );
        }
        Ok(decisions)
    }
}
