#!/bin/bash
# Extract refs/sn_slab.ref.txt.gz from the run of run_tutorial.sh (sn_slab_protocol.sh) named sn_slab:
#   KEFF / POWER / RESID : one value per pseudo-time step (keff_, `pow:` and the step's largest
#   initial residual from `Final neutroResidual ...`), FINALTIME, FLUX0: scalar flux0 per cell at
#   the last step, as written (writePrecision 17).
HERE="$(cd "$(dirname "$0")/.." && pwd)"
R=/home/user/upstream-build/gh1361/runs/sn_slab
L=$R/log.GeN-Foam
OUT=/home/user/upstream-build/gh1361/sn_slab.ref.txt
echo "CASE sn_slab" > "$OUT"
echo "N $(grep -c '^keff_ = ' $L)" >> "$OUT"
echo "KEFF $(grep '^keff_ = ' $L | awk '{print $3}' | tr '\n' ' ')" >> "$OUT"
echo "POWER $(grep '^Final neutroResidual' $L | sed 's/.*pow://' | tr '\n' ' ')" >> "$OUT"
echo "RESID $(grep '^Final neutroResidual' $L | awk '{print $7}' | tr '\n' ' ')" >> "$OUT"
T=$(ls $R | grep -E '^[0-9.e+-]+$' | sort -g | tail -1)
echo "FINALTIME $T" >> "$OUT"
echo "FLUX0 $(awk '/^\(/{f=1;next} /^\)/{f=0} f{print $1}' $R/$T/neutroRegion/flux0 | head -n 100000 | tr '\n' ' ')" >> "$OUT"
gzip -9 -n -c "$OUT" > "$HERE/refs/sn_slab.ref.txt.gz"
ls -l "$HERE/refs/sn_slab.ref.txt.gz"
