#!/usr/bin/env bash
#
# Build the TRISO pebble browser demo into an output directory (default
# web/triso_pebble/dist/). scripts/build-pages.sh calls this to publish it on
# the backend GitHub Pages site under demos/triso-pebble/.
#
#   crates/dhoby-ghaut/web/triso_pebble/build.sh [out-dir]
#   python3 -m http.server -d crates/dhoby-ghaut/web/triso_pebble/dist 8000
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
cargo build --release --target wasm32-unknown-unknown -p dhoby-ghaut --example triso_pebble_web

rm -rf "$dist"
mkdir -p "$dist/data"
wasm-bindgen --target web --no-typescript --out-dir "$dist" --out-name triso_pebble_web \
  "$root/target/wasm32-unknown-unknown/release/examples/triso_pebble_web.wasm"
cp "$here/index.html" "$dist/"

cargo run --release -p dhoby-ghaut --example triso_pebble_web -- --prepare-web-data "$dist/data"

# The geometry review images (the crate's drawing rule), with a page to view
# them, so the geometry is checkable from the web next to the demo.
geo_src="$root/crates/dhoby-ghaut/examples/triso_pebble_web/geometry"
mkdir -p "$dist/geometry"
cp "$geo_src"/*.png "$dist/geometry/"
{
  echo '<!DOCTYPE html><html lang="en"><head><meta charset="utf-8">'
  echo '<meta name="viewport" content="width=device-width, initial-scale=1">'
  echo '<title>TRISO pebble demo: geometry</title>'
  echo '<style>body{font:15px/1.5 system-ui,sans-serif;margin:24px auto;max-width:980px;padding:0 16px;color:#222}'
  echo 'img{max-width:100%;border:1px solid #ccc}figure{margin:28px 0}figcaption{color:#555}</style></head><body>'
  echo '<h1>TRISO pebble demo: geometry as the solver sees it</h1>'
  echo '<p>Every pixel is the material <code>Geometry::locate</code> finds there, on the assembled'
  echo 'geometry, drawn with <code>outram_mc_libs::geometry::plot::render_material_slice</code> (the port of'
  echo "OpenMC's plotter). Regenerate with <code>--render-geometry</code>. <a href=\"../\">Back to the demo</a>.</p>"
  for f in "$dist"/geometry/*.png; do
    n="$(basename "$f")"
    echo "<figure><img src=\"$n\" alt=\"$n\"><figcaption>$n</figcaption></figure>"
  done
  echo '</body></html>'
} > "$dist/geometry/index.html"

echo
echo "Built $dist ($(du -sh "$dist" | cut -f1)). Serve it with any static file server, e.g.:"
echo "  python3 -m http.server -d \"$dist\" 8000   # then open http://localhost:8000"
