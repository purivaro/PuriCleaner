#!/bin/bash
# Run the browser version against the ui/ folder on disk (edit + reload).
# Open http://127.0.0.1:7171/#t=dev
cd "$(dirname "$0")/.."
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
cargo build --release -p puri-cleaner-web || exit 1
PURI_UI_DIR=ui PURI_TOKEN=dev exec ./target/release/puri-cleaner-web --no-open --port 7171 "$@"
