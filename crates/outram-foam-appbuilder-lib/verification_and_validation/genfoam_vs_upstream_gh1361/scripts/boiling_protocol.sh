#!/bin/bash
# Protocol edit for 1D_boiling (gh:#1361 round 3, run_tutorial.sh PRE_RUN hook).
# Never changes physics, mesh or boundary conditions:
#   alpha solver MULES -> implicitUpwind   (the one solver the port has; rows 1 of the 2026-11-11 table)
#   writePrecision 17, write EVERY step (writeControl timeStep) so steps can be compared one by one
#   endTime $END_TIME
#   TIGHT=1: every linear-solver tolerance inside the `solvers` block -> 1e-14, relTol 0
#            (residualControl tolerances in PIMPLE are NOT touched: they set the outer-loop exit)
sed -i 's/^writePrecision .*/writePrecision  17;/' system/controlDict
sed -i "s/^endTime .*/endTime         ${END_TIME:-12};/" system/controlDict
if [ -n "$EVERY_STEP" ]; then
  sed -i 's/^writeControl .*/writeControl    timeStep;/; s/^writeInterval .*/writeInterval   1;/' system/controlDict
fi
f=system/fluidRegion/fvSolution
sed -i 's/solver          "MULES";/solver          implicitUpwind; solvePhase vapour;/' "$f"
sed -i '/adjustSubCycles/d; /alphaMaxCo/d' "$f"
if [ -n "$TIGHT" ]; then
  awk 'BEGIN{ins=0} /^solvers/{ins=1} /^PIMPLE/{ins=0}
       { if(ins && $1=="tolerance"){sub(/tolerance .*/,"tolerance       1e-14;")}
         if(ins && $1=="relTol"){sub(/relTol .*/,"relTol          0;")}
         print }' "$f" > "$f.new" && mv "$f.new" "$f"
fi
# NOUTER=k: PIMPLE nOuterCorrectors k (fields after k outer iterations of step 1; diagnostic for the first divergent operation)
[ -n "$NOUTER" ] && sed -i "s/^\( *\)nOuterCorrectors .*/\1nOuterCorrectors    $NOUTER;/" system/fluidRegion/fvSolution
# NCORR=k: PIMPLE nCorrectors k (the PISO correctors per outer iteration; same purpose)
[ -n "$NCORR" ] && sed -i "s/^\( *\)nCorrectors .*/\1nCorrectors         $NCORR;/" system/fluidRegion/fvSolution
sed -i 's/calulated/calculated/' 0/fluidRegion/* # upstream typo: ESI rejects 'calulated' (alpha.liquid), the port ignores patch types it does not need
