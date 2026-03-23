#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

echo "=== Building WASM release ==="
RUSTFLAGS="--cfg=web_sys_unstable_apis" cargo build \
    --release \
    --target wasm32-unknown-unknown \
    --manifest-path Cargo.toml

echo "=== Running wasm-bindgen ==="
wasm-bindgen \
    --out-dir web \
    --out-name photometric_comparison \
    --target web \
    ../../target/wasm32-unknown-unknown/release/photometric-comparison.wasm

echo "=== Build complete ==="
echo "Files in web/:"
ls -la web/
echo ""
echo "To test locally:"
echo "  cd web && python3 -m http.server 8080"
echo "  Open http://localhost:8080 in Chrome/Edge (WebGPU required)"
echo ""
echo "To deploy to iesna.eu:"
echo "  Copy web/ contents to your server"
