//! 语音模块
//!
//! 提供文本转语音（TTS）和语音转文本（STT）能力。
//! TTS 基于系统命令（macOS `say`、Linux `espeak`、Windows PowerShell）。
//! STT 基于外部 Whisper 命令（需用户自行安装）。

use anyhow::Result;

pub mod stt;
pub mod tts;

pub use stt::WhisperSttBackend;
pub use tts::SystemTtsBackend;

/// 语音后端 trait
#[async_trait::async_trait]
pub trait VoiceBackend: Send + Sync {
    fn name(&self) -> &str;
    async fn transcribe(&self, _audio_bytes: &[u8]) -> Result<String> {
        Err(anyhow::anyhow!("voice transcription not implemented"))
    }
    async fn synthesize(&self, _text: &str) -> Result<Vec<u8>> {
        Err(anyhow::anyhow!("voice synthesis not implemented"))
    }
}

/// 默认空实现
pub struct NoopVoiceBackend;

#[async_trait::async_trait]
impl VoiceBackend for NoopVoiceBackend {
    fn name(&self) -> &str {
        "noop"
    }
}

/// 语音管理器
pub struct VoiceManager {
    tts: SystemTtsBackend,
    stt: WhisperSttBackend,
}

impl VoiceManager {
    pub fn new() -> Self {
        Self {
            tts: SystemTtsBackend::detect(),
            stt: WhisperSttBackend::new(),
        }
    }

    /// TTS 后端名称
    pub fn tts_name(&self) -> &str {
        self.tts.name()
    }

    /// STT 后端名称
    pub fn stt_name(&self) -> &str {
        self.stt.name()
    }

    /// 直接朗读文本
    pub async fn speak(&self, text: &str) -> Result<()> {
        self.tts.speak(text).await
    }

    /// 转写音频文件
    pub async fn transcribe_file(&self, path: &std::path::Path) -> Result<String> {
        self.stt.transcribe_file(path).await
    }

    /// 录音并转写
    pub async fn listen(&self, duration_secs: u64) -> Result<String> {
        let tmp_path = std::env::temp_dir().join("voice_recorded.wav");
        self.stt.record_to_file(&tmp_path, duration_secs).await?;
        self.transcribe_file(&tmp_path).await
    }

    /// 检查 TTS 是否可用
    pub async fn tts_available(&self) -> bool {
        self.tts.is_available().await
    }

    /// 检查 STT 是否可用
    pub async fn stt_available(&self) -> bool {
        self.stt.is_available().await
    }
}

impl Default for VoiceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_manager_default() {
        let vm = VoiceManager::new();
        assert_eq!(vm.tts_name(), "system_tts");
        assert_eq!(vm.stt_name(), "whisper_stt");
    }

    #[test]
    fn test_noop_backend() {
        let noop = NoopVoiceBackend;
        assert_eq!(noop.name(), "noop");
    }
}
