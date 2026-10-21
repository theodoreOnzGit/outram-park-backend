#!/usr/bin/env bash
#
# Build the TRISO-ATOPS and fuel failure browser demo (every rung: triso,
# decay, walk, layers, failure, chemistry, release, source-term) into an
# output directory (default web/triso_atops/dist/). scripts/build-pages.sh
# calls this to publish it on the backend GitHub Pages site under
# demos/triso-atops/ (gh:#540).
#
#   crates/dhoby-ghaut/web/triso_atops/build.sh [out-dir]
#   python3 -m http.server -d crates/dhoby-ghaut/web/triso_atops/dist 8000
#
# Needs the wasm32-unknown-unknown target and the wasm-bindgen CLI at EXACTLY
# the version Cargo.lock pins for the wasm-bindgen crate. Release build
# (workspace rule). No data files: every number is computed in the browser
# from boon-lay (its ENDF/B-VIII.0 decay data are compiled in), except rung
# 8's source term, recorded with its provenance in
# examples/dispersion_web/recorded.rs and shared by both demos.
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
cargo build --release --target wasm32-unknown-unknown -p dhoby-ghaut --example triso_atops_web

rm -rf "$dist"
mkdir -p "$dist"
wasm-bindgen --target web --no-typescript --out-dir "$dist" --out-name triso_atops_web \
  "$root/target/wasm32-unknown-unknown/release/examples/triso_atops_web.wasm"
cp "$here/index.html" "$here/worker.js" "$dist/"
# Stamp the build into every file URL the page and worker load (they carry
# `__BUILD__`): GitHub Pages caches each file for 10 minutes under the same
# name, so without this a reader can get an earlier deploy, or a page and a
# worker from different deploys. A dirty tree gets a `-dirty` suffix.
build="$(git -C "$root" rev-parse --short=10 HEAD 2>/dev/null || echo local)"
if ! git -C "$root" diff --quiet 2>/dev/null; then build="$build-dirty"; fi
sed -i "s/__BUILD__/$build/g" "$dist/index.html" "$dist/worker.js"

echo
echo "Built $dist ($(du -sh "$dist" | cut -f1)). Serve it with any static file server, e.g.:"
echo "  python3 -m http.server -d \"$dist\" 8000   # then open http://localhost:8000"
