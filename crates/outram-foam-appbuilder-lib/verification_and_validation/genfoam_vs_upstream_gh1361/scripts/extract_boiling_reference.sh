#!/bin/bash
# Extract refs/boiling_<name>.ref.txt.gz from a run of the real GeN-Foam binary on 1D_boiling.
# usage: extract_boiling_reference.sh <runs/dir name> <ref name> <time-filter-regex>
# Output lines (one per item, space separated, full doubles as written at writePrecision 17):
#   DT <dt per step>           (log "deltaT = " after each step; step n advanced time by DT[n-1])
#   TIMEPRINT <printed time>
#   CUMERR liquid vapour       (final "Cumulative continuity errors" in the log)
#   F <time-dir> <field> <40 values>   (U.* : x component)
HERE="$(cd "$(dirname "$0")/.." && pwd)"
R=/home/user/upstream-build/gh1361/runs/$1
OUT=/home/user/upstream-build/gh1361/$2.ref.txt
FILT=${3:-.}
L=$R/log.GeN-Foam
: > "$OUT"
echo "N $(grep -c '^Time = ' $L)" >> "$OUT"
echo "DT $(grep '^deltaT = ' $L | awk '{print $3}' | tr '\n' ' ')" >> "$OUT"
echo "TIMEPRINT $(grep '^Time = ' $L | awk '{print $3}' | tr '\n' ' ')" >> "$OUT"
# FIRST: 1-based step index of the first extracted time directory (every-step runs only)
ALLT=$(ls $R | grep -E '^[0-9.e+-]+$' | sort -g | grep -v '^0$')
FIRSTT=$(ls $R | grep -E '^[0-9.e+-]+$' | sort -g | grep -E "$FILT" | grep -v '^0$' | head -1)
echo "FIRST $(( $(echo "$ALLT" | grep -n -x -- "$FIRSTT" | head -1 | cut -d: -f1) ))" >> "$OUT"
echo "CUMERR $(grep 'Cumulative continuity' $L | tail -1 | awk '{print $7, $8}')" >> "$OUT"
for T in $(ls $R | grep -E '^[0-9.e+-]+$' | sort -g | grep -E "$FILT"); do
  [ "$T" = 0 ] && continue
  for f in p p_rgh T.liquid T.vapour T.interface T.fixedPower U.liquid U.vapour alpha.vapour dmdt.liquid.vapour heatFlux.structure; do
    F=$R/$T/fluidRegion/$f
    [ -f "$F" ] || continue
    # OpenFOAM writes a field whose cells are all equal as `internalField uniform V;` (V a scalar
    # or a vector): expand it to NCELLS values, else the comparison would be vacuous.
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
