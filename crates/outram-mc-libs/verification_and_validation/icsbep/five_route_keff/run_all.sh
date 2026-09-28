#!/bin/sh
# Five-route ICSBEP k_eff campaign: 4 cases x 5 routes x $SEEDS seeds, run
# SEQUENTIALLY at $THREADS threads (never two jobs at once), resumable.
#
#   crates/outram-mc-libs/verification_and_validation/icsbep/five_route_keff/run_all.sh
#
# | route | code      | nuclear data                                   |
# |-------|-----------|------------------------------------------------|
# | 1     | OpenMC    | NJOY2016 ACE -> HDF5 (openmc_inputs/ace_to_hdf5_route.py) |
# | 2     | OpenMC    | Rust-NJOY ACE -> HDF5 (same converter)          |
# | 3     | outram-mc | NJOY2016 ACE                                   |
# | 4     | outram-mc | Rust NJOY, ENDF read directly (the default path)|
# | 5     | outram-mc | Rust-NJOY ACE                                  |
#
# Cases: godiva (HEU-MET-FAST-001), jemima (IEU-MET-FAST-002), hst009
# (HEU-SOL-THERM-009 case 1), lct008 (LEU-COMP-THERM-008 case 1, the real
# lattice with lct008_keff.rs's 11-nuclide tier; its own defaults are
# 10000 x [250 + 400], pass PARTICLES/INACTIVE/ACTIVE for it), and lct008s (a
# documented extra: the homogenised 30/70 sphere, not comparable to k = 1).
#
# Per-case history counts: PARTICLES/INACTIVE/ACTIVE apply to every case in the
# invocation, so run lct008 in its own invocation:
#   THREADS=4 CASES=lct008 ROUTES="1 2 4" PARTICLES=10000 INACTIVE=250 ACTIVE=400 ./run_all.sh
#
# A case x route that FAILS (e.g. routes 3/5 on hst009: outram-mc's ACE reader
# refuses F-19's MT=16 law-61 chain, see the V&V record) is logged and skipped;
# the campaign continues and the figure marks it "not run".
#
# RESUMABLE: every finished seed is one row of $CSV. On restart, each
# case x route asks only for the seeds not already in it. An outram-mc
# invocation loads its nuclides once and runs all its missing seeds, appending
# a row after each, so an interruption costs at most the seed in flight.
#
# Environment (all optional):
#   WORK       scratch for libraries and per-run logs   [<repo>/target/five_route_keff]
#   CSV        per-seed results                          [<this dir>/data/per_seed_keff.csv]
#   SEEDS      seeds per case x route (1..SEEDS)         [32]
#   THREADS    threads for every job                     [8]
#   PARTICLES INACTIVE ACTIVE                             [5000 40 120]
#   ROUTES     subset, e.g. "1 2 3"                      [1 2 3 4 5]
#   CASES      subset                                    [godiva jemima hst009 lct008s]
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(git -C "$HERE" rev-parse --show-toplevel)
WORK=${WORK:-$REPO/target/five_route_keff}
CSV=${CSV:-$HERE/data/per_seed_keff.csv}
SEEDS=${SEEDS:-32}
THREADS=${THREADS:-8}
PARTICLES=${PARTICLES:-5000}
INACTIVE=${INACTIVE:-40}
ACTIVE=${ACTIVE:-120}
ROUTES=${ROUTES:-1 2 3 4 5}
CASES=${CASES:-godiva jemima hst009 lct008 lct008s}
PY=${PY:-$HOME/Documents/research/.venv-openmc/bin/python}
OPENMC=${OPENMC:-$HOME/Documents/research/openmcbin/bin/openmc}
export RAYON_NUM_THREADS=$THREADS OMP_NUM_THREADS=$THREADS
COMMIT=$(git -C "$REPO" rev-parse --short=10 HEAD)
mkdir -p "$WORK/logs" "$(dirname "$CSV")"
log() { echo "[$(date '+%F %T')] $*" | tee -a "$WORK/logs/run_all.log"; }

# ── 0. binaries (release, always) ──────────────────────────────────────────
log "building release binaries at $COMMIT"
(cd "$REPO" && cargo build --release -p njoy-outram-park-fork --example write_ace_library \
   && cargo build --release -p outram-mc-libs --features endf-pebble-cases \
        --example icsbep_five_route_keff) >> "$WORK/logs/build.log" 2>&1
# Snapshot the binaries: a rebuild in the working tree while the campaign runs
# must not change what later seeds are run with.
mkdir -p "$WORK/bin/$COMMIT"
cp "$REPO/target/release/examples/icsbep_five_route_keff" "$REPO/target/release/examples/write_ace_library" "$WORK/bin/$COMMIT/"
DRIVER=$WORK/bin/$COMMIT/icsbep_five_route_keff
WRITER=$WORK/bin/$COMMIT/write_ace_library

# ── 1. libraries (each step skips what already exists) ─────────────────────
log "NJOY2016 library"
WORK=$WORK sh "$HERE/scripts/make_njoy_library.sh" >> "$WORK/logs/njoy_library.log" 2>&1
log "Rust-NJOY library"
NUCS=$(grep -v '^#' "$HERE/nuclides.tsv" | awk -F'\t' '{printf " --nuclide %s:%s:%s", $1, $2, $3}')
# shellcheck disable=SC2086
"$WRITER" --out "$WORK/rust" $NUCS --thermal HinH2O:tsl-HinH2O.endf:1:lwtr \
  --thermal-grid-from "$WORK/njoy/293.6K/HinH2O.ace" >> "$WORK/logs/rust_library.log" 2>&1
for lib in njoy rust; do
  if [ ! -s "$WORK/h5_$lib/cross_sections.xml" ]; then
    log "ACE -> HDF5 ($lib)"
    "$PY" "$HERE/openmc_inputs/ace_to_hdf5_route.py" "$WORK/$lib" "$WORK/h5_$lib" \
      >> "$WORK/logs/h5_$lib.log" 2>&1
  fi
done

# Seeds of case $1 / label $2 not yet in $CSV, comma-separated.
missing() {
  s=1; out=""
  while [ $s -le "$SEEDS" ]; do
    if ! grep -q "^$1,$2,[a-z-]*,$s," "$CSV" 2>/dev/null; then out="$out$s,"; fi
    s=$((s + 1))
  done
  echo "${out%,}"
}

# ── 2. the campaign ────────────────────────────────────────────────────────
# OpenMC routes first (minutes in total), then outram-mc, cheapest route first.
for route in $ROUTES; do
  for case in $CASES; do
    label=route$route
    todo=$(missing "$case" "$label")
    [ -n "$todo" ] || { log "$case $label: all $SEEDS seeds present"; continue; }
    log "$case $label: seeds $todo"
    case $route in
      1|2)
        [ "$route" = 1 ] && lib=njoy || lib=rust
        for s in $(echo "$todo" | tr ',' ' '); do
          "$PY" "$HERE/openmc_inputs/icsbep_openmc.py" --case "$case" \
            --xs "$WORK/h5_$lib/cross_sections.xml" --seed "$s" --label "$label" \
            --csv "$CSV" --workdir "$WORK/runs/${case}_${label}_s$s" --threads "$THREADS" \
            --particles "$PARTICLES" --inactive "$INACTIVE" --active "$ACTIVE" \
            --commit "$COMMIT" --openmc "$OPENMC" >> "$WORK/logs/${case}_${label}.log" 2>&1 \
            || log "$case $label seed $s: FAILED (see logs/${case}_${label}.log)"
          rm -f "$WORK/runs/${case}_${label}_s$s"/statepoint.*.h5
        done ;;
      3|4|5)
        case $route in
          3) extra="--route ace --ace-dir $WORK/njoy" ;;
          4) extra="--route endf" ;;
          5) extra="--route ace --ace-dir $WORK/rust" ;;
        esac
        # shellcheck disable=SC2086
        "$DRIVER" --case "$case" $extra --label "$label" --seeds "$todo" --csv "$CSV" \
          --threads "$THREADS" --particles "$PARTICLES" --inactive "$INACTIVE" \
          --active "$ACTIVE" --commit "$COMMIT" >> "$WORK/logs/${case}_${label}.log" 2>&1 \
          || log "$case $label: FAILED (see logs/${case}_${label}.log) -- continuing" ;;
    esac
    log "$case $label: finished"
  done
done
log "campaign complete: $(($(wc -l < "$CSV") - 1)) rows in $CSV"
