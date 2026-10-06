#!/usr/bin/env bash
#
# Generate web-kovan's data folder (GitHub #736, #745):
#
#   build.json          {"commit": "<sha>", "repo": "<owner/name>"}: the commit
#                       the source panel fetches files at
#   code_map.json       kovan-cli code-map --format json
#   graph/index.json    the call graph, one file per crate (kovan-cli
#   graph/search.json   call-graph --split-dir); needs rust-analyzer
#   graph/<crate>.json
#   api/<crate>.json    the rustdoc pages the site has for each crate, so the
#                       page links only to pages that exist
#
#   crates/kovan-web/web/data.sh <out-data-dir> [<api-dir>]
#   crates/kovan-web/web/data.sh --check         # the determinism check
#
# INCREMENTAL (maintainer, 2026-10-06, #745): every crate is indexed, but a
# crate is re-run only when its key changes (`kovan-cli call-graph-keys`: its
# packaged files, its workspace dependencies' keys, the rust-analyzer and
# schema versions). Each crate's own call-graph document is cached as
# $KOVAN_INDEX_CACHE/<crate>/<key>.json (default target/kovan-index, never
# committed; CI keeps it with actions/cache) and the documents are merged
# (`call-graph --merge`), which gives the bytes one run over every crate
# gives: `--check` runs that comparison on two small crates.
#
# Without rust-analyzer the call graph is skipped with a warning and the page
# shows the code map only. A crate whose run fails is skipped with a warning.
#
# KOVAN_INDEX_BUDGET=<seconds> (CI sets it; default 0 = no limit): once that
# much time has gone on indexing, crates not yet in the cache are skipped for
# this run (the page lacks their call graph) instead of letting a cold cache
# run the job into its timeout. CI saves the cache even when the build fails,
# so the next run starts where this one stopped and a cold cache warms up
# over a few runs.
#
# KOVAN_WEB_CRATES=a,b limits the call graph to those crates (local runs).
# Never commits anything. Release build (workspace rule).
set -euo pipefail
root="$(git rev-parse --show-toplevel)"
cd "$root"
JOBS="${PAGES_JOBS:-3}"
CACHE="${KOVAN_INDEX_CACHE:-$root/target/kovan-index}"
BUDGET="${KOVAN_INDEX_BUDGET:-0}"
cargo build --release -q -j "$JOBS" -p kovan --no-default-features --bin kovan-cli
kc="$root/target/release/kovan-cli"
trap '"$kc" lsp-daemon-stop --root "$root" >/dev/null 2>&1 || true' EXIT

if [[ "${1:-}" == "--check" ]]; then
  # Determinism: one run over two crates vs. two single-crate runs merged.
  tmp="$(mktemp -d)"
  "$kc" call-graph --workspace . --crates kovan-common,kovan-codegen -o "$tmp/full.json" 2>/dev/null
  "$kc" call-graph --workspace . --crates kovan-common -o "$tmp/a.json" 2>/dev/null
  "$kc" call-graph --workspace . --crates kovan-codegen -o "$tmp/b.json" 2>/dev/null
  "$kc" call-graph --workspace . --merge "$tmp/b.json" "$tmp/a.json" -o "$tmp/merged.json"
  if cmp -s "$tmp/full.json" "$tmp/merged.json"; then
    echo "data.sh --check: merged single-crate runs are byte-identical to one run ($(wc -c < "$tmp/full.json") bytes)"
    rm -rf "$tmp"
  else
    echo "data.sh --check: MISMATCH, see $tmp" >&2
    exit 1
  fi
  exit 0
fi

out="$(realpath -m "${1:?usage: data.sh <out-data-dir> [<api-dir>]}")"
api="${2:-}"
commit="${PAGES_COMMIT:-${GITHUB_SHA:-$(git rev-parse HEAD)}}"
repo="${GITHUB_REPOSITORY:-theodoreOnzGit/outram-park-backend}"
rm -rf "$out"
mkdir -p "$out/graph" "$out/api"
printf '{"commit":"%s","repo":"%s"}\n' "$commit" "$repo" > "$out/build.json"
"$kc" code-map --workspace . --format json -o "$out/code_map.json"

if command -v rust-analyzer >/dev/null 2>&1; then
  keys_args=()
  [[ -n "${KOVAN_WEB_CRATES:-}" ]] && keys_args=(--crates "$KOVAN_WEB_CRATES")
  docs=()
  fresh=0
  deferred=0
  t0=$SECONDS
  while read -r c key; do
    dir="$CACHE/$c"
    f="$dir/$key.json"
    if [[ ! -f "$f" ]]; then
      if (( BUDGET > 0 && SECONDS - t0 >= BUDGET )); then
        deferred=$((deferred + 1))
        continue
      fi
      mkdir -p "$dir"
      rm -f "$dir"/*.json
      if "$kc" call-graph --workspace . --crates "$c" -o "$f.tmp" 2> "$dir/log.txt"; then
        mv "$f.tmp" "$f"
        fresh=$((fresh + 1))
      else
        echo "data.sh: call graph of $c failed, skipped (see $dir/log.txt)" >&2
        rm -f "$f.tmp"
        continue
      fi
    fi
    docs+=("$f")
  done < <("$kc" call-graph-keys --workspace . "${keys_args[@]}")
  echo "data.sh: call graph of ${#docs[@]} crates, $fresh re-indexed, $((SECONDS - t0)) s"
  if (( deferred > 0 )); then
    echo "data.sh: WARNING: index budget of $BUDGET s spent; $deferred crates not indexed this run (the next run continues from the cache)" >&2
  fi
  if (( ${#docs[@]} > 0 )); then
    "$kc" call-graph --workspace . --merge "${docs[@]}" --split-dir "$out/graph"
  fi
else
  echo "data.sh: rust-analyzer not found: no call graph (the page shows the code map only)" >&2
fi

# The rustdoc pages that exist, per crate (relative to the api/ folder).
# rustdoc names a crate's folder with '_' for '-'; files are written under
# the package name, read from each member's own Cargo.toml.
if [[ -n "$api" && -d "$api" ]]; then
  for m in crates/*/Cargo.toml; do
    pkg="$(awk '/^\[package\]/{p=1;next} /^\[/{p=0} p && /^name[[:space:]]*=/{
          gsub(/^name[[:space:]]*=[[:space:]]*"/,""); gsub(/".*$/,""); print; exit}' "$m")"
    name="${pkg//-/_}"
    [[ -n "$pkg" && -f "$api/$name/index.html" ]] || continue
    (cd "$api" && find "$name" -name '*.html' | LC_ALL=C sort) \
      | sed 's/.*/"&"/' | paste -sd, - | sed 's/^/[/; s/$/]/' > "$out/api/$pkg.json"
  done
fi
echo "data.sh: wrote $out ($(du -sh "$out" | cut -f1))"
