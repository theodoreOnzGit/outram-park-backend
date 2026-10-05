#!/usr/bin/env bash
#
# Build the Monte Carlo browser demo (every rung of its rung table) into an output
# directory (default web/monte_carlo/dist/). scripts/build-pages.sh calls this
# to publish it on the backend GitHub Pages site under demos/monte-carlo/.
#
#   crates/dhoby-ghaut/web/monte_carlo/build.sh [out-dir]
#   python3 -m http.server -d crates/dhoby-ghaut/web/monte_carlo/dist 8000
#
# Needs the wasm32-unknown-unknown target and the wasm-bindgen CLI at EXACTLY
# the version Cargo.lock pins for the wasm-bindgen crate (the two must match):
#
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version <that version> --locked
#
# Everything is built in release (workspace rule). The data step runs the
# NATIVE build of the same example, so the tapes the browser downloads are
# produced by the same `strip_covariances` the native tests prove lossless.
# CARGO_BUILD_JOBS limits the build's parallelism if set.
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
cargo build --release --target wasm32-unknown-unknown -p dhoby-ghaut --example monte_carlo_web

rm -rf "$dist"
mkdir -p "$dist/data"
wasm-bindgen --target web --no-typescript --out-dir "$dist" --out-name monte_carlo_web \
  "$root/target/wasm32-unknown-unknown/release/examples/monte_carlo_web.wasm"
cp "$here/index.html" "$here/worker.js" "$dist/"

cargo run --release -p dhoby-ghaut --example monte_carlo_web -- --prepare-web-data "$dist/data"

# The geometry review images (the crate's drawing rule), with a page to view
# them, so the geometry is checkable from the web next to the demo.
geo_src="$root/crates/dhoby-ghaut/examples/monte_carlo_web/geometry"
mkdir -p "$dist/geometry"
cp "$geo_src"/*.png "$dist/geometry/"
{
  echo '<!DOCTYPE html><html lang="en"><head><meta charset="utf-8">'
  echo '<meta name="viewport" content="width=device-width, initial-scale=1">'
  echo '<title>Monte Carlo demo: geometry</title>'
  echo '<style>body{font:15px/1.5 system-ui,sans-serif;margin:24px auto;max-width:980px;padding:0 16px;color:#222}'
  echo 'img{max-width:100%;border:1px solid #ccc}figure{margin:28px 0}figcaption{color:#555}</style></head><body>'
  echo '<h1>Monte Carlo demo: geometry as the solver sees it</h1>'
  echo '<p>Every pixel is the material <code>Geometry::locate</code> finds there, on the assembled'
  echo 'geometry, drawn with <code>outram_mc_libs::geometry::plot::render_material_slice</code> (the port of'
  echo "OpenMC's plotter), for every rung the Watch mode transports through: the Godiva sphere (godiva_*),"
  echo 'the uranium-graphite cube (ugraphite_*), the lumped cell (lumped_*), the LCT-008 core (lct008_*)'
  echo 'and the TRISO pebble cell (files 1-5). Regenerate with <code>--render-geometry</code>.'
  echo '<a href="../">Back to the demo</a>.</p>'
  for f in "$dist"/geometry/*.png; do
    n="$(basename "$f")"
    echo "<figure><img src=\"$n\" alt=\"$n\"><figcaption>$n</figcaption></figure>"
  done
  echo '</body></html>'
} > "$dist/geometry/index.html"

echo
echo "Built $dist ($(du -sh "$dist" | cut -f1)). Serve it with any static file server, e.g.:"
echo "  python3 -m http.server -d \"$dist\" 8000   # then open http://localhost:8000"
