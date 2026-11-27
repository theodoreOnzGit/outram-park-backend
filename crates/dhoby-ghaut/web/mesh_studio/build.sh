#!/usr/bin/env bash
#
# Build the Mesh Studio browser demo (gh:#1767, #1770) into an output directory
# (default web/mesh_studio/dist/). scripts/build-pages.sh calls this to publish
# it on the backend GitHub Pages site under demos/mesh-studio/.
#
#   crates/dhoby-ghaut/web/mesh_studio/build.sh [out-dir]
#   python3 -m http.server -d crates/dhoby-ghaut/web/mesh_studio/dist 8000
#
# Needs the wasm32-unknown-unknown target and the wasm-bindgen CLI at EXACTLY
# the version Cargo.lock pins for the wasm-bindgen crate. Release build
# (workspace rule). No data files: the surface is authored and volume-meshed
# in the browser (outram-blender's primitives, outram-park-fork-cfmesh's
# tet -> dual -> layers pipeline), in a Web Worker.
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
cargo build --release --target wasm32-unknown-unknown -p dhoby-ghaut --example mesh_studio

rm -rf "$dist"
mkdir -p "$dist"
wasm-bindgen --target web --no-typescript --out-dir "$dist" --out-name mesh_studio \
  "$root/target/wasm32-unknown-unknown/release/examples/mesh_studio.wasm"
cp "$here/index.html" "$here/worker.js" "$dist/"
# Stamp the build into every file URL the page and worker load (they carry
# `__BUILD__`), so a page and its worker always come from the same deploy.
build="$(git -C "$root" rev-parse --short=10 HEAD 2>/dev/null || echo local)"
if ! git -C "$root" diff --quiet 2>/dev/null; then build="$build-dirty"; fi
sed -i "s/__BUILD__/$build/g" "$dist/index.html" "$dist/worker.js"

echo
echo "Built $dist ($(du -sh "$dist" | cut -f1)). Serve it with any static file server, e.g.:"
echo "  python3 -m http.server -d \"$dist\" 8000   # then open http://localhost:8000"
