#!/bin/bash
# Extract refs/<ref name>.ref.txt.gz from a run of the real GeN-Foam binary on the onePhase channel
# (tests/data/genfoam_1d_onephase; run_case.sh + onephase_protocol.sh, EVERY step written).
# usage: extract_onephase_reference.sh <runs/dir name> <ref name> <first N steps> [last]
#   N steps are taken from the start; the word `last` adds the final time directory.
# Output lines (full doubles as written at writePrecision 17, one per item, space separated):
#   N <steps in the log>
#   DT <the deltaT printed after each step; step n advanced time by DT[n-1], step 1 by controlDict deltaT>
#   TIMEPRINT <printed time of each step>
#   CUMERR <final "Cumulative continuity error" [kg/m3]>
#   F <time-dir> <field> <40 values>   (U: x component)
HERE="$(cd "$(dirname "$0")/.." && pwd)"
R=/home/user/upstream-build/gh1361/runs/$1
OUT=/home/user/upstream-build/gh1361/$2.ref.txt
NSTEPS=${3:-40}
L=$R/log.GeN-Foam
: > "$OUT"
echo "N $(grep -c '^Time = ' $L)" >> "$OUT"
echo "DT $(grep '^deltaT = ' $L | awk '{print $3}' | tr '\n' ' ')" >> "$OUT"
echo "TIMEPRINT $(grep '^Time = ' $L | awk '{print $3}' | tr '\n' ' ')" >> "$OUT"
echo "CUMERR $(grep 'Cumulative continuity' $L | tail -1 | awk '{print $5}')" >> "$OUT"
ALLT=$(ls $R | grep -E '^[0-9.e+-]+$' | sort -g | grep -v '^0$')
SEL=$(echo "$ALLT" | head -n "$NSTEPS")
[ "$4" = last ] && SEL="$SEL
$(echo "$ALLT" | tail -1)"
for T in $SEL; do
  for f in p p_rgh T U T.fixedPower htc Re contErr dgdt; do
    F=$R/$T/fluidRegion/$f
    [ -f "$F" ] || continue
    if grep -q "^internalField *uniform" "$F"; then
      V=$(grep "^internalField *uniform" "$F" | sed -E 's/^internalField *uniform *\(?([^ ;)]+).*/\1/')
      echo "F $T $f $(yes "$V" | head -n 40 | tr '\n' ' ')" >> "$OUT"
    else
      echo "F $T $f $(awk '/^\($/{f=1;next} /^\)$/{f=0} f{gsub(/[()]/,""); print $1}' $F | tr '\n' ' ')" >> "$OUT"
    fi
  done
done
gzip -9 -n -c "$OUT" > "$HERE/refs/$2.ref.txt.gz"
ls -l "$HERE/refs/$2.ref.txt.gz"
