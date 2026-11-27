#!/bin/bash
# gh:#1847 / README round 44: extract refs/<ref name>.ref.txt.gz from a run of the real GeN-Foam binary on a 1D_HX onePhase case
# (run_case.sh + fc_protocol.sh, EVERY step written, writePrecision 17). Like extract_fc_reference.sh, plus the z velocity (`Uz`;
# `U`/`Uy` are the x and y components) and `T.HX`.
# usage: extract_hx_reference.sh <runs/dir name> <ref name> <NCELLS> "<step list>" <steps with a solve log>
#   step list: 1-based step numbers whose fields are kept; the word `last` adds the final step.
R=/home/user/upstream-build/gh1361/runs/$1
OUT=/home/user/upstream-build/gh1847/$2.ref.txt
NC=$3
STEPS=$4
NP=${5:-0}
HERE="$(cd "$(dirname "$0")/.." && pwd)"
L=$R/log.GeN-Foam
: > "$OUT"
NSTEP=$(grep -c '^Time = ' $L)
echo "N $NSTEP" >> "$OUT"
echo "DT $(grep '^deltaT = ' $L | awk '{print $3}' | tr '\n' ' ')" >> "$OUT"
echo "TIMEPRINT $(grep '^Time = ' $L | awk '{print $3}' | tr '\n' ' ')" >> "$OUT"
ALLT=$(ls $R | grep -E '^[0-9.e+-]+$' | sort -g | grep -v '^0$')
for s in $STEPS; do
  if [ "$s" = last ]; then s=$NSTEP; fi
  T=$(echo "$ALLT" | sed -n "${s}p")
  [ -z "$T" ] && continue
  for f in p p_rgh T U T.fixedPower T.HX htc Re contErr dgdt; do
    F=$R/$T/fluidRegion/$f
    [ -f "$F" ] || continue
    if [ "$f" = U ]; then
      for comp in 1 2 3; do
        name=$(echo "U Uy Uz" | cut -d' ' -f$comp)
        if grep -q "^internalField *uniform" "$F"; then
          V=$(grep "^internalField *uniform" "$F" | sed -E 's/^internalField *uniform *\(//; s/\).*//' | awk -v c=$comp '{print $c}')
          echo "F $s $name $(yes -- "$V" | head -n $NC | tr '\n' ' ')" >> "$OUT"
        else
          echo "F $s $name $(awk -v c=$comp '/^\($/{f=1;next} /^\)$/{exit} f{gsub(/[()]/,""); print $c}' $F | tr '\n' ' ')" >> "$OUT"
        fi
      done
    elif grep -q "^internalField *uniform" "$F"; then
      V=$(grep "^internalField *uniform" "$F" | sed -E 's/^internalField *uniform *([^ ;]+).*/\1/')
      echo "F $s $f $(yes -- "$V" | head -n $NC | tr '\n' ' ')" >> "$OUT"
    else
      echo "F $s $f $(awk '/^\($/{f=1;next} /^\)$/{exit} f{gsub(/[()]/,""); print $1}' $F | tr '\n' ' ')" >> "$OUT"
    fi
  done
done
if [ "$NP" -gt 0 ]; then
  awk -v np=$NP '/^Time = /{step++; if(step>np) exit; printf "\nP %d", step; next}
       /DICPCG:  Solving for p_rgh/ && step>=1 {
         for(i=1;i<=NF;i++){ if($i=="residual"&&$(i+1)=="="&&!a){a=$(i+2); sub(/,/,"",a)} if($i=="Iterations") n=$(i+1)}
         printf " %s %s", a, n; a=""}
       END{printf "\n"}' $L | grep '^P ' >> "$OUT"
fi
gzip -9 -n -c "$OUT" > "$HERE/refs/$2.ref.txt.gz"
ls -l "$HERE/refs/$2.ref.txt.gz"
