#!/bin/bash
# gh:#1951 round 77: references of a `run_msfr_allrun_gamg.sh th` run.
#   refs/<stem>_log.ref.txt.gz     N <steps>; DT <the `deltaT = ` printed after each step>; GAMGLINES <count of the p_rgh GAMG solves>;
#                                  per solve PRGH/EPS/K/H/TSTAT lines (extract_msfr_th_log.sh)
#   refs/<stem>_fields.ref.txt.gz  (only with a step list) `F <step> <name> v1 v2 ...` for the written step directories (WRITE=step runs:
#                                  step k = the k-th numeric directory above 0), a vector flattened x y z per cell, surface fields internal faces
# usage: extract_msfr_gamg_r77.sh <run dir> <stem> ["<steps>" ["<fields>"]]
HERE="$(cd "$(dirname "$0")/.." && pwd)"
R=$1; STEM=$2; STEPS=$3
FIELDS=${4:-"U p p_rgh T k epsilon nut alphat alphaPhi alphaRhoPhi htc T.fixedTemperature"}
L=$R/log.GeN-Foam
OUT=$(mktemp)
echo "N $(grep -c '^Time = ' "$L")" > "$OUT"
echo "DT $(grep '^deltaT = ' "$L" | awk '{print $3}' | tr '\n' ' ')" >> "$OUT"
echo "GAMGLINES $(grep -c 'GAMG:  Solving for p_rgh' "$L")" >> "$OUT"
bash "$(dirname "$0")/extract_msfr_th_log.sh" "$L" "$OUT.log"
cat "$OUT.log" >> "$OUT"
gzip -9 -n -c "$OUT" > "$HERE/refs/${STEM}_log.ref.txt.gz"
rm -f "$OUT" "$OUT.log"
ls -l "$HERE/refs/${STEM}_log.ref.txt.gz"
if [ -n "$STEPS" ]; then
  F=$(mktemp)
  DIRS=$(ls "$R" | grep -E '^[0-9.e+-]+$' | grep -vx 0 | sort -g)
  for k in $STEPS; do
    t=$(echo "$DIRS" | sed -n "${k}p")
    for f in $FIELDS; do
      file=$R/$t/fluidRegion/$f; [ -f "$file" ] || file=$file.gz
      printf "F %s %s " "$k" "$f" >> "$F"
      (case $file in *.gz) zcat "$file";; *) cat "$file";; esac) \
        | awk '/^\($/{if(!s){s=1;next}} /^\)$/{if(s)exit} s{gsub(/[()]/,""); printf "%s ", $0}' >> "$F"
      echo >> "$F"
    done
    echo "STEPDIR $k $t" >> "$F"
  done
  gzip -9 -n -c "$F" > "$HERE/refs/${STEM}_fields.ref.txt.gz"
  rm -f "$F"
  ls -l "$HERE/refs/${STEM}_fields.ref.txt.gz"
fi
