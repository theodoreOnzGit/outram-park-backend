#!/bin/bash
# gh:#1856 / gh:#1914 / gh:#1361 round 56: stage the WHOLE shipped Tutorials/featureCases/2D_fullCoupling `steadyState` stage (fluidRegion onePhase +
# neutroRegion diffusionNeutronics eigenvalue + thermoMechanicalRegion legacyThermoMechanics, meshDeformation of neutroRegion)
# exactly as the tutorial's Allrun does it (setSteadyState: eigenvalueNeutronics true, model diffusionNeutronics, endTime 100, deltaT 1,
# multiRegionCouplingDict_steadyState), plus the tight protocol of rounds 18/19/55 and a full-precision write of every step.
# usage: make_fullcoupling_coupled_case.sh <out case dir>
# The ONLY edits to the shipped files:
#   neutronicsProperties: `model diffusionNeutronics;` appended, eigenvalueNeutronics true   (Allrun's foamDictionary calls)
#   controlDict         : startTime 0, endTime 100, deltaT 1, writeControl timeStep, writeInterval 1, writePrecision 17;
#                         libs ("libThermoMechanicsSolvers.so") (the built binary lists no legacyThermoMechanics: README round 54)
#   fvSolution (3 regions): every tolerance 1e-14, relTol 0; p_rgh/p_rghFinal GAMG -> PCG/DIC (the named deviation of rounds 17/21/44)
#   constant/multiRegionCouplingDict = multiRegionCouplingDict_steadyState
# Run: LD_LIBRARY_PATH=/home/user/upstream-build/gh1781/lib:$LD_LIBRARY_PATH with-of.sh GeN-Foam -initializeMappedFields -case <dir>
# then the same with `GeN-Foam -case <dir>` (scripts/run_fullcoupling_coupled.sh).
SRC=${GENFOAM_UPSTREAM:-/home/user/upstream-build/GeN-Foam-src}/Tutorials/featureCases/2D_fullCoupling/rootCase
C=$1
rm -rf "$C"
mkdir -p "$C"
cp -r "$SRC/0" "$SRC/constant" "$SRC/system" "$C/"
ctl() { sed -i "s/^$2 .*/$2 $3;/" "$1/system/controlDict"; }
printf 'model diffusionNeutronics;\n' >> $C/constant/neutroRegion/neutronicsProperties
sed -i 's/^eigenvalueNeutronics .*/eigenvalueNeutronics true;/' $C/constant/neutroRegion/neutronicsProperties
ctl $C startTime 0
ctl $C endTime 100
ctl $C deltaT 1
ctl $C writeControl timeStep
ctl $C writeInterval 1
# gh:#1919 round 60: ADJ=1 follows the tutorial's Allrun time control (writeControl adjustableRunTime, writeInterval 100): the last step lands on t = 100
if [ -n "$ADJ" ]; then ctl $C writeControl adjustableRunTime; ctl $C writeInterval 100; fi
ctl $C writePrecision 17
printf 'libs ("libThermoMechanicsSolvers.so");\n' >> $C/system/controlDict
cp $C/constant/multiRegionCouplingDict_steadyState $C/constant/multiRegionCouplingDict
for r in fluidRegion neutroRegion thermoMechanicalRegion; do
  f=$C/system/$r/fvSolution
  sed -i -E 's/^( *)tolerance +[0-9.e+-]+; *$/\1tolerance        1e-14;/; s/^( *)relTol +[0-9.e+-]+; *(\/\/.*)?$/\1relTol           0;/' $f
  perl -0pi -e 's/solver\s+GAMG;\s*smoother\s+DIC;/solver          PCG;\n        preconditioner  DIC;/g' $f
done
