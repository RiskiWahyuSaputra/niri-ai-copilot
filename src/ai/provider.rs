//! Abstraksi provider AI. Implementasi: `groq` (OpenAI-compatible endpoint).

use std::sync::Arc;

use serde::Serialize;

use crate::config::Settings;
use crate::error::Result;

#[derive(Debug, Clone, Serialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub messages: Vec<Message>,
    pub temperature: f32,
    pub max_tokens: u32,
}

impl CompletionRequest {
    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            messages,
            temperature: 0.3,
            max_tokens: 1024,
        }
    }

    pub fn with_max_tokens(mut self, n: u32) -> Self {
        self.max_tokens = n;
        self
    }

    pub fn with_temperature(mut self, t: f32) -> Self {
        self.temperature = t;
        self
    }
}

/// Provider AI teks.
pub trait AiProvider: Send + Sync {
    fn name(&self) -> String;

    /// Kirim request dan kembalikan konten teks respons.
    fn complete(&self, request: &CompletionRequest) -> Result<String>;
}

/// Bangun provider dari settings; error `MissingApiKey` bila key belum diatur.
pub fn build_provider(settings: &Settings) -> Result<Arc<dyn AiProvider>> {
    let provider = settings.ai.provider.as_str();
    match provider {
        "groq" => {
            let api_key = settings
                .resolve_api_key()
                .ok_or_else(|| crate::error::AppError::MissingApiKey(settings.ai.api_key_env.clone()))?;
            Ok(Arc::new(crate::ai::groq::GroqProvider::new(
                api_key,
                &settings.ai.base_url,
                &settings.ai.model,
                settings.ai.timeout_secs,
                settings.ai.max_retries,
            )))
        }
        other => Err(crate::error::AppError::Config(format!(
            "provider AI `{other}` tidak dikenal (saat ini: groq)"
        ))),
    }
}
