#!/bin/bash
# Extract refs/guided_slabs.ref.txt.gz from the two tight-protocol runs of the real
# GeN-Foam binary (run_tutorial.sh with tight_protocol.sh). Per case:
#   CASE <name> ; N <steps> ; TIME / KEFF / POWER / RESID : one value per time step
#   FLUX0 : final-time flux0 per cell ; PTARGET ; KEFF_STATE : uniform/reactorState keff
HERE="$(cd "$(dirname "$0")/.." && pwd)"
OUT=/home/user/upstream-build/gh1361/guided_slabs.ref.txt
: > "$OUT"
for c in 1 2; do
  R=/home/user/upstream-build/gh1361/runs/guided${c}_tight
  L=$R/log.GeN-Foam
  echo "CASE guided$c" >> "$OUT"
  echo "N $(grep -c '^keff_ = ' $L)" >> "$OUT"
  echo "TIME $(grep '^Time = ' $L | awk '{print $3}' | tr '\n' ' ')" >> "$OUT"
  echo "KEFF $(grep '^keff_ = ' $L | awk '{print $3}' | tr '\n' ' ')" >> "$OUT"
  echo "POWER $(grep '^power: ' $L | awk '{print $2}' | tr '\n' ' ')" >> "$OUT"
  echo "RESID $(grep '^neutroResidual (neutroIter: 1)' $L | awk '{print $4}' | tr '\n' ' ')" >> "$OUT"
  T=$(ls $R | grep -E '^[0-9.e+-]+$' | sort -g | tail -1)
  echo "FINALTIME $T" >> "$OUT"
  echo "FLUX0 $(awk '/^\(/{f=1;next} /^\)/{f=0} f{print $1}' $R/$T/neutroRegion/flux0 | head -n 100000 | tr '\n' ' ')" >> "$OUT"
done
gzip -9 -n -c "$OUT" > "$HERE/refs/guided_slabs.ref.txt.gz"
ls -l "$HERE/refs/guided_slabs.ref.txt.gz"
