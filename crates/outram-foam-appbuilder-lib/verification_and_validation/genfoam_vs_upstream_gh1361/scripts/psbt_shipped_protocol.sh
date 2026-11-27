#!/bin/bash
# Protocol edit for the SHIPPED PSBT Phase_Ex1_12223, boundary conditions UNCHANGED (gh:#1361 round 12, gh:#1801; run_tutorial.sh PRE_RUN hook).
# Step 1: nothing (the case is the shipped tutorial).
# Step 2: the precision / tight-tolerance protocol below.
# Never changes physics, mesh or boundary conditions:
#   writePrecision 17, write EVERY step when EVERY_STEP is set, endTime $END_TIME (default 0.6)
#   TIGHT=1: every linear-solver tolerance inside the `solvers` block -> 1e-14, relTol 0
#            (residualControl tolerances in PIMPLE are NOT touched: they set the outer-loop exit)
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
# (round 8's closed variant replaced U.liquid U.vapour p_rgh alpha.vapour here; the shipped files are kept)
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
