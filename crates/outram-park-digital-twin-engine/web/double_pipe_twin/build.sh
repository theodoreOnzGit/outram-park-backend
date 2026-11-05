#!/usr/bin/env bash
#
# Build the double-pipe twin browser demo (gh:#1182) into an output directory (default
# web/double_pipe_twin/dist/). scripts/build-pages.sh calls this to publish it on the
# backend GitHub Pages site under demos/double-pipe-twin/.
#
#   crates/outram-park-digital-twin-engine/web/double_pipe_twin/build.sh [out-dir]
#   python3 -m http.server -d crates/outram-park-digital-twin-engine/web/double_pipe_twin/dist 8000
#
# MOVED 2026-11-05 from crates/dhoby-ghaut/web/ with its example (maintainer:
# "let the web examples deploy from digital twin engine instead"); the
# published path under demos/ is unchanged.
#
# Needs the wasm32-unknown-unknown target and the wasm-bindgen CLI at EXACTLY
# the version Cargo.lock pins for the wasm-bindgen crate. Release build
# (workspace rule). No data files to fetch: the TUAS 1-D UA is computed in the
# browser's worker (the digital-twin engine's one_d::ua), and the precomputed
# KOH / MFK / CFD / filtering artefacts are compiled into the wasm
# (verification_and_validation/double_pipe_twin_web_gh1182/, gh:#1182).
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
cargo build --release --target wasm32-unknown-unknown -p outram-park-digital-twin-engine --example double_pipe_twin_web

rm -rf "$dist"
mkdir -p "$dist"
wasm-bindgen --target web --no-typescript --out-dir "$dist" --out-name double_pipe_twin_web \
  "$root/target/wasm32-unknown-unknown/release/examples/double_pipe_twin_web.wasm"
cp "$here/index.html" "$here/worker.js" "$dist/"
# Stamp the build into every file URL the page and worker load (they carry
# `__BUILD__`), so a page and its worker always come from the same deploy.
build="$(git -C "$root" rev-parse --short=10 HEAD 2>/dev/null || echo local)"
if ! git -C "$root" diff --quiet 2>/dev/null; then build="$build-dirty"; fi
sed -i "s/__BUILD__/$build/g" "$dist/index.html" "$dist/worker.js"

echo
echo "Built $dist ($(du -sh "$dist" | cut -f1)). Serve it with any static file server, e.g.:"
echo "  python3 -m http.server -d \"$dist\" 8000   # then open http://localhost:8000"
