#!/usr/bin/env bash
#
# Build the EBR-II museum simulator (gh:#1741) into an output directory
# (default web/ebr2/dist/). scripts/build-pages.sh calls this to publish it on
# the backend GitHub Pages site under demos/ebr2/.
#
#   crates/outram-park-digital-twin-engine/web/ebr2/build.sh [out-dir]
#   python3 -m http.server -d crates/outram-park-digital-twin-engine/web/ebr2/dist 8000
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
cargo build --release --target wasm32-unknown-unknown -p outram-park-digital-twin-engine --example ebr2_web

rm -rf "$dist"
mkdir -p "$dist"
wasm-bindgen --target web --no-typescript --out-dir "$dist" --out-name ebr2_web \
  "$root/target/wasm32-unknown-unknown/release/examples/ebr2_web.wasm"
cp "$here/index.html" "$here/ebr2_worker.js" "$dist/"
cp "$root/crates/outram-park-digital-twin-engine/web/gate.js" "$dist/"
echo "EBR-II demo built in $dist"
