#!/bin/bash
# gh:#1905 / gh:#1361 round 80: references of a `run_msr1d_fluid_r80.sh` run.
# usage: extract_msr1d_fluid_r80.sh <run dir> <out prefix>
#   <prefix>.ref.txt      `F <time> <name> v...` (internal field; vectors x y z per cell; a uniform field is absent) of every written
#                          time directory except 0, fields U p p_rgh T k epsilon nut alphat alphaPhi htc
#   <prefix>_log.ref.txt  PRGH/K/EPS/H/TSTAT lines (extract_msfr_th_log.sh), `DTNEXT <step> <deltaT>` and `STEPTIME <step> <time name>`
#                          (from the log's `Time = ` lines, so every step has its name even when it was not written)
R=$1; OUT=$2
HERE=$(dirname "$0")
TIMES=$(ls "$R" | grep -E '^[0-9.e+-]+$' | grep -v '^0$' | sort -g | tr '\n' ' ')
bash "$HERE/extract_msfr_th_fields.sh" "$R" "$OUT.ref.txt" "$TIMES" "U p p_rgh T k epsilon nut alphat alphaPhi htc"
bash "$HERE/extract_msfr_th_log.sh" "$R/log.GeN-Foam" "$OUT""_log.ref.txt"
grep "^deltaT = " "$R/log.GeN-Foam" | awk '{n++; print "DTNEXT", n, $3}' >> "$OUT""_log.ref.txt"
grep "^Time = " "$R/log.GeN-Foam" | awk '{n++; print "STEPTIME", n, $3}' >> "$OUT""_log.ref.txt"
