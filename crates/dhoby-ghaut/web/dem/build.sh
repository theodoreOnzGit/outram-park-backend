#!/usr/bin/env bash
#
# Build the DEM pour browser demo (gh:#787) into an output directory (default
# web/dem/dist/). scripts/build-pages.sh calls this to publish it on the
# backend GitHub Pages site under demos/dem/.
#
#   crates/dhoby-ghaut/web/dem/build.sh [out-dir]
#   python3 -m http.server -d crates/dhoby-ghaut/web/dem/dist 8000
#
# Needs the wasm32-unknown-unknown target and the wasm-bindgen CLI at EXACTLY
# the version Cargo.lock pins for the wasm-bindgen crate. Release build
# (workspace rule). No data files: the pour is computed in the browser by the
# LIGGGHTS port (outram-park-fork-liggghts' htr10_fill), and the full-size
# gh:#216 bed is compiled in (examples/common/htr10_beds.zz, 157 kB).
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
cargo build --release --target wasm32-unknown-unknown -p dhoby-ghaut --example dem_web

rm -rf "$dist"
mkdir -p "$dist"
wasm-bindgen --target web --no-typescript --out-dir "$dist" --out-name dem_web \
  "$root/target/wasm32-unknown-unknown/release/examples/dem_web.wasm"
cp "$here/index.html" "$here/worker.js" "$dist/"
# Stamp the build into every file URL the page and worker load (they carry
# `__BUILD__`), so a page and its worker always come from the same deploy.
build="$(git -C "$root" rev-parse --short=10 HEAD 2>/dev/null || echo local)"
if ! git -C "$root" diff --quiet 2>/dev/null; then build="$build-dirty"; fi
sed -i "s/__BUILD__/$build/g" "$dist/index.html" "$dist/worker.js"

echo
echo "Built $dist ($(du -sh "$dist" | cut -f1)). Serve it with any static file server, e.g.:"
echo "  python3 -m http.server -d \"$dist\" 8000   # then open http://localhost:8000"
