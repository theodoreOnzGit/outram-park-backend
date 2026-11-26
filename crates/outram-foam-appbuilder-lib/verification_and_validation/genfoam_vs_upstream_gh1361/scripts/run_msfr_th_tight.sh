#!/bin/bash
# gh:#1812 / gh:#1361 round 18: the 2D_MSFR `steadyStateTH` stage (onePhase, cellCentered, solveFluidMechanics true),
# run with the REAL unmodified GeN-Foam binary at tight settings, for the first steps only.
# Usage: run_msfr_th_tight.sh <name> [RUN_ROOT]    env: END_TIME (default 0.06), TURB=on|off, PSOLVER=pcg|gamg, WRITE=step|adjustable, TOLERANCE=tight|shipped
# Allrun's setSteadyStateTH is replayed with sed/perl (foamDictionary is not built here). The ONLY edits beyond
# Allrun's: every linear-solver tolerance 1e-14 / relTol 0; p_rgh GAMG -> PCG/DIC (GAMG does not reach 1e-14: a
# solver-only change, as in round 16); write every step (writeControl timeStep) at writePrecision 17 (set in the
# source rootCase); endTime shortened; with TURB=off `turbulence false` (the round-17 bisect: isolates UEqn/pEqn).
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
NAME=$1
R=${2:-/home/user/upstream-build/gh1812/runs}
cd "$R" || exit 1
rm -rf "$NAME"; cp -r rootCase "$NAME"; C=$NAME
ctl() { sed -i "s/^$2 .*/$2 $3;/" "$1/system/controlDict"; }
blk() { perl -0pi -e "s/^$2\s*\{[^}]*\}/$2\n{\n$3\n}/m" "$1/system/controlDict"; }
rm -r $C/0/neutroRegion $C/constant/neutroRegion $C/system/neutroRegion $C/0/uniform $C/constant/fluidRegion/polyMeshFine
ctl $C endTime ${END_TIME:-0.06}; ctl $C deltaT 1e-03; ctl $C adjustTimeStep true
if [ "${WRITE:-step}" = adjustable ]; then ctl $C writeInterval 1; else ctl $C writeControl timeStep; ctl $C writeInterval 1; fi   # gh:#1841 round 21: WRITE=adjustable keeps the rootCase's adjustableRunTime (Allrun: writeInterval 1) for the 15 s run
blk $C regionSolvers "    fluidRegion onePhase;"; blk $C removeBaffles "    fluidRegion false;"
cp $C/constant/multiRegionCouplingDict_steadyStateTH $C/constant/multiRegionCouplingDict
rm $C/constant/multiRegionCouplingDict_*
f=$C/system/fluidRegion/fvSolution
if [ "${TOLERANCE:-tight}" = tight ]; then sed -i -E 's/^( *)tolerance +[0-9.e+-]+; *$/\1tolerance        1e-14;/; s/^( *)relTol +[0-9.e+-]+; *(\/\/.*)?$/\1relTol           0;/' $f; fi
if [ "${PSOLVER:-pcg}" = pcg ]; then
  perl -0pi -e 's/solver\s+GAMG;\s*smoother\s+DIC;/solver          PCG;\n        preconditioner  DIC;/' $f
fi
if [ "${TURB:-on}" = off ]; then
  sed -i 's/^\( *\)turbulence .*true;/\1turbulence      false;/' $C/constant/fluidRegion/turbulenceProperties
fi
S=$(date +%s)
GeN-Foam -case $C > $C/log.GeN-Foam 2>&1
echo "$NAME: GeN-Foam exit $? in $(( $(date +%s)-S )) s"
tail -3 $C/log.GeN-Foam
