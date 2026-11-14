#!/usr/bin/env bash
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "$ROOT_DIR"

echo "Building Marupo (release)..."
cargo build --release

echo "Installing to $PREFIX..."
install -Dm755 target/release/Marupo "$PREFIX/bin/Marupo"
install -Dm644 assets/marupo_icon.png \
  "$PREFIX/share/icons/hicolor/256x256/apps/io.github.negxkuniklo.Marupo.png"
install -Dm644 packaging/linux/io.github.negxkuniklo.Marupo.desktop \
  "$PREFIX/share/applications/io.github.negxkuniklo.Marupo.desktop"
install -Dm644 packaging/linux/io.github.negxkuniklo.Marupo.metainfo.xml \
  "$PREFIX/share/metainfo/io.github.negxkuniklo.Marupo.metainfo.xml"
install -Dm644 LICENSE.txt \
  "$PREFIX/share/licenses/io.github.negxkuniklo.Marupo/LICENSE.txt"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$PREFIX/share/applications" || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" || true
fi

echo "Done! Launch 'Marupo' from your app menu or run: $PREFIX/bin/Marupo"
