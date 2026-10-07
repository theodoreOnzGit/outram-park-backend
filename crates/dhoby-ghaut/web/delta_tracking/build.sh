#!/usr/bin/env bash
#
# Build the delta-vs-surface tracking demo (gh:#784) into an output directory
# (default web/delta_tracking/dist/). scripts/build-pages.sh calls this to
# publish it on the backend GitHub Pages site under demos/delta-tracking/,
# AFTER the Monte Carlo demo: this demo reads that one's tapes from
# ../monte-carlo/data/ rather than publishing a second copy.
#
#   crates/dhoby-ghaut/web/delta_tracking/build.sh [out-dir]
#
# To serve it locally, build the Monte Carlo demo beside it and serve the
# parent folder:
#
#   crates/dhoby-ghaut/web/monte_carlo/build.sh /tmp/site/monte-carlo
#   crates/dhoby-ghaut/web/delta_tracking/build.sh /tmp/site/delta-tracking
#   python3 -m http.server -d /tmp/site 8000   # open http://localhost:8000/delta-tracking/
#
# Needs the wasm32-unknown-unknown target and the wasm-bindgen CLI at EXACTLY
# the version Cargo.lock pins for the wasm-bindgen crate. Everything is built
# in release (workspace rule). CARGO_BUILD_JOBS limits the build's
# parallelism if set.
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
cargo build --release --target wasm32-unknown-unknown -p dhoby-ghaut --example delta_tracking_web

rm -rf "$dist"
mkdir -p "$dist"
wasm-bindgen --target web --no-typescript --out-dir "$dist" --out-name delta_tracking_web \
  "$root/target/wasm32-unknown-unknown/release/examples/delta_tracking_web.wasm"
cp "$here/index.html" "$here/worker.js" "$dist/"

echo
echo "Built $dist ($(du -sh "$dist" | cut -f1)). It reads its tapes from ../monte-carlo/data/."
