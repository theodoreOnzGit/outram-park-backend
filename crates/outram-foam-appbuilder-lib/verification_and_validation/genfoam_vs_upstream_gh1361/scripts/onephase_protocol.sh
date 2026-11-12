#!/bin/bash
# Protocol edit for the onePhase channel (gh:#1361 round 4), run_case.sh PRE_RUN hook.
# Never changes physics, mesh or boundary conditions:
#   writePrecision 17, write EVERY step, endTime $END_TIME (default 1)
#   TIGHT=1: every linear-solver tolerance inside `solvers` -> 1e-14, relTol 0
sed -i 's/^writePrecision .*/writePrecision  17;/' system/controlDict
sed -i "s/^endTime .*/endTime         ${END_TIME:-1};/" system/controlDict
sed -i 's/^writeControl .*/writeControl    timeStep;/; s/^writeInterval .*/writeInterval   1;/' system/controlDict
f=system/fluidRegion/fvSolution
if [ -n "$TIGHT" ]; then
  awk 'BEGIN{ins=0} /^solvers/{ins=1} /^PIMPLE/{ins=0}
       { if(ins && $1=="tolerance"){sub(/tolerance .*/,"tolerance       1e-14;")}
         if(ins && $1=="relTol"){sub(/relTol .*/,"relTol          0;")}
         print }' "$f" > "$f.new" && mv "$f.new" "$f"
fi
[ -n "$NOUTER" ] && sed -i "s/^\( *\)nOuterCorrectors .*/\1nOuterCorrectors    $NOUTER;/" "$f"
[ -n "$NCORR" ] && sed -i "s/^\( *\)nCorrectors .*/\1nCorrectors    $NCORR;/" "$f"
exit 0
