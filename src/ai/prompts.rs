//! Prompt builder untuk seluruh fitur AI + parser respons yang toleran.
//!
//! Semua prompt memuat aturan anti-fabrikasi: model dilarang mengarang
//! pengalaman, sertifikasi, atau keahlian yang tidak ada di profil pengguna.

use serde::{Deserialize, Serialize};

use crate::ai::provider::Message;
use crate::config::{AnswerLength, AnswerStyle, Settings};
use crate::error::{AppError, Result};
use crate::storage::profile::Profile;
use crate::storage::session::TranscriptSegment;

/// Saran jawaban terstruktur.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Suggestion {
    pub answer: String,
    pub key_points: Vec<String>,
    pub technical_keywords: Vec<String>,
    pub follow_up_questions: Vec<String>,
    pub category: Option<String>,
}

/// Laporan hasil latihan interview.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PracticeReport {
    pub overall: String,
    pub strengths: Vec<String>,
    pub improvements: Vec<String>,
    pub recommendations: Vec<String>,
}

const ANTI_FABRICATION_RULE: &str = "\
ATURAN KERAS:
- JANGAN mengarang pengalaman, project, sertifikasi, prestasi, atau keahlian yang tidak ada di profil pengguna.
- Jika informasi yang dibutuhkan tidak ada di profil, tulis penanda [PERLU DILENGKAPI: <bagian apa>] pada jawaban.
- Jangan menyembunyikan ketidakpastian. Lebih baik jujur menandai data yang kurang.";

/// Gaya jawaban dalam instruksi prompt.
fn style_instruction(style: AnswerStyle) -> &'static str {
    match style {
        AnswerStyle::Concise => {
            "Gaya: CONCISE — jawaban ringkas dan langsung ke inti, tanpa basa-basi."
        }
        AnswerStyle::Professional => {
            "Gaya: PROFESSIONAL — sopan, terstruktur, cocok untuk interview kerja formal."
        }
        AnswerStyle::Technical => {
            "Gaya: TECHNICAL — tekankan detail teknis, arsitektur, trade-off, dan istilah yang tepat untuk posisi developer."
        }
        AnswerStyle::Star => {
            "Gaya: STAR — susun jawaban dengan struktur Situation, Task, Action, Result secara eksplisit."
        }
        AnswerStyle::BeginnerFriendly => {
            "Gaya: BEGINNER FRIENDLY — jelaskan dengan bahasa sederhana dan analogi, seolah menjelaskan kepada pemula."
        }
        AnswerStyle::EnglishInterview => {
            "Gaya: ENGLISH INTERVIEW — jawab dalam bahasa Inggris profesional, natural seperti percakapan interview nyata."
        }
    }
}

fn length_instruction(length: AnswerLength) -> &'static str {
    match length {
        AnswerLength::Short => "Panjang: SHORT — maksimum 2-3 kalimat.",
        AnswerLength::Medium => "Panjang: MEDIUM — satu paragraf padat (4-6 kalimat).",
        AnswerLength::Detailed => {
            "Panjang: DETAILED — jawaban lengkap dengan konteks dan contoh konkret dari profil pengguna."
        }
    }
}

fn language_instruction(settings: &Settings, question: &str) -> String {
    match settings.ai.response_language.as_str() {
        "id" => "Bahasa jawaban: Bahasa Indonesia.".into(),
        "en" => "Bahasa jawaban: Bahasa Inggris.".into(),
        _ => {
            if settings.ai.answer_style == AnswerStyle::EnglishInterview {
                "Bahasa jawaban: Bahasa Inggris (diminta oleh gaya jawaban).".into()
            } else {
                format!(
                    "Bahasa jawaban: ikuti bahasa pertanyaan interviewer. \
                     Pertanyaan berikut — deteksi bahasanya dan jawab dengan bahasa yang sama: \"{}\"",
                    truncate(question, 200)
                )
            }
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{t}…")
    }
}

/// Konteks transkrip beberapa segmen terakhir, dengan label sumber.
pub fn transcript_context(segments: &[TranscriptSegment], max_chars: usize) -> String {
    let mut out = String::new();
    for seg in segments.iter().rev() {
        let line = format!(
            "[{}] {}: {}\n",
            crate::storage::format_duration_ms(seg.ts_ms),
            seg.source.label(),
            seg.text.trim()
        );
        if out.len() + line.len() > max_chars {
            break;
        }
        out.insert_str(0, &line);
    }
    out.trim().to_string()
}

/// Prompt deteksi pertanyaan (dipakai opsional; default memakai heuristik lokal).
pub fn question_detection_messages(context: &str) -> Vec<Message> {
    vec![
        Message::system(
            "Kamu membantu menganalisis transkrip interview. Jawab HANYA dengan JSON \
             {\"is_question\": bool, \"question\": string, \"category\": string, \"keywords\": [string]} \
             tanpa penjelasan lain. Kategori: introduction, experience, technical, behavioral, \
             motivation, education, salary, logistics, closing, other.",
        ),
        Message::user(format!("Transkrip:\n{context}\n\nApakah segmen terakhir berisi pertanyaan interviewer?")),
    ]
}

/// Prompt saran jawaban utama.
pub fn suggestion_messages(
    settings: &Settings,
    profile: &Profile,
    context: &str,
    question: &str,
) -> Vec<Message> {
    let system = format!(
        "Kamu adalah asisten latihan interview kerja yang jujur dan praktis. \
         Kamu membantu pengguna MENYUSUN jawaban dari data profilnya sendiri.\n\n\
         {ANTI_FABRICATION_RULE}\n\n\
         {style}\n{length}\n{language}\n\n\
         Balas HANYA dengan JSON valid (tanpa markdown fence) berskema:\n\
         {{\"answer\": string, \"key_points\": [string], \"technical_keywords\": [string], \
         \"follow_up_questions\": [string], \"category\": string}}\n\
         Yaitu: contoh jawaban natural seperti percakapan manusia (bukan esai), \
         3-5 key points utama, istilah teknis relevan, dan pertanyaan lanjutan \
         yang mungkin diajukan interviewer.",
        style = style_instruction(settings.ai.answer_style),
        length = length_instruction(settings.ai.answer_length),
        language = language_instruction(settings, question),
    );
    let user = format!(
        "PROFIL PENGGUNA:\n{}\n\nKONTEKS TRANSKRIP TERBARU:\n{}\n\nPERTANYAAN INTERVIEWER:\n{}",
        profile.as_prompt_context(),
        if context.trim().is_empty() { "(belum ada)" } else { context },
        question.trim()
    );
    vec![Message::system(system), Message::user(user)]
}

/// Prompt alternatif jawaban (tombol "Generate Alternative Answer").
pub fn alternative_messages(
    settings: &Settings,
    profile: &Profile,
    question: &str,
    previous_answer: &str,
) -> Vec<Message> {
    let mut messages = suggestion_messages(settings, profile, "", question);
    if let Some(sys) = messages.get_mut(0) {
        sys.content.push_str(
            "\n\nTUGAS KHUSUS: buat ALTERNATIF jawaban yang berbeda dari draf sebelumnya — \
             pendekatan, contoh, atau struktur yang berbeda, bukan sekadar parafrase.",
        );
    }
    messages.push(Message::user(format!(
        "Draf jawaban sebelumnya (jangan diulang mentah):\n{}",
        truncate(previous_answer, 1500)
    )));
    messages
}

/// Prompt penjelasan istilah teknis.
pub fn explain_term_messages(term: &str, context: &str) -> Vec<Message> {
    vec![
        Message::system(
            "Jelaskan istilah teknis interview dengan singkat dan jelas (maksimum 5 kalimat), \
             beri contoh penggunaan dalam konteks kerja, dan tutup dengan satu kalimat siap \
             diucapkan saat interview. Gunakan bahasa yang sama dengan konteks yang diberikan.",
        ),
        Message::user(format!("Istilah: {term}\nKonteks transkrip:\n{context}")),
    ]
}

/// Prompt ringkasan sesi.
pub fn summary_messages(transcript: &str) -> Vec<Message> {
    vec![
        Message::system(
            "Ringkas sesi interview berikut dalam Bahasa Indonesia: 1 paragraf ringkasan, \
             lalu 3-5 poin penting (topik yang dibahas, kekuatan, area yang perlu diperbaiki). \
             Jangan mengarang isi di luar transkrip.",
        ),
        Message::user(format!("Transkrip:\n{}", truncate(transcript, 12_000))),
    ]
}

/// Prompt pertanyaan AI interviewer untuk Practice Mode.
pub fn practice_question_messages(
    category: &str,
    asked: &[String],
    profile: &Profile,
    live: bool,
) -> Vec<Message> {
    let asked_list = if asked.is_empty() {
        "(belum ada)".to_string()
    } else {
        asked
            .iter()
            .rev()
            .take(10)
            .map(|q| format!("- {q}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let lang = if category == "english" { "Bahasa Inggris" } else { "Bahasa Indonesia" };
    vec![
        Message::system(format!(
            "Kamu adalah interviewer {lang} yang realistis untuk latihan wawancara kerja. \
             Ajukan SATU pertanyaan interview baru setiap kali diminta. \
             Pertanyaan harus sesuai kategori `{category}`, realistis, tidak berulang, \
             dan boleh menyesuaikan dengan posisi yang dilamar pengguna. \
             Balas HANYA dengan teks pertanyaannya (tanpa nomor, tanpa penjelasan)."
        )),
        Message::user(format!(
            "Posisi yang dilamar: {}\nKategori latihan: {}\nPertanyaan yang sudah diajukan:\n{asked_list}\n\nAjukan satu pertanyaan berikutnya.",
            if profile.target_position.trim().is_empty() { "(belum diisi)" } else { profile.target_position.trim() },
            category
        )),
        Message::user(if live {
            "Ini wawancara langsung: buat pertanyaan yang umum untuk posisi tersebut."
        } else {
            "Ini sesi latihan: tantang pengguna secukupnya."
        }),
    ]
}

/// Prompt evaluasi sesi latihan.
pub fn practice_evaluation_messages(transcript: &str, questions: &[String]) -> Vec<Message> {
    let q_list = questions
        .iter()
        .map(|q| format!("- {q}"))
        .collect::<Vec<_>>()
        .join("\n");
    vec![
        Message::system(
            "Kamu evaluator latihan interview yang jujur dan konstruktif. \
             Nilai HANYA berdasarkan transkrip. Jangan mengarang jawaban pengguna. \
             Balas HANYA dengan JSON valid berskema: \
             {\"overall\": string, \"strengths\": [string], \"improvements\": [string], \
             \"recommendations\": [string]}",
        ),
        Message::user(format!(
            "Pertanyaan yang diajukan:\n{q_list}\n\nTranskrip jawaban pengguna:\n{}",
            truncate(transcript, 10_000)
        )),
    ]
}

/// Parser JSON toleran: buang fence markdown, ambil objek pertama.
pub fn parse_json_object<T: serde::de::DeserializeOwned>(raw: &str) -> Result<T> {
    let cleaned = strip_code_fences(raw.trim());
    if let Ok(v) = serde_json::from_str::<T>(&cleaned) {
        return Ok(v);
    }
    let start = cleaned.find('{');
    let end = cleaned.rfind('}');
    if let (Some(s), Some(e)) = (start, end) {
        if e > s {
            if let Ok(v) = serde_json::from_str::<T>(&cleaned[s..=e]) {
                return Ok(v);
            }
        }
    }
    Err(AppError::Ai(
        "respons AI tidak dapat diparsing sebagai JSON".into(),
    ))
}

fn strip_code_fences(s: &str) -> String {
    let s = s.trim();
    if s.starts_with("```") {
        let without_open = s.trim_start_matches("```json").trim_start_matches("```");
        let cleaned = without_open.trim_end_matches("```").trim();
        return cleaned.to_string();
    }
    s.to_string()
}

/// Parse saran jawaban dari teks respons.
pub fn parse_suggestion(raw: &str) -> Result<Suggestion> {
    let mut s: Suggestion = parse_json_object(raw)?;
    s.answer = s.answer.trim().to_string();
    if s.answer.is_empty() {
        return Err(AppError::Ai("saran jawaban kosong".into()));
    }
    s.key_points.retain(|p| !p.trim().is_empty());
    s.technical_keywords.retain(|p| !p.trim().is_empty());
    s.follow_up_questions.retain(|p| !p.trim().is_empty());
    Ok(s)
}

/// Parse laporan latihan dari teks respons.
pub fn parse_practice_report(raw: &str) -> Result<PracticeReport> {
    let mut r: PracticeReport = parse_json_object(raw)?;
    r.overall = r.overall.trim().to_string();
    if r.overall.is_empty() && r.strengths.is_empty() && r.improvements.is_empty() {
        return Err(AppError::Ai("laporan latihan kosong".into()));
    }
    Ok(r)
}

/// Render saran menjadi teks Markdown untuk UI/ekspor.
pub fn render_suggestion_markdown(s: &Suggestion) -> String {
    let mut out = String::new();
    out.push_str("**Suggested Answer**\n");
    out.push_str(s.answer.trim());
    out.push('\n');
    if !s.key_points.is_empty() {
        out.push_str("\n**Key Points**\n");
        for p in &s.key_points {
            out.push_str(&format!("- {}\n", p.trim()));
        }
    }
    if !s.technical_keywords.is_empty() {
        out.push_str("\n**Technical Keywords**\n");
        for k in &s.technical_keywords {
            out.push_str(&format!("- {}\n", k.trim()));
        }
    }
    if !s.follow_up_questions.is_empty() {
        out.push_str("\n**Follow-up Questions**\n");
        for q in &s.follow_up_questions {
            out.push_str(&format!("- {}\n", q.trim()));
        }
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;
    use crate::storage::session::SegmentSource;

    fn segment(ts: i64, text: &str) -> TranscriptSegment {
        TranscriptSegment {
            ts_ms: ts,
            source: SegmentSource::Meeting,
            text: text.into(),
        }
    }

    #[test]
    fn parses_plain_json() {
        let raw = r#"{"answer":"Halo","key_points":["a"],"technical_keywords":[],"follow_up_questions":[],"category":"introduction"}"#;
        let s = parse_suggestion(raw).unwrap();
        assert_eq!(s.answer, "Halo");
        assert_eq!(s.key_points, vec!["a"]);
    }

    #[test]
    fn parses_fenced_json_with_prose() {
        let raw = "Berikut hasilnya:\n```json\n{\"answer\":\"Jawaban\",\"key_points\":[],\"technical_keywords\":[],\"follow_up_questions\":[]}\n```\nSemoga membantu!";
        let s = parse_suggestion(raw).unwrap();
        assert_eq!(s.answer, "Jawaban");
    }

    #[test]
    fn rejects_non_json() {
        assert!(parse_suggestion("tentu, ini jawabannya").is_err());
    }

    #[test]
    fn suggestion_prompt_contains_anti_fabrication_and_style() {
        let mut settings = Settings::default();
        settings.ai.answer_style = AnswerStyle::Star;
        settings.ai.answer_length = AnswerLength::Detailed;
        let profile = Profile {
            name: "Riski".into(),
            ..Default::default()
        };
        let msgs = suggestion_messages(&settings, &profile, "[00:01] Meeting: hi", "Tell me about yourself");
        let sys = &msgs[0].content;
        assert!(sys.contains("JANGAN mengarang"));
        assert!(sys.contains("STAR"));
        assert!(sys.contains("DETAILED"));
        assert!(msgs[1].content.contains("Riski"));
        assert!(msgs[1].content.contains("Tell me about yourself"));
    }

    #[test]
    fn transcript_context_keeps_latest_within_budget() {
        let segs = vec![
            segment(1000, "pertama"),
            segment(2000, "kedua"),
            segment(3000, "ketiga"),
        ];
        let ctx = transcript_context(&segs, 500);
        assert!(ctx.contains("ketiga"));
        assert!(ctx.find("kedua").unwrap() < ctx.find("ketiga").unwrap());
    }

    #[test]
    fn renders_markdown_sections() {
        let s = Suggestion {
            answer: "A".into(),
            key_points: vec!["p1".into()],
            technical_keywords: vec!["Rust".into()],
            follow_up_questions: vec!["Q2?".into()],
            category: None,
        };
        let md = render_suggestion_markdown(&s);
        assert!(md.contains("**Suggested Answer**"));
        assert!(md.contains("**Key Points**"));
        assert!(md.contains("**Technical Keywords**"));
        assert!(md.contains("**Follow-up Questions**"));
    }

    #[test]
    fn parses_practice_report() {
        let raw = r#"{"overall":"bagus", "strengths":["jelas"], "improvements":["durasi"], "recommendations":["latihan STAR"]}"#;
        let r = parse_practice_report(raw).unwrap();
        assert_eq!(r.overall, "bagus");
        assert_eq!(r.strengths, vec!["jelas"]);
    }
}
