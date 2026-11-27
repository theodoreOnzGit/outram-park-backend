#!/bin/bash
# gh:#1887 (README round 42): refs/r42_solver.ref.txt.gz and refs/r42_extsrc.ref.txt.gz from the runs of make_r42_cases.sh
# (/home/user/upstream-build/gh1887/runs/<v>), run_r42_extsrc.sh (BR, BS) and r42_sn_hook.sh (gh1361/runs/r42_sn_*).
#   solver: per run  CASE <v> ; N (steps) ; TIME ; KEFF (eigenvalue runs) ; POWER ; RESID1 (first-iteration residual of each step) ;
#           RESIDL (last residual of each step) ; NITER (neutron iterations per step) ; FINALTIME ; FLUX0 / FLUX1 (final-time cells)
#   extsrc: as refs/external_source.ref.txt.gz (round 32): TIME DT POWER NITER RESID FLUX0_100 FLUX1_100 FLUX0_10 PTARGET
HERE="$(cd "$(dirname "$0")/.." && pwd)"
RUNS=/home/user/upstream-build/gh1887/runs
S=/home/user/upstream-build/gh1887/r42_solver.ref.txt
: > "$S"
cell() { awk '/^\(/{f=1;next} /^\)/{f=0} f{print $1}' "$1" | tr '\n' ' '; }
for v in v0r v0m fin s0 s2 s3 s4 n0 n3 n1; do
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
for n in sn_1 sn_13; do
  echo "CASE $n" >> "$S"
  echo "KEFF $(grep '^keff_ = ' /home/user/upstream-build/gh1361/runs/r42_$n/log.GeN-Foam | awk '{print $3}' | tr '\n' ' ')" >> "$S"
done
gzip -9 -n -c "$S" > "$HERE/refs/r42_solver.ref.txt.gz"
X=/home/user/upstream-build/gh1887/r42_extsrc.ref.txt
: > "$X"
for v in BR BS; do
  R=/home/user/upstream-build/gh1887/$v; L=$R/log.GeN-Foam
  echo "CASE $v" >> "$X"
  echo "TIME $(grep '^Time = ' $L | awk '{print $3}' | tr '\n' ' ')" >> "$X"
  echo "DT $(grep '^deltaT = ' $L | awk '{print $3}' | tr '\n' ' ')" >> "$X"
  echo "POWER $(grep '^power: ' $L | awk '{print $2}' | tr '\n' ' ')" >> "$X"
  echo "NITER $(awk '/^Time = /{if(n)print n; n=0} /^Neutron iteration no:/{n++} END{print n}' $L | tr '\n' ' ')" >> "$X"
  echo "RESID $(awk '/^Time = /{if(r!="")print r; r=""} /^neutroResidual \(neutroIter/{r=$4} END{print r}' $L | tr '\n' ' ')" >> "$X"
  echo "FLUX0_100 $(cell $R/100/neutroRegion/flux0)" >> "$X"
  echo "FLUX1_100 $(cell $R/100/neutroRegion/flux1)" >> "$X"
  echo "FLUX0_10 $(cell $R/10/neutroRegion/flux0)" >> "$X"
  echo "PTARGET $(awk '/^pTarget/{gsub(";","");print $2}' $R/100/uniform/reactorState)" >> "$X"
done
gzip -9 -n -c "$X" > "$HERE/refs/r42_extsrc.ref.txt.gz"
ls -l "$HERE/refs/r42_solver.ref.txt.gz" "$HERE/refs/r42_extsrc.ref.txt.gz"
