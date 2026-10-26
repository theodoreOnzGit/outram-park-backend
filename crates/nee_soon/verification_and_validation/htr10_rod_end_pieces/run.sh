#!/usr/bin/env bash
# gh:#1027: the three rod steel-piece variants at full insertion, N = 12,
# one seed (correlated runs). Run from the workspace root.
set -u
D=crates/nee_soon/verification_and_validation/htr10_rod_end_pieces
for v in published top_end_b4c all_b4c; do
  OUTRAM_ROD_CASES="12:394.2" OUTRAM_ROD_STEEL_PIECES=$v \
    OUTRAM_ROD_CSV=$D/$v.csv target/release/examples/htr10_rod_curve > $D/$v.log 2>&1
  echo "$v EXIT $?"
done
