#!/usr/bin/env bash
#
# Build the Fukushima Daiichi Unit 1 browser demo (gh:#1030) into an output
# directory (default web/fukushima/dist/). scripts/build-pages.sh calls this
# to publish it on the backend GitHub Pages site under demos/fukushima/.
#
#   crates/dhoby-ghaut/web/fukushima/build.sh [out-dir]
#   python3 -m http.server -d crates/dhoby-ghaut/web/fukushima/dist 8000
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
cargo build --release --target wasm32-unknown-unknown -p dhoby-ghaut --example fukushima_web

rm -rf "$dist"
mkdir -p "$dist"
wasm-bindgen --target web --no-typescript --out-dir "$dist" --out-name fukushima_web \
  "$root/target/wasm32-unknown-unknown/release/examples/fukushima_web.wasm"
cp "$here/index.html" "$here/fukushima_worker.js" "$dist/"
cp "$root/crates/outram-park-digital-twin-engine/web/gate.js" "$dist/"
echo "Fukushima demo built in $dist"
