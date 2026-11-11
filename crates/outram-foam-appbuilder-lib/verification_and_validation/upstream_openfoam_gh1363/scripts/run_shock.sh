#!/bin/bash
# gh:#1363 row 2. Rebuild the upstream shockTube reference: OpenFOAM-v2506
# (ESI) rhoCentralFoam, line for line (driver/rhoCentralDump.C, see its header
# for the one substitution), on the v2506 tutorial
# compressible/rhoCentralFoam/shockTube, with a FIXED time step so both codes
# take identical steps. Needs the partial v2506 build of
# crates/outram-park-fork-cfmesh/verification_and_validation/upstream_cfmesh_gh1385/README.md
# (libOpenFOAM, libfiniteVolume, libmeshTools, blockMesh); nothing of
# OpenFOAM is committed here.
#   usage: run_shock.sh <deltaT> <nSteps> "<dump steps>"      (default 1e-5 700 "1 2 10 100 700")
set -e
DT=${1:-1e-5}
NSTEPS=${2:-700}
DUMPS=${3:-"1 2 10 100 700"}
HERE=$(cd "$(dirname "$0")/.." && pwd)
W=/home/user/upstream-build/gh1363
WITH=/home/user/upstream-build/scripts/with-of.sh
TUT=/home/user/upstream-build/OpenFOAM-v2506/tutorials/compressible/rhoCentralFoam/shockTube
mkdir -p $W/bin $W/src
rm -rf $W/src/rhoCentralDump; mkdir -p $W/src/rhoCentralDump
cp -r $HERE/driver/* $W/src/rhoCentralDump/
cp /home/user/upstream-build/OpenFOAM-v2506/src/thermophysicalModels/specie/specie/specie.C $W/src/rhoCentralDump/
printf 'rhoCentralDump.C\nspecie.C\n\nEXE = %s/bin/rhoCentralDump\n' $W > $W/src/rhoCentralDump/Make/files
(cd $W/src/rhoCentralDump && WM_NCOMPPROCS=2 $WITH wmake)
rm -rf $W/shock; mkdir -p $W/shock/0 $W/shock/system $W/shock/constant
cp $TUT/system/blockMeshDict $TUT/system/fvSchemes $TUT/system/fvSolution $W/shock/system/
cp $TUT/constant/thermophysicalProperties $W/shock/constant/
cat > $W/shock/system/controlDict <<EOF
FoamFile { version 2.0; format ascii; class dictionary; object controlDict; }
application rhoCentralDump;
startFrom startTime;
startTime 0;
stopAt endTime;
endTime $(awk -v d=$DT -v n=$NSTEPS 'BEGIN{printf "%.17g", d*n + d/2}');
deltaT $DT;
writeControl timeStep;
writeInterval 100000;
writeFormat ascii;
writePrecision 17;
timeFormat general;
timePrecision 8;
runTimeModifiable false;
adjustTimeStep no;
dumpSteps ($DUMPS);
EOF
(cd $W/shock && $WITH blockMesh > blockMesh.log 2>&1)
# setFields equivalent for the tutorial setFieldsDict: cells whose centre is
# inside box (0 -1 -1) (5 1 1) get T 278.746, p 10000; the rest T 348.432, p 100000.
mkfield() { # name dimensions lowvalue highvalue
  {
    echo "FoamFile { version 2.0; format ascii; class volScalarField; object $1; }"
    echo "dimensions $2;"
    echo "internalField nonuniform List<scalar> 100"; echo "("
    for i in $(seq 0 99); do if [ $i -lt 50 ]; then echo "$4"; else echo "$3"; fi; done
    echo ")"; echo ";"
    echo "boundaryField { sides { type zeroGradient; } empty { type empty; } }"
  } > $W/shock/0/$1
}
mkfield T "[0 0 0 1 0 0 0]" 278.746 348.432
mkfield p "[1 -1 -2 0 0 0 0]" 10000 100000
cp $TUT/0.orig/U $W/shock/0/U
(cd $W/shock && $WITH ../bin/rhoCentralDump | grep -E "^# STEP|^# CONSTANTS|^(const|rho|Ux|T|p|e|rhoU|rhoE) [0-9]+ [0-9a-f]{16}$" > ref_shock.txt)
gzip -9n -c $W/shock/ref_shock.txt > $HERE/shock/reference/shock_v2506_rhoCentral.txt.gz
