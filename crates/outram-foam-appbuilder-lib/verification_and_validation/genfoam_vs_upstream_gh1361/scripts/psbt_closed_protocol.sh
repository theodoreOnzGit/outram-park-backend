#!/bin/bash
# Protocol edit for the CLOSED variant of PSBT Phase_Ex1_12223 (gh:#1361 round 8; run_tutorial.sh PRE_RUN hook).
# Step 1: the four boundary-condition files of tests/data/genfoam_psbt_closed_gh1361 replace the shipped ones
#         (the case is otherwise unmodified; see that directory's PROVENANCE.md).
# Step 2: the precision / tight-tolerance protocol below.
# Never changes physics, mesh or boundary conditions:
#   writePrecision 17, write EVERY step when EVERY_STEP is set, endTime $END_TIME (default 0.6)
#   TIGHT=1: every linear-solver tolerance inside the `solvers` block -> 1e-14, relTol 0
#            (residualControl tolerances in PIMPLE are NOT touched: they set the outer-loop exit)
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
for f in U.liquid U.vapour p_rgh alpha.vapour; do cp "$HERE/tests/data/genfoam_psbt_closed_gh1361/0/fluidRegion/$f" 0/fluidRegion/$f; done
sed -i 's/^writePrecision .*/writePrecision  17;/' system/controlDict
sed -i "s/^endTime .*/endTime         ${END_TIME:-0.6};/" system/controlDict
if [ -n "$EVERY_STEP" ]; then
  sed -i 's/^writeControl .*/writeControl    timeStep;/; s/^writeInterval .*/writeInterval   1;/' system/controlDict
fi
f=system/fluidRegion/fvSolution
if [ -n "$TIGHT" ]; then
  awk 'BEGIN{ins=0} /^solvers/{ins=1} /^PIMPLE/{ins=0}
       { if(ins && $1=="tolerance"){sub(/tolerance .*/,"tolerance       1e-14;")}
         if(ins && $1=="relTol"){sub(/relTol .*/,"relTol          0;")}
         print }' "$f" > "$f.new" && mv "$f.new" "$f"
fi
