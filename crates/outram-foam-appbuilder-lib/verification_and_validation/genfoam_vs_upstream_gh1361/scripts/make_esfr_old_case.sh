#!/bin/bash
# gh:#1950 S3/S4, README round 75: stage the SHIPPED Tutorials/reactorCases/3D_SmallESFR_old steady state and its transient (serial), with the
# MINIMAL repairs that the shipped files need to run at all (each found by running the real binary on the files as shipped, README round 75):
#   1. constant/regionProperties holds a git MERGE-CONFLICT (`<<<<<<<< HEAD ... ======== ... >>>>>>>>`) and no region list the binary can read
#      -> rewritten with the three regions (the same list as 3D_SmallESFR_NewSolverVerification);
#   2. system/controlDict `regionSolvers` selects `energyMechNeutroLoop multiPhysicsSolver` while `multiRegionCouplingDict_loosely` has no
#      `multiPhysicsSolvers` (and the transient's dict names `energyNeutroMechLoop`) -> the steady state selects the three regions (the commented block);
#   4. constant/neutroRegion/nuclearData includes `XSTFuel1200K` and `XSTClad1950K` but the files are named `XSTfuel1200K` and `XSTclad1950K` (the file
#      contents equal 3D_SmallESFR_NewSolverVerification's, differing in the name's case only) -> renamed (a case-sensitive file system cannot open them);
#   5. constant/neutroRegion/nuclearData has no `xsVariables` dictionary (mandatory in 652b3da: `Entry 'xsVariables' not found`) -> the block of
#      3D_SmallESFR_NewSolverVerification/legacySolver (TFuel log, TClad lin, rhoCool lin, axExp lin, radExp lin) inserted after line 39; every other line of
#      the file equals that case's;
#   3. system/fluidRegion/fvSolution PIMPLE has no `solveFluidMechanics` (onePhase `get<bool>` is mandatory) -> `solveFluidMechanics true;` for the steady state.
# usage: make_esfr_old_case.sh steady <out dir>
#        make_esfr_old_case.sh transient <out dir> <finished steady dir>
# env: TIGHT=1 (default 0) every `tolerance` 1e-14 / `relTol` 0 in the three fvSolution (GAMG kept, the round 67 protocol); INFLATE=1 gunzip the polyMesh files.
# steady: libs line (the built binary lists no legacyThermoMechanics, README round 54), endTime 200, writePrecision 17, writeControl runTime, writeInterval 200.
# transient: the `setTransient` of the shipped Allrun, verbatim (foamDictionary): eigenvalueNeutronics false, startFrom latestTime, endTime 210, deltaT 1e-05,
#   adjustTimeStep true, writeControl adjustableRunTime, writeInterval 10, regionSolvers -> `energyNeutroMechLoop multiPhysicsSolver`,
#   fluidRegion PIMPLE solveFluidMechanics false, multiRegionCouplingDict_tightly as multiRegionCouplingDict; plus writePrecision 17 and the libs line.
SRC=${GENFOAM_UPSTREAM:-/home/user/upstream-build/GeN-Foam-src}/Tutorials/reactorCases/3D_SmallESFR_old/rootCase
MODE=$1
C=$2
FD=/home/user/upstream-build/scripts/with-of.sh
rm -rf "$C"
mkdir -p "$C"
cat > /tmp/regionProperties.$$ <<'EOF'
FoamFile
{
    version     2.0;
    format      ascii;
    class       dictionary;
    object      regionProperties;
}
regions
(
    fluidRegion       (fluidRegion)
    neutroRegion       (neutroRegion)
    thermoMechanicalRegion       (thermoMechanicalRegion)
);
EOF
if [ "$MODE" = steady ]; then
  cp -r "$SRC/0" "$SRC/constant" "$SRC/system" "$C/"
  cp /tmp/regionProperties.$$ "$C/constant/regionProperties"
  sed -i '39a xsVariables\n{\n    TFuel   log;\n    TClad   lin;\n    rhoCool lin;\n    axExp   lin;\n    radExp  lin;\n}\n' "$C/constant/neutroRegion/nuclearData"
  mv "$C/constant/neutroRegion/XSTfuel1200K" "$C/constant/neutroRegion/XSTFuel1200K"
  mv "$C/constant/neutroRegion/XSTclad1950K" "$C/constant/neutroRegion/XSTClad1950K"
  mv "$C/constant/multiRegionCouplingDict_loosely" "$C/constant/multiRegionCouplingDict"
  python3 -I - "$C/system/controlDict" <<'EOF'
import re, sys
p = sys.argv[1]
t = open(p).read()
t = t.replace('/*fluidRegion     onePhase;', 'fluidRegion     onePhase;').replace('thermoMechanicalRegion legacyThermoMechanics;*/', 'thermoMechanicalRegion legacyThermoMechanics;')
t = re.sub(r'\n\s*energyMechNeutroLoop multiPhysicsSolver;', '', t)
open(p, 'w').write(t)
EOF
  sed -i 's/^\( *\)nOuterCorrectors \(.*\)$/\1nOuterCorrectors \2\n\1solveFluidMechanics true;/' "$C/system/fluidRegion/fvSolution"
  printf 'libs ("libThermoMechanicsSolvers.so");\n' >> "$C/system/controlDict"
  sed -i "s/^endTime .*/endTime 200;/; s/^writePrecision .*/writePrecision  17;/; s/^writeControl .*/writeControl runTime;/; s/^writeInterval .*/writeInterval 200;/" "$C/system/controlDict"
else
  S=$3
  cp -r "$S/constant" "$S/system" "$C/"
  LAST=$(ls "$S" | grep -E '^[0-9.]+$' | sort -g | tail -1)
  cp -r "$S/$LAST" "$C/$LAST"
  cp -r "$S/0" "$C/0"
  cd "$C" || exit 1
  $FD foamDictionary constant/neutroRegion/neutronicsProperties -entry eigenvalueNeutronics -set false
  $FD foamDictionary system/controlDict -entry startFrom -set latestTime
  $FD foamDictionary system/controlDict -entry endTime -set 210
  $FD foamDictionary system/controlDict -entry deltaT -set 1e-05
  $FD foamDictionary system/controlDict -entry adjustTimeStep -set true
  $FD foamDictionary system/controlDict -entry writeControl -set adjustableRunTime
  $FD foamDictionary system/controlDict -entry writeInterval -set 10
  $FD foamDictionary system/controlDict -entry regionSolvers/fluidRegion -remove
  $FD foamDictionary system/controlDict -entry regionSolvers/neutroRegion -remove
  $FD foamDictionary system/controlDict -entry regionSolvers/thermoMechanicalRegion -remove
  $FD foamDictionary system/controlDict -entry regionSolvers/energyNeutroMechLoop -set multiPhysicsSolver
  $FD foamDictionary system/fluidRegion/fvSolution -entry PIMPLE/solveFluidMechanics -set false
  rm constant/multiRegionCouplingDict
  mv constant/multiRegionCouplingDict_tightly constant/multiRegionCouplingDict
  cd - > /dev/null
fi
if [ "${TIGHT:-0}" = 1 ]; then
  for r in fluidRegion neutroRegion thermoMechanicalRegion; do
    sed -i -E 's/^( *)tolerance +[0-9.e+-]+;/\1tolerance        1e-14;/; s/^( *)relTol +[0-9.e+-]+;/\1relTol           0;/' "$C/system/$r/fvSolution"
  done
fi
if [ -n "$INFLATE" ]; then
  for r in fluidRegion neutroRegion thermoMechanicalRegion; do
    (cd "$C/constant/$r/polyMesh" && gunzip -f ./*.gz)
  done
fi
rm -f /tmp/regionProperties.$$
