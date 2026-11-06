#!/usr/bin/env bash
#
# check-wasm-demos.sh -- the wasm32 compile gate for every GitHub Pages demo
# (gh:#1304). Runs on push to develop and on pull requests
# (.github/workflows/wasm-demos.yml), and locally:
#
#   scripts/check-wasm-demos.sh          # check every Pages demo
#   scripts/check-wasm-demos.sh --list   # print "<package> <example>" pairs only
#
# WHY IT EXISTS. A demo that only breaks on wasm32 (a dependency that
# `cfg(not(wasm32))` dropped, as `tampines` was from `helical_sg_web` on
# 2026-11-05) still compiles natively, so the native compile gate stays green
# and the break surfaces about 50 minutes into the Pages build. This is the
# same compile for far less: `cargo check --release --target
# wasm32-unknown-unknown` of each demo's example, no wasm-bindgen, no data.
# (`scripts/check-wasm.sh` is the --lib gate over library crates; it cannot
# see examples.)
#
# THE DEMO LIST IS NOT WRITTEN HERE. It is read from the `cargo build`
# command of every `crates/*/web/*/build.sh` plus `crates/kovan-web/web/build.sh`
# (the Pages demos' own build scripts; a command may span lines with a
# trailing backslash, as htgr's does), so a demo added to Pages is gated the
# moment its build.sh exists. A build.sh with no wasm cargo build in it fails
# the gate rather than being skipped.
#
# Release profile, like everything in this workspace. Examples of one package
# go in one cargo invocation.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

scripts=(crates/*/web/*/build.sh crates/kovan-web/web/build.sh)

# "<package> <example>" for every wasm cargo build in the scripts.
pairs() {
  local f cmd
  for f in "${scripts[@]}"; do
    [[ -f "$f" ]] || continue
    # Join backslash-continued lines, keep the wasm32 cargo build commands.
    cmd="$(sed -e ':a' -e '/\\$/N; s/\\\n[[:space:]]*/ /; ta' "$f" \
      | grep -E '^[[:space:]]*cargo build .*--target wasm32-unknown-unknown' || true)"
    [[ -n "$cmd" ]] || { echo "check-wasm-demos: no wasm32 cargo build in $f" >&2; exit 1; }
    while IFS= read -r line; do
      local pkg ex
      pkg="$(grep -oE -- '-p [A-Za-z0-9_-]+' <<< "$line" | head -1 | cut -d' ' -f2)"
      ex="$(grep -oE -- '--example [A-Za-z0-9_-]+' <<< "$line" | head -1 | cut -d' ' -f2)"
      [[ -n "$pkg" && -n "$ex" ]] || { echo "check-wasm-demos: cannot read -p/--example from: $line ($f)" >&2; exit 1; }
      echo "$pkg $ex"
    done <<< "$cmd"
  done
}

list="$(pairs | sort -u)"
[[ -n "$list" ]] || { echo "check-wasm-demos: no demos found" >&2; exit 1; }
if [[ "${1:-}" == "--list" ]]; then
  echo "$list"
  exit 0
fi

rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
fail=0
for pkg in $(cut -d' ' -f1 <<< "$list" | sort -u); do
  args=()
  for ex in $(awk -v p="$pkg" '$1 == p { print $2 }' <<< "$list"); do
    args+=(--example "$ex")
  done
  echo "check-wasm-demos: $pkg ${args[*]}"
  cargo check --release --target wasm32-unknown-unknown -p "$pkg" "${args[@]}" || fail=1
done
if (( fail )); then
  echo "check-wasm-demos: FAILED: a Pages demo does not compile for wasm32-unknown-unknown" >&2
  exit 1
fi
echo "check-wasm-demos: $(wc -l <<< "$list") demos compile for wasm32-unknown-unknown"
