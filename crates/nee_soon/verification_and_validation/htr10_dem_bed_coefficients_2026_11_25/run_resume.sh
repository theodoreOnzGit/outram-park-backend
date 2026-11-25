#!/bin/bash
# Resumable launch of this record's cases (gh:#1739), written after the
# container restarted at ~06:24 UTC on 2026-11-25 and killed run.sh during
# the poured bed's FUEL-GRAPHITE case. One case per process, each with its
# own log; a case whose log already holds its result line is skipped, so a
# restart loses at most the case in flight. Same driver, statistics and seed
# as run.sh (every case is deterministic at a fixed seed, so a case run in
# its own process is the same calculation). BASE on the poured bed is the
# one run.sh finished (logs/poured.log).
#   cargo build --release -p nee_soon --example htr10_graphite_zone_worth
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../../../.." && pwd)"
cd "$root"
mkdir -p "$here/logs"
bin=target/release/examples/htr10_graphite_zone_worth
common="--base 600 --delta 600 --particles 3000 --inactive 20 --active 80 --threads 4"
dem="--dem-bed reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv"
run() { # bed tag, case, extra args
  local log="$here/logs/case_$1_$2.log"
  if grep -q "^$2 " "$log" 2>/dev/null; then return 0; fi
  local rods=""; [ "$2" = "RODS-IN" ] && rods="--rods-in"
  $bin $common $3 --cases "$2" $rods > "$log" 2> "$here/logs/case_$1_$2.err"
}
for c in FUEL-GRAPHITE DUMMIES KERNEL RODS-IN; do run poured "$c" "$dem"; done
for c in BASE RODS-IN; do run lattice "$c" ""; done
date -u +%FT%TZ > "$here/logs/end_utc.txt"
