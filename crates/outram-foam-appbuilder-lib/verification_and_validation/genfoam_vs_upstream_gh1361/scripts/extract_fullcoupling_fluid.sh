#!/bin/bash
# gh:#1913 / gh:#1361 round 55: references of the fluid-only 2D_fullCoupling run (`make_fullcoupling_fluid_case.sh` + GeN-Foam).
# usage: extract_fullcoupling_fluid.sh <run dir> <out prefix>
#   <prefix>.ref.txt      fields (`F <time> <name> v...`, internal field) of steps 1-100, every 10th step after, and the last  (env FIELDS overrides the field list: round 65 adds `k epsilon nut alphat`)
#   <prefix>_log.ref.txt  one line per linear solve and per `T (avg min max)` (see extract_msfr_th_log.sh), plus `DT <step> <dt>` lines
R=$1; OUT=$2
HERE=$(dirname "$0")
TIMES=$(ls "$R" | grep -E '^[0-9.]+$' | grep -v '^0$' | sort -g | awk '{n++; t[n]=$1} END{for(i=1;i<=n;i++) if(i<=100 || i%10==0 || i==n) print t[i]}' | tr '\n' ' ')
bash "$HERE/extract_msfr_th_fields.sh" "$R" "$OUT.ref.txt" "$TIMES" "${FIELDS:-U p p_rgh T alphaPhi htc T.passiveStructure T.fuelAvForNeutronics}"
ls "$R" | grep -E '^[0-9.]+$' | grep -v '^0$' | sort -g | awk '{n++; print "STEPTIME", n, $1}' > "$OUT.steps.txt"
bash "$HERE/extract_msfr_th_log.sh" "$R/log.GeN-Foam" "$OUT""_log.ref.txt"
grep "^deltaT = " "$R/log.GeN-Foam" | awk '{n++; print "DTNEXT", n, $3}' >> "$OUT""_log.ref.txt"
cat "$OUT.steps.txt" >> "$OUT""_log.ref.txt"; rm "$OUT.steps.txt"
