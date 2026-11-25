#!/bin/bash
# The exact launch of this record (gh:#1739): the gh:#1174 zone-worth driver
# at its statistics, on the poured bed (5 cases) and on the lattice (BASE and
# RODS-IN), 4 threads. Build first:
#   cargo build --release -p nee_soon --example htr10_graphite_zone_worth
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../../../.." && pwd)"
cd "$root"
mkdir -p "$here/logs"
bin=target/release/examples/htr10_graphite_zone_worth
common="--base 600 --delta 600 --particles 3000 --inactive 20 --active 80 --threads 4"
date -u +%FT%TZ > "$here/logs/start_utc.txt"
$bin $common --dem-bed reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv \
  --cases BASE,FUEL-GRAPHITE,DUMMIES,KERNEL,RODS-IN --rods-in > "$here/logs/poured.log" 2> "$here/logs/poured.err"
date -u +%FT%TZ > "$here/logs/poured_end_utc.txt"
$bin $common --cases BASE,RODS-IN --rods-in > "$here/logs/lattice.log" 2> "$here/logs/lattice.err"
date -u +%FT%TZ > "$here/logs/end_utc.txt"
