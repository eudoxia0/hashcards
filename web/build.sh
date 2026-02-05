#!/bin/bash
# Build script for hashcards static site
# Requires: wasm-pack (cargo install wasm-pack)

set -e

cd "$(dirname "$0")/.."

echo "Building WASM module..."
wasm-pack build --target web --out-dir ../../web/pkg crates/hashcards-wasm

echo ""
echo "Build complete! The static site is in the 'web' directory."
echo ""
echo "To serve locally, run:"
echo "  cd web && python3 -m http.server 8000"
echo ""
echo "Then open http://localhost:8000 in your browser."
