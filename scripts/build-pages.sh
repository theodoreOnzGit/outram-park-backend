#!/usr/bin/env bash
# Build the backend GitHub Pages site (gh:#509, #518): ~~the tabbed main menu~~
# the landing page and its three pathways (docs/site/; since 2026-10-16,
# gh:#906: the tabbed menu moved to docs/site/code/), the search index, every deep-dive mdBook (docs/site/deep-dives.txt), rustdoc
# for the crates that have lessons (docs/site/lesson-crates.txt), without
# source pages, the tutorial mdBooks (docs/site/tutorials.txt, gh:#520) and
# the browser demos (gh:#519, #521, #530, #529, #540). The demos need the
# wasm32-unknown-unknown target and the wasm-bindgen CLI at the Cargo.lock
# version; the demo's own build script checks the latter. Run by .github/workflows/pages.yml; runnable locally:
# Phases (gh:#1305). The Pages workflow runs them on separate runners; with no
# phase named, the whole site is built end to end, as before:
#   scripts/build-pages.sh <out-dir> [static] [rustdoc] [callgraph] [demos] [assemble]
#     static     landing page, pathways, search index, lesson checks, code map,
#                ICSBEP page, citation check, mdBook deep dives and tutorials
#     rustdoc    API reference (-> api/)
#     callgraph  web-kovan's data: code map + call graph (-> code-review/data/;
#                needs rust-analyzer and the RAM for it)
#     demos      web-kovan's wasm page (-> code-review/) and every browser demo
#                (-> demos/); needs wasm-bindgen at the Cargo.lock version
#     assemble   needs the output of ALL the others already in <out-dir>: the
#                api lists of web-kovan, then the whole-site passes (@@L: code
#                links, commit stamps, navigation, existence and rung-table
#                checks, search text, link check, asset manifest)
# Dependencies between phases: assemble reads everything the others wrote;
# callgraph no longer reads rustdoc (its api/<crate>.json lists are written by
# assemble, `data.sh --api-lists`); demos' tapes are copied by the demo scripts
# themselves. A phased run does NOT empty <out-dir> first (start from an empty
# one); a run with no phase does.
#   scripts/build-pages.sh target/pages
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
OUT="${1:-target/pages}"
COMMIT="${GITHUB_SHA:-$(git rev-parse HEAD)}"
SHORT="${COMMIT:0:10}"
DATE="$(date -u +%Y-%m-%d)"

ALL_PHASES=(static rustdoc callgraph demos assemble)
FULL=0
PHASES=("${@:2}")
if (( ${#PHASES[@]} == 0 )); then FULL=1; PHASES=("${ALL_PHASES[@]}"); fi
for ph in "${PHASES[@]}"; do
  case " ${ALL_PHASES[*]} " in *" $ph "*) ;; *) echo "build-pages.sh: unknown phase '$ph' (${ALL_PHASES[*]})" >&2; exit 2 ;; esac
done
(( FULL )) && rm -rf "$OUT"
mkdir -p "$OUT/deep-dives"


# ── phase static: the pages, checks, code map, ICSBEP page, citations and mdBooks ──
phase_static() {
cp docs/site/index.html docs/site/style.css docs/site/site-nav.js docs/site/code-map-bar.js docs/site/search.js docs/site/sw.js "$OUT/"
# The three pathways of the landing page (gh:#906, #909): Learn the science,
# Learn to code (the old front page, with the code map by domain) and the demo
# library; then the search index over them and the books (search.js reads it).
mkdir -p "$OUT/code" "$OUT/science/neutronics" "$OUT/science/transport" "$OUT/science/fission-products" "$OUT/demos"
cp docs/site/code/index.html "$OUT/code/"
cp docs/site/science/index.html "$OUT/science/"
cp docs/site/science/neutronics/*.html docs/site/science/neutronics/*.js "$OUT/science/neutronics/"
cp docs/site/science/transport/*.html docs/site/science/transport/*.js "$OUT/science/transport/"
# The Fission products track (2026-10-20); check-fission.js is a node check, not
# a page, but is copied like the other tracks' checks.
cp docs/site/science/fission-products/*.html docs/site/science/fission-products/*.js "$OUT/science/fission-products/"
# The particle slices and the concept map's stylesheet (gh:#1207).
cp docs/site/science/fission-products/*.svg docs/site/science/fission-products/*.css "$OUT/science/fission-products/"
cp docs/site/demos/index.html "$OUT/demos/"
bash scripts/build-search-index.sh "$OUT"

# The lessons' physics (gh:#906): phase shifts against NJOY's facphi, the
# cross-section routes, the boundary condition. A failed check fails the
# build, before anything is deployed.
node docs/site/science/neutronics/check-physics.js
node docs/site/science/transport/check-transport.js
node docs/site/science/fission-products/check-fission.js
# gh:#1207 / #1209: the lessons folded in from the TRISO-ATOPS mdBook, the
# concept map and walks, the diffusion arc and breathing.
node docs/site/science/fission-products/check-decaywalk.js
node docs/site/science/fission-products/check-chemhand.js
node docs/site/science/fission-products/check-concepts.js
node docs/site/science/fission-products/check-diffarc.js
node docs/site/science/fission-products/check-breathing.js

# The code map (gh:#734): every crate placed by its [package.metadata.kovan]
# tag, drawn from `cargo metadata` of THIS checkout by kovan-cli (no
# rust-analyzer). The SVG and the JSON the page reads are generated here and
# never committed; the same Cargo.tomls give byte-identical files. The front
# page shows the SVG as a preview.
mkdir -p "$OUT/code-map"
cp docs/site/code-map/index.html "$OUT/code-map/"
# The Kovan Markdown schema reference (gh:#743): static; its examples are
# parsed by crates/kovan/tests/schema_743.rs, so it cannot drift from the parser.
mkdir -p "$OUT/kovan-schema"
cp docs/site/kovan-schema/index.html "$OUT/kovan-schema/"
code_map() {
  cargo run --release -q -j "${PAGES_JOBS:-3}" -p knowledge-oriented-vv-analysis-for-nuclear-sciences-kovan --no-default-features --bin kovan-cli -- \
    code-map --workspace . "$@"
}
code_map --format svg -o "$OUT/code-map/code_map.svg"
code_map --format json -o "$OUT/code-map/code_map.json"
# The five-route ICSBEP page: the committed k_eff figure, and each case's
# geometry top down, drawn here from the same case() the runs use (no nuclear
# data needed), so the pictures cannot drift from the model.
mkdir -p "$OUT/icsbep"
cp docs/site/icsbep/index.html "$OUT/icsbep/"
cp crates/outram-mc-libs/verification_and_validation/icsbep/five_route_keff/figures/five_route_keff.png "$OUT/icsbep/"
cargo run --release -q -j "${PAGES_JOBS:-3}" -p outram-mc-libs --features endf-pebble-cases \
  --example icsbep_five_route_keff -- --draw "$OUT/icsbep"
for f in godiva_xy jemima_xy hst009_xy hst009_xy_tank lct008_xy_core lct008_xy_zoom; do
  [[ -f "$OUT/icsbep/$f.png" ]] || { echo "missing icsbep/$f.png" >&2; exit 1; }
done
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

# ~~Code walks (gh:#523): every `<!-- code-walk: ... -->` block regenerated
# with rust-analyzer and its snippets checked here.~~ CHANGED 2026-10-14
# (maintainer direction): the lessons no longer carry code walks; they link
# into the Code Review map (code-review/ above), which is generated from the
# call graph on every build, so there is nothing here to go stale.

# The deep-dive and tutorial books, for the citation check below.
walk_dirs=()
while read -r name dir; do
  [[ -z "$name" || "$name" == \#* ]] && continue
  walk_dirs+=("$dir")
done < <(cat docs/site/deep-dives.txt docs/site/tutorials.txt)

# Citations (gh:#789, #797): every `[...](#ref-KEY)` link in a deep dive or a
# tutorial must name a key of docs/site/references.bib, and each page's
# reference list between `<!-- references:begin/end -->` must be what the
# citeproc-js port (kovan_literature::csl, APA 7) renders now. The landing
# page and the demos (HTML, gh:#789) are checked the same way. Check mode
# fails on a stale list or an unknown key; the fix is
# `kovan-cli references --update --bib docs/site/references.bib <book>`.
cargo run --release -q -p knowledge-oriented-vv-analysis-for-nuclear-sciences-kovan --no-default-features --bin kovan-cli -- \
  references --bib docs/site/references.bib "${walk_dirs[@]}" docs/site/code/index.html docs/site/science/neutronics/*.html docs/site/science/transport/*.html docs/site/science/fission-products/*.html \
  crates/dhoby-ghaut/web/{monte_carlo,delta_tracking,nuclear_data,triso_atops,dem}/index.html

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
# ~~The theory book's 15-dlofc.md reused the TRISO tutorial's animation by its
# plain name, so it was copied here.~~ Since 2026-11-04 (gh:#1207) the
# animation lives in the theory book's own src/anim/, which mdBook copies.

# The TRISO-ATOPS tutorial mdBook was folded into the Fission products science
# lessons (gh:#1207, maintainer direction 2026-11-04). Its URLs stay valid: each
# old page redirects to the lesson that now carries its content.
mkdir -p "$OUT/tutorials/triso-atops"
while read -r old new; do
  printf '%s\n' '<!DOCTYPE html><html lang="en"><head><meta charset="utf-8">' \
    "<meta http-equiv=\"refresh\" content=\"0; url=../../$new\"><title>Moved</title></head>" \
    "<body><p>This lesson moved to <a href=\"../../$new\">the Fission products lessons</a>.</p></body></html>" \
    > "$OUT/tutorials/triso-atops/$old"
  [[ "$new" == science/index.html* ]] || [[ -f "docs/site/${new%%#*}" ]] || { echo "TRISO-ATOPS redirect target missing: $new" >&2; exit 1; }
done <<'REDIRECTS'
index.html science/index.html#fission-products
intro.html science/index.html#fission-products
triso.html science/fission-products/barriers-why.html
decay.html science/fission-products/decay-clock.html
walk.html science/fission-products/walk-on-spheres.html
layers.html science/fission-products/barriers-migration.html
failure.html science/fission-products/failure-accident.html
chemistry.html science/fission-products/chem-air.html
coolant.html science/fission-products/atops-coolant.html
handoff.html science/fission-products/atops-source-term.html
REDIRECTS

}

# ── phase rustdoc: the API reference ──
phase_rustdoc() {
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
rm -rf "$OUT/api"   # cp -r into an existing folder would nest it as api/doc
cp -r "$DOC_TARGET/doc" "$OUT/api"
rm -rf "$OUT/api/src" "$OUT/api/.lock"
}

# ── phase callgraph: web-kovan data: code map and call graph ──
phase_callgraph() {
# web-kovan (gh:#736, #745, #772): the read-only Code Review UI at
# code-review/, egui built for wasm32. data.sh writes its data (code map; the
# call graph of every crate from one `rust-analyzer scip` index, incremental
# through target/kovan-index; a crate it cannot rebuild keeps its last cached
# graph and is named in the log and on the page; and the rustdoc pages above,
# so it only links to pages that exist). Source text is not copied: the page fetches it
# from the repository at $COMMIT. The JS code-map/ page stays for now.
# The rustdoc lists (api/<crate>.json) are written by the assemble phase, so this
# phase does not wait for rustdoc (gh:#1305).
PAGES_COMMIT="$COMMIT" bash crates/kovan-web/web/data.sh "$OUT/code-review/data"
}

# ── phase demos: web-kovan page and the browser demos ──
phase_demos() {
# web-kovan's page (gh:#736): the wasm32 build of the Code Review UI.
bash crates/kovan-web/web/build.sh "$OUT/code-review"
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
# The DEM pour demo (gh:#787): HTR-10 pebbles poured and settled in the
# browser by the LIGGGHTS port; the full-size gh:#216 bed is compiled in.
bash crates/dhoby-ghaut/web/dem/build.sh "$OUT/demos/dem"
# The helical-coil steam generator demo (gh:#1157): tampines' 1-D helical-coil
# once-through steam generator march (IF97 water, coil and boiling closures)
# in a Web Worker, one shooting march per message; no data files.
bash crates/dhoby-ghaut/web/helical_sg/build.sh "$OUT/demos/helical-sg"
for f in index.html worker.js helical_sg_web.js helical_sg_web_bg.wasm; do
  [[ -f "$OUT/demos/helical-sg/$f" ]] || { echo "missing demos/helical-sg/$f" >&2; exit 1; }
done
# The double-pipe high-fidelity -> low-fidelity twin (gh:#1182): the TUAS
# 1-D double pipe evaluated live in a Web Worker, beside the committed
# Kennedy-O'Hagan, co-kriging, CFD and filtering artefacts compiled into the
# wasm; no data files. Built from the digital-twin engine since 2026-11-05
# (moved from dhoby-ghaut to break a dependency cycle); same URL.
bash crates/outram-park-digital-twin-engine/web/double_pipe_twin/build.sh "$OUT/demos/double-pipe-twin"
for f in index.html worker.js double_pipe_twin_web.js double_pipe_twin_web_bg.wasm; do
  [[ -f "$OUT/demos/double-pipe-twin/$f" ]] || { echo "missing demos/double-pipe-twin/$f" >&2; exit 1; }
done
# Foam Studio (gh:#1155): a pipe or duct meshed (O-grid / box blockMesh) and
# solved by the pimpleFoam port in a Web Worker, one chunk of time steps per
# message; no data files.
bash crates/dhoby-ghaut/web/foam_studio/build.sh "$OUT/demos/foam-studio"
for f in index.html worker.js foam_studio_web.js foam_studio_web_bg.wasm; do
  [[ -f "$OUT/demos/foam-studio/$f" ]] || { echo "missing demos/foam-studio/$f" >&2; exit 1; }
done
# Delta vs surface tracking, step by step (gh:#784): reads the Monte Carlo
# demo's tapes from ../monte-carlo/data/, so it is built after that one.
bash crates/dhoby-ghaut/web/delta_tracking/build.sh "$OUT/demos/delta-tracking"
for f in index.html worker.js delta_tracking_web_bg.wasm; do
  [[ -f "$OUT/demos/delta-tracking/$f" ]] || { echo "missing demos/delta-tracking/$f" >&2; exit 1; }
done
# The CIET Educational Simulator v2 in the browser (gh:#924): the desktop
# simulator's own stepper as wasm, on three Web Workers (plant, DRACS flow,
# primary flow) under a plain-JS page.
bash crates/outram-park-digital-twin-engine/web/ciet/build.sh "$OUT/demos/ciet"
for f in index.html app.js plant-worker.js flow-worker.js ciet_web.js ciet_web_bg.wasm; do
  [[ -f "$OUT/demos/ciet/$f" ]] || { echo "missing demos/ciet/$f" >&2; exit 1; }
done
# The HTGR simulator in the browser (gh:#986): htgr_sim_v1's own widgets as
# wasm, the plant on a Web Worker. PASSWORD-GATED by web/gate.js, which is a
# sign and not a lock -- everything published here is readable by anyone who
# loads it, and gate.js says so. ~~Not linked from any index page, so the only
# way in is to be given the URL~~ CHANGED 2026-10-27 (maintainer: "Can I have
# the demos on the demos page"): listed under Plant simulators on demos/;
# `noindex` still keeps it out of search.
bash crates/outram-park-digital-twin-engine/web/htgr/build.sh "$OUT/demos/htgr"
for f in index.html htgr_worker.js gate.js htgr_sim_v1.js htgr_sim_v1_bg.wasm; do
  [[ -f "$OUT/demos/htgr/$f" ]] || { echo "missing demos/htgr/$f" >&2; exit 1; }
done
# The Fukushima Daiichi Unit 1 station-blackout demo (gh:#986), moved here
# from the outram-park repository on 2026-10-27: ~~dhoby-ghaut's~~ the
# digital-twin engine's (since 2026-11-05) fukushima_web example, the plant on
# three Web Workers. Gated and listed exactly like the HTGR demo above, by the
# same gate.js. Same URL as before the move.
bash crates/outram-park-digital-twin-engine/web/fukushima/build.sh "$OUT/demos/fukushima"
for f in index.html fukushima_worker.js gate.js fukushima_web.js fukushima_web_bg.wasm; do
  [[ -f "$OUT/demos/fukushima/$f" ]] || { echo "missing demos/fukushima/$f" >&2; exit 1; }
done
mkdir -p "$OUT/demos/triso-pebble/geometry"
cp crates/dhoby-ghaut/web/monte_carlo/triso-pebble-redirect.html "$OUT/demos/triso-pebble/index.html"
printf '%s\n' '<!DOCTYPE html><html lang="en"><head><meta charset="utf-8">' \
  '<meta http-equiv="refresh" content="0; url=../../monte-carlo/geometry/"><title>Moved</title></head>' \
  '<body><p>Moved to <a href="../../monte-carlo/geometry/">the Monte Carlo demo geometry page</a>.</p></body></html>' \
  > "$OUT/demos/triso-pebble/geometry/index.html"
}

# ── phase assemble: the whole-site passes over the finished site ──
phase_assemble() {
# web-kovan's data (callgraph phase) and page (demos phase) are in place; now the
# rustdoc page lists it links to, and the navbar's list of deep dives (2026-10-06).
bash crates/kovan-web/web/data.sh --api-lists "$OUT/code-review/data" "$OUT/api"
cp "$OUT/code-map/site_links.json" "$OUT/code-review/data/site_links.json"
for f in index.html kovan_web_bg.wasm data/build.json data/code_map.json; do
  [[ -f "$OUT/code-review/$f" ]] || { echo "missing $OUT/code-review/$f" >&2; exit 1; }
done

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
  search.js search.json code/index.html science/index.html demos/index.html \
  science/neutronics/{meet-the-neutron,why-it-scatters,moving-nuclei,matter-waves,quantum-scattering,storing-shapes,scattering-angles}.html \
  science/neutronics/{which-nucleus,dbrc,ace-files,two-files}.html \
  science/neutronics/{lesson-kit,kinematics,waves,qscatter,shapes,angles,angular-data}.js \
  science/neutronics/{dbrc,dbrc-data}.js \
  science/transport/{free-flight,moving,surfaces,random-numbers,at-a-collision,a-history,triso-problem,delta-tracking,fission-bank,why-statistics,whole-picture}.html \
  science/transport/{transport,collision-data}.js \
  science/fission-products/{one-fission,split-atoms,prompt-spectrum}.html \
  science/fission-products/{fission-data,fission-pending,fission-physics,one-fission,fp-energy,yields-data,split-physics,split-atoms,spectrum-data,prompt-spectrum}.js \
  science/fission-products/{fragment-flight,lattice-cascade,decay-again,decay-or-move,atom-hop,atoms-swap}.html \
  science/fission-products/{cascade-data,cascade-recorded,hop-physics,hop-worker,fragment-story}.js \
  science/fission-products/kernel-{polycrystal,chemistry,grain-growth,pores,in-out,lumping,booth-grain,many-spheres,two-scales,rb}.html \
  science/fission-products/kernel-{recoil,knockout,sweeping,precursors,bursts,burnup-oxygen,conductivity,hbs,swelling}.html \
  science/fission-products/{kernel-data,kernel-recorded,kernel-physics,kernel-worker,kernel-story}.js \
  science/fission-products/barriers-{why,migration,sic,whole}.html \
  science/fission-products/failure-{vessel,weibull,thinning,accident,release,leaves-out}.html \
  science/fission-products/{barrier-data,failure-data,barrier-recorded,barrier-physics,barrier-worker,barrier-story,plot-kit}.js \
  science/fission-products/atops-{big-picture,groups,diffusion,release-models,normal-operation,coolant,accident,simplifications}.html \
  science/fission-products/{atops-data,atops-physics,atops-story}.js \
  science/fission-products/{decay-clock,walk-on-spheres,chem-air,chem-steam,atops-source-term}.html \
  science/fission-products/{decaywalk-data,decaywalk-story,chemhand-data,chemhand-story}.js \
  science/fission-products/{diffusion-three-ways,atops-conservative,atops-breathing}.html \
  science/fission-products/{diffarc-data,diffarc-story,breathing-data,breathing-story}.js \
  science/fission-products/{atops-concepts,atops-walk-cs137,atops-walk-accident,atops-walk-silver}.html \
  science/fission-products/{atops-concepts-data,atops-concepts,atops-walk-numbers,atops-walk-recorded}.js \
  science/fission-products/atops-concepts.css \
  science/fission-products/{triso-htr10-slice,triso-crp6-slice}.svg \
  kovan-schema/index.html \
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
  demos/htgr/index.html demos/htgr/htgr_sim_v1_bg.wasm \
  demos/fukushima/index.html demos/fukushima/fukushima_web_bg.wasm \
  api/sembawang/index.html \
  deep-dives/nuclear-data/index.html api/njoy_outram_park_fork/index.html \
  demos/nuclear-data/index.html demos/nuclear-data/nuclear_data_web_bg.wasm \
  tutorials/monte-carlo/lct008.html tutorials/monte-carlo/triso.html \
  demos/triso-atops/index.html demos/triso-atops/triso_atops_web_bg.wasm \
  tutorials/triso-atops/index.html \
  demos/dem/index.html demos/dem/dem_web_bg.wasm tutorials/monte-carlo/dem.html \
  tutorials/monte-carlo/benchmarks.html \
  tutorials/monte-carlo/tutorial3-hst009.html tutorials/monte-carlo/tutorial4-triso-particle.html \
  tutorials/monte-carlo/tutorial6-lct008.html tutorials/monte-carlo/tutorial7-seker-layer.html \
  tutorials/monte-carlo/tutorial8-seker-bed.html \
  tutorials/fp-transport-theory/index.html tutorials/fp-transport-theory/20-htgr-sim-v1-chain.html; do
  [[ -f "$OUT/$f" ]] || { echo "missing $OUT/$f" >&2; exit 1; }
done

# The demo's rung table drives the links both ways (gh:#520): every rung's
# lesson page must exist, and every page link into the demo must name a rung the
# table has. The table is `table.rs`, written out by hand since 2026-11-05
# (gh:#1253; it was the `rung_table!` macro in main.rs, and this script kept
# reading the macro, which found no rungs and would have failed the build): each
# rung's `Rung::X => &<crate::MODULE::X as McRung>::INFO,` arm names its module.
# Each rung's `name:` and `lesson:` lines are in its `<module>/mod.rs`, read as written.
mc=crates/dhoby-ghaut/examples/monte_carlo_web
rung_mods="$(grep -oE '&<crate::[a-z0-9_]+::[A-Za-z0-9]+ as McRung>::INFO' "$mc/table.rs" | sed -E 's/^&<crate::([a-z0-9_]+)::.*/\1/' | awk '!seen[$0]++')"
[[ -n "$rung_mods" ]] || { echo "no rungs read from $mc/table.rs" >&2; exit 1; }
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
done < <(grep -rhoE '(demos/|")monte-carlo/\?rung=[a-z0-9_-]+' "$OUT" --include='*.html' | sed 's/.*rung=//' | sort -u)
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
done < <(grep -rhoE '(demos/|")dispersion/\?rung=[a-z0-9_-]+' "$OUT" --include='*.html' | sed 's/.*rung=//' | sort -u)

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
done < <(grep -rhoE '(demos/|")nuclear-data/\?rung=[a-z0-9_-]+' "$OUT" --include='*.html' | sed 's/.*rung=//' | sort -u)
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
done < <(grep -rhoE '(demos/|")triso-atops/\?rung=[a-z0-9_-]+' "$OUT" --include='*.html' | sed 's/.*rung=//' | sort -u)

# Full-text search (gh:#876 item 8): every built page into one inverted index,
# search-text.json, read by search.js. Run here, after every book and demo
# page exists. The tokeniser check fails the build if search.js would split a
# query differently from the index.
node docs/site/check-search.js
cargo run --release -q -j "${PAGES_JOBS:-3}" -p knowledge-oriented-vv-analysis-for-nuclear-sciences-kovan --no-default-features --bin kovan-cli -- \
  site-index "$OUT"
[[ -s "$OUT/search-text.json" ]] || { echo "the full-text search index was not written" >&2; exit 1; }

# Every relative link from the main menu, the three pathway pages, the deep
# dives and the tutorials must land on a page in the site (a lesson link into
# the API 404s silently otherwise).
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
done < <(find "$OUT/index.html" "$OUT/code" "$OUT/science" "$OUT/demos/index.html" "$OUT/deep-dives" "$OUT/tutorials" -name '*.html' -print0)
(( bad == 0 )) || exit 1

# The content-hashed file cache (gh:#923): every non-HTML file by its SHA-256,
# read by sw.js. Written last, over the finished site. check-sw.js runs the
# service worker's cache logic under node first.
node docs/site/check-sw.js
node docs/site/check-sw.js --manifest "$OUT"
[[ -s "$OUT/asset-manifest.json" ]] || { echo "asset-manifest.json was not written" >&2; exit 1; }

echo "site built in $OUT ($(du -sh "$OUT" | cut -f1)), commit $SHORT"

}

for ph in "${PHASES[@]}"; do
 echo "build-pages.sh: phase $ph"; "phase_$ph"
done
