//! Memory manager migration functions.
//!
//! Functions for memory migration, backup, and rollback.

use super::helpers::{infer_memory_importance, is_safe_memory_backup_id, normalize_for_duplicate};
use super::MemoryManager;
use crate::memory::extraction::infer_memory_tags;
use crate::memory::files::{
    collect_memory_file_paths, legacy_markdown_section_parts, legacy_markdown_sections,
};
use crate::memory::reports::MemoryMigrationReport;
use crate::memory::types::{
    MemoryCandidate, MemoryEvidenceKind, MemoryEvidenceRef, MemoryProjection, MemoryProvenance,
    MemoryRecord, MemoryScope, MemoryStatus,
};
use std::collections::HashSet;
use tracing::debug;

impl MemoryManager {
    pub fn memory_migration_dry_run(&self) -> MemoryMigrationReport {
        let (local_files, mut issues) = self
            .provider_registry
            .local_migration_file_reports()
            .unwrap_or_else(|error| (Vec::new(), vec![format!("local_provider: {error}")]));
        let files = local_files.into_iter().map(Into::into).collect();
        if let Err(error) = self.provider_registry.local_memory_records_raw() {
            issues.push(format!("records_jsonl: {error}"));
        }
        let projection_drift = self.memory_record_summary().projection_drift;
        MemoryMigrationReport {
            action: "dry-run".to_string(),
            dry_run: true,
            backup_id: None,
            backup_path: None,
            files,
            issues,
            projection_drift,
            repair_proposals: self.projection_repair_proposals(200).len(),
            restored_files: 0,
        }
    }

    pub fn memory_migration_backup(&self) -> anyhow::Result<MemoryMigrationReport> {
        let dry_run = self.memory_migration_dry_run();
        let backup_id = format!(
            "mem-{}-{}",
            chrono::Utc::now().format("%Y%m%dT%H%M%SZ"),
            uuid::Uuid::new_v4().simple()
        );
        let backup = self
            .provider_registry
            .backup_local_memory_files(&backup_id)?;
        Ok(MemoryMigrationReport {
            action: "backup".to_string(),
            dry_run: false,
            backup_id: Some(backup.backup_id),
            backup_path: Some(backup.backup_path.display().to_string()),
            files: backup.files.into_iter().map(Into::into).collect(),
            issues: dry_run.issues,
            projection_drift: dry_run.projection_drift,
            repair_proposals: dry_run.repair_proposals,
            restored_files: 0,
        })
    }

    pub fn memory_migration_rollback(
        &self,
        backup_id: &str,
    ) -> anyhow::Result<MemoryMigrationReport> {
        if !is_safe_memory_backup_id(backup_id) {
            anyhow::bail!("invalid memory backup id");
        }
        let rollback = self
            .provider_registry
            .rollback_local_memory_files(backup_id)?;
        Ok(MemoryMigrationReport {
            action: "rollback".to_string(),
            dry_run: false,
            backup_id: Some(rollback.backup_id),
            backup_path: Some(rollback.backup_path.display().to_string()),
            files: rollback.files.into_iter().map(Into::into).collect(),
            issues: Vec::new(),
            projection_drift: self.memory_record_summary().projection_drift,
            repair_proposals: self.projection_repair_proposals(200).len(),
            restored_files: rollback.restored_files,
        })
    }

    pub fn import_legacy_markdown_records(&self) -> usize {
        let mut existing_records = self.memory_records();
        let mut seen = existing_records
            .iter()
            .map(|record| normalize_for_duplicate(&record.content))
            .collect::<HashSet<_>>();
        let mut imported = 0usize;

        let mut sources = vec![
            (self.memory_path.clone(), "MEMORY.md".to_string(), "learned"),
            (self.user_path.clone(), "USER.md".to_string(), "preference"),
        ];
        sources.extend(
            collect_memory_file_paths(&self.memory_dir, false)
                .into_iter()
                .map(|path| {
                    let projection = self.projection_path(&path);
                    (path, projection, "learned")
                }),
        );

        for (path, projection_path, default_category) in sources {
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            for section in legacy_markdown_sections(&content) {
                if section.contains("memory-id:") {
                    continue;
                }
                let Some((category, body)) =
                    legacy_markdown_section_parts(&section, default_category)
                else {
                    continue;
                };
                let normalized = normalize_for_duplicate(&body);
                if normalized.is_empty() || !seen.insert(normalized) {
                    continue;
                }
                let Ok(assessment) = crate::memory::quality::assess_memory_candidate(
                    &body,
                    &category,
                    "",
                    category == "preference",
                ) else {
                    continue;
                };
                let mut candidate = MemoryCandidate::new(
                    body.clone(),
                    category.clone(),
                    MemoryScope::local("legacy-markdown-import"),
                    MemoryProvenance::local("legacy_markdown_import"),
                )
                .confidence(assessment.score)
                .importance(infer_memory_importance(&body, &category))
                .with_tags({
                    let mut tags = infer_memory_tags(&body, &category);
                    tags.push("legacy_import".to_string());
                    tags.sort();
                    tags.dedup();
                    tags
                })
                .explicit(category == "preference");
                let evidence_kind = if category == "preference" {
                    MemoryEvidenceKind::UserStatement
                } else {
                    MemoryEvidenceKind::Inference
                };
                candidate.evidence.push(MemoryEvidenceRef::new(
                    evidence_kind,
                    projection_path.clone(),
                    "Imported from existing Markdown memory projection",
                    if category == "preference" { 0.7 } else { 0.45 },
                ));
                let mut record = MemoryRecord::from_candidate(
                    candidate,
                    MemoryStatus::Accepted,
                    assessment.score,
                    assessment.future_utility,
                    assessment.sensitivity,
                );
                record.projection = Some(MemoryProjection {
                    path: projection_path.clone(),
                    heading: format!("[{}]", category.to_uppercase()),
                });
                existing_records.push(record);
                imported += 1;
            }
        }

        if imported > 0 {
            if let Err(error) = self.provider_registry.replace_local_memory_records(
                &existing_records,
                "legacy_markdown_import",
                "import legacy markdown projections into canonical records",
            ) {
                debug!("Failed to import legacy Markdown memory records: {}", error);
                return 0;
            }
        }
        imported
    }
}
