#!/bin/bash
# gh:#1893 (README round 43, D7): refs/r43_stationary.ref.txt.gz from the runs of make_r42_cases.sh
#   n4 = nuclearData v3 (given discontinuity factors) + CRmove.d4 (the rod moves 0.035 m in the 3.5e-4 s), eigen=false
#   n5 = nuclearData v0 (unity factors)               + CRmove.d4, eigen=false
# Same format as refs/r42_solver.ref.txt.gz (extract_r42_reference.sh): per run CASE, N, TIME, KEFF, POWER, RESID1, RESIDL,
# NITER, FINALTIME, FLUX0, FLUX1.
HERE="$(cd "$(dirname "$0")/.." && pwd)"
RUNS=/home/user/upstream-build/gh1887/runs
S=$(mktemp)
cell() { awk '/^\(/{f=1;next} /^\)/{f=0} f{print $1}' "$1" | tr '\n' ' '; }
for v in n4 n5; do
  R=$RUNS/$v; L=$R/log.GeN-Foam
  echo "CASE $v" >> "$S"
  echo "N $(grep -c '^Time = ' $L)" >> "$S"
  echo "TIME $(grep '^Time = ' $L | awk '{print $3}' | tr '\n' ' ')" >> "$S"
  echo "KEFF $(grep '^keff_ = ' $L | awk '{print $3}' | tr '\n' ' ')" >> "$S"
  echo "POWER $( (grep "^power: " $L | awk "{print \$2}"; [ -z "$(grep -m1 "^power: " $L)" ] && grep " - pow:" $L | sed -E "s/.* - pow://") | tr "\n" " ")" >> "$S"
  echo "RESID1 $(grep '^neutroResidual (neutroIter: 1)' $L | awk '{print $4}' | tr '\n' ' ')" >> "$S"
  echo "RESIDL $(awk '/^Time = /{if(r!="")print r; r=""} /^neutroResidual \(neutroIter/{r=$4} END{print r}' $L | tr '\n' ' ')" >> "$S"
  echo "NITER $(awk '/^Time = /{if(n)print n; n=0} /^Neutron iteration no:/{n++} END{print n}' $L | tr '\n' ' ')" >> "$S"
  T=$(ls $R | grep -E '^[0-9.e+-]+$' | sort -g | tail -1)
  echo "FINALTIME $T" >> "$S"
  for g in 0 1; do echo "FLUX$g $(cell $R/$T/neutroRegion/flux$g)" >> "$S"; done
done
gzip -9 -n -c "$S" > "$HERE/refs/r43_stationary.ref.txt.gz"
rm -f "$S"
ls -l "$HERE/refs/r43_stationary.ref.txt.gz"
