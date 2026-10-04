#!/usr/bin/env bash
#
# Build the nuclear data browser demo (every rung) into an output directory
# (default web/nuclear_data/dist/). scripts/build-pages.sh calls this to
# publish it on the backend GitHub Pages site under demos/nuclear-data/.
#
#   crates/dhoby-ghaut/web/nuclear_data/build.sh [out-dir]
#   python3 -m http.server -d crates/dhoby-ghaut/web/nuclear_data/dist 8000
#
# Needs the wasm32-unknown-unknown target and the wasm-bindgen CLI at EXACTLY
# the version Cargo.lock pins for the wasm-bindgen crate. Everything is built
# in release (workspace rule). The data step runs the NATIVE build of the same
# example, so the browser gets the same covariance-stripped tapes the native
# engine reads. CARGO_BUILD_JOBS limits the build's parallelism if set.
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
cargo build --release --target wasm32-unknown-unknown -p dhoby-ghaut --example nuclear_data_web

rm -rf "$dist"
mkdir -p "$dist/data"
wasm-bindgen --target web --no-typescript --out-dir "$dist" --out-name nuclear_data_web \
  "$root/target/wasm32-unknown-unknown/release/examples/nuclear_data_web.wasm"
cp "$here/index.html" "$here/worker.js" "$dist/"

cargo run --release -p dhoby-ghaut --example nuclear_data_web -- --prepare-web-data "$dist/data"

echo
echo "Built $dist ($(du -sh "$dist" | cut -f1)). Serve it with any static file server, e.g.:"
echo "  python3 -m http.server -d \"$dist\" 8000   # then open http://localhost:8000"
