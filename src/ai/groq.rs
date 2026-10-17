//! Provider Groq (endpoint OpenAI-compatible `/chat/completions`).
//!
//! Aturan keamanan & keandalan:
//! - API key hanya dikirim via header Authorization; tidak pernah di-log.
//! - Semua pesan error melewati `redact_secrets` agar key tidak bocor.
//! - Retry maksimum terbatas (`max_retries`), hanya untuk 429/5xx/error
//!   jaringan, dengan backoff eksponensial dan menghormati `Retry-After`.

use std::time::Duration;

use serde_json::json;

use crate::ai::provider::{AiProvider, CompletionRequest};
use crate::error::{AppError, Result};
use crate::privacy::redact::redact_secrets;

pub struct GroqProvider {
    api_key: String,
    base_url: String,
    model: String,
    agent: ureq::Agent,
    max_retries: u32,
}

impl GroqProvider {
    pub fn new(
        api_key: String,
        base_url: &str,
        model: &str,
        timeout_secs: u64,
        max_retries: u32,
    ) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(timeout_secs.clamp(5, 180))))
            .http_status_as_error(false)
            .user_agent("niri-ai-copilot/0.1")
            .build();
        Self {
            api_key,
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            agent: ureq::Agent::from(config),
            max_retries: max_retries.min(5),
        }
    }

    fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }

    fn post_once(&self, request: &CompletionRequest) -> std::result::Result<HttpOutcome, AppError> {
        let body = json!({
            "model": self.model,
            "messages": request.messages,
            "temperature": request.temperature,
            "max_tokens": request.max_tokens,
        });
        let mut resp = self
            .agent
            .post(&self.endpoint())
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .send_json(&body)
            .map_err(map_transport_error)?;

        let status = resp.status().as_u16();
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok());

        if !(200..300).contains(&status) {
            let body_text = resp
                .body_mut()
                .read_to_string()
                .unwrap_or_else(|_| String::new());
            return Err(classify_http_error(status, &body_text, &self.api_key, retry_after));
        }

        let value: serde_json::Value = resp
            .body_mut()
            .read_json()
            .map_err(|e| AppError::Ai(format!("respons provider tidak valid: {e}")))?;
        let content = value
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| AppError::Ai("respons provider tidak berisi teks".into()))?;

        Ok(HttpOutcome {
            content,
            prompt_tokens: value
                .get("usage")
                .and_then(|u| u.get("prompt_tokens"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            completion_tokens: value
                .get("usage")
                .and_then(|u| u.get("completion_tokens"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
        })
    }
}

struct HttpOutcome {
    content: String,
    prompt_tokens: u64,
    completion_tokens: u64,
}

impl AiProvider for GroqProvider {
    fn name(&self) -> String {
        format!("groq ({})", self.model)
    }

    fn complete(&self, request: &CompletionRequest) -> Result<String> {
        let mut attempt: u32 = 0;
        loop {
            match self.post_once(request) {
                Ok(outcome) => {
                    let _ = (outcome.prompt_tokens, outcome.completion_tokens); // metrik lokal, tidak dikirim ke mana pun
                    return Ok(outcome.content);
                }
                Err(e) => {
                    let retryable = matches!(
                        e,
                        AppError::RateLimited { .. } | AppError::Network(_) | AppError::Timeout
                    );
                    if !retryable || attempt >= self.max_retries {
                        return Err(e);
                    }
                    let wait = match &e {
                        AppError::RateLimited { retry_after } => *retry_after,
                        _ => 1u64 << attempt.min(3),
                    };
                    attempt += 1;
                    std::thread::sleep(Duration::from_secs(wait.clamp(1, 60)));
                }
            }
        }
    }
}

fn map_transport_error(e: ureq::Error) -> AppError {
    match e {
        ureq::Error::Timeout(_) => AppError::Timeout,
        other => AppError::Network(other.to_string()),
    }
}

fn classify_http_error(
    status: u16,
    body_text: &str,
    api_key: &str,
    retry_after: Option<u64>,
) -> AppError {
    let scrubbed_body = redact_secrets(&redact_payload(body_text), &[api_key]);
    match status {
        401 | 403 => AppError::Ai(
            "API key ditolak provider (401/403). Periksa kembali API key di Settings.".into(),
        ),
        429 => AppError::RateLimited {
            retry_after: retry_after.unwrap_or(20),
        },
        400 => AppError::Ai(format!(
            "permintaan ditolak provider (400): {scrubbed_body} — periksa nama model di Settings."
        )),
        s if s >= 500 => AppError::Ai(format!("provider bermasalah (HTTP {s}): {scrubbed_body}")),
        s => AppError::Ai(format!("provider mengembalikan HTTP {s}: {scrubbed_body}")),
    }
}

/// Ambil pesan error JSON `{"error":{"message":..}}` bila ada, agar pesan rapi.
fn redact_payload(body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(msg) = v
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
        {
            return truncate(msg, 300);
        }
    }
    truncate(body, 300)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_401() {
        let e = classify_http_error(401, "unauthorized", "gsk_secret123456789", None);
        assert!(matches!(e, AppError::Ai(_)));
        assert!(!e.to_string().contains("gsk_secret"));
    }

    #[test]
    fn classifies_429_with_retry_after() {
        let e = classify_http_error(429, "", "k", Some(42));
        match e {
            AppError::RateLimited { retry_after } => assert_eq!(retry_after, 42),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn extracts_json_error_message() {
        let body = r#"{"error":{"message":"model not found"}}"#;
        assert_eq!(redact_payload(body), "model not found");
    }
}
