use super::*;

#[derive(Debug, Clone, Default)]
pub struct LabCostTokens {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub reasoning_tokens: u64,
    pub cached_tokens: u64,
    pub cache_write_tokens: u64,
    pub cycle_id: Option<String>,
    pub meeting_id: Option<String>,
}

pub(super) fn next_id(prefix: &str) -> String {
    format!(
        "{}_{}_{}",
        prefix,
        Utc::now().format("%Y%m%d%H%M%S"),
        Uuid::new_v4().simple()
    )
}

pub(super) fn lease_owner() -> String {
    let host = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown-host".to_string());
    format!("pid:{}:host:{}", std::process::id(), host)
}

pub(super) fn read_json<T: for<'de> serde::Deserialize<'de>>(path: &Path) -> anyhow::Result<T> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))
}

pub(super) fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".{}.tmp", Uuid::new_v4().simple()));
    let bytes = serde_json::to_vec_pretty(value)?;
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(&bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

pub(super) fn atomic_write_text(path: &Path, value: &str) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".{}.tmp", Uuid::new_v4().simple()));
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(value.as_bytes())?;
        if !value.ends_with('\n') {
            file.write_all(b"\n")?;
        }
        file.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

pub(super) fn enum_json<T: Serialize>(value: &T) -> anyhow::Result<String> {
    Ok(serde_json::to_string(value)?.trim_matches('"').to_string())
}

pub(super) fn optional_enum_json<T: Serialize>(
    value: Option<&T>,
) -> anyhow::Result<Option<String>> {
    value.map(enum_json).transpose()
}

pub(super) fn sqlite_count(conn: &Connection, table: &str) -> anyhow::Result<usize> {
    let sql = match table {
        "lab_runs" => "SELECT COUNT(*) FROM lab_runs",
        "lab_artifacts" => "SELECT COUNT(*) FROM lab_artifacts",
        "lab_events" => "SELECT COUNT(*) FROM lab_events",
        "lab_tasks" => "SELECT COUNT(*) FROM lab_tasks",
        _ => return Err(anyhow!("unsupported Lab SQLite count table: {table}")),
    };
    let count: i64 = conn.query_row(sql, [], |row| row.get(0))?;
    Ok(count.max(0) as usize)
}

pub(super) fn latest_sqlite_artifact_for_role(
    conn: &Connection,
    lab_run_id: &str,
    artifact_types: &[&str],
) -> anyhow::Result<Option<LabSqliteArtifactSummary>> {
    let placeholders = artifact_types
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT artifact_id, artifact_type, stage, status, validation_status
         FROM lab_artifacts
         WHERE lab_run_id = ? AND artifact_type IN ({placeholders})
         ORDER BY rowid DESC
         LIMIT 1"
    );
    let mut params = Vec::with_capacity(artifact_types.len() + 1);
    params.push(lab_run_id);
    params.extend(artifact_types.iter().copied());
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(rusqlite::params_from_iter(params))?;
    if let Some(row) = rows.next()? {
        Ok(Some(LabSqliteArtifactSummary {
            artifact_id: row.get(0)?,
            artifact_type: row.get(1)?,
            stage: row.get(2)?,
            status: row.get(3)?,
            validation_status: row.get(4)?,
        }))
    } else {
        Ok(None)
    }
}

pub(super) fn append_jsonl<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, value)?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}

pub(super) fn remove_file_if_exists(path: &Path) -> anyhow::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

pub(super) fn safe_path_component(value: &str) -> String {
    let safe: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if safe.is_empty() {
        "stage".to_string()
    } else {
        safe
    }
}

pub(super) fn clean_string_vec(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

pub(super) fn sync_open_task(run: &mut LabRun, task: &GraduateTask) {
    if task.status.is_open() {
        if !run.open_task_ids.iter().any(|id| id == &task.task_id) {
            run.open_task_ids.push(task.task_id.clone());
        }
        if !run
            .resume_cursor
            .open_task_ids
            .iter()
            .any(|id| id == &task.task_id)
        {
            run.resume_cursor.open_task_ids.push(task.task_id.clone());
        }
    } else {
        run.open_task_ids.retain(|id| id != &task.task_id);
        run.resume_cursor
            .open_task_ids
            .retain(|id| id != &task.task_id);
    }
}

pub(super) fn evidence_metadata_hash(reference: &str) -> Option<String> {
    let path = Path::new(reference);
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let payload = format!("{}:{}:{}", path.display(), metadata.len(), modified);
    Some(crate::engine::prompt_context::stable_fingerprint(&payload))
}

pub(super) fn note_or_default<'a>(note: &'a str, default: &'a str) -> &'a str {
    let note = note.trim();
    if note.is_empty() {
        default
    } else {
        note
    }
}
