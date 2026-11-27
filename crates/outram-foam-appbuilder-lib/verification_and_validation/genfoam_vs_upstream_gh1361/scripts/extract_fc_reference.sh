#!/bin/bash
# gh:#1878 round 38: extract refs/<ref name>.ref.txt.gz from a run of the real GeN-Foam binary on a face-centred onePhase case
# (run_case.sh + fc_protocol.sh, EVERY step written, writePrecision 17).
# usage: extract_fc_reference.sh <runs/dir name> <ref name> <NCELLS> "<step list>" <steps with a solve log>
#   step list: 1-based step numbers whose fields are kept (e.g. "1 2 3 10 50 100"); the word `last` adds the final step.
# Output lines (full doubles as written):
#   N <steps in the log>
#   DT <the deltaT printed after each step>          TIMEPRINT <printed time of each step>
#   CUMERR <final "Cumulative continuity error">
#   F <step number> <field> <NCELLS values>          (U is written as fields `U` = x component and `Uy` = y component)
#   P <step> <initial residual> <iterations> ...     every p_rgh solve of the step, in log order (steps 1..<last arg>)
R=/home/user/upstream-build/gh1361/runs/$1
OUT=/home/user/upstream-build/gh1361/$2.ref.txt
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
echo "CUMERR $(grep 'Cumulative continuity' $L | tail -1 | awk '{print $5}')" >> "$OUT"
ALLT=$(ls $R | grep -E '^[0-9.e+-]+$' | sort -g | grep -v '^0$')
for s in $STEPS; do
  if [ "$s" = last ]; then s=$NSTEP; fi
  T=$(echo "$ALLT" | sed -n "${s}p")
  [ -z "$T" ] && continue
  for f in p p_rgh T U T.fixedPower htc Re contErr dgdt; do
    F=$R/$T/fluidRegion/$f
    [ -f "$F" ] || continue
    if grep -q "^internalField *uniform" "$F"; then
      if [ "$f" = U ]; then
        X=$(grep "^internalField *uniform" "$F" | sed -E 's/^internalField *uniform *\(([^ ]+) ([^ ]+) .*/\1/')
        Y=$(grep "^internalField *uniform" "$F" | sed -E 's/^internalField *uniform *\(([^ ]+) ([^ ]+) .*/\2/')
        echo "F $s U $(yes -- "$X" | head -n $NC | tr '\n' ' ')" >> "$OUT"
        echo "F $s Uy $(yes -- "$Y" | head -n $NC | tr '\n' ' ')" >> "$OUT"
      else
        V=$(grep "^internalField *uniform" "$F" | sed -E 's/^internalField *uniform *([^ ;]+).*/\1/')
        echo "F $s $f $(yes -- "$V" | head -n $NC | tr '\n' ' ')" >> "$OUT"
      fi
    elif [ "$f" = U ]; then
      echo "F $s U $(awk '/^\($/{f=1;next} /^\)$/{exit} f{gsub(/[()]/,""); print $1}' $F | tr '\n' ' ')" >> "$OUT"
      echo "F $s Uy $(awk '/^\($/{f=1;next} /^\)$/{exit} f{gsub(/[()]/,""); print $2}' $F | tr '\n' ' ')" >> "$OUT"
    else
      echo "F $s $f $(awk '/^\($/{f=1;next} /^\)$/{exit} f{gsub(/[()]/,""); print $1}' $F | tr '\n' ' ')" >> "$OUT"
    fi
  done
done
if [ "$NP" -gt 0 ]; then
  awk -v np=$NP '/^Time = /{step++; if(step>np) exit; printf "\nP %d", step; next}
       /(DICPCG|GAMG):  Solving for p_rgh/ && step>=1 {
         for(i=1;i<=NF;i++){ if($i=="residual"&&$(i+1)=="="&&!a){a=$(i+2); sub(/,/,"",a)} if($i=="Iterations") n=$(i+1)}
         printf " %s %s", a, n; a=""}
       END{printf "\n"}' $L | grep '^P ' >> "$OUT"
fi
gzip -9 -n -c "$OUT" > "$HERE/refs/$2.ref.txt.gz"
ls -l "$HERE/refs/$2.ref.txt.gz"
