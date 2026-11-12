#!/bin/sh
# Run the four friction-ablation cells with the patched upstream build and write
# ablate_mu<mu>_mur<mur>.csv (wide trajectory CSV, frames every 50 000 steps).
#   ./run_ablate.sh <lmp_serial> [workdir]
# Cells: mu in {0.1, 0.4} x mu_r in {0, 0.1} (labels mu10/mu40, mur00/mur10).
set -eu
lmp=$(realpath "$1")
here=$(cd "$(dirname "$0")" && pwd)
work=${2:-$(mktemp -d)}
cd "$work"
sh "$here/csv2data.sh" "$here/pebble_bed_init.csv" ablate.data 0.01 2500 -0.005 0.35
for cell in "0.1 0.0 mu10_mur00 in.ablate" "0.4 0.0 mu40_mur00 in.ablate" \
            "0.1 0.1 mu10_mur10 in.ablate_cdt" "0.4 0.1 mu40_mur10 in.ablate_cdt"; do
  set -- $cell
  rm -f dump.ablate
  "$lmp" -var MU "$1" -var MUR "$2" -in "$here/$4" > "log.$3" 2>&1
  sh "$here/dump2traj.sh" dump.ablate "$here/ablate_$3.csv"
done
