//! Deteksi pertanyaan interviewer dari aliran transkrip.
//!
//! Strategi (menghemat panggilan API):
//! 1. **Buffering** — segmen baru ditambahkan ke buffer kalimat.
//! 2. **Deteksi akhir kalimat** — kandidat hanya dievaluasi saat tanda akhir
//!    (`?`, `.`, `!`) muncul.
//! 3. **Heuristik pertanyaan** — kata tanya Indonesia/Inggris, pola permintaan
//!    (tell me / describe / walk me through / coba jelaskan …), atau tanda `?`.
//! 4. **Debounce** — jarak minimum antar kandidat + deduplikasi teks.
//!
//! Dengan begitu, satu pertanyaan menghasilkan paling banyak satu kandidat,
//! dan pemanggil bisa mengirim tepat satu request AI per kandidat.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionCandidate {
    pub text: String,
    /// Tebakan kategori lokal (bisa dikoreksi oleh AI).
    pub category: &'static str,
    pub keywords: Vec<String>,
}

pub struct QuestionDetector {
    buffer: String,
    last_emitted: Option<String>,
    last_emit_ms: i64,
    debounce_ms: i64,
    min_words: usize,
}

impl QuestionDetector {
    pub fn new(debounce_ms: u64, min_words: usize) -> Self {
        Self {
            buffer: String::new(),
            last_emitted: None,
            last_emit_ms: i64::MIN / 2,
            debounce_ms: debounce_ms as i64,
            min_words: min_words.max(2),
        }
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.last_emitted = None;
        self.last_emit_ms = i64::MIN / 2;
    }

    /// Masukkan satu segmen transkrip; hasilkan kandidat bila memenuhi syarat.
    pub fn feed(&mut self, text: &str, now_ms: i64) -> Option<QuestionCandidate> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        if !self.buffer.is_empty() {
            self.buffer.push(' ');
        }
        self.buffer.push_str(text);
        self.trim_buffer();

        // Hanya evaluasi saat ada tanda akhir kalimat.
        if !self
            .buffer
            .chars()
            .last()
            .is_some_and(|c| matches!(c, '?' | '.' | '!' | '…'))
        {
            return None;
        }

        let sentence = self.buffer.clone();
        self.buffer.clear();

        if !looks_like_question(&sentence, self.min_words) {
            return None;
        }
        let normalized = normalize(&sentence);
        if let Some(last) = &self.last_emitted {
            if *last == normalized {
                return None;
            }
        }
        if now_ms - self.last_emit_ms < self.debounce_ms {
            return None;
        }
        self.last_emit_ms = now_ms;
        self.last_emitted = Some(normalized.clone());

        Some(QuestionCandidate {
            text: sentence.trim().to_string(),
            category: guess_category(&normalized),
            keywords: extract_keywords(&sentence),
        })
    }

    fn trim_buffer(&mut self) {
        // Batasi memori: simpan maksimum 600 karakter terakhir.
        if self.buffer.chars().count() > 600 {
            let skipped: String = self.buffer.chars().rev().take(600).collect();
            self.buffer = skipped.chars().rev().collect();
        }
    }
}

/// Heuristik pertanyaan (tanpa jaringan).
pub fn looks_like_question(text: &str, min_words: usize) -> bool {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() < min_words {
        return false;
    }
    if text.trim_end().ends_with('?') {
        return true;
    }
    let lower = normalize(text);
    const STARTERS: [&str; 24] = [
        "what ", "why ", "how ", "when ", "where ", "who ", "which ", "can you ", "could you ",
        "do you ", "did you ", "are you ", "have you ", "tell me ", "describe ", "explain ",
        "walk me through", "apa ", "kenapa ", "mengapa ", "bagaimana ", "coba jelaskan",
        "ceritakan ", "bisa cerita",
    ];
    STARTERS.iter().any(|s| lower.starts_with(s))
}

/// Kategori tebakan cepat (AI dapat mengoreksi saat menghasilkan saran).
pub fn guess_category(lower: &str) -> &'static str {
    const MAP: [(&str, &str); 12] = [
        ("tell me about yourself", "introduction"),
        ("perkenalkan diri", "introduction"),
        ("latar belakang", "introduction"),
        ("pengalaman", "experience"),
        ("experience", "experience"),
        ("project", "experience"),
        ("skill", "technical"),
        ("teknis", "technical"),
        ("teknologi", "technical"),
        ("gaji", "salary"),
        ("salary", "salary"),
        ("motivasi", "motivation"),
    ];
    for (needle, cat) in MAP {
        if lower.contains(needle) {
            return cat;
        }
    }
    "other"
}

/// Ekstrak kata kunci dari pertanyaan (kata bermakna, tanpa stopword umum).
pub fn extract_keywords(text: &str) -> Vec<String> {
    const STOPWORDS: [&str; 42] = [
        "the", "a", "an", "and", "or", "to", "of", "in", "on", "for", "with", "about", "your",
        "you", "me", "my", "can", "could", "would", "do", "does", "did", "tell", "what", "why",
        "how", "when", "where", "who", "yang", "dan", "atau", "untuk", "dengan", "tentang",
        "apa", "kenapa", "mengapa", "bagaimana", "saya", "kamu", "anda",
    ];
    let mut out: Vec<String> = Vec::new();
    for word in text.split(|c: char| !c.is_alphanumeric()) {
        let w = word.trim();
        if w.chars().count() < 3 {
            continue;
        }
        let lower = w.to_lowercase();
        if STOPWORDS.contains(&lower.as_str()) {
            continue;
        }
        if !out.iter().any(|k| k.to_lowercase() == lower) {
            out.push(w.to_string());
        }
        if out.len() >= 8 {
            break;
        }
    }
    out
}

fn normalize(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_english_question() {
        let mut d = QuestionDetector::new(2000, 3);
        assert!(d.feed("Can you tell me about yourself?", 10_000).is_some());
    }

    #[test]
    fn detects_indonesian_question() {
        let mut d = QuestionDetector::new(2000, 3);
        let c = d.feed("Coba jelaskan tentang project terakhir kamu?", 10_000);
        assert!(c.is_some());
    }

    #[test]
    fn detects_statement_with_starter_without_question_mark() {
        let mut d = QuestionDetector::new(2000, 3);
        assert!(d.feed("Tell me about your backend experience.", 10_000).is_some());
    }

    #[test]
    fn ignores_short_or_plain_statements() {
        let mut d = QuestionDetector::new(2000, 3);
        assert!(d.feed("oke.", 10_000).is_none());
        assert!(d.feed("Saya bekerja di perusahaan itu selama tiga tahun.", 11_000).is_none());
    }

    #[test]
    fn buffers_until_sentence_end() {
        let mut d = QuestionDetector::new(2000, 3);
        assert!(d.feed("Can you tell me about", 10_000).is_none());
        assert!(d.feed("your backend experience?", 10_100).is_some());
    }

    #[test]
    fn debounce_blocks_rapid_questions() {
        let mut d = QuestionDetector::new(5000, 3);
        assert!(d.feed("What is your biggest weakness?", 10_000).is_some());
        assert!(d.feed("How do you handle conflict?", 11_000).is_none());
        assert!(d.feed("How do you handle conflict again?", 16_000).is_some());
    }

    #[test]
    fn deduplicates_identical_question() {
        let mut d = QuestionDetector::new(500, 3);
        assert!(d.feed("Why do you want this job?", 10_000).is_some());
        assert!(d.feed("Why do you want this job?", 20_000).is_none());
    }

    #[test]
    fn extracts_keywords_without_stopwords() {
        let kws = extract_keywords("Can you tell me about your backend experience with PostgreSQL?");
        assert!(kws.iter().any(|k| k == "backend"));
        assert!(kws.iter().any(|k| k == "PostgreSQL"));
        assert!(!kws.iter().any(|k| k.to_lowercase() == "the"));
    }

    #[test]
    fn guesses_category() {
        assert_eq!(guess_category("can you tell me about yourself"), "introduction");
        assert_eq!(guess_category("apa motivasi anda"), "motivation");
    }
}
