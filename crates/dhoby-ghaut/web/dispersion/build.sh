#!/usr/bin/env bash
#
# Build the dispersion browser demo (every rung: plume, sigmas, rise-wake,
# puffs, deposition, dose, capstone) into an output directory (default
# web/dispersion/dist/). scripts/build-pages.sh calls this to publish it on
# the backend GitHub Pages site under demos/dispersion/ (gh:#530).
#
#   crates/dhoby-ghaut/web/dispersion/build.sh [out-dir]
#   python3 -m http.server -d crates/dhoby-ghaut/web/dispersion/dist 8000
#
# Needs the wasm32-unknown-unknown target and the wasm-bindgen CLI at EXACTLY
# the version Cargo.lock pins for the wasm-bindgen crate. Release build
# (workspace rule). No data files: every number is computed in the browser
# from buangkok and changi, except the capstone's, which are recorded in the
# example (examples/dispersion_web/recorded.rs) with their provenance.
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
cargo build --release --target wasm32-unknown-unknown -p dhoby-ghaut --example dispersion_web

rm -rf "$dist"
mkdir -p "$dist"
wasm-bindgen --target web --no-typescript --out-dir "$dist" --out-name dispersion_web \
  "$root/target/wasm32-unknown-unknown/release/examples/dispersion_web.wasm"
cp "$here/index.html" "$here/worker.js" "$dist/"

echo
echo "Built $dist ($(du -sh "$dist" | cut -f1)). Serve it with any static file server, e.g.:"
echo "  python3 -m http.server -d \"$dist\" 8000   # then open http://localhost:8000"
