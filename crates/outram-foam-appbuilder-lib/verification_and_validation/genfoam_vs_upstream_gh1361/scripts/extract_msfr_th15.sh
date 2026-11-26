#!/bin/bash
# gh:#1361 round 21 (gh:#1841): the references of the 15 s `steadyStateTH` stage from a
# `WRITE=adjustable END_TIME=15 run_msfr_th_tight.sh` run.
#   refs/${NAME}_log.ref.txt.gz     N <steps>;  DT <the `deltaT = ` printed after each step>
#   refs/msfr_th15_fields.ref.txt.gz  F <time> <name> v1 v2 ...  (a vector flattened x y z per cell; surface fields: internal faces)
#                                     all nine fields + alphaRhoPhi, htc, T.fixedTemperature at 15 and 5; U, p_rgh, epsilon, T at 1 and 10
# usage: extract_msfr_th15.sh <run dir> [output stem = msfr_th15]
HERE="$(cd "$(dirname "$0")/.." && pwd)"
R=$1
NAME=${2:-msfr_th15}   # output stem (round 21: msfr_th15 = tight, msfr_th15_shipped = shipped GAMG)
L=$R/log.GeN-Foam
OUT=$(mktemp)
echo "N $(grep -c '^Time = ' "$L")" > "$OUT"
echo "DT $(grep '^deltaT = ' "$L" | awk '{print $3}' | tr '\n' ' ')" >> "$OUT"
gzip -9 -n -c "$OUT" > "$HERE/refs/${NAME}_log.ref.txt.gz"
rm -f "$OUT"
F1=$(mktemp); F2=$(mktemp); F3=$(mktemp)
bash "$(dirname "$0")/extract_msfr_th_fields.sh" "$R" "$F1" "15 5" "U p p_rgh T k epsilon nut alphat alphaPhi alphaRhoPhi htc T.fixedTemperature"
bash "$(dirname "$0")/extract_msfr_th_fields.sh" "$R" "$F2" "1 10" "U p_rgh epsilon T"
# one file: the header line of the second run is dropped
(cat "$F1"; grep -v '^TIMEPRINT' "$F2") | gzip -9 -n -c > "$HERE/refs/${NAME}_fields.ref.txt.gz"
rm -f "$F1" "$F2" "$F3"
ls -l "$HERE"/refs/${NAME}_*.ref.txt.gz
