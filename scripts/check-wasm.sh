#!/usr/bin/env bash
#
# check-wasm.sh — the wasm32-unknown-unknown gate for the OUTRAM PARK workspace.
#
# Bead op-okqo.6. Shell, not Python, per the workspace "No Python for
# documentation or accounting" rule and to keep the gate dependency-free.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHAT THIS CHECKS, AND WHAT IT DOES NOT
#
# It checks that every in-scope crate COMPILES for wasm32-unknown-unknown.
# It does NOT check that anything RUNS there, and the difference is large:
#
#   std::thread::spawn  compiles, then PANICS at run time
#   std::time::Instant  compiles, then PANICS at run time
#   std::fs             compiles, then returns errors at run time
#
# chem-eng-real-time-process-control-simulator is the standing proof: it passes
# this gate today while containing 5 thread::spawn sites and 10 files using
# std::fs. Passing here means "the types line up", not "this works in a
# browser". Making a crate actually run on wasm is per-crate work tracked
# separately — see epic op-eeqw (GH #39), whose Phases 2 and 3 are exactly that
# job for the htgr_sim_v1 dependency chain.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHY --lib AND NOT --all-targets
#
# The Android rule learned the hard way that a --lib-only gate silently misses
# broken examples, tests and benches (the godiva_gpu_benchmark incident), and
# its acceptance command is --all-targets for that reason.
#
# wasm cannot follow it. Several crates carry egui/eframe GUI examples and
# terminal binaries that legitimately cannot build for wasm, so --all-targets
# would be permanently red and would therefore be ignored. This gate is --lib,
# and that limitation is stated here rather than papered over: a broken
# wasm-facing example will NOT be caught by this script.
#
# Usage:
#   scripts/check-wasm.sh            # gate every in-scope crate
#   scripts/check-wasm.sh -v         # show the first error line for failures
#
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

VERBOSE=0
[ "${1:-}" = "-v" ] && VERBOSE=1

# ─────────────────────────────────────────────────────────────────────────────
# EXCLUSIONS — deliberate, with reasons. Not a to-do list.
#
# Excluded by maintainer decision on 2026-09-04. Each is EXCLUDED, not
# "failing": nothing here is expected to be fixed later unless the decision
# changes.
#
#   kovan, kovan-discovery, kovan-metrics, kovan-semantics
#       A local-first developer CLI/TUI that walks the filesystem and shells out
#       to language servers. kovan alone has 36 files using std::fs. "Compiles
#       for wasm" would be close to meaningless. Bead op-okqo.4 (cancelled).
#
#       NOTE the other three kovan crates — kovan-codegen, kovan-common,
#       kovan-literature — are deliberately NOT excluded. They already pass, so
#       gating them costs nothing and protects the pure-data half of the kovan
#       layer from regressing.
#
#   bedok, outram-blender
#       Not wanted as wasm targets. Both fail only on faer's default feature set
#       (faer's `rayon` feature pulls spindle -> atomic-wait, which has no wasm
#       backend); the fix is known and small but not worth making. Bead
#       op-okqo.2 (cancelled).
EXCLUDED=(
  kovan
  kovan-discovery
  kovan-metrics
  kovan-semantics
  bedok
  outram-blender
)

is_excluded() {
  local c="$1"
  for e in "${EXCLUDED[@]}"; do [ "$c" = "$e" ] && return 0; done
  return 1
}

# Derive the member list from the tree itself, so a new crate is gated the day
# it is added rather than the day someone remembers to edit this script.
#
# Read from each member's own [package] name rather than from `cargo metadata`:
# metadata's JSON carries a "name" field for every TARGET (each example, bin and
# test) as well as for each package, and separating those without a JSON parser
# is exactly the kind of fragile grep that produced a 197-entry "failure" list on
# this script's first run. Every workspace member lives under crates/.
MEMBERS=$(for m in crates/*/Cargo.toml; do
  [ -f "$m" ] || continue
  awk '/^\[package\]/{p=1;next} /^\[/{p=0} p && /^name[[:space:]]*=/{
        gsub(/^name[[:space:]]*=[[:space:]]*"/,""); gsub(/".*$/,""); print; exit}' "$m"
done | sort -u)

if [ -z "$MEMBERS" ]; then
  echo "check-wasm: could not read workspace members from cargo metadata" >&2
  exit 2
fi

if ! rustup target list --installed 2>/dev/null | grep -q '^wasm32-unknown-unknown$'; then
  echo "check-wasm: the wasm32-unknown-unknown target is not installed." >&2
  echo "            run: rustup target add wasm32-unknown-unknown" >&2
  exit 2
fi

pass=0; fail=0; skipped=0
failed_names=()

for c in $MEMBERS; do
  if is_excluded "$c"; then
    printf '  SKIP  %-40s (excluded — see the list in this script)\n' "$c"
    skipped=$((skipped + 1))
    continue
  fi
  err=$(cargo check -q -p "$c" --lib --target wasm32-unknown-unknown 2>&1)
  if echo "$err" | grep -qE '^error'; then
    printf '  FAIL  %-40s\n' "$c"
    [ "$VERBOSE" = "1" ] && echo "$err" | grep -E '^error' | head -3 | sed 's/^/          /'
    fail=$((fail + 1))
    failed_names+=("$c")
  else
    printf '  ok    %-40s\n' "$c"
    pass=$((pass + 1))
  fi
done

echo
echo "wasm32-unknown-unknown gate: ${pass} ok, ${fail} failed, ${skipped} excluded"

if [ "$fail" -gt 0 ]; then
  echo
  echo "Failed: ${failed_names[*]}"
  echo "Re-run with -v for the first error lines, or:"
  echo "  cargo check -p <crate> --lib --target wasm32-unknown-unknown"
  echo
  echo "Common causes and the fix this workspace uses (bead op-okqo):"
  echo "  * rayon        — target-gate OFF WASM ONLY, never feature-gate and"
  echo "                   never gate off Android; rayon is Android-in-scope."
  echo "                   Use the crate's src/wasm_par.rs serial shim."
  echo "  * getrandom    — needs BOTH a feature and the cfg flag in"
  echo "                   .cargo/config.toml. Prefer cutting the dependency."
  echo "  * 32-bit usize — wasm32 usize is 32 bits. Widen the type; do not"
  echo "                   truncate the constant."
  exit 1
fi
exit 0
