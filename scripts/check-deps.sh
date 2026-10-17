#!/usr/bin/env bash
# Cek dependensi Niri AI Copilot. Keluar dengan kode 1 bila dependensi
# esensial tidak ditemukan (yang opsional hanya diberi peringatan).
set -u

ok=0
warn=0

status() { # status <OK|WARN|MISS> <nama> <detail>
    printf '%-6s %-22s %s\n' "$1" "$2" "$3"
    if [ "$1" = "MISS" ]; then ok=1; fi
    if [ "$1" = "WARN" ]; then warn=$((warn + 1)); fi
}

have() { command -v "$1" >/dev/null 2>&1; }

echo "== Niri AI Copilot: pemeriksaan dependensi =="
echo

# --- Esensial untuk build ---
if have cargo; then
    status OK "cargo" "$(cargo --version)"
else
    status MISS "cargo" "pasang: sudo pacman -S rust"
fi
if have rustc; then
    status OK "rustc" "$(rustc --version)"
else
    status MISS "rustc" "pasang: sudo pacman -S rust"
fi
if have pkg-config && pkg-config --exists gtk4 2>/dev/null; then
    status OK "gtk4 (dev)" "pkg-config gtk4 $(pkg-config --modversion gtk4)"
else
    status MISS "gtk4 (dev)" "pasang: sudo pacman -S gtk4"
fi

# --- Esensial untuk runtime audio ---
if have pactl; then
    status OK "pactl" "$(pactl --version 2>/dev/null | head -1)"
else
    status MISS "pactl" "pasang: sudo pacman -S libpulse (atau pipewire-pulse)"
fi
if have pw-record; then
    status OK "pw-record" "tersedia (pipewire)"
else
    status MISS "pw-record" "pasang: sudo pacman -S pipewire"
fi

# --- Opsional ---
if have whisper-cli; then
    status OK "whisper-cli" "$(command -v whisper-cli) (paket whisper-cpp)"
else
    status WARN "whisper-cli" "belum ada — transkripsi butuh: sudo pacman -S whisper-cpp"
fi
if have pdftotext; then
    status OK "pdftotext" "impor CV PDF tersedia (poppler)"
else
    status WARN "pdftotext" "tidak ada — impor CV PDF butuh: sudo pacman -S poppler"
fi
if pkg-config --exists gtk4-layer-shell 2>/dev/null; then
    status OK "gtk4-layer-shell" "mode overlay dapat dibuild (--features layer-shell)"
else
    status WARN "gtk4-layer-shell" "opsional — sudo pacman -S gtk4-layer-shell"
fi

echo
if [ "$ok" -ne 0 ]; then
    echo "Hasil: dependensi ESENSIAL belum lengkap (lihat baris MISS)."
    exit 1
fi
if [ "$warn" -gt 0 ]; then
    echo "Hasil: dependensi esensial OK; $warn item opsional belum ada."
else
    echo "Hasil: semua dependensi tersedia."
fi
