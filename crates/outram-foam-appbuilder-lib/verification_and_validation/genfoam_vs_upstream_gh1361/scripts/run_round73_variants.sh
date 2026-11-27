#!/bin/bash
# gh:#1939, #1940 round 73: run the real GeN-Foam binary on each variant of make_round73_variants.sh (END_TIME default 4.9: 20 steps) and extract
# the references <out>/fc_fluid_r73_<name>.ref.txt / _log.ref.txt.   usage: run_round73_variants.sh <variants dir> <out dir> [variant names...]
VAR=$1
OUT=$2
shift 2
HERE=$(dirname "$0")
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
mkdir -p "$OUT"
NAMES=${*:-f1_facecentered f2_extrap f3_varrho f4_fc_varrho f5_byregime f5b_byregime_interp f6_fc_nonorth f7_zerograd_p_rgh f8_fc_zerograd}
for v in $NAMES; do
  c="$VAR/$v"
  sed -i "s/^endTime .*/endTime         ${END_TIME:-4.9};/" "$c/system/controlDict"
  (cd "$VAR" && GeN-Foam -case "$v" > "$v/log.GeN-Foam" 2>&1; echo "$v exit $?")
  bash "$HERE/extract_fullcoupling_fluid.sh" "$c" "$OUT/fc_fluid_r73_$v"
done
