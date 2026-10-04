#!/usr/bin/env bash
#
# Build the TRISO pebble browser demo into web/triso_pebble/dist/.
#
#   crates/dhoby-ghaut/web/triso_pebble/build.sh
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
dist="$here/dist"

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

echo
echo "Built $dist ($(du -sh "$dist" | cut -f1)). Serve it with any static file server, e.g.:"
echo "  python3 -m http.server -d \"$dist\" 8000   # then open http://localhost:8000"
