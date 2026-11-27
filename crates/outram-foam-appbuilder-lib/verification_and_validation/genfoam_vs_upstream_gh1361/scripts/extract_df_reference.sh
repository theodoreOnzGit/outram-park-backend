#!/bin/bash
# gh:#1785 / #1474 (README round 39): write refs/df_slab_solver.ref.txt.gz and refs/df_slab_xs.ref.txt.gz from the runs of
# scripts/make_df_driveline_cases.sh (solver logs) and of upstream_driver/dump_xs_df (xsdump).
#   solver: per variant  CASE <v> ; N ; TIME / KEFF / POWER / RESID (one value per step) ; FLUX0 / FLUX1 (final-time cells)
#   xs:     per variant  CASE <v> ; per probe time  T <idx> <time> ; CR ; DFLIST ; ARR <name> <cells>
HERE="$(cd "$(dirname "$0")/.." && pwd)"
RUNS=/home/user/upstream-build/gh1785/runs
S=/home/user/upstream-build/gh1785/df_slab_solver.ref.txt
X=/home/user/upstream-build/gh1785/df_slab_xs.ref.txt
: > "$S"
for v in v0 v1 v2 v3 v0_d1 v1_d1 v2_d1 v0_d3; do
  R=$RUNS/$v; L=$R/log.GeN-Foam
  echo "CASE $v" >> "$S"
  echo "N $(grep -c '^keff_ = ' $L)" >> "$S"
  echo "TIME $(grep '^Time = ' $L | awk '{print $3}' | tr '\n' ' ')" >> "$S"
  echo "KEFF $(grep '^keff_ = ' $L | awk '{print $3}' | tr '\n' ' ')" >> "$S"
  echo "POWER $(grep '^power: ' $L | awk '{print $2}' | tr '\n' ' ')" >> "$S"
  echo "RESID $(grep '^neutroResidual (neutroIter: 1)' $L | awk '{print $4}' | tr '\n' ' ')" >> "$S"
  T=$(ls $R | grep -E '^[0-9.e+-]+$' | sort -g | tail -1)
  echo "FINALTIME $T" >> "$S"
  for g in 0 1; do
    echo "FLUX$g $(awk '/^\(/{f=1;next} /^\)/{f=0} f{print $1}' $R/$T/neutroRegion/flux$g | tr '\n' ' ')" >> "$S"
  done
done
gzip -9 -n -c "$S" > "$HERE/refs/df_slab_solver.ref.txt.gz"
: > "$X"
declare -A TIMES=( [v1]="0 1 2 3 4 5 6 7 8 9" [v1_d1]="0 5 10 12.5 15 20 25 29 30 40" [v0_d1]="0 5 10 12.5 15 20 25 29 30 40" [v0_d3]="0 1 5 10 20 30" )
for v in v1 v1_d1 v0_d1 v0_d3; do
  echo "CASE $v" >> "$X"
  i=0
  for t in ${TIMES[$v]}; do
    i=$((i+1)); D=$RUNS/$v/xsdump/$i
    echo "T $i $t" >> "$X"
    echo "CR $(tr '\n' ' ' < $D/CRposition)" >> "$X"
    echo "DFLIST $(tr '\n' ' ' < $D/dfList)" >> "$X"
    for a in D0 D1 nuSigmaEff0 nuSigmaEff1 sigmaRemoval0 sigmaRemoval1 discFactor0 discFactor1 IV0 IV1 sigmaFromTo_0_1 BetaTot; do
      echo "ARR $a $(tr '\n' ' ' < $D/$a)" >> "$X"
    done
  done
done
gzip -9 -n -c "$X" > "$HERE/refs/df_slab_xs.ref.txt.gz"
ls -l "$HERE/refs/df_slab_solver.ref.txt.gz" "$HERE/refs/df_slab_xs.ref.txt.gz"
