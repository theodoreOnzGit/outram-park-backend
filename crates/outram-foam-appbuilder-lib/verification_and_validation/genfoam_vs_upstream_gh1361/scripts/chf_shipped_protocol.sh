#!/bin/bash
# Protocol edit for the SHIPPED 1D_CHF imposedPower (gh:#1361 round 12; gh:#1815, gh:#1801; run_tutorial.sh PRE_RUN hook).
# Boundary conditions, mesh, closures, power model and its powerTimeProfile are the shipped files.
#   writePrecision 17; write every WRITE_INTERVAL seconds (default 1; adjustableRunTime: the step is shortened to land on it);
#   endTime $END_TIME (default 13); every linear-solver tolerance inside `solvers` -> 1e-14, relTol 0
#   (residualControl in PIMPLE is NOT touched).
sed -i 's/^writePrecision .*/writePrecision  17;/' system/controlDict
sed -i "s/^endTime .*/endTime         ${END_TIME:-13};/" system/controlDict
sed -i "s/^writeInterval .*/writeInterval   ${WRITE_INTERVAL:-1};/" system/controlDict
f=system/fluidRegion/fvSolution
awk 'BEGIN{ins=0} /^solvers/{ins=1} /^PIMPLE/{ins=0}
     { if(ins && $1=="tolerance"){sub(/tolerance .*/,"tolerance       1e-14;")}
       if(ins && $1=="relTol"){sub(/relTol .*/,"relTol          0;")}
       print }' "$f" > "$f.new" && mv "$f.new" "$f"
