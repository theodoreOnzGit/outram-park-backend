#!/bin/bash
# gh:#1887 (README round 47): refs/r47_solver.ref.txt.gz from the runs of make_r47_cases.sh
# (/home/user/upstream-build/gh1887/runs/<run>). Format as refs/r42_solver.ref.txt.gz (CASE N TIME KEFF POWER RESID1 RESIDL
# NITER FINALTIME FLUX0 FLUX1) plus, from the log (17 digits):
#   CLAMPS  number of `Warning: keff` lines        DF1  the 8 factors printed after step 1 (group-major: group 0 zones, group 1 zones)
#   DFL     the 8 factors printed after the last step      DFALL  every printed factor of the run (8 per step)
# A run that dies (FPE) has the steps it printed; FINALTIME / FLUX are then absent.
HERE="$(cd "$(dirname "$0")/.." && pwd)"
RUNS=/home/user/upstream-build/gh1887/runs
S=$(mktemp)
cell() { awk '/^\(/{f=1;next} /^\)/{f=0} f{print $1}' "$1" | tr '\n' ' '; }
# <case name in the reference> <run directory>
for pair in s_i0:i0_sp3 s_i1:i1_s0 s_i2:i2_s0 s_i1_shipped:i1_s1; do
  v=${pair%%:*}; r=${pair##*:}
  R=$RUNS/$r; L=$R/log.GeN-Foam
  echo "CASE $v" >> "$S"
  echo "N $(grep -c '^Time = ' $L)" >> "$S"
  echo "TIME $(grep '^Time = ' $L | awk '{print $3}' | tr '\n' ' ')" >> "$S"
  echo "KEFF $(grep '^keff_ = ' $L | awk '{print $3}' | tr '\n' ' ')" >> "$S"
  echo "POWER $( (grep "^power: " $L | awk "{print \$2}"; [ -z "$(grep -m1 "^power: " $L)" ] && grep " - pow:" $L | sed -E "s/.* - pow://") | tr "\n" " ")" >> "$S"
  echo "RESID1 $(grep '^neutroResidual (neutroIter: 1)' $L | awk '{print $4}' | tr '\n' ' ')" >> "$S"
  echo "RESIDL $(awk '/^Time = /{if(r!="")print r; r=""} /^neutroResidual \(neutroIter/{r=$4} END{print r}' $L | tr '\n' ' ')" >> "$S"
  echo "NITER $(awk '/^Time = /{if(n)print n; n=0} /^Neutron iteration no:/{n++} END{print n}' $L | tr '\n' ' ')" >> "$S"
  echo "CLAMPS $(grep -c 'Warning: keff' $L)" >> "$S"
  echo "DFALL $(grep 'discFactorList' $L | awk '{print $6}' | tr '\n' ' ')" >> "$S"
  T=$(ls $R | grep -E '^[0-9.e+-]+$' | sort -g | tail -1)
  if [ "$T" != "0" ]; then
    echo "FINALTIME $T" >> "$S"
    for g in 0 1; do echo "FLUX$g $(cell $R/$T/neutroRegion/flux$g)" >> "$S"; done
  fi
done
gzip -9 -n -c "$S" > "$HERE/refs/r47_solver.ref.txt.gz"
rm -f "$S"
ls -l "$HERE/refs/r47_solver.ref.txt.gz"
