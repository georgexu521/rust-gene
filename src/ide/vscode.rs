//! VS Code / Cursor CLI 包装器
//!
//! 支持 `code --goto`, `code --reuse-window` 等操作

use std::path::{Path, PathBuf};
use tokio::process::Command;

/// VS Code / Cursor 客户端
pub struct VscodeClient {
    cli: String,
    is_cursor: bool,
}

impl VscodeClient {
    /// 创建新的 VS Code / Cursor 客户端
    pub fn new(cli: impl Into<String>, is_cursor: bool) -> Self {
        Self {
            cli: cli.into(),
            is_cursor,
        }
    }

    /// 尝试自动检测并创建客户端
    pub fn detect() -> Option<Self> {
        if let Ok(cursor) = std::process::Command::new("which").arg("cursor").output() {
            if cursor.status.success() {
                let path = String::from_utf8_lossy(&cursor.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(Self::new(path, true));
                }
            }
        }

        if let Ok(code) = std::process::Command::new("which").arg("code").output() {
            if code.status.success() {
                let path = String::from_utf8_lossy(&code.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(Self::new(path, false));
                }
            }
        }

        None
    }

    /// 获取 IDE 名称
    pub fn name(&self) -> &str {
        if self.is_cursor {
            "Cursor"
        } else {
            "VS Code"
        }
    }

    /// 打开文件（可选行和列）
    pub async fn open_file(
        &self,
        path: impl AsRef<Path>,
        line: Option<u32>,
        column: Option<u32>,
    ) -> anyhow::Result<String> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        let goto = match (line, column) {
            (Some(l), Some(c)) => format!("{}:{}:{}", path_str, l, c),
            (Some(l), None) => format!("{}:{}", path_str, l),
            _ => path_str,
        };

        let output = Command::new(&self.cli)
            .arg("--goto")
            .arg(&goto)
            .output()
            .await?;

        if output.status.success() {
            Ok(format!("Opened {} in {}", goto, self.name()))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("{} open failed: {}", self.name(), stderr))
        }
    }

    /// 在资源管理器中显示文件
    pub async fn reveal(&self, path: impl AsRef<Path>) -> anyhow::Result<String> {
        let output = Command::new(&self.cli)
            .arg("--reuse-window")
            .arg("--goto")
            .arg(path.as_ref())
            .output()
            .await?;

        if output.status.success() {
            Ok(format!(
                "Revealed {} in {}",
                path.as_ref().display(),
                self.name()
            ))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("{} reveal failed: {}", self.name(), stderr))
        }
    }

    /// 在集成终端中运行命令
    pub async fn run_in_terminal(&self, command: &str) -> anyhow::Result<String> {
        // 安全地序列化 JSON 参数，防止注入
        let args_json = serde_json::json!({ "text": format!("{}\n", command) });
        let args_str = serde_json::to_string(&args_json)?;

        let output = Command::new(&self.cli)
            .args([
                "--command",
                "workbench.action.terminal.sendSequence",
                "--args",
                &args_str,
            ])
            .output()
            .await?;

        if output.status.success() {
            Ok(format!("Sent command to {} terminal", self.name()))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!(
                "{} terminal command failed: {}",
                self.name(),
                stderr
            ))
        }
    }

    /// 获取当前打开的文件列表（尽力而为）
    pub async fn get_open_files(&self) -> anyhow::Result<Vec<String>> {
        // VS Code 没有直接的 CLI 来获取打开的文件列表
        // 尝试读取 .vscode/settings.json 或工作区状态（macOS 示例路径）
        let mut files = Vec::new();

        #[cfg(target_os = "macos")]
        {
            let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            let state_dir = if self.is_cursor {
                home.join("Library/Application Support/Cursor")
            } else {
                home.join("Library/Application Support/Code")
            };

            if let Ok(entries) = std::fs::read_dir(state_dir.join("Global Storage")) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    if name.to_string_lossy().starts_with("state.") {
                        // 无法直接解析二进制状态文件，返回提示
                        break;
                    }
                }
            }
        }

        if files.is_empty() {
            files.push("Unable to list open files via CLI.".to_string());
        }

        Ok(files)
    }
}

/// 便捷函数：在 VS Code / Cursor 中打开文件
pub async fn open_in_vscode(
    path: impl AsRef<Path>,
    line: Option<u32>,
    column: Option<u32>,
) -> anyhow::Result<String> {
    let client = VscodeClient::detect()
        .ok_or_else(|| anyhow::anyhow!("VS Code or Cursor CLI not found in PATH"))?;
    client.open_file(path, line, column).await
}

/// 便捷函数：检测当前可用的 VS Code / Cursor
pub fn detect_vscode() -> Option<VscodeClient> {
    VscodeClient::detect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vscode_client_name() {
        let cursor = VscodeClient::new("cursor", true);
        assert_eq!(cursor.name(), "Cursor");

        let code = VscodeClient::new("code", false);
        assert_eq!(code.name(), "VS Code");
    }
}
