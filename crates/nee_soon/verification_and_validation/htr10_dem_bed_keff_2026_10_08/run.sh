#!/bin/bash
# The exact launch of this record (gh:#787 option A): the recorded driver on
# the gh:#216 DEM bed cut to the lattice's 16 681 balls at N = 12,
# 10 000 x [5 + 135], ENDF/B-VIII.0 defaults, seed 20260917, 5 threads.
# Run from anywhere; it starts the binary from the repository root so the
# per-run performance report lands in the gitignored root local_perf/.
# Build first: cargo build --release -j 5 -p nee_soon --example htr10_rmc_keff
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../../../.." && pwd)"
cd "$root"
mkdir -p "$here/logs"
date -u +%FT%TZ > "$here/logs/start_utc.txt"
OUTRAM_HTR10_DEM_BED=reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv \
OUTRAM_HTR10_HISTORIES=10000 \
OUTRAM_HTR10_INACTIVE=5 \
OUTRAM_HTR10_ACTIVE=135 \
OUTRAM_HTR10_RINGS=14 \
OUTRAM_HTR10_LAYERS=12 \
OUTRAM_HTR10_THREADS=5 \
  target/release/examples/htr10_rmc_keff > "$here/logs/run_dem_e8_N12.log" 2>&1
date -u +%FT%TZ > "$here/logs/end_utc.txt"
