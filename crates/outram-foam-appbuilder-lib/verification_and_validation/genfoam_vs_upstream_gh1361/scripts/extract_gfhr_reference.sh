#!/bin/bash
# Extract refs/gfhr_chain.ref.txt.gz from the real-binary run of run_gfhr.sh (writePrecision 17).
# usage: extract_gfhr_reference.sh <time dir>
# Samples every STRIDE-th pebble-bed cell (TpS > 0) of the 292,500, plus the TpS extremes, and writes
# `COL <field> v0 v1 ...` (full doubles) for the pebble chain stages upstream writes: TpS, Tmout, Tmav,
# TfS, Tfav, Tfmax and keffmatrix, plus `IDX` (the sampled cell indices) and `NCELLS`.
HERE="$(cd "$(dirname "$0")/.." && pwd)"
R=/home/user/upstream-build/gh1361/runs/gfhr
T=$1
STRIDE=${STRIDE:-100}
D=$R/$T/fluidRegion
OUT=/home/user/upstream-build/gh1361/gfhr_chain.ref.txt
col() { awk '/^\($/{f=1;next} /^\)$/{if(f) exit} f{print $1}' "$1"; }
col "$D/TpS.nuclearSteadyStatePebble" > /tmp/gfhr_tps.txt
NC=$(wc -l < /tmp/gfhr_tps.txt)
awk -v s="$STRIDE" '{ v=$1; if (v > 0) { if (mn == "" || v < mn) { mn=v; imn=NR } if (mx == "" || v > mx) { mx=v; imx=NR } if ((NR-1) % s == 0) keep[NR]=1 } }
  END { keep[imn]=1; keep[imx]=1; for (i=1;i<=NR;i++) if (keep[i]) print i }' /tmp/gfhr_tps.txt > /tmp/gfhr_idx.txt
: > "$OUT"
echo "NCELLS $NC" >> "$OUT"
echo "IDX $(tr '\n' ' ' < /tmp/gfhr_idx.txt)" >> "$OUT"
for f in TpS Tmout Tmav TfS Tfav Tfmax keffmatrix; do
  F="$D/$f.nuclearSteadyStatePebble"
  if grep -q "^internalField *uniform" "$F"; then
    # a field whose cells are all equal is written `uniform V;`: one value, applying to every sampled cell
    echo "UNIFORM $f $(grep "^internalField *uniform" "$F" | sed -E 's/^internalField *uniform *([^ ;]+).*/\1/')" >> "$OUT"
    continue
  fi
  col "$F" > /tmp/gfhr_col.txt
  echo "COL $f $(awk 'NR==FNR{want[$1]=1; next} (FNR in want){print $1}' /tmp/gfhr_idx.txt /tmp/gfhr_col.txt | tr '\n' ' ')" >> "$OUT"
done
gzip -9 -n -c "$OUT" > "$HERE/refs/gfhr_chain.ref.txt.gz"
ls -l "$HERE/refs/gfhr_chain.ref.txt.gz"
