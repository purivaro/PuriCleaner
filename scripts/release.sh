#!/bin/bash
# Build every download for a GitHub release into dist/release/ with stable
# names (README links use releases/latest/download/<name>).
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
export LC_ALL=en_US.UTF-8 LANG=en_US.UTF-8

# macOS: one app for Apple Silicon and Intel.
rm -f target/universal-apple-darwin/release/bundle/dmg/*.dmg
(cd src-tauri && npx --yes @tauri-apps/cli@2 build --target universal-apple-darwin)

# Windows (cross-built with MinGW).
./scripts/build-windows.sh

R=dist/release
rm -rf "$R" && mkdir -p "$R"
cp target/universal-apple-darwin/release/bundle/dmg/*.dmg "$R/PuriCleaner-mac.dmg"
cp dist/windows/*-setup.exe "$R/PuriCleaner-windows-setup.exe"
cp "dist/windows/Puri Cleaner portable.zip" "$R/PuriCleaner-windows-portable.zip"
cp dist/windows/puri-cleaner-web.exe "$R/PuriCleaner-web-windows.exe"
(cd "$R" && shasum -a 256 PuriCleaner-* > SHA256SUMS.txt)
ls -la "$R"
