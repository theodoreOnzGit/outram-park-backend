#!/bin/bash
# gh:#1363 row 4. Build the two ESI v2506 coupled-temperature boundary
# conditions (src/thermoTools, NOT part of the built closure: libthermoTools
# needs incompressibleMultiphaseSystems and others) as a small scratch library
# libchtBCs, from the UNMODIFIED sources except for ONE edit:
# temperatureCoupledBase.C loses its `multiphaseInterSystem` include and the
# one block of kappa() that serves `kappaMethod phaseSystem` (not used here).
# The diff is written next to the copy (temperatureCoupledBase.diff).
set -e
OF=/home/user/upstream-build/OpenFOAM-v2506
S=$OF/src/thermoTools/derivedFvPatchFields
W=/home/user/upstream-build/gh1363/src/chtBCs
HERE=$(cd "$(dirname "$0")" && pwd)
WITH=/home/user/upstream-build/scripts/with-of.sh
rm -rf $W; mkdir -p $W/Make
cp $S/temperatureCoupledBase/temperatureCoupledBase.C $W/
python3 -I - $W/temperatureCoupledBase.C <<'PY'
import sys
p = sys.argv[1]
t = open(p).read()
t = t.replace('#include "multiphaseInterSystem.H"\n', '')
a = t.index('            {\n                const auto* ptr =\n                    mesh.cfindObject<multiphaseInterSystem>')
b = t.index('            FatalErrorInFunction', a)
t = t[:a] + t[b:]
open(p, 'w').write(t)
PY
diff $S/temperatureCoupledBase/temperatureCoupledBase.C $W/temperatureCoupledBase.C > $HERE/build_closure/temperatureCoupledBase.diff || true
cat > $W/Make/files <<EOT
temperatureCoupledBase.C
$S/turbulentTemperatureRadCoupledMixed/turbulentTemperatureRadCoupledMixedFvPatchScalarField.C
$S/turbulentTemperatureCoupledBaffleMixed/turbulentTemperatureCoupledBaffleMixedFvPatchScalarField.C

LIB = /home/user/upstream-build/gh1363/bin/libchtBCs
EOT
cat > $W/Make/options <<EOT
EXE_INC = \\
    -I$S/temperatureCoupledBase \\
    -I$S/turbulentTemperatureRadCoupledMixed \\
    -I$S/turbulentTemperatureCoupledBaffleMixed \\
    -I\$(LIB_SRC)/finiteVolume/lnInclude \\
    -I\$(LIB_SRC)/meshTools/lnInclude \\
    -I\$(LIB_SRC)/sampling/lnInclude \\
    -I\$(LIB_SRC)/transportModels/compressible/lnInclude \\
    -I\$(LIB_SRC)/thermophysicalModels/basic/lnInclude \\
    -I\$(LIB_SRC)/thermophysicalModels/specie/lnInclude \\
    -I\$(LIB_SRC)/thermophysicalModels/solidThermo/lnInclude \\
    -I\$(LIB_SRC)/thermophysicalModels/solidSpecie/lnInclude \\
    -I\$(LIB_SRC)/thermophysicalModels/reactionThermo/lnInclude \\
    -I\$(LIB_SRC)/thermophysicalModels/thermophysicalProperties/lnInclude \\
    -I\$(LIB_SRC)/thermophysicalModels/radiation/lnInclude \\
    -I\$(LIB_SRC)/TurbulenceModels/compressible/lnInclude \\
    -I\$(LIB_SRC)/TurbulenceModels/turbulenceModels/lnInclude

LIB_LIBS = \\
    -lfiniteVolume -lmeshTools -lsampling -lcompressibleTransportModels \\
    -lfluidThermophysicalModels -lspecie -lsolidThermo -lradiationModels \\
    -lturbulenceModels -lcompressibleTurbulenceModels
EOT
mkdir -p /home/user/upstream-build/gh1363/bin
(cd $W && WM_NCOMPPROCS=2 $WITH wmake)
rm -rf $W/Make/linux*
ls -l /home/user/upstream-build/gh1363/bin/libchtBCs.so
