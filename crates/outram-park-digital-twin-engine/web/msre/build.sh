#!/usr/bin/env bash
#
# Build the MSRE museum simulator (gh:#1569, #1570) into an output
# directory (default web/msre/dist/). scripts/build-pages.sh calls this
# to publish it on the backend GitHub Pages site under demos/msre/.
#
#   crates/outram-park-digital-twin-engine/web/msre/build.sh [out-dir]
#   python3 -m http.server -d crates/outram-park-digital-twin-engine/web/msre/dist 8000
#
#
# The demo is PASSWORD-GATED by the workspace's one shared gate,
# crates/outram-park-digital-twin-engine/web/gate.js (copied in at build
# time, so the password is rotated in one place for every demo). That gate is
# a sign, not a lock: everything served here is readable by whoever loads it.
#
# Needs the wasm32-unknown-unknown target and the wasm-bindgen CLI at EXACTLY
# the version Cargo.lock pins. Everything is built in release (workspace rule).
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
cargo build --release --target wasm32-unknown-unknown -p outram-park-digital-twin-engine --example msre_web

rm -rf "$dist"
mkdir -p "$dist"
wasm-bindgen --target web --no-typescript --out-dir "$dist" --out-name msre_web \
  "$root/target/wasm32-unknown-unknown/release/examples/msre_web.wasm"
cp "$here/index.html" "$here/msre_worker.js" "$dist/"
# The k_eff tab's tapes (gh:#1571), stripped and compressed, beside the page.
cargo run --release -p outram-park-digital-twin-engine --example msre_web -- --prepare-web-data "$dist/data"
cp "$root/crates/outram-park-digital-twin-engine/web/gate.js" "$dist/"
echo "MSRE demo built in $dist"
