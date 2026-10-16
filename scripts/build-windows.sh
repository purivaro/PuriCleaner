#!/bin/bash
# Cross-build the Windows version on a Mac.
# Needs: rustup target add x86_64-pc-windows-gnu; brew install mingw-w64 nsis
# Output: dist/windows/ (installer, portable app, browser version)
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
# Homebrew's makensis crashes (std::bad_alloc) under the C locale.
export LC_ALL=en_US.UTF-8 LANG=en_US.UTF-8
T=x86_64-pc-windows-gnu
rm -f "target/$T/release/bundle/nsis/"*-setup.exe   # keep only this version's installer
(cd src-tauri && npx --yes @tauri-apps/cli@2 build --target "$T")
cargo build --release -p puri-cleaner-web --target "$T"
rm -rf dist/windows && mkdir -p dist/windows
cp "target/$T/release/bundle/nsis/"*-setup.exe dist/windows/
# Portable: the GNU build loads WebView2Loader.dll at runtime, so ship it alongside.
P="dist/windows/Puri Cleaner portable"
rm -rf "$P" && mkdir -p "$P"
cp "target/$T/release/puri-cleaner.exe" "$P/Puri Cleaner.exe"
cp "target/$T/release/WebView2Loader.dll" "$P/"
(cd dist/windows && rm -f "Puri Cleaner portable.zip" && zip -qr "Puri Cleaner portable.zip" "Puri Cleaner portable" && rm -rf "Puri Cleaner portable")
cp "target/$T/release/puri-cleaner-web.exe" dist/windows/
ls -la dist/windows
