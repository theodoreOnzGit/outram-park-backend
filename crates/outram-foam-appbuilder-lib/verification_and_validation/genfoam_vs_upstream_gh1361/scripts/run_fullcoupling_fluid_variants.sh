#!/bin/bash
# gh:#1913 round 55: run the real GeN-Foam binary on each variant of make_fullcoupling_fluid_variants.sh for END_TIME (default 4.9: 20 steps)
# and extract the references <name>.ref.txt / <name>_log.ref.txt into <out dir>.
# usage: run_fullcoupling_fluid_variants.sh <variants dir> <out dir>
VAR=$1
OUT=$2
HERE=$(dirname "$0")
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
mkdir -p "$OUT"
for v in v1_nopassive v2_consthtc v3_smooth v4_pintest v5_pintest_dict v5b_pin_dict; do
  c="$VAR/$v"
  sed -i "s/^endTime .*/endTime         ${END_TIME:-4.9};/" "$c/system/controlDict"
  (cd "$VAR" && GeN-Foam -case "$v" > "$v/log.GeN-Foam" 2>&1; echo "$v exit $?")
  bash "$HERE/extract_fullcoupling_fluid.sh" "$c" "$OUT/fc_fluid_r55_$v"
done
