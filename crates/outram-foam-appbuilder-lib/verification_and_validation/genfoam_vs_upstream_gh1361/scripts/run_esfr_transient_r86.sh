#!/bin/bash
# gh:#1955 (README round 86): the real 3D_SmallESFR_old loose transients of round 86, from ONE real loose steady state.
# usage: run_esfr_transient_r86.sh <scratch dir> <out prefix dir>
#   <d>/ss        the loose steady state (make_esfr_old_case.sh steady, INFLATE=1), 200 steps
#   <d>/tr_plain  make_esfr_old_case.sh transient from ss (the shipped Allrun's setTransient; no -initializeMappedFields): rounds 75/78
#   <d>/tr_init   the same, then `GeN-Foam -initializeMappedFields` (round 84's run_esfr_baffle_less_r84.sh)
#   <d>/sens      tr_plain's start with every internal 200/fluidRegion/T value times (1 + 1e-12) (perturb_esfr_start_rel.py)
#   <d>/ts        tr_plain with writeControl timeStep; writeInterval 1, extracted with ALL_FIELDS=1 and the key fields kept at steps 1,2,52
# Each transient's log is extracted with extract_esfr_transient.py into <out>/<name>_log.txt.
D=$1; O=$2
S=$(dirname "$(readlink -f "$0")")
W=/home/user/upstream-build/scripts/with-of.sh
export LD_LIBRARY_PATH=/home/user/upstream-build/gh1781/lib:$LD_LIBRARY_PATH
mkdir -p "$D" "$O"
INFLATE=1 bash "$S/make_esfr_old_case.sh" steady "$D/ss"
(cd "$D/ss" && $W GeN-Foam -case "$D/ss" > log.GeN-Foam 2>&1)
for v in tr_plain tr_init sens ts; do
  bash "$S/make_esfr_old_case.sh" transient "$D/$v" "$D/ss" > /dev/null
done
(cd "$D/tr_init" && $W GeN-Foam -initializeMappedFields -case "$D/tr_init" > log.init 2>&1)
python3 -I "$S/perturb_esfr_start_rel.py" "$D/sens/200/fluidRegion/T" 1e-12
sed -i "s/^writeControl .*/writeControl timeStep;/; s/^writeInterval .*/writeInterval 1;/" "$D/ts/system/controlDict"
for v in tr_plain tr_init sens; do
  (cd "$D/$v" && $W GeN-Foam -case "$D/$v" > log.GeN-Foam 2>&1)
  python3 -I "$S/extract_esfr_transient.py" "$D/$v" "$O/$v"
done
(cd "$D/ts" && $W GeN-Foam -case "$D/ts" > log.GeN-Foam 2>&1) &
ALL_FIELDS=1 python3 -I "$S/extract_esfr_transient.py" "$D/ts" "$O/ts" 1,2,52
wait
