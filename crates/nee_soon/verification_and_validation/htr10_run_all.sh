#!/bin/bash
# Reusable launcher for the HTR-10 k-vs-height sweep against Li, Yu & Wei (2014)
# RMC (example `htr10_rmc_keff`). Runs every (library, N) case in a pool of
# pinned slots and writes logs/run_<lib>_N<n>.log into OUT_DIR. A case whose log
# already holds a k_eff line is skipped, so a relaunch resumes.
#
# Usage:
#   cargo build --release -p nee_soon --example htr10_rmc_keff
#   crates/nee_soon/verification_and_validation/htr10_run_all.sh <OUT_DIR>
#
# Knobs (environment, defaults = the reference paper's statistics):
#   HISTORIES=10000 INACTIVE=5 ACTIVE=135   particles per cycle and cycles
#   LAYERS="10 14 17 20 12 16 19 11 13 15 18"  Seker layers N, run in this order
#   LIBS="e8 e7"        e8 = ENDF/B-VIII.0, e7 = ENDF/B-VII.0 (OUTRAM_HTR10_ENDF7=1)
#   SLOTS=5 TPS=7       concurrent runs x threads each; slot s uses CPUs s*TPS..
#   EXTRA_ENV="K=V ..." extra variables for every run (e.g. an ablation knob)
#   BIN=<path>          example binary (default target/release/examples/htr10_rmc_keff)
set -u
OUT=${1:?usage: htr10_run_all.sh <OUT_DIR>}
ROOT=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
BIN=${BIN:-$ROOT/target/release/examples/htr10_rmc_keff}
HISTORIES=${HISTORIES:-10000}; INACTIVE=${INACTIVE:-5}; ACTIVE=${ACTIVE:-135}
LAYERS=${LAYERS:-"10 14 17 20 12 16 19 11 13 15 18"}; LIBS=${LIBS:-"e8 e7"}
SLOTS=${SLOTS:-5}; TPS=${TPS:-7}; EXTRA_ENV=${EXTRA_ENV:-}
[ -x "$BIN" ] || { echo "no binary at $BIN; build it first" >&2; exit 1; }
mkdir -p "$OUT/logs"; cd "$OUT"   # logs are written relative to OUT

run_one() {  # slot lib n
  local slot=$1 lib=$2 n=$3
  local log=logs/run_${lib}_N${n}.log
  local lo=$((slot * TPS)) hi=$((slot * TPS + TPS - 1))
  local e7=""; [ "$lib" = e7 ] && e7=1
  # Run from the repository root: the binary writes a per-run performance
  # report to ./verification_and_validation/local_perf/, which is gitignored
  # only at the root.
  (cd "$ROOT" && exec env OUTRAM_HTR10_HISTORIES=$HISTORIES OUTRAM_HTR10_INACTIVE=$INACTIVE \
      OUTRAM_HTR10_ACTIVE=$ACTIVE OUTRAM_HTR10_RINGS=14 OUTRAM_HTR10_LAYERS=$n \
      OUTRAM_HTR10_THREADS=$TPS ${e7:+OUTRAM_HTR10_ENDF7=1} $EXTRA_ENV \
      taskset -c $lo-$hi "$BIN") > "$log" 2>&1 &
  local pid=$! peak=0 r
  while kill -0 $pid 2>/dev/null; do
    r=$(awk '/VmHWM/{print $2}' /proc/$pid/status 2>/dev/null)
    [ -n "$r" ] && [ "$r" -gt "$peak" ] && peak=$r
    sleep 20
  done
  wait $pid; local rc=$?
  echo "  peak RSS     = $peak kB (VmHWM, sampled every 20 s)" >> "$log"
  echo "  cpus         = $lo-$hi (taskset), exit code $rc" >> "$log"
  echo "DONE $lib N=$n slot=$slot rc=$rc $(date -u +%FT%TZ)"
}

# Queue: interleave libraries so both curves fill in evenly.
queue=()
for n in $LAYERS; do for lib in $LIBS; do queue+=("$lib:$n"); done; done

declare -A busy
for ((s = 0; s < SLOTS; s++)); do busy[$s]=""; done
i=0
while [ $i -lt ${#queue[@]} ]; do
  for ((s = 0; s < SLOTS; s++)); do
    p=${busy[$s]}
    if [ -n "$p" ] && kill -0 "$p" 2>/dev/null; then continue; fi
    while [ $i -lt ${#queue[@]} ]; do
      spec=${queue[$i]}; i=$((i + 1))
      lib=${spec%%:*}; n=${spec##*:}
      log=logs/run_${lib}_N${n}.log
      if [ -f "$log" ] && grep -q "k_eff        =" "$log"; then continue; fi
      echo "START $lib N=$n slot=$s $(date -u +%FT%TZ)"
      run_one $s $lib $n &
      busy[$s]=$!
      break
    done
  done
  sleep 30
done
wait
echo "ALL DONE $(date -u +%FT%TZ)"
