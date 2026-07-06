use crate::{AleError, Result};
use async_trait::async_trait;
use std::sync::Mutex;

/// 语音合成trait
#[async_trait]
pub trait TextToSpeech: Send + Sync {
    /// 合成语音
    ///
    /// 对于 SystemTts，音频会直接通过系统扬声器播放。
    /// 返回的 Vec<u8> 在系统 TTS 场景下为空，调用方应检查长度判断是否需要云端 TTS 回退。
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>>;

    /// 流式合成语音
    async fn synthesize_stream(&self, text: &str) -> Result<Box<dyn tokio::io::AsyncRead + Unpin>>;

    /// 获取可用语音列表
    fn available_voices(&self) -> Vec<String>;

    /// 获取模型信息
    fn model_info(&self) -> crate::ModelInfo;
}

/// 系统TTS引擎
///
/// 使用操作系统内置的 TTS 引擎：
/// - macOS: AVSpeechSynthesizer (via tts crate)
/// - Windows: SAPI
/// - Linux: speech-dispatcher
///
/// `speak()` 通过系统扬声器直接播放音频，不返回原始音频字节。
pub struct SystemTts {
    voice: Option<String>,
    // tts::Tts 的方法需要 &mut self，用 Mutex 提供内部可变性
    tts_engine: Mutex<Option<tts::Tts>>,
}

impl SystemTts {
    pub async fn new(voice: Option<&str>) -> Result<Self> {
        let voice = voice.map(|v| v.to_string());

        Ok(Self {
            voice,
            tts_engine: Mutex::new(None),
        })
    }

    fn ensure_initialized(&self) -> Result<()> {
        let mut guard = self
            .tts_engine
            .lock()
            .map_err(|e| AleError::TtsError(format!("TTS engine lock poisoned: {}", e)))?;

        if guard.is_some() {
            return Ok(());
        }

        let mut engine = tts::Tts::default()
            .map_err(|e| AleError::TtsError(format!("Failed to initialize TTS engine: {}", e)))?;

        // 设置语音
        if let Some(voice_name) = &self.voice {
            let voices = engine
                .voices()
                .map_err(|e| AleError::TtsError(format!("Failed to get voices: {}", e)))?;

            for v in &voices {
                if v.name() == *voice_name {
                    engine
                        .set_voice(v)
                        .map_err(|e| AleError::TtsError(format!("Failed to set voice: {}", e)))?;
                    break;
                }
            }
        }

        *guard = Some(engine);
        Ok(())
    }
}

#[async_trait]
impl TextToSpeech for SystemTts {
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>> {
        self.ensure_initialized()?;

        let mut guard = self
            .tts_engine
            .lock()
            .map_err(|e| AleError::TtsError(format!("TTS engine lock poisoned: {}", e)))?;

        let engine = guard
            .as_mut()
            .ok_or_else(|| AleError::TtsError("TTS engine not initialized".to_string()))?;

        // 通过系统扬声器播放语音
        engine
            .speak(text, false)
            .map_err(|e| AleError::TtsError(format!("TTS speak failed: {}", e)))?;

        // 系统 TTS 直接播放音频，不返回原始字节
        // 返回空 Vec 表示播放成功，调用方可以据此判断
        Ok(Vec::new())
    }

    async fn synthesize_stream(&self, text: &str) -> Result<Box<dyn tokio::io::AsyncRead + Unpin>> {
        // 系统 TTS 不支持流式音频字节输出
        // 先通过 synthesize 播放，然后返回空的 reader
        self.synthesize(text).await?;

        Ok(Box::new(tokio::io::empty()))
    }

    fn available_voices(&self) -> Vec<String> {
        // 尝试从引擎获取实际可用语音列表
        if let Ok(mut guard) = self.tts_engine.lock() {
            if guard.is_none() {
                // 尝试初始化以获取语音列表
                if let Ok(engine) = tts::Tts::default() {
                    if let Ok(voices) = engine.voices() {
                        let names: Vec<String> = voices.iter().map(|v| v.name().to_string()).collect();
                        if !names.is_empty() {
                            *guard = Some(engine);
                            return names;
                        }
                    }
                    *guard = Some(engine);
                }
            }

            if let Some(ref engine) = *guard {
                if let Ok(voices) = engine.voices() {
                    return voices.iter().map(|v| v.name().to_string()).collect();
                }
            }
        }

        // 回退：返回常见默认语音名
        vec![
            "default".to_string(),
            "male".to_string(),
            "female".to_string(),
        ]
    }

    fn model_info(&self) -> crate::ModelInfo {
        let loaded = self
            .tts_engine
            .lock()
            .map(|g| g.is_some())
            .unwrap_or(false);

        crate::ModelInfo {
            name: "system-tts".to_string(),
            version: "1.0".to_string(),
            device: "system".to_string(),
            loaded,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_system_tts_new() {
        let tts = SystemTts::new(None).await;
        assert!(tts.is_ok());
    }

    #[tokio::test]
    async fn test_system_tts_with_voice() {
        let tts = SystemTts::new(Some("female")).await;
        assert!(tts.is_ok());
    }

    #[tokio::test]
    async fn test_model_info_not_loaded() {
        let tts = SystemTts::new(None).await.unwrap();
        let info = tts.model_info();
        assert_eq!(info.name, "system-tts");
        assert!(!info.loaded);
    }
}
