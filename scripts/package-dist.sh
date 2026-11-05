#!/usr/bin/env bash
# Copia el instalador de Windows a dist/installer con un nombre estable.
# Uso (tras compilar):
#   bash scripts/package-dist.sh                          # compilación nativa en Windows
#   bash scripts/package-dist.sh x86_64-pc-windows-msvc   # compilación cruzada desde WSL/Linux
set -euo pipefail
cd "$(dirname "$0")/.."
TARGET="${1:-}"
TARGET_DIR="${CARGO_TARGET_DIR:-src-tauri/target}"
NSIS_DIR="$TARGET_DIR/${TARGET:+$TARGET/}release/bundle/nsis"
VERSION=$(node -p "require('./package.json').version")
SETUP=$(ls "$NSIS_DIR"/*_"${VERSION}"_*-setup.exe | head -1)   # solo el de la versión actual
rm -rf dist && mkdir -p dist/installer
cp "$SETUP" dist/installer/TikTok-OBS-Connector-Setup.exe
sha256sum dist/installer/TikTok-OBS-Connector-Setup.exe
ls -la dist/installer
