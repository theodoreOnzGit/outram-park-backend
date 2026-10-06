#!/usr/bin/env bash
# Build the backend GitHub Pages site (gh:#509, #518): the tabbed main menu
# (docs/site/), every deep-dive mdBook (docs/site/deep-dives.txt), rustdoc
# for the crates that have lessons (docs/site/lesson-crates.txt), without
# source pages, the tutorial mdBooks (docs/site/tutorials.txt, gh:#520) and
# the browser demos (gh:#519, #521, #530, #529, #540). The demos need the
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
cp docs/site/index.html docs/site/style.css docs/site/site-nav.js docs/site/code-map-bar.js "$OUT/"

# The code map (gh:#734): every crate placed by its [package.metadata.kovan]
# tag, drawn from `cargo metadata` of THIS checkout by kovan-cli (no
# rust-analyzer). The SVG and the JSON the page reads are generated here and
# never committed; the same Cargo.tomls give byte-identical files. The front
# page shows the SVG as a preview.
mkdir -p "$OUT/code-map"
cp docs/site/code-map/index.html "$OUT/code-map/"
code_map() {
  cargo run --release -q -j "${PAGES_JOBS:-3}" -p kovan --no-default-features --bin kovan-cli -- \
    code-map --workspace . "$@"
}
code_map --format svg -o "$OUT/code-map/code_map.svg"
code_map --format json -o "$OUT/code-map/code_map.json"
# The code map is also a navigation strip on every lesson and API page
# (docs/site/code-map-bar.js, maintainer 2026-10-06). It sends a tap on a
# crate to the crate's deep dive or rustdoc, so it needs to know which books
# the site has and where their sources live; that comes from the same lists
# this script builds from, never from a second hand-kept list.
books_json() {
  local sep="" name dir
  while read -r name dir; do
    [[ -z "$name" || "$name" == \#* ]] && continue
    printf '%s{"url":"%s/%s/","dir":"%s"}' "$sep" "$2" "$name" "$dir"
    sep=","
  done < "$1"
}
{
  printf '{"deep_dives":[%s],' "$(books_json docs/site/deep-dives.txt deep-dives)"
  printf '"tutorials":[%s],"api":[' "$(books_json docs/site/tutorials.txt tutorials)"
  sep=""
  while read -r c; do
    [[ -z "$c" || "$c" == \#* ]] && continue
    printf '%s"%s"' "$sep" "$c"; sep=","
  done < docs/site/lesson-crates.txt
  printf ']}\n'
} > "$OUT/code-map/site_links.json"

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

# Code walks (gh:#523, the lesson CI of #512): every `<!-- code-walk: ... -->`
# block in a deep dive or a tutorial is regenerated with rust-analyzer and must match what is
# committed; a stale walk, a broken chain or a hand-filled hop whose function
# is gone fails the build. Skipped (with a warning) when no lesson has a block
# yet or rust-analyzer is absent, unless CODE_WALK_REQUIRED=1.
walk_dirs=()
while read -r name dir; do
  [[ -z "$name" || "$name" == \#* ]] && continue
  walk_dirs+=("$dir")
done < <(cat docs/site/deep-dives.txt docs/site/tutorials.txt)
if grep -rqs -- '<!-- code-walk:' "${walk_dirs[@]}"; then
  # `command -v` is not enough: rustup installs a `rust-analyzer` proxy even
  # when the component is absent, and that stub fails as soon as it is run
  # (it took down every Pages build on 2026-10-04). Ask it for its version.
  if rust-analyzer --version >/dev/null 2>&1; then
    cargo run --release -q -p kovan --no-default-features --bin kovan-cli -- \
      code-walk-check "${walk_dirs[@]}"
  elif [[ -n "${CODE_WALK_REQUIRED:-}" ]]; then
    echo "code walks need rust-analyzer (rustup component add rust-analyzer)" >&2
    exit 1
  else
    echo "warning: rust-analyzer not installed (gh:#544); code walks NOT checked" >&2
  fi
fi
# Code walks show each hop's code inline (2026-10-05) through mdBook
# `{{#include file:start:end}}`, with ranges fixed when the walk was
# generated. Each snippet carries `<!-- snippet-check: <file>:<line> <text> -->`
# comments; check them here, without rust-analyzer, so a range that drifted
# since the walk was regenerated fails the build instead of showing the wrong
# lines. The fix is `kovan-cli code-walk-check --update <book>`.
bad=0
while read -r loc needle; do
  file="${loc%:*}"; line="${loc##*:}"
  if ! sed -n "${line}p" "$file" 2>/dev/null | grep -qF -- "$needle"; then
    echo "stale code-walk snippet: $file:$line no longer holds '$needle'" >&2
    bad=1
  fi
done < <(grep -rhoE -- '<!-- snippet-check: [^ ]+:[0-9]+ [^>]*-->' "${walk_dirs[@]}" \
           | sed -E 's/^<!-- snippet-check: //; s/ -->$//' | sort -u)
(( bad == 0 )) || { echo "regenerate with: kovan-cli code-walk-check --update <book>" >&2; exit 1; }

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
# Demo build scripts run through bash, so a clone with core.fileMode=false
# cannot drop the executable bit and break the site (it did, twice, on
# 2026-10-04/05).
bash crates/dhoby-ghaut/web/monte_carlo/build.sh "$OUT/demos/monte-carlo"
# The dispersion demo (gh:#530): one app, a rung of the dispersion lessons at
# a time; computed in the browser from buangkok and changi, no data files.
bash crates/dhoby-ghaut/web/dispersion/build.sh "$OUT/demos/dispersion"
# The nuclear data demo (gh:#529): one app, a rung of the nuclear data track
# at a time; ENDF/B-VIII.0 tapes processed in the browser by njoy-outram-park-fork.
bash crates/dhoby-ghaut/web/nuclear_data/build.sh "$OUT/demos/nuclear-data"
# The TRISO-ATOPS and fuel failure demo (gh:#540): one app, a rung of the
# boon-lay lessons at a time; computed in the browser from boon-lay, no data files.
bash crates/dhoby-ghaut/web/triso_atops/build.sh "$OUT/demos/triso-atops"
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

# Site navigation (maintainer, 2026-10-05): every page except the main menu
# gets a "Home" breadcrumb and an "Up" link to the page one level up, from one
# script (docs/site/site-nav.js) added before </head> with the relative path
# back to the site root. Books, demos and rustdoc need no per-page edits.
while IFS= read -r -d '' d; do
  rel="${d#"$OUT"/}"
  up=""
  IFS=/ read -ra segs <<< "$rel"
  for _ in "${segs[@]}"; do up+="../"; done
  find "$d" -maxdepth 1 -name '*.html' -print0 \
    | xargs -0 -r sed -i "0,/<\/head>/s|</head>|<script src=\"${up}site-nav.js\" defer></script></head>|"
done < <(find "$OUT" -mindepth 1 -type d -print0)

# Every page must exist where the main menu points.
for f in index.html site-nav.js code-map-bar.js code-map/index.html code-map/code_map.svg \
  code-map/code_map.json code-map/site_links.json \
  api/outram_mc_libs/index.html api/changi/index.html \
  api/buangkok/index.html api/boon_lay/index.html \
  deep-dives/{monte-carlo,dispersion,triso-atops}/index.html \
  demos/monte-carlo/index.html demos/monte-carlo/monte_carlo_web_bg.wasm \
  demos/monte-carlo/geometry/index.html \
  demos/triso-pebble/index.html demos/triso-pebble/geometry/index.html \
  tutorials/monte-carlo/index.html tutorials/monte-carlo/godiva.html \
  tutorials/monte-carlo/ugraphite.html tutorials/monte-carlo/lumped.html \
  demos/dispersion/index.html demos/dispersion/dispersion_web_bg.wasm \
  api/sembawang/index.html \
  deep-dives/nuclear-data/index.html api/njoy_outram_park_fork/index.html \
  demos/nuclear-data/index.html demos/nuclear-data/nuclear_data_web_bg.wasm \
  tutorials/monte-carlo/lct008.html tutorials/monte-carlo/triso.html \
  demos/triso-atops/index.html demos/triso-atops/triso_atops_web_bg.wasm \
  tutorials/triso-atops/index.html; do
  [[ -f "$OUT/$f" ]] || { echo "missing $OUT/$f" >&2; exit 1; }
done

# The demo's rung table drives the links both ways (gh:#520): every rung's
# lesson page must exist, and every page link into the demo must name a rung
# the table has. The table is the `rung_table!` invocation in main.rs (one
# `module: Marker,` line per rung); each rung's `name:` and `lesson:` lines are
# in its `<module>/mod.rs`, read as written.
mc=crates/dhoby-ghaut/examples/monte_carlo_web
rung_mods="$(sed -n '/^rung_table! {/,/^}/s/^ *\([a-z0-9_]*\): *[A-Z][A-Za-z0-9]*,$/\1/p' "$mc/main.rs")"
[[ -n "$rung_mods" ]] || { echo "no rungs read from $mc/main.rs" >&2; exit 1; }
rung_names=""
for m in $rung_mods; do
  n="$(sed -n 's/^ *name: "\([a-z0-9_-]*\)",$/\1/p' "$mc/$m/mod.rs")"
  l="$(sed -n 's/^ *lesson: "\([^"]*\)",$/\1/p' "$mc/$m/mod.rs")"
  [[ -n "$n" && -n "$l" ]] || { echo "rung $m: no name/lesson line in $mc/$m/mod.rs" >&2; exit 1; }
  [[ -f "$OUT/$l" ]] || { echo "rung lesson page missing: $l ($mc/$m/mod.rs)" >&2; exit 1; }
  rung_names+="$n"$'\n'
done
while read -r r; do
  grep -qx "$r" <<< "$rung_names" || { echo "a page links to demo rung '$r', not in the rung table" >&2; exit 1; }
done < <(grep -rhoE 'demos/monte-carlo/\?rung=[a-z0-9_-]+' "$OUT" --include='*.html' | sed 's/.*rung=//' | sort -u)
# Every tape a rung processes must have been published for the browser.
for t in n-092_U_234-ENDF8.0.endf n-092_U_235-ENDF8.0.endf n-092_U_238.endf \
  n-006_C_012-ENDF8.0.endf n-006_C_013-ENDF8.0.endf tsl-crystalline-graphite.endf \
  n-008_O_016-ENDF8.0.endf n-005_B_010-ENDF8.0.endf n-005_B_011-ENDF8.0.endf \
  tsl-reactor-graphite-30P.endf; do
  [[ -f "$OUT/demos/monte-carlo/data/$t.zz" ]] || { echo "missing demo tape $t" >&2; exit 1; }
done

# The dispersion demo's rung table (gh:#530) drives its links both ways too:
# every rung's lesson page must exist, and every page link into the demo must
# name a rung in the table (examples/dispersion_web/rungs.rs, one `name:` and
# one `lesson:` line per rung).
dt=crates/dhoby-ghaut/examples/dispersion_web/rungs.rs
d_names="$(sed -n 's/^ *name: "\([a-z0-9_-]*\)",$/\1/p' "$dt")"
d_lessons="$(sed -n 's/^ *lesson: "\([^"]*\)",$/\1/p' "$dt")"
[[ -n "$d_names" && -n "$d_lessons" ]] || { echo "no rungs read from $dt" >&2; exit 1; }
for l in $d_lessons; do
  [[ -f "$OUT/$l" ]] || { echo "dispersion rung lesson page missing: $l ($dt)" >&2; exit 1; }
done
while read -r r; do
  grep -qx "$r" <<< "$d_names" || { echo "a page links to dispersion demo rung '$r', not in $dt" >&2; exit 1; }
done < <(grep -rhoE 'demos/dispersion/\?rung=[a-z0-9_-]+' "$OUT" --include='*.html' | sed 's/.*rung=//' | sort -u)

# The nuclear data demo's rung table (gh:#529), the same two checks
# (examples/nuclear_data_web/rungs.rs, one `name:` and one `lesson:` line per
# rung), and its tapes must have been published.
nt=crates/dhoby-ghaut/examples/nuclear_data_web/rungs.rs
n_names="$(sed -n 's/^ *name: "\([a-z0-9_-]*\)",$/\1/p' "$nt")"
n_lessons="$(sed -n 's/^ *lesson: "\([^"]*\)",$/\1/p' "$nt")"
[[ -n "$n_names" && -n "$n_lessons" ]] || { echo "no rungs read from $nt" >&2; exit 1; }
for l in $n_lessons; do
  [[ -f "$OUT/$l" ]] || { echo "nuclear data rung lesson page missing: $l ($nt)" >&2; exit 1; }
done
while read -r r; do
  grep -qx "$r" <<< "$n_names" || { echo "a page links to nuclear data demo rung '$r', not in $nt" >&2; exit 1; }
done < <(grep -rhoE 'demos/nuclear-data/\?rung=[a-z0-9_-]+' "$OUT" --include='*.html' | sed 's/.*rung=//' | sort -u)
for t in n-092_U_238.endf tsl-crystalline-graphite.endf tsl-HinH2O.endf; do
  [[ -f "$OUT/demos/nuclear-data/data/$t.zz" ]] || { echo "missing nuclear data demo tape $t" >&2; exit 1; }
done

# The TRISO-ATOPS demo's rung table (gh:#540), the same two checks
# (examples/triso_atops_web/rungs.rs, one `name:` and one `lesson:` line per rung).
tt=crates/dhoby-ghaut/examples/triso_atops_web/rungs.rs
t_names="$(sed -n 's/^ *name: "\([a-z0-9_-]*\)",$/\1/p' "$tt")"
t_lessons="$(sed -n 's/^ *lesson: "\([^"]*\)",$/\1/p' "$tt")"
[[ -n "$t_names" && -n "$t_lessons" ]] || { echo "no rungs read from $tt" >&2; exit 1; }
for l in $t_lessons; do
  [[ -f "$OUT/$l" ]] || { echo "TRISO-ATOPS rung lesson page missing: $l ($tt)" >&2; exit 1; }
done
while read -r r; do
  grep -qx "$r" <<< "$t_names" || { echo "a page links to TRISO-ATOPS demo rung '$r', not in $tt" >&2; exit 1; }
done < <(grep -rhoE 'demos/triso-atops/\?rung=[a-z0-9_-]+' "$OUT" --include='*.html' | sed 's/.*rung=//' | sort -u)

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
