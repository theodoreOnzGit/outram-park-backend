#!/bin/bash
# gh:#1361 round 6: the 2D_MSFR tutorial's Allrun chain without foamDictionary (not built here).
# Usage: run_msfr_chain.sh <stage: steadyStateTH|steadyStateEN|transient> [RUN_ROOT]
# Replays upstream's Allrun setSteadyStateTH / setSteadyStateEN / setTransient with sed/perl, then
# runs the real GeN-Foam binary serially, logging to <stage>/log.GeN-Foam.
# The ONLY edits beyond Allrun's own: writePrecision 17 in rootCase (done by the caller before
# cloning) and, for the transient stage, PROTOCOL=tight (every linear solver 1e-14 / relTol 0, write
# every step, END_TIME) so the 50-step comparison is not solver-noise limited.
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
STAGE=$1
R=${2:-/home/user/upstream-build/gh1361/runs/msfr}
cd "$R" || exit 1
endTimeTH=15; endTimeEN=165; endTimeTransient=175
ctl() { sed -i "s/^$2 .*/$2 $3;/" "$1/system/controlDict"; }
blk() { # blk <case> <name> <body>: replace the top-level "name { ... }" block of controlDict
  perl -0pi -e "s/^$2\s*\{[^}]*\}/$2\n{\n$3\n}/m" "$1/system/controlDict"; }
if [ "$STAGE" = steadyStateTH ]; then
  rm -rf steadyStateTH; cp -r rootCase steadyStateTH; C=steadyStateTH
  rm -r $C/0/neutroRegion $C/constant/neutroRegion $C/system/neutroRegion $C/0/uniform $C/constant/fluidRegion/polyMeshFine
  ctl $C endTime $endTimeTH; ctl $C deltaT 1e-03; ctl $C adjustTimeStep true; ctl $C writeInterval 1
  blk $C regionSolvers "    fluidRegion onePhase;"; blk $C removeBaffles "    fluidRegion false;"
  cp $C/constant/multiRegionCouplingDict_steadyStateTH $C/constant/multiRegionCouplingDict
  rm $C/constant/multiRegionCouplingDict_*
elif [ "$STAGE" = steadyStateEN ]; then
  rm -rf steadyStateEN; cp -r rootCase steadyStateEN; C=steadyStateEN
  cp -r steadyStateTH/$endTimeTH $C/$endTimeTH; cp -r $C/0/neutroRegion $C/$endTimeTH/; cp -r $C/0/uniform $C/$endTimeTH/
  rm -r $C/constant/neutroRegion/polyMeshFine $C/constant/fluidRegion/polyMeshFine
  ctl $C startTime $endTimeTH; ctl $C endTime $endTimeEN; ctl $C deltaT 0.25; ctl $C adjustTimeStep false; ctl $C writeInterval 50
  blk $C regionSolvers "    fluidRegion onePhase;\n    neutroRegion diffusionNeutronics;"
  blk $C removeBaffles "    fluidRegion false;\n    neutroRegion false;"
  sed -i 's/^\( *\)solveFluidMechanics .*/\1solveFluidMechanics false;/' $C/system/fluidRegion/fvSolution
  cp $C/constant/multiRegionCouplingDict_steadyStateEN $C/constant/multiRegionCouplingDict
  rm $C/constant/multiRegionCouplingDict_*
elif [ "$STAGE" = transient ]; then
  rm -rf transient; cp -r rootCase transient; C=transient
  cp -r steadyStateEN/$endTimeEN $C/$endTimeEN
  rm -r $C/constant/neutroRegion/polyMeshFine $C/constant/fluidRegion/polyMeshFine
  sed -i 's/^eigenvalueNeutronics .*/eigenvalueNeutronics false;/' $C/constant/neutroRegion/neutronicsProperties
  ctl $C startTime $endTimeEN; ctl $C endTime ${END_TIME:-$endTimeTransient}; ctl $C deltaT 1e-03; ctl $C adjustTimeStep true; ctl $C writeInterval 10
  blk $C regionSolvers "    energyNeutroLoop multiPhysicsSolver;"
  perl -0pi -e 's/^removeBaffles\s*\{[^}]*\}\n//m' $C/system/controlDict
  sed -i 's/^\( *\)solveFluidMechanics .*/\1solveFluidMechanics false;/' $C/system/fluidRegion/fvSolution
  cp $C/constant/multiRegionCouplingDict_transient $C/constant/multiRegionCouplingDict
  rm $C/constant/multiRegionCouplingDict_*
  if [ "$PROTOCOL" = tight ]; then
    for f in $C/system/*/fvSolution; do
      # only plain `tolerance x;` / `relTol x;` lines: the neutroRegion file has a /* ... relTol 0.01;*/ block
      sed -i -E 's/^( *)tolerance +[0-9.e+-]+; *$/\1tolerance        1e-14;/; s/^( *)relTol +[0-9.e+-]+; *(\/\/.*)?$/\1relTol           0;/' "$f"
    done
    sed -i 's/^writeControl .*/writeControl    timeStep;/; s/^writeInterval .*/writeInterval   1;/' $C/system/controlDict
  fi
else echo "unknown stage $STAGE"; exit 2; fi
S=$(date +%s)
GeN-Foam -case $C > $C/log.GeN-Foam 2>&1
echo "$STAGE: GeN-Foam exit $? in $(( $(date +%s)-S )) s"
tail -3 $C/log.GeN-Foam
