//! Practice Mode — mode default aplikasi.
//!
//! Orchestrator ini menyimpan state latihan (kategori, pertanyaan yang sudah
//! diajukan) dan memilih strategi bertanya:
//! - Ada provider AI → pertanyaan dibuat AI agar lebih dinamis/dinamis.
//! - Tidak ada API key / offline → fallback ke bank pertanyaan lokal.

use crate::ai::provider::AiProvider;
use crate::storage::profile::Profile;

pub mod bank;

pub struct PracticeSession {
    pub category: String,
    pub asked: Vec<String>,
    salt: u64,
}

impl PracticeSession {
    pub fn new(category: &str) -> Self {
        let salt = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            category: category.to_string(),
            asked: Vec::new(),
            salt,
        }
    }

    /// Pertanyaan berikutnya. `provider` opsional; bila `None` atau gagal,
    /// fallback ke bank lokal (dan hasil fallback tetap dicatat).
    pub fn next_question(
        &mut self,
        provider: Option<&dyn AiProvider>,
        profile: &Profile,
    ) -> (String, bool) {
        if let Some(p) = provider {
            match crate::ai::actions::ask_practice_question(
                p,
                &self.category,
                &self.asked,
                profile,
                false,
            ) {
                Ok(q) => {
                    self.asked.push(q.clone());
                    return (q, true);
                }
                Err(_) => { /* fallback di bawah */ }
            }
        }
        let q = bank::next_question(&self.category, &self.asked, self.salt);
        self.asked.push(q.clone());
        (q, false)
    }

    pub fn reset(&mut self) {
        self.asked.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_bank_when_no_provider() {
        let mut s = PracticeSession::new("hr");
        let (q, ai) = s.next_question(None, &Profile::default());
        assert!(!ai);
        assert!(!q.is_empty());
        assert_eq!(s.asked.len(), 1);
    }

    #[test]
    fn falls_back_when_provider_fails() {
        struct FailProvider;
        impl AiProvider for FailProvider {
            fn name(&self) -> String {
                "fail".into()
            }
            fn complete(
                &self,
                _r: &crate::ai::provider::CompletionRequest,
            ) -> crate::error::Result<String> {
                Err(crate::error::AppError::Network("offline".into()))
            }
        }
        let mut s = PracticeSession::new("technical");
        let (q, ai) = s.next_question(Some(&FailProvider), &Profile::default());
        assert!(!ai);
        assert!(!q.is_empty());
    }
}
