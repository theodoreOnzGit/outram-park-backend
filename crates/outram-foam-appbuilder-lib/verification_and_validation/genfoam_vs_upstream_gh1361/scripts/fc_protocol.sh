#!/bin/bash
# gh:#1878 round 38: protocol edit for the face-centred onePhase cases, run_case.sh PRE_RUN hook (cwd = the work dir).
# Never changes physics, mesh or boundary conditions:
#   writePrecision 17, write EVERY step, endTime $END_TIME
#   TIGHT=1 : every `tolerance` -> 1e-14 and `relTol` -> 0 inside `solvers`
#   PSOLVER=pcg : `solver GAMG; smoother DIC;` -> `solver PCG; preconditioner DIC;` and every `maxIter` line inside `solvers` removed
#                 (PCG at 1e-14 needs more than the 25 sweeps shipped for GAMG; default maxIter 1000 applies in port and upstream)
#   NOUTER, NCORR, NNONORTH : override the PIMPLE counts
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
if [ "${PSOLVER:-gamg}" = pcg ]; then
  perl -0pi -e 's/solver\s+GAMG;\s*smoother\s+DIC;/solver          PCG;\n        preconditioner  DIC;/g' "$f"
  awk 'BEGIN{ins=0} /^solvers/{ins=1} /^PIMPLE/{ins=0} { if(ins && $1=="maxIter") next; print }' "$f" > "$f.new" && mv "$f.new" "$f"
fi
[ -n "$NOUTER" ] && sed -i "s/^\( *\)nOuterCorrectors .*/\1nOuterCorrectors    $NOUTER;/" "$f"
[ -n "$NCORR" ] && sed -i "s/^\( *\)nCorrectors .*/\1nCorrectors    $NCORR;/" "$f"
[ -n "$NNONORTH" ] && sed -i "s/^\( *\)nNonOrthogonalCorrectors .*/\1nNonOrthogonalCorrectors    $NNONORTH;/" "$f"
exit 0
