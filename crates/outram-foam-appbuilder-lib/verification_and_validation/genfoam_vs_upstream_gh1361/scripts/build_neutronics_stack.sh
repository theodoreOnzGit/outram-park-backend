#!/bin/bash
# gh:#1361 step 3: build GeN-Foam 652b3da0cc55's libMultiRegion, libNeutronicsSolvers
# and the UNMODIFIED GeN-Foam.C (+ include/setDeltaT.C) as an executable that
# links only the neutronics path. Sources are untouched; only the link lines
# in Make/options are pruned (libraries that are not built here and that the
# neutronics path does not call: fvOptions, regionModels, solidSpecie, ODE,
# genericPatchFields, thermo/turbulence, FFS*, ThermoMechanics, offbeat, OFSolvers).
# Per-library wall time and peak RSS go to /home/user/upstream-build/build-genfoam-*.log
# (`/usr/bin/time -v`). Objects are deleted after each link (disk).
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export WM_NCOMPPROCS=2
SRC=/home/user/upstream-build/GeN-Foam-src
PRUNE='/-lfvOptions/d;/-lregionModels/d;/-lsolidSpecie/d;/-lODE/d;/-lgenericPatchFields/d;/-lcompressibleTransportModels/d;/-lfluidThermophysicalModels/d;/-lsolidThermo/d;/-lspecie/d;/-lcompressibleTurbulenceModels/d;/-lturbulenceModels/d;/-lFFSSrc2/d;/-lFFSSolvers2/d;/-lThermoMechanicsSolvers/d;/-loffbeatMain/d;/-loffbeat /d;/-lsciantix/d;/-lOFSolvers/d'

buildlib() {
  d=$1; name=$2
  cd "$SRC/src/classes/$d" || return 1
  sed -i "$PRUNE" Make/options
  wmakeLnInclude .
  /usr/bin/time -v wmake -j2 libso . > /home/user/upstream-build/build-genfoam-$name.log 2>&1
  echo "$name exit $? : $(grep -E 'Elapsed' /home/user/upstream-build/build-genfoam-$name.log)"
  du -sk Make/linux64GccDPInt32Opt | sed "s/^/$name objects (kB): /"
  find Make/linux64GccDPInt32Opt -name '*.o' -delete
}

buildlib multiRegion multiregion
buildlib neutronics neutronics
cd "$SRC/GeN-Foam" || exit 1
sed -i "$PRUNE" Make/options
/usr/bin/time -v wmake > /home/user/upstream-build/build-genfoam-exe.log 2>&1
echo "GeN-Foam exit $? : $(grep -E 'Elapsed' /home/user/upstream-build/build-genfoam-exe.log)"
du -sk Make/linux64GccDPInt32Opt | sed "s/^/GeN-Foam objects (kB): /"
find Make/linux64GccDPInt32Opt -name '*.o' -delete
ls -l /root/OpenFOAM/user-v2506/platforms/linux64GccDPInt32Opt/lib /root/OpenFOAM/user-v2506/platforms/linux64GccDPInt32Opt/bin
