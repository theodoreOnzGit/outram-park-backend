#!/usr/bin/env bash
#
# Build the CIET Educational Simulator browser demo (gh:#924) into an output
# directory (default web/ciet/dist/). scripts/build-pages.sh calls this to
# publish it on the backend GitHub Pages site under demos/ciet/.
#
#   crates/outram-park-digital-twin-engine/web/ciet/build.sh [out-dir]
#   python3 -m http.server -d crates/outram-park-digital-twin-engine/web/ciet/dist 8000
#
# Needs the wasm32-unknown-unknown target and the wasm-bindgen CLI at EXACTLY
# the version Cargo.lock pins for the wasm-bindgen crate. Everything is built
# in release (workspace rule).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../../../.." && pwd)"
dist="$(realpath -m "${1:-$here/dist}")"

want="$(grep -A1 '^name = "wasm-bindgen"$' "$root/Cargo.lock" | sed -n 's/^version = "\(.*\)"$/\1/p')"
have="$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)"
if [[ "$have" != "$want" ]]; then
  echo "wasm-bindgen CLI is '${have:-missing}', Cargo.lock needs '$want':" >&2
  echo "  cargo install wasm-bindgen-cli --version $want --locked" >&2
  exit 1
fi

cd "$root"
cargo build --release --target wasm32-unknown-unknown -p outram-park-digital-twin-engine --example ciet_web

rm -rf "$dist"
mkdir -p "$dist"
wasm-bindgen --target web --no-typescript --out-dir "$dist" --out-name ciet_web \
  "$root/target/wasm32-unknown-unknown/release/examples/ciet_web.wasm"
cp "$here/index.html" "$here/app.js" "$here/plant-worker.js" "$here/flow-worker.js" "$dist/"
echo "CIET demo built in $dist"
