#!/usr/bin/env bash
# Build the backend GitHub Pages site (gh:#509, #518): the tabbed main menu
# (docs/site/), every deep-dive mdBook (docs/site/deep-dives.txt), rustdoc
# for the crates that have lessons (docs/site/lesson-crates.txt), without
# source pages, and the browser demos (gh:#519). The demos need the
# wasm32-unknown-unknown target and the wasm-bindgen CLI at the Cargo.lock
# version; the demo's own build script checks the latter. Run by .github/workflows/pages.yml; runnable locally:
#   scripts/build-pages.sh target/pages
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
OUT="${1:-target/pages}"
COMMIT="${GITHUB_SHA:-$(git rev-parse HEAD)}"
SHORT="${COMMIT:0:10}"
DATE="$(date -u +%Y-%m-%d)"

rm -rf "$OUT"
mkdir -p "$OUT/deep-dives"
cp docs/site/index.html docs/site/style.css "$OUT/"

# Rustdoc, release profile (root CLAUDE.md), no dependencies' docs. Source
# pages are not published: lessons show anchored snippets and link to GitHub.
pkgs=()
while read -r c; do
  [[ -z "$c" || "$c" == \#* ]] && continue
  pkgs+=(-p "$c")
done < docs/site/lesson-crates.txt
# Own target dir, so only these crates' pages (and search index) are published,
# never whatever else a local `target/doc` happens to hold.
DOC_TARGET=target/site-doc
# Memory cap: the default `-j 12` OOMs the 16 GB dev box. Jobs default to 3,
# and where a systemd user session exists (locally, not on CI runners) cargo
# runs in a scope that is killed past PAGES_MEM_MAX of RAM, swap disallowed,
# so the build fails instead of the machine. Override either via env.
JOBS="${PAGES_JOBS:-3}"
MEM_MAX="${PAGES_MEM_MAX:-12G}"
cap=()
if [[ -z "${CI:-}" ]] && systemd-run --user --scope -q true 2>/dev/null; then
  cap=(systemd-run --user --scope -q -p "MemoryMax=$MEM_MAX" -p MemorySwapMax=0)
  echo "cargo doc capped at $MEM_MAX RAM, -j $JOBS"
fi
"${cap[@]}" cargo doc --release --no-deps --lib -j "$JOBS" --target-dir "$DOC_TARGET" "${pkgs[@]}"
cp -r "$DOC_TARGET/doc" "$OUT/api"
rm -rf "$OUT/api/src" "$OUT/api/.lock"

# Deep-dive books.
while read -r name dir; do
  [[ -z "$name" || "$name" == \#* ]] && continue
  mdbook build "$dir" -d "$PWD/$OUT/deep-dives/$name"
done < docs/site/deep-dives.txt

# Browser demos (gh:#519): wasm build, its JS glue, the ENDF tapes it
# processes in the browser, and its geometry review images.
crates/dhoby-ghaut/web/triso_pebble/build.sh "$OUT/demos/triso-pebble"
# Pin every GitHub link and the footer to the build commit.
grep -rlZ -e '@@COMMIT@@' -e '@@COMMIT_SHORT@@' -e '@@BUILD_DATE@@' "$OUT" \
  | xargs -0 -r sed -i -e "s/@@COMMIT@@/$COMMIT/g" -e "s/@@COMMIT_SHORT@@/$SHORT/g" -e "s/@@BUILD_DATE@@/$DATE/g"

# Every page must exist where the main menu points.
for f in index.html api/outram_mc_libs/index.html api/changi/index.html \
  api/buangkok/index.html api/boon_lay/index.html \
  deep-dives/{monte-carlo,dispersion,triso-atops}/index.html \
  demos/triso-pebble/index.html demos/triso-pebble/triso_pebble_web_bg.wasm \
  demos/triso-pebble/geometry/index.html; do
  [[ -f "$OUT/$f" ]] || { echo "missing $OUT/$f" >&2; exit 1; }
done

# Every relative link from the main menu and the deep dives must land on a
# page in the site (a lesson link into the API 404s silently otherwise).
# Anchors are not checked.
bad=0
while IFS= read -r -d '' page; do
  dir="$(dirname "$page")"
  while IFS= read -r href; do
    target="${href%%#*}"
    [[ -z "$target" ]] && continue
    p="$dir/$target"
    [[ -d "$p" ]] && p="$p/index.html"
    [[ -e "$p" ]] || { echo "broken link in ${page#"$OUT"/}: $href" >&2; bad=1; }
  done < <(grep -oE 'href="[^"]+"' "$page" | sed -E 's/^href="(.*)"$/\1/' \
             | grep -vE '^([a-z]+:|/|#)')
done < <(find "$OUT/index.html" "$OUT/deep-dives" -name '*.html' -print0)
(( bad == 0 )) || exit 1

echo "site built in $OUT ($(du -sh "$OUT" | cut -f1)), commit $SHORT"
