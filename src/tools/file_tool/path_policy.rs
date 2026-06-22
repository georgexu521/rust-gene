//! File tool support module.
//!
//! Separates read, write, edit matching, path policy, and mutation history from the file tool entrypoint.

use std::path::{Path, PathBuf};

pub(crate) fn is_unc_or_network_path(path: &str) -> bool {
    path.starts_with("\\\\") || path.starts_with("//")
}

/// 解析路径（支持相对路径和绝对路径），带路径穿越保护
pub fn resolve_path(path: &str, working_dir: &Path) -> Result<PathBuf, String> {
    resolve_path_with_policy(path, working_dir, false)
}

/// 解析只读路径。相对路径仍限制在工作区内；绝对路径除工作区和临时目录外，
/// 允许读取用户桌面、runtime tool-result artifacts 和
/// `PRIORITY_AGENT_READ_ROOTS` 声明的只读根目录。
pub fn resolve_read_path(path: &str, working_dir: &Path) -> Result<PathBuf, String> {
    resolve_path_with_policy(path, working_dir, true)
}

fn resolve_path_with_policy(
    path: &str,
    working_dir: &Path,
    read_only: bool,
) -> Result<PathBuf, String> {
    let expanded_input = expand_home_path(path);
    let input = expanded_input.as_path();
    let normalized_working_dir = normalize_path(working_dir);

    let candidate = if input.is_absolute() {
        normalize_path(input)
    } else {
        normalize_path(&normalized_working_dir.join(input))
    };

    if input.is_absolute() {
        if !is_allowed_path_for_policy(&candidate, &normalized_working_dir, read_only) {
            return Err(format!(
                "Access denied: absolute path '{}' is outside allowed roots",
                path
            ));
        }
    } else if !candidate.starts_with(&normalized_working_dir) {
        return Err(format!(
            "Access denied: path '{}' escapes working directory",
            path
        ));
    }

    // working_dir 不存在时无法进行可靠的 realpath 比较，保留词法边界检查结果。
    if !normalized_working_dir.exists() {
        return Ok(candidate);
    }

    // 第二层防护：解析已存在祖先的真实路径，阻止通过 symlink 逃逸目录边界。
    let real_candidate = realpath_deepest_existing(&candidate)?;
    let real_working_dir = canonicalize_or_normalize(&normalized_working_dir);

    if input.is_absolute() {
        if !is_allowed_path_for_policy(&real_candidate, &real_working_dir, read_only) {
            return Err(format!(
                "Access denied: absolute path '{}' resolves outside allowed roots",
                path
            ));
        }
    } else if !real_candidate.starts_with(&real_working_dir) {
        return Err(format!(
            "Access denied: path '{}' escapes working directory via symlink",
            path
        ));
    }

    Ok(candidate)
}

fn is_allowed_path_for_policy(path: &Path, working_dir: &Path, read_only: bool) -> bool {
    if read_only {
        is_allowed_read_absolute_path(path, working_dir)
    } else {
        is_allowed_absolute_path(path, working_dir)
    }
}

fn expand_home_path(path: &str) -> PathBuf {
    if path == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home);
        }
    } else if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}

pub fn is_allowed_absolute_path(path: &Path, working_dir: &Path) -> bool {
    let normalized_path = normalize_path(path);
    let normalized_working = normalize_path(working_dir);

    if normalized_path.starts_with(&normalized_working) {
        return true;
    }

    // 如果 working_dir 在 /tmp 下，只允许访问 working_dir 内的路径
    // 防止 /tmp/foo 工作目录下访问 /tmp/bar
    let tmp_dir = normalize_path(&std::env::temp_dir());
    let in_tmp = normalized_working.starts_with(&tmp_dir)
        || normalized_working.starts_with(Path::new("/tmp"))
        || normalized_working.starts_with(Path::new("/var/tmp"));
    if in_tmp {
        return false;
    }

    // working_dir 不在 /tmp 下时，允许访问 /tmp 下的项目临时文件
    let allowed_roots = [
        normalize_path(Path::new("/tmp")),
        normalize_path(Path::new("/var/tmp")),
        tmp_dir,
    ];
    let canonical_path = canonicalize_or_normalize(&normalized_path);
    allowed_roots
        .into_iter()
        .any(|root| normalized_path.starts_with(&root) || canonical_path.starts_with(&root))
}

pub fn is_allowed_read_absolute_path(path: &Path, working_dir: &Path) -> bool {
    if is_allowed_absolute_path(path, working_dir) {
        return true;
    }

    let normalized_path = normalize_path(path);
    let canonical_path = canonicalize_or_normalize(&normalized_path);
    read_allowed_roots().into_iter().any(|root| {
        let normalized_root = normalize_path(&root);
        let canonical_root = canonicalize_or_normalize(&normalized_root);
        normalized_path.starts_with(&normalized_root) || canonical_path.starts_with(&canonical_root)
    })
}

fn read_allowed_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(home).join("Desktop"));
    }
    if let Some(data_dir) = dirs::data_local_dir() {
        roots.push(data_dir.join("priority-agent").join("tool-results"));
    }
    if let Ok(raw) = std::env::var("PRIORITY_AGENT_READ_ROOTS") {
        roots.extend(
            raw.split(':')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(expand_home_path),
        );
    }
    roots
}

pub fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                let _ = normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

pub fn canonicalize_or_normalize(path: &Path) -> PathBuf {
    match std::fs::canonicalize(path) {
        Ok(p) => normalize_path(&p),
        Err(_) => normalize_path(path),
    }
}

fn realpath_deepest_existing(path: &Path) -> Result<PathBuf, String> {
    let mut current = path.to_path_buf();
    let mut deepest_existing: Option<PathBuf> = None;

    loop {
        if std::fs::symlink_metadata(&current).is_ok() {
            deepest_existing = Some(current.clone());
            break;
        }
        if !current.pop() {
            break;
        }
    }

    let deepest_existing = deepest_existing
        .ok_or_else(|| format!("Access denied: cannot resolve path '{}'", path.display()))?;

    let real_base = std::fs::canonicalize(&deepest_existing).map_err(|e| {
        format!(
            "Access denied: failed to resolve symlink for '{}': {}",
            path.display(),
            e
        )
    })?;

    let suffix = path
        .strip_prefix(&deepest_existing)
        .map_err(|_| format!("Access denied: invalid path '{}'", path.display()))?;

    Ok(normalize_path(&real_base.join(suffix)))
}
