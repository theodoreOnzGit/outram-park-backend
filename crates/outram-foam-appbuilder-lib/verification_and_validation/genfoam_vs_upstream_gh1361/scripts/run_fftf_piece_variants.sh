#!/bin/bash
# gh:#1870 round 62: run the REAL GeN-Foam binary on each variant of make_fftf_piece_variants.py (END_TIME default 4.9: 20 steps) and extract the
# references fftf_r62_<variant>.ref.txt / _log.ref.txt into <out dir> (same extractor as round 55).
# usage: run_fftf_piece_variants.sh <variants dir> <out dir>
VAR=$1
OUT=$2
HERE=$(dirname "$0")
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
mkdir -p "$OUT"
for v in w1_dilu w2_half_cc w2_recon05 w2_recon1 w2_recon0 w3_pump w4_churchill w5_noslip; do
  c="$VAR/$v"
  sed -i "s/^endTime .*/endTime         ${END_TIME:-4.9};/" "$c/system/controlDict"
  (cd "$VAR" && GeN-Foam -case "$v" > "$v/log.GeN-Foam" 2>&1; echo "$v exit $?")
  bash "$HERE/extract_fullcoupling_fluid.sh" "$c" "$OUT/fftf_r62_$v"
done
