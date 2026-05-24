use crate::{AleError, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// 云端API提供商
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CloudProvider {
    OpenAI,
    Anthropic,
    Google,
    Azure,
    Custom(String),
}

/// 云端API配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudConfig {
    pub provider: CloudProvider,
    pub api_key: String,
    pub api_url: String,
    pub model: String,
    pub max_tokens: usize,
    pub timeout: Duration,
    pub retry_count: u32,
}

impl Default for CloudConfig {
    fn default() -> Self {
        Self {
            provider: CloudProvider::OpenAI,
            api_key: String::new(),
            api_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o".to_string(),
            max_tokens: 1024,
            timeout: Duration::from_secs(30),
            retry_count: 3,
        }
    }
}

/// 云端API响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudResponse {
    pub content: String,
    pub tokens_used: usize,
    pub model: String,
    pub provider: CloudProvider,
}

/// 云端API trait
#[async_trait]
pub trait CloudApi: Send + Sync {
    /// 发送文本请求
    async fn chat(&self, messages: Vec<CloudMessage>) -> Result<CloudResponse>;

    /// 发送图像请求
    async fn vision(&self, image_data: &[u8], prompt: &str) -> Result<CloudResponse>;

    /// 语音识别
    async fn transcribe(&self, audio_data: &[u8]) -> Result<CloudResponse>;

    /// 语音合成
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>>;

    /// 检查连接状态
    async fn health_check(&self) -> Result<bool>;
}

/// 云端消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudMessage {
    pub role: String,
    pub content: String,
}

/// OpenAI API 实现
pub struct OpenAIApi {
    config: CloudConfig,
    client: reqwest::Client,
}

impl OpenAIApi {
    pub fn new(config: CloudConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .unwrap();

        Self { config, client }
    }
}

#[async_trait]
impl CloudApi for OpenAIApi {
    async fn chat(&self, messages: Vec<CloudMessage>) -> Result<CloudResponse> {
        let url = format!("{}/chat/completions", self.config.api_url);

        let request_body = serde_json::json!({
            "model": self.config.model,
            "messages": messages,
            "max_tokens": self.config.max_tokens,
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AleError::CloudApiError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AleError::CloudApiError(format!(
                "API error: {}",
                error_text
            )));
        }

        let response_body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AleError::CloudApiError(format!("Parse error: {}", e)))?;

        let content = response_body["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let tokens_used = response_body["usage"]["total_tokens"].as_u64().unwrap_or(0) as usize;

        Ok(CloudResponse {
            content,
            tokens_used,
            model: self.config.model.clone(),
            provider: self.config.provider.clone(),
        })
    }

    async fn vision(&self, image_data: &[u8], prompt: &str) -> Result<CloudResponse> {
        let url = format!("{}/chat/completions", self.config.api_url);

        // 将图像转换为base64
        let image_base64 = general_purpose::STANDARD.encode(image_data);

        let request_body = serde_json::json!({
            "model": "gpt-4o",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": prompt
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": format!("data:image/jpeg;base64,{}", image_base64)
                            }
                        }
                    ]
                }
            ],
            "max_tokens": self.config.max_tokens,
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AleError::CloudApiError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AleError::CloudApiError(format!(
                "API error: {}",
                error_text
            )));
        }

        let response_body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AleError::CloudApiError(format!("Parse error: {}", e)))?;

        let content = response_body["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let tokens_used = response_body["usage"]["total_tokens"].as_u64().unwrap_or(0) as usize;

        Ok(CloudResponse {
            content,
            tokens_used,
            model: "gpt-4o".to_string(),
            provider: CloudProvider::OpenAI,
        })
    }

    async fn transcribe(&self, audio_data: &[u8]) -> Result<CloudResponse> {
        let url = format!("{}/audio/transcriptions", self.config.api_url);

        // 创建multipart表单
        let form = reqwest::multipart::Form::new()
            .part(
                "file",
                reqwest::multipart::Part::bytes(audio_data.to_vec())
                    .file_name("audio.wav")
                    .mime_str("audio/wav")
                    .unwrap(),
            )
            .text("model", "whisper-1");

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| AleError::CloudApiError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AleError::CloudApiError(format!(
                "API error: {}",
                error_text
            )));
        }

        let response_body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AleError::CloudApiError(format!("Parse error: {}", e)))?;

        let text = response_body["text"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        Ok(CloudResponse {
            content: text,
            tokens_used: 0,
            model: "whisper-1".to_string(),
            provider: CloudProvider::OpenAI,
        })
    }

    async fn synthesize(&self, text: &str) -> Result<Vec<u8>> {
        let url = format!("{}/audio/speech", self.config.api_url);

        let request_body = serde_json::json!({
            "model": "tts-1",
            "input": text,
            "voice": "alloy",
            "response_format": "wav",
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AleError::CloudApiError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AleError::CloudApiError(format!(
                "API error: {}",
                error_text
            )));
        }

        let audio_data = response
            .bytes()
            .await
            .map_err(|e| AleError::CloudApiError(format!("Failed to read audio: {}", e)))?;

        Ok(audio_data.to_vec())
    }

    async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/models", self.config.api_url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .send()
            .await
            .map_err(|e| AleError::CloudApiError(format!("Health check failed: {}", e)))?;

        Ok(response.status().is_success())
    }
}

/// 云端API工厂
pub struct CloudApiFactory;

impl CloudApiFactory {
    pub fn create(config: CloudConfig) -> Box<dyn CloudApi> {
        match config.provider {
            CloudProvider::OpenAI => Box::new(OpenAIApi::new(config)),
            _ => {
                // 其他提供商的实现
                Box::new(OpenAIApi::new(config))
            }
        }
    }
}
