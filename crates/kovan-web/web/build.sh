#!/usr/bin/env bash
#
# Build web-kovan's page (the wasm32 build of examples/web.rs, its JS glue
# and index.html) into an output directory (default web/dist/). The data
# folder next to it is data.sh's job. scripts/build-pages.sh publishes both
# under code-review/ (GitHub #736).
#
#   bash crates/kovan-web/web/build.sh [out-dir]
#   python3 -m http.server -d <out-dir> 8000
#
# Needs the wasm32-unknown-unknown target and the wasm-bindgen CLI at EXACTLY
# the version Cargo.lock pins for the wasm-bindgen crate (same check as the
# dhoby-ghaut demos' build scripts). Release build (workspace rule).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
dist="$(realpath -m "${1:-$here/dist}")"

want="$(grep -A1 '^name = "wasm-bindgen"$' "$root/Cargo.lock" | sed -n 's/^version = "\(.*\)"$/\1/p')"
have="$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)"
if [[ "$have" != "$want" ]]; then
  echo "wasm-bindgen CLI is '${have:-missing}', Cargo.lock needs '$want':" >&2
  echo "  cargo install wasm-bindgen-cli --version $want --locked" >&2
  exit 1
fi

cd "$root"
cargo build --release -j "${PAGES_JOBS:-3}" --target wasm32-unknown-unknown -p kovan-web --example web

mkdir -p "$dist"
rm -f "$dist"/kovan_web.js "$dist"/kovan_web_bg.wasm
wasm-bindgen --target web --no-typescript --out-dir "$dist" --out-name kovan_web \
  "$root/target/wasm32-unknown-unknown/release/examples/web.wasm"
cp "$here/index.html" "$dist/"
echo "Built $dist ($(du -sh "$dist" | cut -f1))."
