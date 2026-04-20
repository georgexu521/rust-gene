//! IDE 集成模块
//!
//! 检测并集成宿主 IDE：VS Code、Cursor、JetBrains 等

pub mod vscode;

use std::path::Path;

/// 检测到的 IDE 类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdeKind {
    VsCode,
    Cursor,
    JetBrains,
    Unknown,
}

/// IDE 环境检测
pub struct IdeDetector;

impl IdeDetector {
    /// 检测当前使用的 IDE
    pub fn detect() -> IdeKind {
        // 1. 检查环境变量
        if std::env::var("CURSOR_TRACE_ID").is_ok()
            || std::env::var("CURSOR_TERMINAL_EDITOR").is_ok()
        {
            return IdeKind::Cursor;
        }

        if std::env::var("TERM_PROGRAM")
            .map(|v| v == "vscode")
            .unwrap_or(false)
        {
            return IdeKind::VsCode;
        }

        // 2. 检查进程名
        if Self::is_process_running("Cursor") || Self::is_process_running("cursor") {
            return IdeKind::Cursor;
        }

        if Self::is_process_running("Code") || Self::is_process_running("code") {
            return IdeKind::VsCode;
        }

        // 3. 检查 .cursorrules 文件
        if Path::new(".cursorrules").exists() {
            return IdeKind::Cursor;
        }

        // 4. 检查 .vscode 目录
        if Path::new(".vscode").is_dir() {
            return IdeKind::VsCode;
        }

        IdeKind::Unknown
    }

    /// 获取当前 IDE 的 CLI 命令
    pub fn cli_command() -> Option<String> {
        match Self::detect() {
            IdeKind::VsCode => Some(find_executable("code").unwrap_or_else(|| "code".to_string())),
            IdeKind::Cursor => {
                Some(find_executable("cursor").unwrap_or_else(|| "cursor".to_string()))
            }
            _ => None,
        }
    }

    /// 检查进程是否正在运行（简单实现）
    #[cfg(target_os = "macos")]
    fn is_process_running(name: &str) -> bool {
        let output = std::process::Command::new("pgrep")
            .arg("-x")
            .arg(name)
            .output();
        matches!(output, Ok(out) if out.status.success())
    }

    #[cfg(target_os = "linux")]
    fn is_process_running(name: &str) -> bool {
        let output = std::process::Command::new("pgrep")
            .arg("-x")
            .arg(name)
            .output();
        matches!(output, Ok(out) if out.status.success())
    }

    #[cfg(target_os = "windows")]
    fn is_process_running(name: &str) -> bool {
        let output = std::process::Command::new("tasklist")
            .arg("/FI")
            .arg(format!("IMAGENAME eq {}.exe", name))
            .output();
        matches!(output, Ok(out) if String::from_utf8_lossy(&out.stdout).contains(name))
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    fn is_process_running(_name: &str) -> bool {
        false
    }
}

/// 查找可执行文件路径
fn find_executable(name: &str) -> Option<String> {
    let output = std::process::Command::new("which")
        .arg(name)
        .output()
        .ok()?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Some(path);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_returns_some_kind() {
        let kind = IdeDetector::detect();
        // 至少能返回一个值，不 panic
        let _ = format!("{:?}", kind);
    }
}
