# Integrasi Niri — Niri AI Copilot

Aplikasi ini tidak memasang global shortcut sendiri. Widget dikendalikan lewat
socket IPC, sehingga integrasi cukup dengan menambah `binds` di konfigurasi
Niri Anda.

## 1. Jalankan aplikasi saat login

Tambahkan ke `~/.config/niri/config.kdl`:

```kdl
spawn-at-startup "niri-ai-copilot"
```

Atau jalankan manual dari terminal saat dibutuhkan.

## 2. Keybind contoh

```kdl
binds {
    // Tampilkan / sembunyikan widget
    Mod+Shift+C { spawn-sh "niri-ai-copilot toggle"; }

    // Mulai sesi (dialog izin tetap muncul dan wajib dikonfirmasi)
    Mod+Shift+S { spawn-sh "niri-ai-copilot start"; }

    // Jeda / lanjutkan perekaman
    Mod+Shift+P { spawn-sh "niri-ai-copilot pause"; }
    Mod+Shift+R { spawn-sh "niri-ai-copilot resume"; }

    // Akhiri sesi
    Mod+Shift+X { spawn-sh "niri-ai-copilot stop"; }

    // Perluas / kecilkan widget
    Mod+Shift+E { spawn-sh "niri-ai-copilot expand"; }
    Mod+Shift+Q { spawn-sh "niri-ai-copilot collapse"; }

    // Minta alternatif jawaban terakhir (butuh API key)
    Mod+Shift+A { spawn-sh "niri-ai-copilot alt-answer"; }
}
```

Catatan:

- `spawn-sh` menjalankan lewat shell, jadi pastikan `niri-ai-copilot` ada di
  `PATH` (mis. `~/.local/bin` atau `/usr/bin`).
- `start` dari shortcut hanya membuka dialog izin; perekaman **tidak akan**
  dimulai tanpa konfirmasi eksplisit di UI (kecuali pengguna menonaktifkan
  konfirmasi di Pengaturan → Privasi — tidak disarankan).
- Semua perintah mengembalikan balasan teks yang bisa dicek manual:
  `niri-ai-copilot status`.

## 3. Mode overlay layer-shell (opsional)

Secara default widget tampil sebagai window GTK biasa (dapat di-pin/diatur
seperti window lain, mis. lewat rules Niri). Bila Anda ingin widget selalu
mengambang di layer overlay (seperti notifikasi), pasang
`gtk4-layer-shell` dan build dengan feature tersebut:

```sh
sudo pacman -S gtk4-layer-shell
cargo build --release --features layer-shell
```

Tanpa feature ini, aplikasi tetap berjalan normal; fungsi layer-shell hanya
tidak aktif.

Contoh rule Niri untuk memaksa widget mengambang di workspace (window biasa):

```kdl
window-rule {
    match app-id="io.github.riskiwahyusaputra.niri-ai-copilot"
    open-floating true
}
```

## 4. Verifikasi

1. Jalankan `niri-ai-copilot` lalu `niri-ai-copilot status` — harus menjawab
   status aplikasi (bukan error koneksi).
2. Tekan keybind `toggle` — widget muncul/hilang.
3. Tekan `start`, pilih sumber audio, konfirmasi izin — indikator REC menyala.
4. Bicara beberapa detik; transkrip lokal muncul di panel Transcript.

Bila `status` menjawab "tidak dapat terhubung", aplikasi belum berjalan atau
socket `$XDG_RUNTIME_DIR/niri-ai-copilot.sock` tidak ada.
