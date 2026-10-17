//! State yang dibagikan antara logika aplikasi (`app.rs`) dan UI.

use crate::ai::prompts::{PracticeReport, Suggestion};
use crate::audio::capture::AudioSourceKind;
use crate::storage::session::SessionKind;

/// Status aplikasi yang ditampilkan di widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetStatus {
    Idle,
    Listening,
    Paused,
    Processing,
    Error,
}

impl WidgetStatus {
    pub fn label(&self) -> &'static str {
        match self {
            WidgetStatus::Idle => "Idle",
            WidgetStatus::Listening => "Listening",
            WidgetStatus::Paused => "Paused",
            WidgetStatus::Processing => "Processing",
            WidgetStatus::Error => "Error",
        }
    }

    pub fn css_class(&self) -> &'static str {
        match self {
            WidgetStatus::Idle => "pill-idle",
            WidgetStatus::Listening => "pill-listening",
            WidgetStatus::Paused => "pill-paused",
            WidgetStatus::Processing => "pill-processing",
            WidgetStatus::Error => "pill-error",
        }
    }

    pub fn all_classes() -> [&'static str; 5] {
        [
            "pill-idle",
            "pill-listening",
            "pill-paused",
            "pill-processing",
            "pill-error",
        ]
    }
}

/// Event yang dikirim worker (audio/STT/AI/IPC) ke UI thread.
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// Potongan transkrip siap tampil.
    Segment {
        source: AudioSourceKind,
        ts_ms: i64,
        text: String,
    },
    /// Pertanyaan AI interviewer untuk Practice Mode.
    PracticeQuestion { text: String, from_ai: bool },
    /// Saran jawaban selesai digenerate.
    SuggestionReady {
        question_id: i64,
        suggestion: Suggestion,
    },
    /// Alternatif jawaban selesai.
    AlternativeReady {
        question_id: i64,
        suggestion: Suggestion,
    },
    /// Penjelasan istilah selesai.
    ExplanationReady(String),
    /// Ringkasan sesi selesai.
    SummaryReady(String),
    /// Laporan latihan selesai.
    PracticeReportReady(PracticeReport),
    /// Error yang perlu tampil di status widget.
    Error(String),
    /// Engine audio berhenti sendiri (mic terputus dsb).
    CaptureEnded {
        source: AudioSourceKind,
        message: Option<String>,
    },
}

/// Pilihan saat memulai sesi (hasil dialog izin).
#[derive(Debug, Clone)]
pub struct StartOptions {
    pub kind: SessionKind,
    pub mic_device: Option<String>,
    pub meeting_device: Option<String>,
    pub consent_confirmed: bool,
    pub external_ai_ok: bool,
}
