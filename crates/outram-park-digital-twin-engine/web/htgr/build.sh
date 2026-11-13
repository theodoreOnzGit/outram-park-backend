#!/usr/bin/env bash
#
# Build the HTGR simulator browser demo (gh:#986) into an output directory
# (default web/htgr/dist/). Mirrors web/ciet/build.sh, which this is modelled
# on, so the two demos are built and published the same way.
#
#   crates/outram-park-digital-twin-engine/web/htgr/build.sh [out-dir]
#   python3 -m http.server -d crates/outram-park-digital-twin-engine/web/htgr/dist 8000
#
# The demo is PASSWORD-GATED (web/gate.js). That gate is a sign, not a lock:
# everything served here is readable by whoever loads it. Do not publish
# anything behind it that would matter if it were read.
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
cargo build --release --target wasm32-unknown-unknown \
  -p outram-park-digital-twin-engine --example htgr_sim_v1

rm -rf "$dist"
mkdir -p "$dist"
wasm-bindgen --target web --no-typescript --out-dir "$dist" --out-name htgr_sim_v1 \
  "$root/target/wasm32-unknown-unknown/release/examples/htgr_sim_v1.wasm"
cp "$here/index.html" "$here/htgr_worker.js" "$dist/"
cp "$here/../gate.js" "$dist/"
echo "HTGR demo built in $dist"
