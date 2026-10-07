#!/usr/bin/env bash
#
# Generate web-kovan's data folder (GitHub #736, #745, #772):
#
#   build.json          {"commit": "<sha>", "repo": "<owner/name>",
#                       "call_graph": {...}}: the commit the source panel
#                       fetches files at, and how complete the call graph is
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
# BACKEND (#772, maintainer 2026-10-07): KOVAN_CALL_GRAPH_BACKEND=scip (the
# default) or lsp. With scip, `rust-analyzer scip` indexes the whole
# workspace ONCE (about 4 min, about 16 GiB peak memory, measured
# 2026-10-07; see crates/kovan/docs/call-graph-scip-vs-lsp.md) and every
# crate's call graph is read from that index (`call-graph --scip`, 1 to 3 s
# a crate). The lsp backend (one definition query per call token, about
# 33 min cold for the workspace) is kept for comparison.
#
# INCREMENTAL (maintainer, 2026-10-06, #745): a crate is re-run only when its
# key changes (`kovan-cli call-graph-keys`: its packaged files, its workspace
# dependencies' keys, the rust-analyzer and schema versions). Each crate's own
# call-graph document is cached as $KOVAN_INDEX_CACHE/<backend>/<crate>/<key>.json
# (default target/kovan-index, never committed; CI keeps it with
# actions/cache) and the documents are merged (`call-graph --merge`), which
# gives the bytes one run over every crate gives: `--check` runs that
# comparison on two small crates. When every key is cached, rust-analyzer is
# not run at all. The SCIP index itself is kept at target/kovan-scip/index.scip
# with the hash of every crate's key beside it (index.key), and reused while
# that hash is unchanged (local reruns; CI starts without one).
#
# LEAK BEFORE BREAK (docs/kovan.md): the call graph is never silently
# partial. When rust-analyzer is missing, `rust-analyzer scip` fails (for
# example out of memory) or one crate's run fails, that crate falls back to
# its LAST CACHED graph (built from older source, so its line numbers may be
# off) or, with none, is left out. Either way the crate is named, loudly, in
# the build log (and as a GitHub Actions warning), and in build.json
# (`call_graph.stale` / `call_graph.missing`), which web-kovan shows. The
# build itself does not fail on this (#772: never fails the build).
#
# KOVAN_INDEX_BUDGET=<seconds> (lsp backend only; default 0 = no limit): once
# that much time has gone on indexing, crates not yet in the cache fall back
# as above instead of letting a cold cache run the job into its timeout.
#
# KOVAN_WEB_CRATES=a,b limits the call graph to those crates (local runs).
# KOVAN_SCIP_THREADS=<n> is passed to `rust-analyzer scip --num-threads`.
# Never commits anything. Release build (workspace rule); rust-analyzer's own
# build-script `cargo check` is given `--release` too, so no target/debug.
set -euo pipefail
root="$(git rev-parse --show-toplevel)"
cd "$root"
JOBS="${PAGES_JOBS:-3}"
CACHE="${KOVAN_INDEX_CACHE:-$root/target/kovan-index}"
BUDGET="${KOVAN_INDEX_BUDGET:-0}"
BACKEND="${KOVAN_CALL_GRAPH_BACKEND:-scip}"
case "$BACKEND" in
  scip|lsp) ;;
  *) echo "data.sh: KOVAN_CALL_GRAPH_BACKEND must be scip or lsp, not '$BACKEND'" >&2; exit 2 ;;
esac
SCIP_DIR="$root/target/kovan-scip"
SCIP="$SCIP_DIR/index.scip"
cargo build --release -q -j "$JOBS" -p knowledge-oriented-vv-analysis-for-nuclear-sciences-kovan --no-default-features --bin kovan-cli
kc="$root/target/release/kovan-cli"
trap '"$kc" lsp-daemon-stop --root "$root" >/dev/null 2>&1 || true' EXIT

# A loud line: stderr, and a GitHub Actions annotation when on CI.
loud() {
  echo "data.sh: $*" >&2
  if [[ -n "${GITHUB_ACTIONS:-}" ]]; then echo "::warning title=web-kovan call graph::$*"; fi
}

# `command -v` is not enough: rustup installs a `rust-analyzer` proxy even
# when the component is absent (scripts/build-pages.sh, 2026-10-04).
ra_version() { rust-analyzer --version 2>/dev/null || true; }

# Runs `rust-analyzer scip` over the workspace into $SCIP, unless the index
# there was written for the same keys ($1, the hash of every crate's key).
# Logs the time and rust-analyzer's peak resident memory (VmHWM, sampled
# every 2 s, so a lower bound). Returns non-zero when the run fails.
ensure_scip() {
  local want="$1"
  if [[ -f "$SCIP" && -f "$SCIP_DIR/index.key" && "$(cat "$SCIP_DIR/index.key")" == "$want" ]]; then
    echo "data.sh: reusing $SCIP (written for these keys)"
    return 0
  fi
  mkdir -p "$SCIP_DIR"
  rm -f "$SCIP" "$SCIP_DIR/index.key"
  # Keep rust-analyzer's build-script `cargo check` in the release profile.
  printf '{"cargo":{"extraArgs":["--release"]}}\n' > "$SCIP_DIR/ra-config.json"
  local threads=()
  [[ -n "${KOVAN_SCIP_THREADS:-}" ]] && threads=(--num-threads "$KOVAN_SCIP_THREADS")
  echo "data.sh: rust-analyzer scip over the workspace ($(ra_version)); log in $SCIP_DIR/rust-analyzer-scip.log"
  if command -v free >/dev/null 2>&1; then free -m | sed 's/^/data.sh: memory: /'; fi
  local t0=$SECONDS pid peak=0 h p rc=0
  # oom_score_adj 1000: if memory runs out, the kernel kills rust-analyzer
  # (which falls back loudly, below), not the CI runner's own agent.
  (
    echo 1000 > /proc/self/oom_score_adj 2>/dev/null || true
    exec rust-analyzer scip "$root" --output "$SCIP" --config-path "$SCIP_DIR/ra-config.json" "${threads[@]}"
  ) > "$SCIP_DIR/rust-analyzer-scip.log" 2>&1 &
  pid=$!
  while kill -0 "$pid" 2>/dev/null; do
    # The rustup proxy may run the real binary as a child: take the largest.
    for p in "$pid" $(pgrep -P "$pid" 2>/dev/null || true); do
      h="$(awk '/^VmHWM:/{print $2}' "/proc/$p/status" 2>/dev/null || true)"
      if [[ -n "$h" ]] && (( h > peak )); then peak=$h; fi
    done
    sleep 2
  done
  wait "$pid" || rc=$?
  echo "data.sh: rust-analyzer scip: exit $rc after $((SECONDS - t0)) s, peak resident memory >= $((peak / 1024)) MiB"
  if (( rc != 0 )) || [[ ! -s "$SCIP" ]]; then
    tail -n 20 "$SCIP_DIR/rust-analyzer-scip.log" | sed 's/^/data.sh: rust-analyzer: /' >&2
    rm -f "$SCIP"
    return 1
  fi
  printf '%s\n' "$want" > "$SCIP_DIR/index.key"
}

# One call-graph run of crate(s) $1 into $2 with the chosen backend.
call_graph() {
  if [[ "$BACKEND" == scip ]]; then
    "$kc" call-graph --workspace . --scip "$SCIP" --crates "$1" -o "$2"
  else
    "$kc" call-graph --workspace . --backend lsp --crates "$1" -o "$2"
  fi
}

# The hash of every crate's key: what the whole-workspace index depends on.
keys_hash() { "$kc" call-graph-keys --workspace . | sha256sum | cut -d' ' -f1; }

if [[ "${1:-}" == "--check" ]]; then
  # Determinism: one run over two crates vs. two single-crate runs merged.
  tmp="$(mktemp -d)"
  if [[ "$BACKEND" == scip ]]; then ensure_scip "$(keys_hash)"; fi
  call_graph kovan-common,kovan-codegen "$tmp/full.json" 2>/dev/null
  call_graph kovan-common "$tmp/a.json" 2>/dev/null
  call_graph kovan-codegen "$tmp/b.json" 2>/dev/null
  "$kc" call-graph --workspace . --merge "$tmp/b.json" "$tmp/a.json" -o "$tmp/merged.json"
  if cmp -s "$tmp/full.json" "$tmp/merged.json"; then
    echo "data.sh --check ($BACKEND): merged single-crate runs are byte-identical to one run ($(wc -c < "$tmp/full.json") bytes)"
    rm -rf "$tmp"
  else
    echo "data.sh --check ($BACKEND): MISMATCH, see $tmp" >&2
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
"$kc" code-map --workspace . --format json -o "$out/code_map.json"

# The call graph. Cache layout before #772 was $CACHE/<crate>/; those
# documents (schema 2 keys) can never match again, so they are dropped.
mkdir -p "$CACHE/$BACKEND"
for d in "$CACHE"/*/; do
  case "$(basename "$d")" in lsp|scip) ;; *) rm -rf "$d" ;; esac
done
ra="$(ra_version)"
[[ -n "$ra" ]] || ra="rust-analyzer missing"
echo "data.sh: call graph backend $BACKEND, $ra"
all_keys="$("$kc" call-graph-keys --workspace .)"
want_hash="$(printf '%s\n' "$all_keys" | sha256sum | cut -d' ' -f1)"
wanted=""
[[ -n "${KOVAN_WEB_CRATES:-}" ]] && wanted=",${KOVAN_WEB_CRATES// /},"
docs=()
stale=()
missing=()
fresh=0
indexer_ok=1
indexer_tried=0
t0=$SECONDS
if [[ "$ra" == "rust-analyzer missing" ]]; then
  indexer_ok=0
  loud "rust-analyzer not found: no crate can be re-indexed (rustup component add rust-analyzer)"
fi
while read -r c key; do
  [[ -z "$c" ]] && continue
  [[ -n "$wanted" && "$wanted" != *",$c,"* ]] && continue
  dir="$CACHE/$BACKEND/$c"
  f="$dir/$key.json"
  if [[ ! -f "$f" ]]; then
    mkdir -p "$dir"
    ok=0
    if (( indexer_ok )); then
      if [[ "$BACKEND" == scip ]] && (( ! indexer_tried )); then
        indexer_tried=1
        if ! ensure_scip "$want_hash"; then
          indexer_ok=0
          loud "rust-analyzer scip FAILED: every crate whose source changed keeps its last cached call graph"
        fi
      fi
      if [[ "$BACKEND" == lsp ]] && (( BUDGET > 0 && SECONDS - t0 >= BUDGET )); then
        (( indexer_ok )) && loud "index budget of $BUDGET s spent: the remaining changed crates keep their last cached call graph"
        indexer_ok=0
      fi
    fi
    if (( indexer_ok )); then
      if call_graph "$c" "$f.tmp" 2> "$dir/log.txt"; then
        # Only now is the previous document replaced.
        find "$dir" -name '*.json' ! -name "$key.json.tmp" -delete
        mv "$f.tmp" "$f"
        fresh=$((fresh + 1))
        ok=1
      else
        rm -f "$f.tmp"
        loud "call graph of $c FAILED (see $dir/log.txt)"
      fi
    fi
    if (( ! ok )); then
      old="$(find "$dir" -name '*.json' -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -n1 | cut -d' ' -f2-)"
      if [[ -n "$old" ]]; then
        stale+=("$c")
        docs+=("$old")
      else
        missing+=("$c")
      fi
      continue
    fi
  fi
  docs+=("$f")
done <<< "$all_keys"
echo "data.sh: call graph of ${#docs[@]} crates, $fresh re-indexed, $((SECONDS - t0)) s"
if (( ${#docs[@]} > 0 )); then
  "$kc" call-graph --workspace . --merge "${docs[@]}" --split-dir "$out/graph"
fi
if (( ${#stale[@]} + ${#missing[@]} > 0 )); then
  loud "call graph INCOMPLETE: ${#stale[@]} crate(s) STALE (an older commit's graph): ${stale[*]:-none}; ${#missing[@]} crate(s) MISSING: ${missing[*]:-none}"
else
  echo "data.sh: call graph complete: every crate built from this commit"
fi
json_list() { local s="" x; for x in "$@"; do s+="${s:+,}\"$x\""; done; printf '[%s]' "$s"; }
printf '{"commit":"%s","repo":"%s","call_graph":{"backend":"%s","rust_analyzer":"%s","crates":%d,"reindexed":%d,"stale":%s,"missing":%s}}\n' \
  "$commit" "$repo" "$BACKEND" "$ra" "${#docs[@]}" "$fresh" \
  "$(json_list "${stale[@]}")" "$(json_list "${missing[@]}")" > "$out/build.json"

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
