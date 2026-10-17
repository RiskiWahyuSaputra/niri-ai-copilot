# Arsitektur — Niri AI Copilot

Dokumen ini merangkum struktur modul, model threading, alur data, dan
keputusan teknis utama. Ditulis agar dapat diverifikasi terhadap kode di `src/`.

## Peta modul

```
src/
├── main.rs            Entry point: parse CLI → run aplikasi / kirim IPC
├── cli.rs             Parser argumen + teks bantuan (tanpa dependency)
├── app.rs             Orkestrasi: lifecycle sesi, event loop, job AI, IPC handler
├── error.rs           AppError (Io/Config/Storage/Audio/Stt/Ai/Ipc/Unavailable)
├── ipc.rs             Unix socket server & client (line-based)
├── config/
│   ├── paths.rs       Lokasi XDG (config/data/credentials/socket) + override env
│   └── settings.rs    TOML settings, default, normalize, API key (env/file 0600)
├── storage/
│   ├── db.rs          Buka DB + migrasi schema v1, helper format waktu
│   ├── session.rs     Repositori sesi/transkrip/pertanyaan/saran
│   ├── profile.rs     Profil konteks AI (data nyata pengguna)
│   ├── export.rs      Ekspor sesi TXT/Markdown
│   └── cv.rs          Ekstraksi teks CV (PDF via pdftotext, teks langsung)
├── privacy/
│   ├── redact.rs      Redaksi rahasia (API key dsb.) di log/pesan error
│   └── consent.rs     ConsentGate: izin perekaman + record sumber audio
├── audio/
│   ├── devices.rs     Enumerasi source PipeWire via `pactl -f json list sources`
│   ├── pcm.rs         Konversi PCM→WAV + Chunker (baca N byte dari stream)
│   └── capture.rs     Subprocess `pw-record` per sumber, thread pembaca
├── transcription/
│   ├── engine.rs      Trait SttEngine (+ NullEngine untuk test)
│   ├── whisper_cli.rs Backend whisper.cpp via subprocess `whisper-cli`
│   └── pipeline.rs    SttWorker: antrean bounded drop-oldest + event hasil
├── ai/
│   ├── provider.rs    Trait AiProvider + CompletionRequest/Message
│   ├── groq.rs        Provider Groq (ureq 3), retry bounded, redaksi error
│   ├── prompts.rs     Prompt builder + aturan anti-fabrikasi + parser JSON longgar
│   ├── question.rs    QuestionDetector lokal (kata tanya + kata kunci teknis)
│   └── actions.rs     Aksi tingkat tinggi: saran, alternatif, ringkasan, evaluasi, dll.
├── practice/
│   ├── mod.rs         Orchestrator Practice Mode
│   └── bank.rs        Bank pertanyaan offline per kategori
└── ui/
    ├── widget.rs      Widget utama (compact/expanded), transcript, panel
    ├── dialog.rs      Dialog izin perekaman, error, konfirmasi
    ├── settings.rs    Halaman Pengaturan (Appearance/Audio/AI/Privacy/Perf)
    ├── history.rs     Riwayat sesi + buka/ekspor
    ├── profile.rs     Editor profil konteks AI + impor CV
    ├── panels.rs      Builder panel/tombol yang dipakai berulang
    ├── state.rs       AppEvent, WidgetStatus, StartOptions
    └── theme.rs       CSS dasar + CSS dinamis (opacity, font size)
```

## Alur data

```
pw-record (subprocess per sumber)
   │  PCM s16le mono 16 kHz dari stdout
   ▼
capture.rs reader thread ──► chunk WAV (Chunker, default 5 detik)
   │  sync_channel(8)  (backpressure ke pw-record bila konsumen lambat)
   ▼
UI poll (glib timeout 120 ms) ──► SttWorker.submit (antrean drop-oldest)
   │                                    │ worker thread
   │                                    ▼
   │                             whisper-cli (subprocess per chunk)
   │                                    │ teks
   │◄─────────────── mpsc SttEvent ─────┘
   ▼
AppEvent::Segment ──► insert Transkrip ke SQLite + tampilkan
   │
   ├─ Mode Live: QuestionDetector.feed() → AppEvent → job AI saran jawaban
   └─ Mode Practice: pertanyaan dari AI/bank → jawaban disimpan → evaluasi akhir
```

- **Bounded di mana-mana**: channel capture (8), antrean STT
  (`max_chunk_queue`, drop-oldest + hitung yang dibuang), retry AI bounded,
  context transkrip dipotong (`context_segments`, `max_transcript_context`).
- **Satu job AI sekaligus**: `AtomicBool ai_pending`; permintaan baru diabaikan
  saat ada job berjalan (anti-spam & hemat kuota).
- **Tanpa blocking UI thread**: semua proses berat (STT, HTTP, pw-record)
  berjalan di thread/process terpisah; UI hanya polling ringan.

## Model threading

| Thread | Tugas |
|---|---|
| Main (GTK) | Widget, dialog, polling `poll_events` tiap 120 ms, eksekusi perintah IPC |
| capture-{mic\|meeting} | Baca stdout `pw-record`, susun chunk WAV, kirim ke channel |
| stt-worker | Ambil job dari antrean, panggil `whisper-cli`, kirim `SttEvent` |
| AI job (spawn per request) | Panggilan HTTP ke Groq, kirim `AppEvent` hasil |
| IPC server | Terima koneksi socket, teruskan perintah via channel ke UI |

Semua event UI masuk lewat satu pintu: `AppEvent` + `poll_events()`.
Tidak ada state UI yang disentuh dari thread lain.

## Storage (SQLite, schema v1)

- `sessions` — jenis sesi (practice/live/transcription), judul, timestamp,
  durasi pause, ringkasan/laporan akhir.
- `transcripts` — segmen per sesi: `ts_ms`, sumber (mic/meeting), teks.
- `questions` — pertanyaan terdeteksi: teks, kategori, kata kunci (JSON).
- `suggestions` — saran/alternatif jawaban per pertanyaan (JSON payload).
- `profile` — satu baris profil konteks AI (peran, skill, pengalaman, CV).
- `kv` — metadata skema/migrasi.

Audio hanya ditulis ke `recordings/session-<id>/` bila pengguna
mengaktifkan **simpan audio**, dan dibersihkan sesuai `audio_retention_days`
oleh `privacy::enforce_audio_retention` (dijalankan saat start & akhir sesi).

## Keputusan teknis

1. **`pw-record` subprocess, bukan binding PipeWire.** Menghindari dependensi
   C besar; `pw-record` adalah bagian paket `pipewire` yang pasti ada.
   Backpressure ditangani lewat bounded channel.
2. **`whisper-cli` subprocess.** Model/turunan whisper.cpp dapat diganti tanpa
   rebuild; deteksi kesiapan eksplisit (`check_ready`) dengan pesan yang
   mengarahkan pengguna memasang paket/model.
3. **ureq, bukan tokio/reqwest.** Satu kebutuhan HTTP sinkron sederhana;
   thread per job lebih mudah diaudit daripada runtime async penuh.
4. **rusqlite `bundled`.** Tidak butuh paket SQLite sistem, versi konsisten.
5. **API key tidak pernah di database.** Sumber: env `GROQ_API_KEY` atau file
   kredensial 0600. Semua jalur error melewati `privacy::redact_all`.
6. **Prompt anti-fabrikasi.** Prompt builder mewajibkan model menyatakan
   ketidakpastian dan hanya memakai konteks yang diberikan; parser JSON longgar
   + fallback teks agar output model yang tidak rapi tidak mematikan fitur.
7. **layer-shell opsional (cargo feature).** `gtk4-layer-shell` tidak selalu
   terpasang; aplikasi tetap berfungsi sebagai window biasa tanpa fitur ini.

## Protokol IPC

Socket: `$XDG_RUNTIME_DIR/niri-ai-copilot.sock` (line-based, satu perintah per
baris, balasan diakhiri newline). Perintah: `toggle`, `show`, `hide`, `start`,
`pause`, `resume`, `stop`, `expand`, `collapse`, `alt-answer`, `status`,
`quit`. Client: `niri-ai-copilot <perintah>` (subperintah CLI memakai client
internal yang sama). Bila tidak ada instans berjalan, CLI mengembalikan error
yang jelas (bukan menggagalkan secara senyap).
