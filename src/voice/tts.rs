//! 文本转语音（TTS）后端
//!
//! 基于系统命令实现，无需额外音频 crate：
//! - macOS: `say`
//! - Linux: `espeak` 或 `spd-say`
//! - Windows: PowerShell System.Speech

use super::VoiceBackend;
use anyhow::{Context, Result};
use base64::Engine;
use std::process::Stdio;
use tracing::debug;

/// 系统命令 TTS 后端
pub struct SystemTtsBackend {
    platform: Platform,
}

#[derive(Debug, Clone, Copy)]
enum Platform {
    MacOS,
    Linux,
    Windows,
    Unknown,
}

impl SystemTtsBackend {
    pub fn detect() -> Self {
        let platform = if cfg!(target_os = "macos") {
            Platform::MacOS
        } else if cfg!(target_os = "linux") {
            Platform::Linux
        } else if cfg!(target_os = "windows") {
            Platform::Windows
        } else {
            Platform::Unknown
        };
        Self { platform }
    }

    /// 检查 TTS 命令是否可用
    pub async fn is_available(&self) -> bool {
        match self.platform {
            Platform::MacOS => command_exists("say").await,
            Platform::Linux => {
                command_exists("espeak").await || command_exists("spd-say").await
            }
            Platform::Windows => true, // PowerShell 内置于 Windows
            Platform::Unknown => false,
        }
    }
}

#[async_trait::async_trait]
impl VoiceBackend for SystemTtsBackend {
    fn name(&self) -> &str {
        "system_tts"
    }

    async fn synthesize(&self, _text: &str) -> Result<Vec<u8>> {
        // TTS 不返回音频字节，而是直接播放
        // 这里返回空字节，实际播放由 speak 方法处理
        Ok(Vec::new())
    }
}

impl SystemTtsBackend {
    /// 直接朗读文本（在 spawn_blocking 中执行，避免长时间占用 tokio worker 线程）
    pub async fn speak(&self, text: &str) -> Result<()> {
        debug!("TTS speaking: {}", &text[..text.len().min(100)]);

        let text = text.to_string();
        let platform = self.platform;
        tokio::task::spawn_blocking(move || {
            match platform {
                Platform::MacOS => {
                    let output = std::process::Command::new("say")
                        .arg(&text)
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::piped())
                        .output()
                        .context("Failed to spawn `say` command")?;

                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Err(anyhow::anyhow!("say command failed: {}", stderr));
                    }
                    Ok(())
                }
                Platform::Linux => {
                    // 优先尝试 espeak，其次是 spd-say
                    if command_exists_sync("espeak") {
                        let output = std::process::Command::new("espeak")
                            .arg(&text)
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::piped())
                            .output()
                            .context("Failed to spawn `espeak` command")?;

                        if !output.status.success() {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            return Err(anyhow::anyhow!("espeak command failed: {}", stderr));
                        }
                        Ok(())
                    } else if command_exists_sync("spd-say") {
                        let output = std::process::Command::new("spd-say")
                            .arg(&text)
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::piped())
                            .output()
                            .context("Failed to spawn `spd-say` command")?;

                        if !output.status.success() {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            return Err(anyhow::anyhow!("spd-say command failed: {}", stderr));
                        }
                        Ok(())
                    } else {
                        Err(anyhow::anyhow!(
                            "No TTS command found. Install `espeak` or `spd-say`."
                        ))
                    }
                }
                Platform::Windows => {
                    // 使用 Base64 编码避免命令注入（PowerShell -EncodedCommand 需要 UTF-16 LE）
                    let ps_script = format!(
                        "Add-Type -AssemblyName System.Speech; \
                         $synth = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
                         $synth.Speak('{}');",
                        text.replace('\'', "''")
                    );
                    let utf16_bytes: Vec<u8> = ps_script
                        .encode_utf16()
                        .flat_map(|c| c.to_le_bytes())
                        .collect();
                    let encoded = base64::engine::general_purpose::STANDARD.encode(&utf16_bytes);
                    let output = std::process::Command::new("powershell.exe")
                        .args(["-EncodedCommand", &encoded])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::piped())
                        .output()
                        .context("Failed to spawn PowerShell TTS command")?;

                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Err(anyhow::anyhow!(
                            "PowerShell TTS command failed: {}",
                            stderr
                        ));
                    }
                    Ok(())
                }
                Platform::Unknown => Err(anyhow::anyhow!(
                    "TTS not supported on this platform"
                )),
            }
        })
        .await
        .context("TTS speak task panicked")?
    }
}

/// 检查系统命令是否存在（异步）
async fn command_exists(cmd: &str) -> bool {
    let which_cmd = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };

    match tokio::process::Command::new(which_cmd)
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
    {
        Ok(status) => status.success(),
        Err(_) => false,
    }
}

/// 检查系统命令是否存在（同步版本，用于 spawn_blocking 内）
fn command_exists_sync(cmd: &str) -> bool {
    let which_cmd = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };

    match std::process::Command::new(which_cmd)
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(status) => status.success(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detection() {
        let backend = SystemTtsBackend::detect();
        if cfg!(target_os = "macos") {
            assert!(matches!(backend.platform, Platform::MacOS));
        } else if cfg!(target_os = "linux") {
            assert!(matches!(backend.platform, Platform::Linux));
        } else if cfg!(target_os = "windows") {
            assert!(matches!(backend.platform, Platform::Windows));
        }
    }

    #[tokio::test]
    async fn test_command_exists_say() {
        if cfg!(target_os = "macos") {
            assert!(command_exists("say").await);
        }
    }
}
