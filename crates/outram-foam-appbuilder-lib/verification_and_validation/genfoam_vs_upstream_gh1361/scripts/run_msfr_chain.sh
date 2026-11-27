#!/bin/bash
# gh:#1361 round 6: the 2D_MSFR tutorial's Allrun chain without foamDictionary (not built here).
# Usage: run_msfr_chain.sh <stage: steadyStateTH|steadyStateEN|transient> [RUN_ROOT]
# gh:#1945 round 79: NEUTRO_FVSOLUTION=<file> replaces the EN stage's neutroRegion fvSolution.
# gh:#1951 round 77: TH_END overrides endTimeTH (default 15; the dir steadyStateTH/$TH_END must exist), EN_END and END_TIME as before.
# Replays upstream's Allrun setSteadyStateTH / setSteadyStateEN / setTransient with sed/perl, then
# runs the real GeN-Foam binary serially, logging to <stage>/log.GeN-Foam.
# The ONLY edits beyond Allrun's own: writePrecision 17 in rootCase (done by the caller before
# cloning) and, for the transient stage, PROTOCOL=tight (every linear solver 1e-14 / relTol 0, write
# every step, END_TIME) so the 50-step comparison is not solver-noise limited.
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
STAGE=$1
R=${2:-/home/user/upstream-build/gh1361/runs/msfr}
cd "$R" || exit 1
endTimeTH=${TH_END:-15}; endTimeEN=${EN_END:-165}; endTimeTransient=175   # gh:#1814 round 14: EN_END, EN_WRITE override the EN end time and write interval (the light chain uses 20 and 20)
tighten_solvers() {
  for f in $1/system/*/fvSolution; do
    # only plain `tolerance x;` / `relTol x;` lines: the neutroRegion file has a /* ... relTol 0.01;*/ block
    sed -i -E 's/^( *)tolerance +[0-9.e+-]+; *$/\1tolerance        1e-14;/; s/^( *)relTol +[0-9.e+-]+; *(\/\/.*)?$/\1relTol           0;/' "$f"
  done
}
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
  ctl $C startTime $endTimeTH; ctl $C endTime $endTimeEN; ctl $C deltaT 0.25; ctl $C adjustTimeStep false; ctl $C writeInterval ${EN_WRITE:-50}
  blk $C regionSolvers "    fluidRegion onePhase;\n    neutroRegion diffusionNeutronics;"
  blk $C removeBaffles "    fluidRegion false;\n    neutroRegion false;"
  sed -i 's/^\( *\)solveFluidMechanics .*/\1solveFluidMechanics false;/' $C/system/fluidRegion/fvSolution
  cp $C/constant/multiRegionCouplingDict_steadyStateEN $C/constant/multiRegionCouplingDict
  rm $C/constant/multiRegionCouplingDict_*
  # gh:#1814 round 11: PROTOCOL=tight for the EN stage tightens the linear solvers only (writeInterval stays 50:
  # a write per step of 600 steps would not fit the disk); the per-step numbers come from the log at precision 17.
  # gh:#1945 round 79: NEUTRO_FVSOLUTION replaces system/neutroRegion/fvSolution (the one-edit GAMG variant cases/msfr_neutro/fvSolution.gamg)
  if [ -n "$NEUTRO_FVSOLUTION" ]; then cp "$NEUTRO_FVSOLUTION" $C/system/neutroRegion/fvSolution; fi
  if [ "$PROTOCOL" = tight ]; then tighten_solvers "$C"; fi
elif [ "$STAGE" = transient ]; then
  rm -rf transient; cp -r rootCase transient; C=transient
  cp -r steadyStateEN/$endTimeEN $C/$endTimeEN
  rm -r $C/constant/neutroRegion/polyMeshFine $C/constant/fluidRegion/polyMeshFine
  sed -i 's/^eigenvalueNeutronics .*/eigenvalueNeutronics false;/' $C/constant/neutroRegion/neutronicsProperties
  ctl $C startTime $endTimeEN; ctl $C endTime ${END_TIME:-$endTimeTransient}; ctl $C deltaT 1e-03; ctl $C adjustTimeStep true; ctl $C writeInterval 10
  blk $C regionSolvers "    energyNeutroLoop multiPhysicsSolver;"
  perl -0pi -e 's/^removeBaffles\s*\{[^}]*\}\n//m' $C/system/controlDict
  sed -i 's/^\( *\)solveFluidMechanics .*/\1solveFluidMechanics false;/' $C/system/fluidRegion/fvSolution
  # gh:#1814 round 14: COUPLING_ITERS overrides maxIterations of the coupling loop (the light chain uses 1)
  if [ -n "$COUPLING_ITERS" ]; then sed -i "s/^\( *\)maxIterations .*/\1maxIterations $COUPLING_ITERS;/" $C/constant/multiRegionCouplingDict_transient; fi
  # gh:#1813 round 22: MIN_RESIDUAL overrides minResidual of the coupling loop (an entry of upstream's own dictionary)
  if [ -n "$MIN_RESIDUAL" ]; then sed -i "s/^\( *\)minResidual .*/\1minResidual $MIN_RESIDUAL;/" $C/constant/multiRegionCouplingDict_transient; fi
  # gh:#1813 round 22: HX_TEMPERATURE_PROFILE=1 adds a `temperatureTimeProfile` (a table of scaling factors, the form of
  # fixedTemperature.H's own example) to the `hx` fixedTemperature powerModel: 1 until 165 s, then a linear ramp to 0.95 at 175 s
  if [ "$HX_TEMPERATURE_PROFILE" = 1 ]; then
    perl -0pi -e 's/(type\s+fixedTemperature;\n\s+volumetricArea\s+200;\n\s+T\s+900;\n)/$1            temperatureTimeProfile\n            {\n                type            table;\n                startTime       0;\n                table           table\n                (\n                    (0 1)\n                    (165 1)\n                    (175 0.95)\n                );\n            }\n/' $C/constant/fluidRegion/phaseProperties
    grep -c temperatureTimeProfile $C/constant/fluidRegion/phaseProperties
  fi
  cp $C/constant/multiRegionCouplingDict_transient $C/constant/multiRegionCouplingDict
  rm $C/constant/multiRegionCouplingDict_*
  if [ "$PROTOCOL" = tight ]; then
    tighten_solvers "$C"
    sed -i 's/^writeControl .*/writeControl    timeStep;/; s/^writeInterval .*/writeInterval   1;/' $C/system/controlDict
  fi
else echo "unknown stage $STAGE"; exit 2; fi
S=$(date +%s)
GeN-Foam -case $C > $C/log.GeN-Foam 2>&1
echo "$STAGE: GeN-Foam exit $? in $(( $(date +%s)-S )) s"
tail -3 $C/log.GeN-Foam
