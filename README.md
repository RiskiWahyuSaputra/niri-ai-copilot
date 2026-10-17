# Niri AI Copilot

Asisten interview real-time untuk **Arch Linux + Niri (Wayland)**: transkripsi
lokal, deteksi pertanyaan, dan saran jawaban berbasis AI — tampil sebagai widget
mengambang di atas layar.

> Status: tahap awal. Modul inti + UI GTK4 sudah berjalan dan teruji
> (67 unit test + smoke test GUI). Fitur layer-shell belum dapat diuji di
> lingkungan pengembangan ini — lihat [docs/TESTING.md](docs/TESTING.md).

## Fitur

- **Transkripsi lokal** — audio diproses via `pw-record` + `whisper.cpp`
  (`whisper-cli`) di perangkat Anda. Tidak ada audio yang dikirim ke jaringan.
- **Practice Mode** — AI interviewer mengajukan pertanyaan; Anda menjawab dengan
  suara. Fallback bank pertanyaan offline bila API key tidak tersedia.
- **Interview Langsung** — deteksi pertanyaan interviewer dari transkrip, lalu
  saran jawaban yang memakai konteks profil/CV Anda.
- **Kontrol privasi** — gerbang izin perekaman, indikator REC selalu tampil,
  audio tidak disimpan secara default, retensi audio otomatis, redaksi rahasia
  di semua pesan error/log, dan disclosure sebelum teks dikirim ke AI eksternal.
- **Riwayat** — sesi, transkrip, pertanyaan, dan saran tersimpan di SQLite
  lokal; ekspor TXT/Markdown.
- **Integrasi Niri** — widget dikendalikan lewat socket IPC
  (`niri-ai-copilot toggle|start|pause|stop|...`) sehingga mudah dipasang ke
  `binds` di `~/.config/niri/config.kdl`.

## Persyaratan

| Komponen | Paket Arch | Catatan |
|---|---|---|
| GTK4 | `gtk4` | ≥ 4.10 (AlertDialog) |
| PipeWire | `pipewire` | `pw-record`, `pactl` |
| STT | `whisper-cpp` (opsional, runtime) | + model GGML, mis. `ggml-base.bin` |
| Rust | `rust` / `rustup` | dibangun dengan Rust 1.99 |
| gtk4-layer-shell | `gtk4-layer-shell` (opsional) | hanya untuk mode overlay layer-shell |

SQLite tidak perlu dipasang — `rusqlite` memakai fitur `bundled`.

Cek otomatis: `scripts/check-deps.sh`.

## Build & Jalankan

```sh
cargo build --release --locked
./target/release/niri-ai-copilot            # jalankan widget
./target/release/niri-ai-copilot --help     # bantuan CLI/IPC
cargo run -- --smoke-test                   # uji start GUI lalu keluar otomatis
```

Mode overlay layer-shell (opsional, butuh `gtk4-layer-shell` terpasang):

```sh
cargo build --release --features layer-shell
```

## Konfigurasi

- Config: `~/.config/niri-ai-copilot/config.toml` (dibuat/dirapikan oleh UI
  Pengaturan; boleh diedit manual lalu disimpan ulang lewat UI).
- API key Groq: environment variable `GROQ_API_KEY`, atau simpan lewat
  Pengaturan → AI. File kredensial `~/.config/niri-ai-copilot/credentials.toml`
  disimpan dengan permission `0600` dan **tidak pernah** masuk database.
- Data: `~/.local/share/niri-ai-copilot/` (`copilot.db`, `recordings/`).
- Override lokasi untuk testing:
  `NIRI_AI_COPILOT_CONFIG_DIR`, `NIRI_AI_COPILOT_DATA_DIR`.

Pengaturan penting: device microphone/meeting, binary & model whisper,
bahasa STT, model AI, panjang/gaya jawaban, retensi audio, dan privasi
(izinkan simpan audio, konfirmasi perekaman, timer REC).

## Integrasi Niri

Tambahkan ke `~/.config/niri/config.kdl` (contoh lengkap di
[docs/NIRI_SETUP.md](docs/NIRI_SETUP.md)):

```kdl
binds {
    Mod+Shift+C { spawn-sh "niri-ai-copilot toggle"; }
    Mod+Shift+S { spawn-sh "niri-ai-copilot start"; }
    Mod+Shift+X { spawn-sh "niri-ai-copilot stop"; }
}
```

Perintah IPC: `toggle`, `show`, `hide`, `start`, `pause`, `resume`, `stop`,
`expand`, `collapse`, `alt-answer`, `status`, `quit`.

## Dokumentasi

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — arsitektur & keputusan teknis
- [docs/NIRI_SETUP.md](docs/NIRI_SETUP.md) — integrasi Niri & layer-shell
- [docs/PRIVACY.md](docs/PRIVACY.md) — aliran data & kontrol privasi
- [docs/TESTING.md](docs/TESTING.md) — cara menguji + status keterujian

## Lisensi

MIT — lihat [LICENSE](LICENSE).
