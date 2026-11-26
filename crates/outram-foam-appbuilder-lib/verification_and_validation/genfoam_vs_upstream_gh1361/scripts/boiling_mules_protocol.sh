#!/bin/bash
# Protocol edit for 1D_boiling AS SHIPPED, alpha solver MULES (gh:#1361 round 7, run_tutorial.sh PRE_RUN hook).
# Unlike boiling_protocol.sh it keeps `alpha { solver "MULES"; adjustSubCycles true; alphaMaxCo 0.25; }`.
# Never changes physics, mesh or boundary conditions:
#   writePrecision 17, write EVERY step when EVERY_STEP is set, endTime $END_TIME (default 12)
#   TIGHT=1: every linear-solver tolerance inside the `solvers` block -> 1e-14, relTol 0
#            (residualControl tolerances in PIMPLE are NOT touched: they set the outer-loop exit)
sed -i 's/^writePrecision .*/writePrecision  17;/' system/controlDict
sed -i "s/^endTime .*/endTime         ${END_TIME:-12};/" system/controlDict
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
sed -i 's/calulated/calculated/' 0/fluidRegion/* # upstream typo: ESI rejects 'calulated' (alpha.liquid)
