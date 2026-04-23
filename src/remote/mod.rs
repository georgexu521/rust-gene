//! 远程开发支持
//!
//! 提供远程环境检测、SSH 会话管理和容器环境识别。
//! 复刻 Claude Code 的远程开发能力，让 agent 在 SSH/容器/WSL 等环境中正常工作。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ------------------------------------------------------------------------
// RemoteEnvDetector - 远程环境检测
// ------------------------------------------------------------------------

/// 远程环境类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteEnvType {
    /// 本地开发环境
    Local,
    /// SSH 连接
    Ssh,
    /// Docker 容器
    Docker,
    /// Windows Subsystem for Linux
    Wsl,
    /// GitHub Codespaces
    Codespaces,
    /// GitPod
    Gitpod,
    /// VS Code Remote 开发环境
    VsCodeRemote,
    /// 其他/未知远程环境
    Other,
}

impl std::fmt::Display for RemoteEnvType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RemoteEnvType::Local => write!(f, "local"),
            RemoteEnvType::Ssh => write!(f, "ssh"),
            RemoteEnvType::Docker => write!(f, "docker"),
            RemoteEnvType::Wsl => write!(f, "wsl"),
            RemoteEnvType::Codespaces => write!(f, "codespaces"),
            RemoteEnvType::Gitpod => write!(f, "gitpod"),
            RemoteEnvType::VsCodeRemote => write!(f, "vscode_remote"),
            RemoteEnvType::Other => write!(f, "other"),
        }
    }
}

/// 远程环境信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteEnvInfo {
    /// 环境类型
    pub env_type: RemoteEnvType,
    /// 是否为远程环境
    pub is_remote: bool,
    /// 主机名
    pub hostname: String,
    /// 用户名
    pub username: String,
    /// 工作目录
    pub working_dir: PathBuf,
    /// 检测到的环境变量
    pub detected_env_vars: Vec<String>,
    /// 额外元数据
    pub metadata: HashMap<String, String>,
}

impl Default for RemoteEnvInfo {
    fn default() -> Self {
        Self {
            env_type: RemoteEnvType::Local,
            is_remote: false,
            hostname: String::new(),
            username: String::new(),
            working_dir: PathBuf::from("."),
            detected_env_vars: Vec::new(),
            metadata: HashMap::new(),
        }
    }
}

/// 远程环境检测器
pub struct RemoteEnvDetector;

impl RemoteEnvDetector {
    /// 检测当前环境
    pub fn detect() -> RemoteEnvInfo {
        let env_vars: HashMap<String, String> = std::env::vars().collect();
        Self::detect_with_env(&env_vars)
    }

    /// 使用指定环境变量检测（便于测试）
    pub fn detect_with_env(env_vars: &HashMap<String, String>) -> RemoteEnvInfo {
        let hostname = env_vars
            .get("HOSTNAME")
            .or_else(|| env_vars.get("COMPUTERNAME"))
            .cloned()
            .unwrap_or_default();
        let username = env_vars
            .get("USER")
            .or_else(|| env_vars.get("USERNAME"))
            .cloned()
            .unwrap_or_default();
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        let mut env_type = RemoteEnvType::Local;
        let mut is_remote = false;
        let mut detected_env_vars = Vec::new();
        let mut metadata = HashMap::new();

        if let Some(et) = Self::detect_codespaces_env(env_vars) {
            env_type = et;
            is_remote = true;
            detected_env_vars.push("CODESPACES=true".to_string());
        } else if let Some(et) = Self::detect_gitpod_env(env_vars) {
            env_type = et;
            is_remote = true;
            detected_env_vars.push("GITPOD_WORKSPACE_ID".to_string());
        } else if let Some(et) = Self::detect_vscode_remote_env(env_vars) {
            env_type = et;
            is_remote = true;
            detected_env_vars.push("VSCODE_REMOTE".to_string());
        } else if let Some(et) = Self::detect_docker_env(env_vars) {
            env_type = et;
            is_remote = true;
            detected_env_vars.push("container=docker".to_string());
        } else if let Some(et) = Self::detect_wsl_env(env_vars) {
            env_type = et;
            is_remote = true;
            detected_env_vars.push("WSL_DISTRO_NAME".to_string());
        } else if let Some(et) = Self::detect_ssh_env(env_vars) {
            env_type = et;
            is_remote = true;
            detected_env_vars.push("SSH_CLIENT".to_string());
        }

        if is_remote {
            if let Some(container_id) = env_vars.get("HOSTNAME") {
                if env_type == RemoteEnvType::Docker {
                    metadata.insert("container_id".to_string(), container_id.clone());
                }
            }
            if let Some(workspace) = env_vars.get("CODESPACE_NAME") {
                metadata.insert("codespace_name".to_string(), workspace.clone());
            }
            if let Some(repo) = env_vars.get("GITHUB_REPOSITORY") {
                metadata.insert("github_repository".to_string(), repo.clone());
            }
            if let Some(distro) = env_vars.get("WSL_DISTRO_NAME") {
                metadata.insert("wsl_distro".to_string(), distro.clone());
            }
            if let Some(wsl_interop) = env_vars.get("WSL_INTEROP") {
                metadata.insert("wsl_interop".to_string(), wsl_interop.clone());
            }
        }

        RemoteEnvInfo {
            env_type,
            is_remote,
            hostname,
            username,
            working_dir,
            detected_env_vars,
            metadata,
        }
    }

    /// 检测是否在 GitHub Codespaces 中
    fn detect_codespaces() -> Option<RemoteEnvType> {
        let env_vars: HashMap<String, String> = std::env::vars().collect();
        Self::detect_codespaces_env(&env_vars)
    }

    fn detect_codespaces_env(env_vars: &HashMap<String, String>) -> Option<RemoteEnvType> {
        if env_vars.get("CODESPACES")? == "true" {
            Some(RemoteEnvType::Codespaces)
        } else {
            None
        }
    }

    /// 检测是否在 GitPod 中
    fn detect_gitpod() -> Option<RemoteEnvType> {
        let env_vars: HashMap<String, String> = std::env::vars().collect();
        Self::detect_gitpod_env(&env_vars)
    }

    fn detect_gitpod_env(env_vars: &HashMap<String, String>) -> Option<RemoteEnvType> {
        if env_vars.contains_key("GITPOD_WORKSPACE_ID") {
            Some(RemoteEnvType::Gitpod)
        } else {
            None
        }
    }

    /// 检测是否在 VS Code Remote 中
    fn detect_vscode_remote() -> Option<RemoteEnvType> {
        let env_vars: HashMap<String, String> = std::env::vars().collect();
        Self::detect_vscode_remote_env(&env_vars)
    }

    fn detect_vscode_remote_env(env_vars: &HashMap<String, String>) -> Option<RemoteEnvType> {
        if env_vars.contains_key("VSCODE_REMOTE") || env_vars.contains_key("REMOTE_CONTAINERS") {
            Some(RemoteEnvType::VsCodeRemote)
        } else {
            None
        }
    }

    /// 检测是否在 Docker 容器中
    fn detect_docker() -> Option<RemoteEnvType> {
        let env_vars: HashMap<String, String> = std::env::vars().collect();
        Self::detect_docker_env(&env_vars)
    }

    fn detect_docker_env(env_vars: &HashMap<String, String>) -> Option<RemoteEnvType> {
        if env_vars
            .get("container")
            .map(|s| s.contains("docker"))
            .unwrap_or(false)
        {
            return Some(RemoteEnvType::Docker);
        }
        if let Ok(content) = std::fs::read_to_string("/proc/1/cgroup") {
            if content.contains("docker") || content.contains("containerd") {
                return Some(RemoteEnvType::Docker);
            }
        }
        if std::path::Path::new("/.dockerenv").exists() {
            return Some(RemoteEnvType::Docker);
        }
        None
    }

    /// 检测是否在 WSL 中
    fn detect_wsl() -> Option<RemoteEnvType> {
        let env_vars: HashMap<String, String> = std::env::vars().collect();
        Self::detect_wsl_env(&env_vars)
    }

    fn detect_wsl_env(env_vars: &HashMap<String, String>) -> Option<RemoteEnvType> {
        if env_vars.contains_key("WSL_DISTRO_NAME") || env_vars.contains_key("WSL_INTEROP") {
            return Some(RemoteEnvType::Wsl);
        }
        if let Ok(content) = std::fs::read_to_string("/proc/version") {
            if content.contains("microsoft") || content.contains("Microsoft") {
                return Some(RemoteEnvType::Wsl);
            }
        }
        None
    }

    /// 检测是否通过 SSH 连接
    fn detect_ssh() -> Option<RemoteEnvType> {
        let env_vars: HashMap<String, String> = std::env::vars().collect();
        Self::detect_ssh_env(&env_vars)
    }

    fn detect_ssh_env(env_vars: &HashMap<String, String>) -> Option<RemoteEnvType> {
        if env_vars.contains_key("SSH_CLIENT")
            || env_vars.contains_key("SSH_CONNECTION")
            || env_vars.contains_key("SSH_TTY")
        {
            Some(RemoteEnvType::Ssh)
        } else {
            None
        }
    }
}

// ------------------------------------------------------------------------
// RemoteSession - 单个远程会话
// ------------------------------------------------------------------------

/// 远程会话状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteSessionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

/// 远程会话配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSessionConfig {
    /// 会话名称（用户可读标识）
    pub name: String,
    /// 主机地址
    pub host: String,
    /// SSH 端口
    pub port: u16,
    /// 用户名
    pub username: String,
    /// 认证方式
    pub auth: RemoteAuth,
    /// 远程工作目录
    pub remote_working_dir: Option<PathBuf>,
    /// 额外 SSH 选项
    pub ssh_options: Vec<String>,
}

impl Default for RemoteSessionConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            host: "localhost".to_string(),
            port: 22,
            username: std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .unwrap_or_else(|_| "user".to_string()),
            auth: RemoteAuth::Agent,
            remote_working_dir: None,
            ssh_options: Vec::new(),
        }
    }
}

/// 远程认证方式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteAuth {
    /// SSH Agent
    Agent,
    /// 私钥路径
    KeyFile { path: PathBuf },
    /// 密码（不推荐，仅用于测试）
    Password { password: String },
}

/// 远程会话
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSession {
    /// 会话 ID
    pub id: String,
    /// 配置
    pub config: RemoteSessionConfig,
    /// 状态
    pub status: RemoteSessionStatus,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 最后连接时间
    pub last_connected_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 错误信息
    pub error_message: Option<String>,
}

impl RemoteSession {
    pub fn new(id: impl Into<String>, config: RemoteSessionConfig) -> Self {
        Self {
            id: id.into(),
            config,
            status: RemoteSessionStatus::Disconnected,
            created_at: chrono::Utc::now(),
            last_connected_at: None,
            error_message: None,
        }
    }
}

// ------------------------------------------------------------------------
// RemoteSessionManager - 远程会话管理器
// ------------------------------------------------------------------------

/// 远程会话管理器
pub struct RemoteSessionManager {
    sessions: std::sync::Mutex<Vec<RemoteSession>>,
    persistence_path: PathBuf,
}

impl RemoteSessionManager {
    /// 创建新的管理器
    pub fn new() -> Self {
        let persistence_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".priority-agent")
            .join("remote_sessions.json");
        Self::with_path(persistence_path)
    }

    /// 使用指定持久化路径创建管理器
    pub fn with_path(persistence_path: PathBuf) -> Self {
        let sessions = Self::load_sessions(&persistence_path);
        Self {
            sessions: std::sync::Mutex::new(sessions),
            persistence_path,
        }
    }

    /// 创建并保存会话
    pub fn create_session(&self, config: RemoteSessionConfig) -> RemoteSession {
        let id = format!(
            "remote_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        let session = RemoteSession::new(id.clone(), config);
        {
            let mut sessions = self.sessions.lock().unwrap();
            sessions.push(session.clone());
        }
        self.save_sessions().ok();
        session
    }

    /// 列出所有会话
    pub fn list_sessions(&self) -> Vec<RemoteSession> {
        let sessions = self.sessions.lock().unwrap();
        sessions.clone()
    }

    /// 获取单个会话
    pub fn get_session(&self, id: &str) -> Option<RemoteSession> {
        let sessions = self.sessions.lock().unwrap();
        sessions.iter().find(|s| s.id == id).cloned()
    }

    /// 删除会话
    pub fn remove_session(&self, id: &str) -> bool {
        {
            let mut sessions = self.sessions.lock().unwrap();
            let len_before = sessions.len();
            sessions.retain(|s| s.id != id);
            if sessions.len() == len_before {
                return false;
            }
        }
        self.save_sessions().ok();
        true
    }

    /// 更新会话状态
    pub fn update_session_status(&self, id: &str, status: RemoteSessionStatus) -> bool {
        {
            let mut sessions = self.sessions.lock().unwrap();
            if let Some(session) = sessions.iter_mut().find(|s| s.id == id) {
                session.status = status;
                if status == RemoteSessionStatus::Connected {
                    session.last_connected_at = Some(chrono::Utc::now());
                    session.error_message = None;
                }
            } else {
                return false;
            }
        }
        self.save_sessions().ok();
        true
    }

    /// 设置会话错误
    pub fn set_session_error(&self, id: &str, error: impl Into<String>) -> bool {
        {
            let mut sessions = self.sessions.lock().unwrap();
            if let Some(session) = sessions.iter_mut().find(|s| s.id == id) {
                session.status = RemoteSessionStatus::Error;
                session.error_message = Some(error.into());
            } else {
                return false;
            }
        }
        self.save_sessions().ok();
        true
    }

    /// 生成 SSH 连接命令
    pub fn build_ssh_command(&self, id: &str) -> Option<std::process::Command> {
        let session = self.get_session(id)?;
        let mut cmd = std::process::Command::new("ssh");
        cmd.arg("-p").arg(session.config.port.to_string());

        for opt in &session.config.ssh_options {
            cmd.arg(opt);
        }

        match &session.config.auth {
            RemoteAuth::Agent => {
                // 默认使用 SSH agent
            }
            RemoteAuth::KeyFile { path } => {
                cmd.arg("-i").arg(path);
            }
            RemoteAuth::Password { .. } => {
                // 密码认证需要使用 sshpass 或交互式输入，这里不处理
            }
        }

        let target = format!("{}@{}", session.config.username, session.config.host);
        cmd.arg(target);

        if let Some(dir) = &session.config.remote_working_dir {
            cmd.arg("-t")
                .arg(format!("cd {} && exec $SHELL -l", dir.display()));
        }

        Some(cmd)
    }

    /// 执行远程命令（通过 SSH）
    pub async fn execute_remote(
        &self,
        id: &str,
        command: &str,
    ) -> anyhow::Result<(String, String, i32)> {
        let session = self
            .get_session(id)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", id))?;

        self.update_session_status(id, RemoteSessionStatus::Connecting);

        let mut cmd = tokio::process::Command::new("ssh");
        cmd.arg("-p")
            .arg(session.config.port.to_string())
            .arg("-o")
            .arg("StrictHostKeyChecking=no")
            .arg("-o")
            .arg("ConnectTimeout=10");

        for opt in &session.config.ssh_options {
            cmd.arg(opt);
        }

        match &session.config.auth {
            RemoteAuth::Agent => {}
            RemoteAuth::KeyFile { path } => {
                cmd.arg("-i").arg(path);
            }
            RemoteAuth::Password { .. } => {
                anyhow::bail!("Password authentication not supported in remote execution");
            }
        }

        let target = format!("{}@{}", session.config.username, session.config.host);
        cmd.arg(target).arg(command);

        let output = cmd.output().await?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let code = output.status.code().unwrap_or(-1);

        if output.status.success() {
            self.update_session_status(id, RemoteSessionStatus::Connected);
        } else {
            self.set_session_error(id, format!("Exit code {}: {}", code, stderr.trim()));
        }

        Ok((stdout, stderr, code))
    }

    // --- Persistence ---

    fn load_sessions(path: &PathBuf) -> Vec<RemoteSession> {
        if !path.exists() {
            return Vec::new();
        }
        let content = std::fs::read_to_string(path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    }

    fn save_sessions(&self) -> anyhow::Result<()> {
        let sessions = self.sessions.lock().unwrap();
        if let Some(parent) = self.persistence_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&*sessions)?;
        std::fs::write(&self.persistence_path, content)?;
        Ok(())
    }
}

impl Default for RemoteSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

// ------------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_env_detector_codespaces() {
        let mut env = HashMap::new();
        env.insert("CODESPACES".to_string(), "true".to_string());
        env.insert("CODESPACE_NAME".to_string(), "my-codespace".to_string());
        let info = RemoteEnvDetector::detect_with_env(&env);
        assert_eq!(info.env_type, RemoteEnvType::Codespaces);
        assert!(info.is_remote);
        assert_eq!(
            info.metadata.get("codespace_name"),
            Some(&"my-codespace".to_string())
        );
    }

    #[test]
    fn test_remote_env_detector_ssh() {
        let mut env = HashMap::new();
        env.insert("SSH_CLIENT".to_string(), "192.168.1.1 12345 22".to_string());
        let info = RemoteEnvDetector::detect_with_env(&env);
        assert_eq!(info.env_type, RemoteEnvType::Ssh);
        assert!(info.is_remote);
    }

    #[test]
    fn test_remote_env_detector_local() {
        let env = HashMap::new();
        let info = RemoteEnvDetector::detect_with_env(&env);
        assert_eq!(info.env_type, RemoteEnvType::Local);
        assert!(!info.is_remote);
    }

    #[test]
    fn test_session_manager_crud() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let manager = RemoteSessionManager::with_path(temp_file.path().to_path_buf());
        let config = RemoteSessionConfig {
            name: "test-server".to_string(),
            host: "example.com".to_string(),
            port: 2222,
            username: "admin".to_string(),
            ..Default::default()
        };

        let session = manager.create_session(config);
        assert!(!session.id.is_empty());
        assert_eq!(session.config.name, "test-server");
        assert_eq!(session.status, RemoteSessionStatus::Disconnected);

        let sessions = manager.list_sessions();
        assert_eq!(sessions.len(), 1);

        let found = manager.get_session(&session.id);
        assert!(found.is_some());

        let updated = manager.update_session_status(&session.id, RemoteSessionStatus::Connected);
        assert!(updated);

        let found = manager.get_session(&session.id).unwrap();
        assert_eq!(found.status, RemoteSessionStatus::Connected);
        assert!(found.last_connected_at.is_some());

        let removed = manager.remove_session(&session.id);
        assert!(removed);
        assert!(manager.get_session(&session.id).is_none());
    }

    #[test]
    fn test_build_ssh_command() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let manager = RemoteSessionManager::with_path(temp_file.path().to_path_buf());
        let config = RemoteSessionConfig {
            name: "prod".to_string(),
            host: "prod.example.com".to_string(),
            port: 2222,
            username: "deploy".to_string(),
            auth: RemoteAuth::KeyFile {
                path: PathBuf::from("~/.ssh/id_rsa"),
            },
            remote_working_dir: Some(PathBuf::from("/var/app")),
            ssh_options: vec!["-o".to_string(), "ServerAliveInterval=60".to_string()],
        };
        let session = manager.create_session(config);
        let cmd = manager.build_ssh_command(&session.id);
        assert!(cmd.is_some());
    }

    #[test]
    fn test_remote_env_type_display() {
        assert_eq!(RemoteEnvType::Ssh.to_string(), "ssh");
        assert_eq!(RemoteEnvType::Docker.to_string(), "docker");
        assert_eq!(RemoteEnvType::Local.to_string(), "local");
    }
}
