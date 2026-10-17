//! Redaksi rahasia agar API key tidak pernah muncul di log, UI, atau pesan error.

/// Ganti setiap `secret` yang tidak kosong di `text` dengan `[REDACTED]`.
pub fn redact_secrets(text: &str, secrets: &[&str]) -> String {
    let mut out = text.to_string();
    for secret in secrets {
        let secret = secret.trim();
        if secret.len() < 8 {
            continue;
        }
        out = out.replace(secret, "[REDACTED]");
    }
    out
}

/// Deteksi pola token API yang umum (Groq `gsk_...`, OpenAI `sk-...`) dan
/// redaksi bila ditemukan, tanpa perlu tahu nilainya.
pub fn redact_all(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for token in text.split_inclusive(|c: char| c.is_whitespace() || c == '"' || c == '\'') {
        let (core, trail) = split_trailing_punct(token);
        if looks_like_api_key(core) {
            result.push_str("[REDACTED]");
        } else {
            result.push_str(core);
        }
        result.push_str(trail);
    }
    result
}

fn split_trailing_punct(token: &str) -> (&str, &str) {
    let trimmed = token.trim_end_matches(|c: char| c.is_whitespace());
    (trimmed, &token[trimmed.len()..])
}

fn looks_like_api_key(core: &str) -> bool {
    let c = core.trim();
    // Dukung bentuk `key=<token>` / `Authorization=Bearer` dsb.
    let c = c.rsplit('=').next().unwrap_or(c).trim();
    (c.starts_with("gsk_") || c.starts_with("sk-")) && c.len() >= 20
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_known_secret() {
        let msg = "request gagal dengan Authorization: Bearer gsk_abcdef1234567890";
        let out = redact_secrets(msg, &["gsk_abcdef1234567890"]);
        assert!(!out.contains("gsk_abcdef1234567890"));
        assert!(out.contains("[REDACTED]"));
    }

    #[test]
    fn ignores_short_secrets() {
        let msg = "nilai abc tidak diubah";
        assert_eq!(redact_secrets(msg, &["abc"]), msg);
    }

    #[test]
    fn redacts_unknown_token_patterns() {
        let msg = "key=gsk_someunknownkeyvalue1234567890 dan sk-proj-AAAAAAAAAAAAAAAA";
        let out = redact_all(msg);
        assert!(!out.contains("gsk_someunknownkeyvalue1234567890"));
        assert!(!out.contains("sk-proj-AAAAAAAAAAAAAAAA"));
        assert_eq!(out.matches("[REDACTED]").count(), 2);
    }

    #[test]
    fn keeps_normal_text() {
        let msg = "can you tell me about yourself?";
        assert_eq!(redact_all(msg), msg);
    }
}
