#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
BUILD_DIR="$ROOT_DIR/target/flatpak-build"
REPO_DIR="$ROOT_DIR/target/flatpak-repo"
BUNDLE_OUT="${1:-$ROOT_DIR/target/Marupo.flatpak}"

cd "$SCRIPT_DIR"

if ! command -v flatpak-builder >/dev/null 2>&1; then
  echo "Error: flatpak-builder is not installed."
  echo "Install it on Ubuntu/Debian/Mint with: sudo apt install flatpak flatpak-builder"
  exit 1
fi

# Ensure Flathub remote and required runtime/SDK are installed
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user -y flathub \
  org.freedesktop.Platform//24.08 \
  org.freedesktop.Sdk//24.08 \
  org.freedesktop.Sdk.Extension.rust-stable//24.08

mkdir -p "$(dirname "$BUNDLE_OUT")"

flatpak-builder \
  --user \
  --force-clean \
  --install-deps-from=flathub \
  --repo="$REPO_DIR" \
  "$BUILD_DIR" \
  "$SCRIPT_DIR/io.github.negxkuniklo.Marupo.yml"

flatpak build-bundle \
  "$REPO_DIR" \
  "$BUNDLE_OUT" \
  io.github.negxkuniklo.Marupo

echo ""
echo "Flatpak bundle created: $BUNDLE_OUT"
echo "Install locally with: flatpak install --user -y \"$BUNDLE_OUT\""
echo "Run with:             flatpak run io.github.negxkuniklo.Marupo"
