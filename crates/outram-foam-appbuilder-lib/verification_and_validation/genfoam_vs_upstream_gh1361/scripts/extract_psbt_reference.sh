#!/bin/bash
# Extract refs/psbt_ex1.ref.txt.gz from the run of the real GeN-Foam binary on
# featureCases/1D_PSBT_SC/PhaseII_Ex1_01_5215 (run_tutorial.sh with precision17_protocol.sh).
# usage: extract_psbt_reference.sh <runs dir> <time dir>
# One line per field: `COL <name> <v0> <v1> ...` (full doubles; a tensor field gives its first
# component; a uniform field is expanded to the cell count of alpha.liquid), plus the tail of the log:
# `ALPHA_VAPOUR_MAX_LOG`, `T_FIXEDPOWER_MAX_LOG` (the values the tutorial's own Alltest checks).
HERE="$(cd "$(dirname "$0")/.." && pwd)"
R=/home/user/upstream-build/gh1361/runs/$1
T=$2
OUT=/home/user/upstream-build/gh1361/psbt_$1_t$T.ref.txt
: > "$OUT"
NC=$(awk '/^internalField/{getline; print $1; exit}' "$R/$T/fluidRegion/alpha.liquid")
for f in Re.liquid.structure Kd.liquid.structure alpha.structure alpha.liquid alphaRhoMagU.liquid alpha.vapour T.fixedPower p T.liquid; do
  F=$R/$T/fluidRegion/$f
  [ -f "$F" ] || { echo "missing $f" >&2; continue; }
  if grep -q "^internalField *uniform" "$F"; then
    V=$(grep "^internalField *uniform" "$F" | sed -E 's/^internalField *uniform *\(?([^ ;)]+).*/\1/')
    echo "COL $f $(yes "$V" | head -n "$NC" | tr '\n' ' ')" >> "$OUT"
  else
    echo "COL $f $(awk '/^\($/{f=1;next} /^\)$/{if(f) exit} f{gsub(/[()]/,""); print $1}' "$F" | tr '\n' ' ')" >> "$OUT"
  fi
done
echo "NSTEPS $(grep -c '^Time = ' $R/log.GeN-Foam)" >> "$OUT"
gzip -9 -n -c "$OUT" > "$HERE/refs/psbt_$1_t$T.ref.txt.gz"
ls -l "$HERE/refs/psbt_$1_t$T.ref.txt.gz"
