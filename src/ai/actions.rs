//! Operasi AI tingkat tinggi: membuat saran, alternatif, penjelasan istilah,
//! pertanyaan latihan, ringkasan, dan evaluasi sesi.
//!
//! Semua fungsi sinkron (dijalankan di worker thread, bukan UI thread).

use crate::ai::prompts::{self, PracticeReport, Suggestion};
use crate::ai::provider::{AiProvider, CompletionRequest};
use crate::config::{AnswerLength, Settings};
use crate::error::Result;
use crate::storage::profile::Profile;
use crate::storage::session::TranscriptSegment;

fn max_tokens_for(length: AnswerLength) -> u32 {
    match length {
        AnswerLength::Short => 600,
        AnswerLength::Medium => 1100,
        AnswerLength::Detailed => 1800,
    }
}

/// Buat saran jawaban untuk sebuah pertanyaan.
pub fn generate_suggestion(
    provider: &dyn AiProvider,
    settings: &Settings,
    profile: &Profile,
    context: &str,
    question: &str,
) -> Result<Suggestion> {
    let messages = prompts::suggestion_messages(settings, profile, context, question);
    let req = CompletionRequest::new(messages)
        .with_max_tokens(max_tokens_for(settings.ai.answer_length))
        .with_temperature(0.4);
    let raw = provider.complete(&req)?;
    prompts::parse_suggestion(&raw)
}

/// Buat alternatif jawaban dari draf sebelumnya.
pub fn generate_alternative(
    provider: &dyn AiProvider,
    settings: &Settings,
    profile: &Profile,
    question: &str,
    previous_answer: &str,
) -> Result<Suggestion> {
    let messages = prompts::alternative_messages(settings, profile, question, previous_answer);
    let req = CompletionRequest::new(messages)
        .with_max_tokens(max_tokens_for(settings.ai.answer_length))
        .with_temperature(0.7);
    let raw = provider.complete(&req)?;
    prompts::parse_suggestion(&raw)
}

/// Jelaskan istilah teknis singkat.
pub fn explain_term(
    provider: &dyn AiProvider,
    term: &str,
    context: &str,
) -> Result<String> {
    let messages = prompts::explain_term_messages(term, context);
    let req = CompletionRequest::new(messages)
        .with_max_tokens(500)
        .with_temperature(0.3);
    provider.complete(&req)
}

/// Minta pertanyaan berikutnya dari AI interviewer (Practice Mode).
pub fn ask_practice_question(
    provider: &dyn AiProvider,
    category: &str,
    asked: &[String],
    profile: &Profile,
    live: bool,
) -> Result<String> {
    let messages = prompts::practice_question_messages(category, asked, profile, live);
    let req = CompletionRequest::new(messages)
        .with_max_tokens(200)
        .with_temperature(0.8);
    let raw = provider.complete(&req)?;
    let q = raw.trim().trim_matches('"').trim().to_string();
    if q.is_empty() {
        return Err(crate::error::AppError::Ai("AI tidak mengembalikan pertanyaan".into()));
    }
    Ok(q)
}

/// Ringkas transkrip sesi.
pub fn summarize_session(
    provider: &dyn AiProvider,
    segments: &[TranscriptSegment],
) -> Result<String> {
    let transcript = transcript_plain_text(segments, 12_000);
    let messages = prompts::summary_messages(&transcript);
    let req = CompletionRequest::new(messages)
        .with_max_tokens(700)
        .with_temperature(0.3);
    provider.complete(&req)
}

/// Evaluasi sesi latihan lengkap.
pub fn evaluate_practice_session(
    provider: &dyn AiProvider,
    segments: &[TranscriptSegment],
    questions: &[String],
) -> Result<PracticeReport> {
    let transcript = transcript_plain_text(segments, 10_000);
    let messages = prompts::practice_evaluation_messages(&transcript, questions);
    let req = CompletionRequest::new(messages)
        .with_max_tokens(1200)
        .with_temperature(0.3);
    let raw = provider.complete(&req)?;
    prompts::parse_practice_report(&raw)
}

/// Teks transkrip polos (untuk prompt evaluasi/ringkasan).
pub fn transcript_plain_text(segments: &[TranscriptSegment], max_chars: usize) -> String {
    let mut out = String::new();
    for seg in segments {
        out.push_str(&format!(
            "[{}] {}: {}\n",
            crate::storage::format_duration_ms(seg.ts_ms),
            seg.source.label(),
            seg.text.trim()
        ));
        if out.len() >= max_chars {
            out.push_str("[...dipotong...]\n");
            break;
        }
    }
    out
}

/// Render laporan latihan menjadi Markdown.
pub fn render_report_markdown(r: &PracticeReport) -> String {
    let mut out = String::new();
    out.push_str("**Ringkasan Hasil Latihan**\n");
    out.push_str(r.overall.trim());
    out.push('\n');
    if !r.strengths.is_empty() {
        out.push_str("\n**Kekuatan**\n");
        for s in &r.strengths {
            out.push_str(&format!("- {}\n", s.trim()));
        }
    }
    if !r.improvements.is_empty() {
        out.push_str("\n**Perlu Diperbaiki**\n");
        for s in &r.improvements {
            out.push_str(&format!("- {}\n", s.trim()));
        }
    }
    if !r.recommendations.is_empty() {
        out.push_str("\n**Rekomendasi Latihan Berikutnya**\n");
        for s in &r.recommendations {
            out.push_str(&format!("- {}\n", s.trim()));
        }
    }
    out.trim_end().to_string()
}

/// Tes koneksi provider (dipakai tombol "Test connection" di Settings).
pub fn test_connection(provider: &dyn AiProvider) -> Result<String> {
    let req = CompletionRequest::new(vec![crate::ai::provider::Message::user(
        "Balas dengan satu kata: OK",
    )])
    .with_max_tokens(16)
    .with_temperature(0.0);
    let raw = provider.complete(&req)?;
    Ok(raw.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::provider::Message;
    use crate::error::{AppError, Result};
    use std::sync::Mutex;

    /// Provider palsu untuk test alur tanpa jaringan.
    struct FakeProvider {
        reply: Mutex<String>,
    }

    impl AiProvider for FakeProvider {
        fn name(&self) -> String {
            "fake".into()
        }
        fn complete(&self, _request: &CompletionRequest) -> Result<String> {
            Ok(self.reply.lock().unwrap().clone())
        }
    }

    fn seg(ts: i64, text: &str) -> TranscriptSegment {
        TranscriptSegment {
            ts_ms: ts,
            source: crate::storage::session::SegmentSource::Mic,
            text: text.into(),
        }
    }

    #[test]
    fn generates_suggestion_end_to_end() {
        let provider = FakeProvider {
            reply: Mutex::new(
                r#"{"answer":"Saya fokus di backend.","key_points":["fokus"],"technical_keywords":["Rust"],"follow_up_questions":["Q?"],"category":"introduction"}"#
                    .into(),
            ),
        };
        let settings = Settings::default();
        let profile = Profile::default();
        let s = generate_suggestion(&provider, &settings, &profile, "", "Tell me about yourself").unwrap();
        assert!(s.answer.contains("backend"));
    }

    #[test]
    fn propagates_provider_error() {
        struct FailingProvider;
        impl AiProvider for FailingProvider {
            fn name(&self) -> String {
                "fail".into()
            }
            fn complete(&self, _r: &CompletionRequest) -> Result<String> {
                Err(AppError::RateLimited { retry_after: 7 })
            }
        }
        let settings = Settings::default();
        let err = generate_suggestion(&FailingProvider, &settings, &Profile::default(), "", "Q?").unwrap_err();
        assert!(matches!(err, AppError::RateLimited { retry_after: 7 }));
    }

    #[test]
    fn practice_question_trims_quotes() {
        let provider = FakeProvider {
            reply: Mutex::new("\"Apa project favoritmu?\"".into()),
        };
        let q = ask_practice_question(&provider, "hr", &[], &Profile::default(), false).unwrap();
        assert_eq!(q, "Apa project favoritmu?");
    }

    #[test]
    fn plain_text_contains_timestamps() {
        let text = transcript_plain_text(&[seg(65_000, "halo")], 10_000);
        assert!(text.contains("00:01:05"));
        assert!(text.contains("halo"));
    }
}
