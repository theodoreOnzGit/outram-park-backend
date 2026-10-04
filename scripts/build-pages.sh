#!/usr/bin/env bash
# Build the backend GitHub Pages site (gh:#509, #518): the tabbed main menu
# (docs/site/), every deep-dive mdBook (docs/site/deep-dives.txt), rustdoc
# for the crates that have lessons (docs/site/lesson-crates.txt), without
# source pages, the tutorial mdBooks (docs/site/tutorials.txt, gh:#520) and
# the browser demos (gh:#519, #521). The demos need the
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

# Tutorial books (gh:#520): one per track, one page per rung.
while read -r name dir; do
  [[ -z "$name" || "$name" == \#* ]] && continue
  mdbook build "$dir" -d "$PWD/$OUT/tutorials/$name"
done < docs/site/tutorials.txt

# Browser demos (gh:#519, #521): ONE Monte Carlo demo with a rung table
# (crates/dhoby-ghaut/examples/monte_carlo_web/rungs.rs): wasm build, its JS
# glue, the ENDF tapes it processes in the browser, and its geometry review
# images. The TRISO pebble demo's old URL redirects to its rung.
crates/dhoby-ghaut/web/monte_carlo/build.sh "$OUT/demos/monte-carlo"
mkdir -p "$OUT/demos/triso-pebble/geometry"
cp crates/dhoby-ghaut/web/monte_carlo/triso-pebble-redirect.html "$OUT/demos/triso-pebble/index.html"
printf '%s\n' '<!DOCTYPE html><html lang="en"><head><meta charset="utf-8">' \
  '<meta http-equiv="refresh" content="0; url=../../monte-carlo/geometry/"><title>Moved</title></head>' \
  '<body><p>Moved to <a href="../../monte-carlo/geometry/">the Monte Carlo demo geometry page</a>.</p></body></html>' \
  > "$OUT/demos/triso-pebble/geometry/index.html"
# Code links that cannot go stale (gh:#521): a page writes
#   .../blob/@@COMMIT@@/<file>#@@L:<file>:<selector>@@
# and the build replaces the token with the line range at THIS commit, so a
# link follows its function when the file changes. Selectors, joined by ';'
# ('+' stands for a space):
#   fn=NAME | struct=NAME | enum=NAME | const=NAME  the item (to its closing brace)
#   anchor=NAME   the lines between mdBook's `ANCHOR: NAME` / `ANCHOR_END: NAME`
#   text=TEXT     the first line containing TEXT
#   after=TEXT    start searching at the first line containing TEXT
# An unresolved token fails the build.
lines_for() {
  local file="$1" sel="$2" from=1 kind name s e p
  [[ -f "$file" ]] || { echo "ERR"; return; }
  IFS=';' read -ra parts <<< "$sel"
  for p in "${parts[@]}"; do
    kind="${p%%=*}"; name="${p#*=}"; name="${name//+/ }"
    case "$kind" in
      after)
        from="$(awk -v f="$from" -v t="$name" 'NR >= f && index($0, t) { print NR; exit }' "$file")"
        [[ -n "$from" ]] || { echo "ERR"; return; } ;;
      text)
        s="$(awk -v f="$from" -v t="$name" 'NR >= f && index($0, t) { print NR; exit }' "$file")"
        [[ -n "$s" ]] && { echo "L$s"; return; } ;;
      anchor)
        s="$(grep -nE "ANCHOR: ${name}([^A-Za-z0-9_-]|\$)" "$file" | head -1 | cut -d: -f1)"
        e="$(grep -nE "ANCHOR_END: ${name}([^A-Za-z0-9_-]|\$)" "$file" | head -1 | cut -d: -f1)"
        [[ -n "$s" && -n "$e" ]] && { echo "L$((s + 1))-L$((e - 1))"; return; } ;;
      fn|struct|enum|const)
        s="$(awk -v f="$from" -v re="(^|[^A-Za-z0-9_])$kind $name([^A-Za-z0-9_]|\$)" 'NR >= f && $0 ~ re { print NR; exit }' "$file")"
        [[ -n "$s" ]] || { echo "ERR"; return; }
        # The item ends where its braces close (or at its `;`).
        e="$(awk -v s="$s" 'NR >= s {
               for (i = 1; i <= length($0); i++) { c = substr($0, i, 1)
                 if (c == "{") { d++; o = 1 } else if (c == "}") { d--; if (o && d == 0) { print NR; exit } } }
               if (!o && $0 ~ /;[[:space:]]*$/) { print NR; exit } }' "$file")"
        echo "L$s-L${e:-$s}"; return ;;
      *) echo "ERR"; return ;;
    esac
  done
  echo "ERR"
}
bad=0
while IFS= read -r tok; do
  spec="${tok#@@L:}"; spec="${spec%@@}"
  r="$(lines_for "${spec%%:*}" "${spec#*:}")"
  if [[ "$r" == ERR ]]; then echo "unresolved code link $tok" >&2; bad=1; continue; fi
  esc="$(printf '%s' "$tok" | sed -e 's/[]\/$*.^[|]/\\&/g')"
  grep -rlZF "$tok" "$OUT" | xargs -0 -r sed -i "s|$esc|$r|g"
done < <(grep -rhoE '@@L:[^@]+@@' "$OUT" --include='*.html' | sort -u)
(( bad == 0 )) || exit 1

# Pin every GitHub link and the footer to the build commit.
grep -rlZ -e '@@COMMIT@@' -e '@@COMMIT_SHORT@@' -e '@@BUILD_DATE@@' "$OUT" \
  | xargs -0 -r sed -i -e "s/@@COMMIT@@/$COMMIT/g" -e "s/@@COMMIT_SHORT@@/$SHORT/g" -e "s/@@BUILD_DATE@@/$DATE/g"

# Every page must exist where the main menu points.
for f in index.html api/outram_mc_libs/index.html api/changi/index.html \
  api/buangkok/index.html api/boon_lay/index.html \
  deep-dives/{monte-carlo,dispersion,triso-atops}/index.html \
  demos/monte-carlo/index.html demos/monte-carlo/monte_carlo_web_bg.wasm \
  demos/monte-carlo/geometry/index.html \
  demos/triso-pebble/index.html demos/triso-pebble/geometry/index.html \
  tutorials/monte-carlo/index.html tutorials/monte-carlo/godiva.html; do
  [[ -f "$OUT/$f" ]] || { echo "missing $OUT/$f" >&2; exit 1; }
done

# The demo's rung table drives the links both ways (gh:#520): every rung's
# lesson page must exist, and every page link into the demo must name a rung
# the table has. The table's `name:` and `lesson:` lines are read as written.
rungs_rs=crates/dhoby-ghaut/examples/monte_carlo_web/rungs.rs
rung_names="$(sed -n 's/^ *name: "\([a-z0-9_-]*\)",$/\1/p' "$rungs_rs")"
[[ -n "$rung_names" ]] || { echo "no rungs read from $rungs_rs" >&2; exit 1; }
while read -r lesson; do
  [[ -f "$OUT/$lesson" ]] || { echo "rung lesson page missing: $lesson ($rungs_rs)" >&2; exit 1; }
done < <(sed -n 's/^ *lesson: "\([^"]*\)",$/\1/p' "$rungs_rs")
while read -r r; do
  grep -qx "$r" <<< "$rung_names" || { echo "a page links to demo rung '$r', not in $rungs_rs" >&2; exit 1; }
done < <(grep -rhoE 'demos/monte-carlo/\?rung=[a-z0-9_-]+' "$OUT" --include='*.html' | sed 's/.*rung=//' | sort -u)
# Every tape a rung processes must have been published for the browser.
for t in n-092_U_234-ENDF8.0.endf n-092_U_235-ENDF8.0.endf n-092_U_238.endf; do
  [[ -f "$OUT/demos/monte-carlo/data/$t.zz" ]] || { echo "missing demo tape $t" >&2; exit 1; }
done

# Every relative link from the main menu, the deep dives and the tutorials must land on a
# page in the site (a lesson link into the API 404s silently otherwise).
# Anchors are not checked.
bad=0
while IFS= read -r -d '' page; do
  dir="$(dirname "$page")"
  while IFS= read -r href; do
    target="${href%%#*}"
    target="${target%%\?*}"
    [[ -z "$target" ]] && continue
    p="$dir/$target"
    [[ -d "$p" ]] && p="$p/index.html"
    [[ -e "$p" ]] || { echo "broken link in ${page#"$OUT"/}: $href" >&2; bad=1; }
  done < <(grep -oE 'href="[^"]+"' "$page" | sed -E 's/^href="(.*)"$/\1/' \
             | grep -vE '^([a-z]+:|/|#)')
done < <(find "$OUT/index.html" "$OUT/deep-dives" "$OUT/tutorials" -name '*.html' -print0)
(( bad == 0 )) || exit 1

echo "site built in $OUT ($(du -sh "$OUT" | cut -f1)), commit $SHORT"
