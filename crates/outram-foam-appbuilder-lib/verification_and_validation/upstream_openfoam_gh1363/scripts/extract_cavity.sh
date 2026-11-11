#!/bin/bash
# gh:#1363 row 1. Extract the committed reference (U, p per cell at chosen
# steps; initial residuals per step) from a finished upstream icoFoam run in $1
# (the case directory, run by scripts/run_cavity.sh).
# Writes cavity/reference/cavity_dev_icoFoam.txt.gz, residuals.txt and the
# gzipped solver log. Plain awk/gzip, nothing else.
set -e
CASE=${1:-/home/user/upstream-build/gh1363/cavity}
HERE=$(cd "$(dirname "$0")/.." && pwd)
OUT=$HERE/cavity/reference
{
  for t in 0.005 0.01 0.025 0.05 0.125 0.25 0.5; do
    echo "#time $t"
    awk '/^\($/{f=1;next} /^\)$/{f=0} f{gsub(/[()]/,"");print "U",$1,$2,$3}' $CASE/$t/U
    awk '/^\($/{f=1;next} /^\)$/{f=0} f{print "p",$1}' $CASE/$t/p
    awk '/^\($/{f=1;next} /^\)$/{f=0} f{print "phi",$1}' $CASE/$t/phi | head -760
  done
} | gzip -9n > $OUT/cavity_dev_icoFoam.txt.gz
# initial residuals per time step: Ux Uy p(corrector 1) p(corrector 2)
awk '/^Time = /{gsub("s","",$3); t=$3}
     /Solving for Ux/{gsub(",","");ux=$8}
     /Solving for Uy/{gsub(",","");uy=$8}
     /Solving for p/{gsub(",","");pp[++k]=$8; if(k==2){print t,ux,uy,pp[1],pp[2]; k=0}}' $CASE/log.icoFoam > $OUT/residuals.txt
gzip -9n -c $CASE/log.icoFoam > $OUT/log.icoFoam.gz
