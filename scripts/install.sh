#!/usr/bin/env bash
# Build dan pasang Niri AI Copilot untuk pengguna saat ini.
#
# Pemakaian:
#   scripts/install.sh                 # pasang ke ~/.local/bin
#   PREFIX=/usr/local scripts/install.sh
#   scripts/install.sh --features layer-shell
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
FEATURES=""

while [ $# -gt 0 ]; do
    case "$1" in
        --features)
            FEATURES="$2"
            shift 2
            ;;
        *)
            echo "argumen tidak dikenal: $1" >&2
            exit 2
            ;;
    esac
done

echo "== Membangun release =="
if [ -n "$FEATURES" ]; then
    cargo build --release --locked --features "$FEATURES"
else
    cargo build --release --locked
fi

BIN="$ROOT/target/release/niri-ai-copilot"
if [ ! -x "$BIN" ]; then
    echo "error: binary hasil build tidak ditemukan: $BIN" >&2
    exit 1
fi

echo "== Memasang ke $PREFIX =="
install -Dm755 "$BIN" "$PREFIX/bin/niri-ai-copilot"
DOCDIR="$PREFIX/share/doc/niri-ai-copilot"
install -Dm644 README.md "$DOCDIR/README.md"
install -Dm644 LICENSE "$DOCDIR/LICENSE"
for f in docs/*.md; do
    install -Dm644 "$f" "$DOCDIR/$(basename "$f")"
done

echo
echo "Instalasi selesai: $PREFIX/bin/niri-ai-copilot"
case ":$PATH:" in
    *":$PREFIX/bin:"*) ;;
    *) echo "Catatan: pastikan $PREFIX/bin ada di PATH Anda." ;;
esac

cat <<'EOF'

Contoh keybind Niri (~/.config/niri/config.kdl):

  spawn-at-startup "niri-ai-copilot"
  binds {
      Mod+Shift+C { spawn-sh "niri-ai-copilot toggle"; }
      Mod+Shift+S { spawn-sh "niri-ai-copilot start"; }
      Mod+Shift+X { spawn-sh "niri-ai-copilot stop"; }
  }

Detail: docs/NIRI_SETUP.md
EOF
