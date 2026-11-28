#!/bin/bash
# gh:#1905 / gh:#1361 round 80, step 4: references of a `run_msr1d_transient_r80.sh` run (coupled fluid + liquid-fuel point kinetics).
# usage: extract_msr1d_transient_r80.sh <run dir> <out prefix> [EVERY=1]
#   <prefix>.ref.txt      `F <time> <region>/<name> v...` of every EVERY-th written time directory after 100 (fluid: U p p_rgh T k epsilon
#                         nut alphat; neutro: precPKStar0..7 powerDensity TFuel TCool rhoCool)
#   <prefix>_log.ref.txt  the fluid solve lines (extract_msfr_th_log.sh) and, per step: `PREC <step> <group> <initial residual> <iterations>`
#                         (every precPKStar solve, in order), `NRES <step> <iteration> <neutroResidual>`, `POWER <step> <totalPower>`,
#                         `REACT <step> <totalReactivity pcm>`, `LFBETA <step> <liquidFuelBeta pcm>`, `PEQ <step> <precEquilibriumReactivity pcm>`,
#                         `DTNEXT`, `STEPTIME`
# Round 81 (gh:#1962): `MSR_EXTRA_FIELDS="<region/name> ..."` appends fields to the list below.
R=$1; OUT=$2; EVERY=${3:-1}
HERE=$(dirname "$0")
TIMES=$(ls "$R" | grep -E '^[0-9.e+-]+$' | awk '$1 > 100' | sort -g | awk -v e="$EVERY" '{n++; if (n % e == 0) print}' | tr '\n' ' ')
: > "$OUT.ref.txt"
for t in $TIMES; do
  for rf in fluidRegion/U fluidRegion/p fluidRegion/p_rgh fluidRegion/T fluidRegion/k fluidRegion/epsilon fluidRegion/nut fluidRegion/alphat \
            neutroRegion/precPKStar0 neutroRegion/precPKStar1 neutroRegion/precPKStar2 neutroRegion/precPKStar3 neutroRegion/precPKStar4 \
            neutroRegion/precPKStar5 neutroRegion/precPKStar6 neutroRegion/precPKStar7 neutroRegion/powerDensity neutroRegion/TFuel \
            neutroRegion/TCool neutroRegion/rhoCool $MSR_EXTRA_FIELDS; do
    file=$R/$t/$rf
    [ -f "$file" ] || continue
    printf "F %s %s " "$t" "$rf" >> "$OUT.ref.txt"
    awk '/^\($/{if(!s){s=1;next}} /^\)$/{if(s)exit} s{gsub(/[()]/,""); printf "%s ", $0}' "$file" >> "$OUT.ref.txt"
    echo >> "$OUT.ref.txt"
  done
done
bash "$HERE/extract_msfr_th_log.sh" "$R/log.GeN-Foam" "$OUT""_log.ref.txt"
awk '
  /^Time = /{s++}
  /Solving for precPKStar/{split($0,a,"precPKStar"); split(a[2],g,","); split($0,b,"Initial residual = "); split(b[2],c,","); split($0,it,"No Iterations "); printf "PREC %d %s %s %s\n", s, g[1], c[1], it[2]}
  /^neutroResidual/{gsub(/[():]/," "); printf "NRES %d %s %s\n", s, $3, $4}
  /totalPower = /{printf "POWER %d %s\n", s, $3}
  /totalReactivity/{printf "REACT %d %s\n", s, $3}
  /^liquidFuelBeta/{printf "LFBETA %d %s\n", s, $3}
  /^precEquilibriumReactivity/{printf "PEQ %d %s\n", s, $3}
' "$R/log.GeN-Foam" >> "$OUT""_log.ref.txt"
grep "^deltaT = " "$R/log.GeN-Foam" | awk '{n++; print "DTNEXT", n, $3}' >> "$OUT""_log.ref.txt"
grep "^Time = " "$R/log.GeN-Foam" | awk '{n++; print "STEPTIME", n, $3}' >> "$OUT""_log.ref.txt"
