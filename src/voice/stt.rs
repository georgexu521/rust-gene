//! 语音转文本（STT）后端
//!
//! 基于外部 Whisper 命令实现，支持两种模式：
//! 1. 转写已有音频文件（调用 `whisper` / `whisper.cpp`）
//! 2. 录音后转写（使用系统录音命令 + whisper）

use super::VoiceBackend;
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Stdio;
use tracing::debug;

/// Whisper STT 后端
pub struct WhisperSttBackend {
    /// whisper 可执行文件路径（默认 "whisper"）
    whisper_cmd: String,
    /// 模型名称（如 "base", "small", "medium"）
    model: String,
    /// 语言（如 "en", "zh", "auto"）
    language: String,
}

impl WhisperSttBackend {
    pub fn new() -> Self {
        Self {
            whisper_cmd: std::env::var("PRIORITY_AGENT_WHISPER_CMD")
                .unwrap_or_else(|_| "whisper".to_string()),
            model: std::env::var("PRIORITY_AGENT_WHISPER_MODEL")
                .unwrap_or_else(|_| "base".to_string()),
            language: std::env::var("PRIORITY_AGENT_WHISPER_LANGUAGE")
                .unwrap_or_else(|_| "auto".to_string()),
        }
    }

    /// 检查 whisper 命令是否可用
    pub async fn is_available(&self) -> bool {
        command_exists(&self.whisper_cmd).await
    }

    /// 转写音频文件为文本
    pub async fn transcribe_file(&self, audio_path: impl AsRef<Path>) -> Result<String> {
        let path = audio_path.as_ref();
        if !path.exists() {
            return Err(anyhow::anyhow!("Audio file not found: {}", path.display()));
        }

        debug!("Transcribing audio file: {}", path.display());

        let mut cmd = tokio::process::Command::new(&self.whisper_cmd);
        cmd.arg(path)
            .arg("--model")
            .arg(&self.model)
            .arg("--output_format")
            .arg("txt")
            .arg("--output_dir")
            .arg(std::env::temp_dir())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if self.language != "auto" {
            cmd.arg("--language").arg(&self.language);
        }

        let output = cmd.output().await.with_context(|| {
            format!("Failed to run whisper command: {}", self.whisper_cmd)
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Whisper failed: {}", stderr));
        }

        // whisper 会生成 .txt 文件，读取它
        let txt_path = path.with_extension("txt");
        let text = if txt_path.exists() {
            tokio::fs::read_to_string(&txt_path)
                .await
                .with_context(|| format!("Failed to read transcription output: {}", txt_path.display()))?
        } else {
            // 如果 whisper 输出到 stdout，直接使用
            String::from_utf8_lossy(&output.stdout).to_string()
        };

        let trimmed = text.trim().to_string();
        if trimmed.is_empty() {
            return Err(anyhow::anyhow!("Transcription result is empty"));
        }

        debug!("Transcription result: {}", &trimmed[..trimmed.len().min(100)]);
        Ok(trimmed)
    }

    /// 录音到指定路径（使用系统录音命令）
    pub async fn record_to_file(
        &self,
        output_path: impl AsRef<Path>,
        duration_secs: u64,
    ) -> Result<()> {
        let path = output_path.as_ref();
        debug!("Recording audio for {}s to {}", duration_secs, path.display());

        let platform = detect_platform();
        match platform {
            Platform::MacOS => {
                // 使用 sox (rec) 或 afrecord
                if command_exists("rec").await {
                    let output = tokio::process::Command::new("rec")
                        .args([
                            "-r", "16000", "-c", "1", "-b", "16",
                            path.to_str().unwrap_or("/tmp/recorded.wav"),
                            "trim", "0", &format!("{}", duration_secs),
                        ])
                        .stdout(Stdio::null())
                        .stderr(Stdio::piped())
                        .output()
                        .await
                        .context("Failed to run `rec` command")?;

                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Err(anyhow::anyhow!("Recording failed: {}", stderr));
                    }
                    Ok(())
                } else if command_exists("ffmpeg").await {
                    let output = tokio::process::Command::new("ffmpeg")
                        .args([
                            "-f", "avfoundation",
                            "-i", ":0",
                            "-t", &format!("{}", duration_secs),
                            "-ar", "16000",
                            "-ac", "1",
                            path.to_str().unwrap_or("/tmp/recorded.wav"),
                            "-y",
                        ])
                        .stdout(Stdio::null())
                        .stderr(Stdio::piped())
                        .output()
                        .await
                        .context("Failed to run `ffmpeg` command")?;

                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Err(anyhow::anyhow!("Recording failed: {}", stderr));
                    }
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "No recording command found. Install `sox` (provides `rec`) or `ffmpeg`."
                    ))
                }
            }
            Platform::Linux => {
                if command_exists("arecord").await {
                    let output = tokio::process::Command::new("arecord")
                        .args([
                            "-D", "plughw:1,0",
                            "-f", "S16_LE",
                            "-r", "16000",
                            "-c", "1",
                            "-d", &format!("{}", duration_secs),
                            path.to_str().unwrap_or("/tmp/recorded.wav"),
                        ])
                        .stdout(Stdio::null())
                        .stderr(Stdio::piped())
                        .output()
                        .await
                        .context("Failed to run `arecord` command")?;

                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Err(anyhow::anyhow!("Recording failed: {}", stderr));
                    }
                    Ok(())
                } else if command_exists("ffmpeg").await {
                    let output = tokio::process::Command::new("ffmpeg")
                        .args([
                            "-f", "alsa",
                            "-i", "default",
                            "-t", &format!("{}", duration_secs),
                            "-ar", "16000",
                            "-ac", "1",
                            path.to_str().unwrap_or("/tmp/recorded.wav"),
                            "-y",
                        ])
                        .stdout(Stdio::null())
                        .stderr(Stdio::piped())
                        .output()
                        .await
                        .context("Failed to run `ffmpeg` command")?;

                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Err(anyhow::anyhow!("Recording failed: {}", stderr));
                    }
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "No recording command found. Install `alsa-utils` (provides `arecord`) or `ffmpeg`."
                    ))
                }
            }
            Platform::Windows => {
                // Windows 没有内置命令行录音工具，提示用户使用 PowerShell 脚本或其他工具
                Err(anyhow::anyhow!(
                    "Command-line recording on Windows requires additional setup. \
                     Please use an audio recording tool and provide the file path to transcribe."
                ))
            }
            Platform::Unknown => Err(anyhow::anyhow!("Recording not supported on this platform")),
        }
    }
}

#[async_trait::async_trait]
impl VoiceBackend for WhisperSttBackend {
    fn name(&self) -> &str {
        "whisper_stt"
    }

    async fn transcribe(&self, audio_bytes: &[u8]) -> Result<String> {
        if audio_bytes.is_empty() {
            return Err(anyhow::anyhow!("Empty audio data"));
        }

        // 将字节写入临时文件，然后转写
        let tmp_path = std::env::temp_dir().join("voice_input.wav");
        tokio::fs::write(&tmp_path, audio_bytes)
            .await
            .context("Failed to write temporary audio file")?;

        self.transcribe_file(&tmp_path).await
    }
}

#[derive(Debug, Clone, Copy)]
enum Platform {
    MacOS,
    Linux,
    Windows,
    Unknown,
}

fn detect_platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::MacOS
    } else if cfg!(target_os = "linux") {
        Platform::Linux
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else {
        Platform::Unknown
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whisper_backend_default() {
        let backend = WhisperSttBackend::new();
        assert_eq!(backend.name(), "whisper_stt");
    }

    #[tokio::test]
    async fn test_command_exists_whisper() {
        // whisper 可能未安装，但函数不应 panic
        let _ = command_exists("whisper").await;
    }
}
