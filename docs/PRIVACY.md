# Privasi — Niri AI Copilot

Prinsip utama: **audio tidak pernah meninggalkan perangkat Anda**, dan teks
hanya dikirim ke AI eksternal dengan persetujuan eksplisit.

## Aliran data

| Data | Lokasi | Dikirim keluar? |
|---|---|---|
| Audio mikrofon/meeting | RAM (chunk sementara) | Tidak |
| File audio (opsional) | `~/.local/share/niri-ai-copilot/recordings/` | Tidak |
| Transkrip | SQLite lokal `copilot.db` | Hanya potongan konteks ke Groq jika sesi Live/Practice & API key aktif |
| Pertanyaan & saran | SQLite lokal `copilot.db` | — |
| Profil/CV (teks) | SQLite lokal `copilot.db` | Potongan relevan saat meminta saran (bila AI aktif) |
| API key | env `GROQ_API_KEY` atau `credentials.toml` (0600) | Hanya sebagai header ke Groq |

## Kontrol yang tersedia

1. **Gerbang izin perekaman (ConsentGate)** — setiap sesi dimulai dengan dialog
   konfirmasi; `start_session` menolak berjalan jika `consent_confirmed` tidak
   benar. Izin dicabut otomatis saat sesi berakhir. Konfirmasi dapat
   dinonaktifkan di Pengaturan → Privasi (tidak disarankan; default: wajib).
2. **Indikator REC** — selalu terlihat selama perekaman; timer opsional.
3. **Simpan audio default OFF** — audio hanya ditulis ke disk bila diaktifkan.
   Bila aktif, file lama dihapus otomatis sesuai `audio_retention_days`
   (`privacy::enforce_audio_retention`, dijalankan saat start & akhir sesi;
   `0` = hapus semua).
4. **Disclosure AI eksternal** — checkbox penjelasan sebelum teks dikirim ke
   Groq; hanya transkrip terbaru (dipotong sesuai `context_segments` /
   `max_transcript_context`) dan potongan profil yang relevan yang dikirim.
5. **Redaksi rahasia** — semua pesan error dari provider/HTTP/STT melewati
   `privacy::redact_all` sebelum tampil di UI atau tersimpan; API key tidak
   pernah ditulis ke database, log, atau pesan error.
6. **Tanpa telemetry** — aplikasi tidak mengirim data apa pun kecuali request
   AI yang Anda izinkan ke endpoint yang dikonfigurasi.

## Yang perlu Anda pastikan sendiri

- **Izin peserta**: beberapa yurisdiksi mewajibkan persetujuan semua pihak
  sebelum merekam percakapan. Teks konfirmasi di dialog bukan nasihat hukum.
- **Kebijakan layanan AI**: teks yang dikirim ke Groq tunduk pada kebijakan
  provider tersebut. Jika Anda tidak ingin teks keluar sama sekali, gunakan
  mode Transkripsi atau Practice Mode tanpa API key (bank offline).
- **Keamanan endpoint**: provider AI dapat diganti di Pengaturan → AI; pastikan
  Anda mempercayai endpoint yang dipakai.

## Menghapus data

- Hapus semua riwayat: tutup aplikasi lalu hapus
  `~/.local/share/niri-ai-copilot/copilot.db`.
- Hapus audio: hapus direktori `recordings/` (atau set retensi `0`).
- Hapus API key: hapus `~/.config/niri-ai-copilot/credentials.toml` dan/atau
  unset `GROQ_API_KEY`.
