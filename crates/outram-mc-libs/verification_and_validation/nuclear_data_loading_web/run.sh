#!/usr/bin/env bash
#
# Build the nuclear-data loading probe for wasm, serve the repository, and run
# probe.html in headless Chromium (gh #935, #936). Test harness, not a demo.
#
#   crates/outram-mc-libs/verification_and_validation/nuclear_data_loading_web/run.sh [--skip-godiva]
#
# Needs: the wasm32-unknown-unknown target, the wasm-bindgen CLI at exactly the
# version Cargo.lock pins, chromium (or $CHROMIUM), node >= 22, python3 (for the
# static file server). Everything builds in release (workspace rule).
# CARGO_BUILD_JOBS limits the build if set. The page reads reference-data/ from
# the served repository: no network fetch leaves this machine.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../../../.." && pwd)"
want="$(grep -A1 '^name = "wasm-bindgen"$' "$root/Cargo.lock" | sed -n 's/^version = "\(.*\)"$/\1/p')"
have="$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)"
if [[ "$have" != "$want" ]]; then
  echo "wasm-bindgen CLI is '${have:-missing}', Cargo.lock needs '$want'" >&2
  exit 1
fi

cd "$root"
cargo build --release --target wasm32-unknown-unknown -p outram-mc-libs --example nd_wasm_probe
target="${CARGO_TARGET_DIR:-$root/target}"
rm -rf "$here/pkg"
wasm-bindgen --target web --no-typescript --out-dir "$here/pkg" --out-name nd_wasm_probe \
  "$target/wasm32-unknown-unknown/release/examples/nd_wasm_probe.wasm"

port=8935
python3 -m http.server "$port" --bind 127.0.0.1 -d "$root" >/dev/null 2>&1 &
server=$!
trap 'kill $server' EXIT
sleep 1
query=""
[[ "${1:-}" == "--skip-godiva" ]] && query="?skip-godiva"
rel="${here#"$root"/}"
node "$here/run_headless.mjs" "http://127.0.0.1:$port/$rel/probe.html$query" 3600 "$here/pkg/result.json"
