# Testing — Niri AI Copilot

Dokumen ini mencatat **cara menguji** dan **status keterujian** yang sebenarnya
(tanpa klaim berlebihan). Perbarui kolom status bila Anda menguji di mesin
nyata.

## Unit test

```sh
cargo test
```

Saat ini: **68 test** mencakup konfigurasi, storage (skema + repositori +
ekspor + CV), redaksi privasi (+ retensi audio), parsing device PipeWire,
chunker PCM, pipeline STT (termasuk drop-oldest), provider AI (termasuk retry
dan redaksi), parser JSON longgar, detector pertanyaan, bank latihan, CLI, dan
transkripsi end-to-end dengan **stub binary whisper** (`transcribe_with_stub_binary_end_to_end`).

## Smoke test GUI

```sh
cargo run -- --smoke-test
```

Menjalankan aplikasi, menunggu 1 detik, mencetak
`smoke-test: aplikasi berhasil start tanpa error`, lalu keluar. Cocok untuk CI
atau memastikan perubahan UI tidak membuat aplikasi gagal start.

## Status keterujian

| Area | Status | Catatan |
|---|---|---|
| Unit test (logika inti) | ✅ teruji | `cargo test`, 68 test |
| Start GUI (Wayland/Niri nyata) | ✅ teruji | via `--smoke-test` (debug & release) |
| Release build | ✅ teruji | `cargo build --release`, binary 5.2 MB, smoke lolos |
| Transkripsi (stub whisper) | ✅ teruji | spawn subprocess nyata |
| **IPC saat aplikasi berjalan** | ✅ teruji | `status`, `toggle`, `expand`, `pause`, `quit` terhadap instans nyata; `quit` menutup aplikasi dengan bersih |
| **Capture audio nyata (pw-record)** | ⚠️ belum diuji end-to-end | device nyata + bicara perlu sesi manual |
| **whisper-cli nyata + model GGML** | ⚠️ belum diuji | butuh paket `whisper-cpp` + model |
| **Panggilan Groq nyata** | ⚠️ belum diuji di sesi ini | butuh API key; logika provider teruji dengan mock |
| **gtk4-layer-shell** | ❌ belum teruji | paket tidak terpasang di mesin pengembangan |
| Ekspor sesi dari UI | ⚠️ belum diuji manual | logika ekspor teruji di unit test |

## Checklist uji manual (disarankan di mesin target)

1. **Audio → transkrip**: jalankan app, Pengaturan → Audio isi path
   `whisper-cli` + model, mulai sesi Transcription, bicara ±10 detik,
   pastikan transkrip muncul dan tanpa `[BLANK_AUDIO]` liar.
2. **Practice Mode offline** (tanpa API key): mulai sesi Practice, tekan
   "Pertanyaan ▸" — pertanyaan dari bank offline muncul; jawab; akhiri sesi —
   transkrip tersimpan di Riwayat.
3. **AI aktif**: set `GROQ_API_KEY`, jalankan sesi Live singkat, uji
   Pengaturan → AI "Test koneksi", uji "Alternatif ✓" dan "Jelaskan".
4. **Privasi**: aktifkan simpan audio + retensi 0 — pastikan direktori
   `recordings/` dibersihkan setelah sesi selesai. Pastikan API key tidak
   pernah muncul di pesan error (uji dengan key salah).
5. **IPC**: saat app berjalan, jalankan `niri-ai-copilot toggle`, `pause`,
   `resume`, `stop`, `status`; cek keybind Niri sesuai
   [NIRI_SETUP.md](NIRI_SETUP.md).
6. **Riwayat & ekspor**: buka Riwayat, buka sesi, ekspor TXT/MD, buka profil
   dan impor CV teks/PDF (`pdftotext` harus ada untuk PDF).

## Utang teknis yang diketahui (jujur, belum diperbaiki)

- `ComboBoxText` deprecated di binding gtk4-rs sejak GTK 4.10 (pengganti:
  `DropDown`). Masih berfungsi pada GTK 4.x; terjadi banyak warning saat build.
  Migrasi direncanakan; lihat komentar di `src/ui/{dialog,settings,profile}.rs`.
- Sebagian API modul (mis. `practice::PracticeSession`, `rename_session`,
  `suggestions_for`, `NullEngine`) belum dipakai UI sehingga muncul warning
  `dead_code`. Beberapa akan dipakai saat pemolesan Riwayat/Practice.
- `layer-shell` tidak dapat diuji di mesin pengembangan (tanpa sudo), jadi
  fitur ini disediakan sebagai feature opt-in yang belum diverifikasi.
