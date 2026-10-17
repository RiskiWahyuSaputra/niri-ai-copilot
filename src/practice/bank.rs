//! Bank pertanyaan latihan offline.
//!
//! Digunakan sebagai fallback ketika API key belum tersedia, sehingga Practice
//! Mode tetap bisa berjalan tanpa jaringan.

pub const CATEGORIES: [&str; 5] = ["mixed", "hr", "behavioral", "technical", "english"];

const HR: [&str; 12] = [
    "Coba ceritakan tentang diri Anda dan latar belakang Anda.",
    "Apa yang membuat Anda tertarik melamar posisi ini?",
    "Apa kelebihan dan kekurangan Anda?",
    "Dalam 5 tahun ke depan, Anda ingin berada di posisi seperti apa?",
    "Mengapa Anda meninggalkan pekerjaan sebelumnya?",
    "Ekspektasi gaji Anda untuk posisi ini berapa?",
    "Bagaimana Anda menangani tekanan dan tenggat waktu yang ketat?",
    "Apa yang Anda ketahui tentang perusahaan ini?",
    "Apakah Anda bersedia bekerja secara hybrid atau on-site?",
    "Apa pencapaian yang paling Anda banggakan?",
    "Bagaimana Anda mendefinisikan kesuksesan dalam pekerjaan?",
    "Ada pertanyaan untuk kami?",
];

const BEHAVIORAL: [&str; 12] = [
    "Ceritakan situasi di mana Anda menyelesaikan konflik dalam tim.",
    "Berikan contoh saat Anda gagal; apa yang Anda pelajari?",
    "Ceritakan saat Anda harus belajar teknologi baru dengan cepat.",
    "Bagaimana Anda menangani rekan kerja yang tidak menyelesaikan bagiannya?",
    "Ceritakan saat Anda mengambil inisiatif tanpa diminta.",
    "Berikan contoh ketika Anda menerima kritik keras; bagaimana reaksi Anda?",
    "Ceritakan proyek paling menantang dan bagaimana Anda menyelesaikannya.",
    "Bagaimana Anda memprioritaskan pekerjaan saat semuanya terasa mendesak?",
    "Ceritakan saat Anda harus menjelaskan hal teknis kepada non-teknis.",
    "Berikan contoh keputusan sulit yang pernah Anda ambil.",
    "Ceritakan saat kerja sama tim Anda berhasil luar biasa.",
    "Bagaimana Anda menangani kesalahan yang Anda buat sendiri?",
];

const TECHNICAL: [&str; 12] = [
    "Jelaskan cara kerja HTTP request dari browser sampai server merespons.",
    "Apa perbedaan proses dan thread? Kapan Anda memilih salah satunya?",
    "Bagaimana Anda mendesain database untuk aplikasi dengan trafik tinggi?",
    "Jelaskan konsep indexing di database dan trade-off-nya.",
    "Apa yang terjadi saat Anda menjalankan `git commit`?",
    "Bagaimana Anda mengamankan API dari serangan umum?",
    "Jelaskan perbedaan protokol TCP dan UDP beserta contoh penggunaannya.",
    "Bagaimana Anda melakukan debugging pada sistem yang sedang produksi?",
    "Apa itu caching dan di mana Anda menempatkannya dalam arsitektur?",
    "Bagaimana strategi testing yang Anda terapkan pada project Anda?",
    "Jelaskan cara kerja garbage collector dan dampaknya pada performa.",
    "Bagaimana Anda menangani migrasi data tanpa downtime?",
];

const ENGLISH: [&str; 12] = [
    "Can you walk me through your resume briefly?",
    "Tell me about a project you are most proud of.",
    "What is your biggest strength as a developer?",
    "Describe a time you had to meet a tight deadline.",
    "Why should we hire you for this position?",
    "How do you keep your technical skills up to date?",
    "Explain a complex technical concept in simple terms.",
    "What kind of work environment helps you do your best work?",
    "Tell me about a time you disagreed with your team lead.",
    "How do you handle feedback you disagree with?",
    "Where do you see your career in five years?",
    "Do you have any questions for me?",
];

/// Semua pertanyaan untuk kategori tertentu.
pub fn questions_for(category: &str) -> Vec<&'static str> {
    match category {
        "hr" => HR.to_vec(),
        "behavioral" => BEHAVIORAL.to_vec(),
        "technical" => TECHNICAL.to_vec(),
        "english" => ENGLISH.to_vec(),
        _ => {
            let mut all = Vec::new();
            all.extend(HR);
            all.extend(BEHAVIORAL);
            all.extend(TECHNICAL);
            all
        }
    }
}

/// Pilih pertanyaan berikutnya yang belum diajukan. `salt` memungkinkan variasi
/// urutan antar sesi. Jika semua sudah diajukan, mulai lagi dari awal.
pub fn next_question(category: &str, asked: &[String], salt: u64) -> String {
    let pool = questions_for(category);
    if pool.is_empty() {
        return "Ceritakan tentang diri Anda.".to_string();
    }
    let start = (salt as usize) % pool.len();
    for i in 0..pool.len() {
        let q = pool[(start + i) % pool.len()];
        if !asked.iter().any(|a| a == q) {
            return q.to_string();
        }
    }
    pool[start].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_fresh_questions_before_repeating() {
        let mut asked: Vec<String> = Vec::new();
        for i in 0..5 {
            let q = next_question("hr", &asked, i);
            assert!(!asked.contains(&q), "pertanyaan berulang: {q}");
            asked.push(q);
        }
    }

    #[test]
    fn falls_back_to_repeat_when_exhausted() {
        let asked: Vec<String> = questions_for("technical")
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        let q = next_question("technical", &asked, 3);
        assert!(!q.is_empty());
    }

    #[test]
    fn mixed_includes_all_banks() {
        let all = questions_for("mixed");
        assert!(all.len() >= 36);
    }
}
