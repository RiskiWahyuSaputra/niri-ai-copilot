#!/usr/bin/env bash
# Hapus pemasangan Niri AI Copilot.
#
# Pemakaian:
#   scripts/uninstall.sh                # hapus binary + dokumen
#   scripts/uninstall.sh --purge        # + hapus config & data (riwayat, audio)
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"
PURGE=0
for arg in "$@"; do
    case "$arg" in
        --purge) PURGE=1 ;;
        *) echo "argumen tidak dikenal: $arg" >&2; exit 2 ;;
    esac
done

BIN="$PREFIX/bin/niri-ai-copilot"
DOCDIR="$PREFIX/share/doc/niri-ai-copilot"
CFG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/niri-ai-copilot"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/niri-ai-copilot"

if [ -e "$BIN" ]; then
    rm -f "$BIN"
    echo "Dihapus: $BIN"
else
    echo "Tidak ada: $BIN (dilewati)"
fi
[ -d "$DOCDIR" ] && rm -rf "$DOCDIR" && echo "Dihapus: $DOCDIR"

if [ "$PURGE" -eq 1 ]; then
    echo
    echo "PERINGATAN: menghapus riwayat sesi, profil, database, dan file audio:"
    echo "  - $CFG_DIR (config + kredensial API key)"
    echo "  - $DATA_DIR (copilot.db + recordings/)"
    printf "Ketik 'hapus' untuk melanjutkan: "
    read -r answer
    if [ "$answer" = "hapus" ]; then
        rm -rf "$CFG_DIR" "$DATA_DIR"
        echo "Data pengguna dihapus."
    else
        echo "Dibatalkan — data pengguna tidak disentuh."
    fi
fi

echo "Selesai."
