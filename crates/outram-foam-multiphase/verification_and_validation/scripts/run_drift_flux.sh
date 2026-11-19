#!/bin/bash
# gh:#1367 row 1. Rebuild reference/drift_flux_v2506.txt.gz: compiles
# driver/driftFluxDump.C together with ESI OpenFOAM-v2506's own driftFluxFoam
# sources (unmodified) against the partial v2506 build, meshes the RAS/dahl
# tutorial with blockMesh and runs the driver. Nothing of OpenFOAM is committed.
set -e
HERE=$(cd "$(dirname "$0")/.." && pwd)
W=/home/user/upstream-build/gh1367
OF=/home/user/upstream-build/OpenFOAM-v2506
S=$OF/applications/solvers/multiphase/driftFluxFoam
WITH=/home/user/upstream-build/scripts/with-of.sh
rm -rf $W/src/driftFluxDump $W/dahl
mkdir -p $W/bin $W/src/driftFluxDump/Make $HERE/reference
cp $HERE/driver/driftFluxDump.C $W/src/driftFluxDump/
cat > $W/src/driftFluxDump/Make/options <<EOF
EXE_INC = \\
    -I$S/incompressibleTwoPhaseInteractingMixture \\
    -I$S/mixtureViscosityModels/mixtureViscosityModel \\
    -I$S/mixtureViscosityModels/plastic \\
    -I$S/mixtureViscosityModels/BinghamPlastic \\
    -I$S/mixtureViscosityModels/slurry \\
    -I$S/relativeVelocityModels/relativeVelocityModel \\
    -I$S/relativeVelocityModels/simple \\
    -I$S/relativeVelocityModels/general \\
    -I\$(LIB_SRC)/finiteVolume/lnInclude \\
    -I\$(LIB_SRC)/meshTools/lnInclude \\
    -I\$(LIB_SRC)/transportModels \\
    -I\$(LIB_SRC)/transportModels/compressible/lnInclude \\
    -I\$(LIB_SRC)/transportModels/twoPhaseMixture/lnInclude

EXE_LIBS = \\
    -lfiniteVolume \\
    -lmeshTools \\
    -lincompressibleTransportModels \\
    -ltwoPhaseMixture \\
    -lcompressibleTransportModels
EOF
cat > $W/src/driftFluxDump/Make/files <<EOF
driftFluxDump.C
$S/incompressibleTwoPhaseInteractingMixture/incompressibleTwoPhaseInteractingMixture.C
$S/mixtureViscosityModels/mixtureViscosityModel/mixtureViscosityModel.C
$S/mixtureViscosityModels/mixtureViscosityModel/mixtureViscosityModelNew.C
$S/mixtureViscosityModels/plastic/plastic.C
$S/mixtureViscosityModels/BinghamPlastic/BinghamPlastic.C
$S/mixtureViscosityModels/slurry/slurry.C
$S/relativeVelocityModels/relativeVelocityModel/relativeVelocityModel.C
$S/relativeVelocityModels/simple/simple.C
$S/relativeVelocityModels/general/general.C

EXE = $W/bin/driftFluxDump
EOF
(cd $W/src/driftFluxDump && WM_NCOMPPROCS=1 $WITH wmake)
cp -r $OF/tutorials/multiphase/driftFluxFoam/RAS/dahl $W/dahl
(cd $W/dahl && $WITH blockMesh > blockMesh.log 2>&1)
(cd $W/dahl && $WITH $W/bin/driftFluxDump -case $W/dahl | grep -E '^(alpha|alpha2|const|rho_mix|rho_rvm|Udm_|tauDm_|pow10_a|exp_a)' | gzip -9n > $HERE/reference/drift_flux_v2506.txt.gz)
