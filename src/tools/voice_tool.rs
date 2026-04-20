//! Voice 语音工具
//!
//! 提供文本朗读（TTS）和语音转写（STT）功能。
//! TTS 使用系统命令：macOS `say`、Linux `espeak`、Windows PowerShell。
//! STT 使用外部 Whisper 命令（需用户自行安装）。

use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// Voice 语音工具
pub struct VoiceTool;

#[async_trait]
impl Tool for VoiceTool {
    fn name(&self) -> &str {
        "voice"
    }

    fn description(&self) -> &str {
        "Use voice capabilities: 'speak' (read text aloud using system TTS), \
'status' (check voice backend availability), 'transcribe' (transcribe an audio file using Whisper). \
Use this when the user wants hands-free interaction or audio output."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["speak", "status", "transcribe"],
                    "description": "The voice action to perform"
                },
                "text": {
                    "type": "string",
                    "description": "Text to speak (for 'speak' action)"
                },
                "audio_path": {
                    "type": "string",
                    "description": "Path to audio file to transcribe (for 'transcribe' action)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("");
        if action.is_empty() {
            return ToolResult::error("Missing required parameter: action");
        }

        match action {
            "speak" => {
                let text = match params["text"].as_str() {
                    Some(t) => t,
                    None => return ToolResult::error("Missing required parameter: text"),
                };

                let vm = crate::voice::VoiceManager::new();
                match vm.speak(text).await {
                    Ok(()) => ToolResult::success(format!(
                        "Spoke text ({} chars) using {} backend",
                        text.len(),
                        vm.tts_name()
                    )),
                    Err(e) => ToolResult::error(format!("TTS failed: {}", e)),
                }
            }
            "status" => {
                let vm = crate::voice::VoiceManager::new();

                let tts_backend = crate::voice::tts::SystemTtsBackend::detect();
                let tts_available = tts_backend.is_available().await;

                let stt_backend = crate::voice::stt::WhisperSttBackend::new();
                let stt_available = stt_backend.is_available().await;

                let data = json!({
                    "tts": {
                        "backend": vm.tts_name(),
                        "available": tts_available,
                    },
                    "stt": {
                        "backend": vm.stt_name(),
                        "available": stt_available,
                    }
                });

                ToolResult::success_with_data(
                    format!(
                        "Voice status: TTS ({}) = {}, STT ({}) = {}",
                        vm.tts_name(),
                        if tts_available { "available" } else { "not available" },
                        vm.stt_name(),
                        if stt_available { "available" } else { "not available" },
                    ),
                    data,
                )
            }
            "transcribe" => {
                let audio_path = match params["audio_path"].as_str() {
                    Some(p) => std::path::PathBuf::from(p),
                    None => return ToolResult::error("Missing required parameter: audio_path"),
                };

                let vm = crate::voice::VoiceManager::new();
                match vm.transcribe_file(&audio_path).await {
                    Ok(text) => ToolResult::success(format!(
                        "Transcription result:\n{}",
                        text
                    )),
                    Err(e) => ToolResult::error(format!("Transcription failed: {}", e)),
                }
            }
            _ => ToolResult::error(format!("Unknown voice action: {}", action)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_tool_name() {
        let tool = VoiceTool;
        assert_eq!(tool.name(), "voice");
    }

    #[test]
    fn test_voice_tool_params() {
        let tool = VoiceTool;
        let params = tool.parameters();
        assert!(params.get("properties").is_some());
    }
}
