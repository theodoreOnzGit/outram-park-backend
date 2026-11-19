#!/bin/bash
# Rebuild the reference: the first-step pressure matrix of an ESI v2506 pimpleFoam lid-driven
# cavity (20x20x1) and, for the SAME matrix, the real upstream solvers' per-iteration histories.
# Needs the ESI v2506 partial build in /home/user/upstream-build (scripts/with-of.sh) and the
# built pimpleFoam in /home/user/upstream-build/gh1363/bin; builds nothing in the repo.
set -e
HERE=$(cd "$(dirname "$0")/.." && pwd)
W=/home/user/upstream-build/solv1362
WITH=/home/user/upstream-build/scripts/with-of.sh
NITER=${NITER:-30}
mkdir -p $W/lib
rm -rf $W/lib/dumpSolver
cp -r $HERE/driver/dumpSolver $W/lib/dumpSolver
(cd $W/lib/dumpSolver && WM_NCOMPPROCS=1 $WITH wmake libso >/dev/null)
C=$W/case
rm -rf $C
mkdir -p $C/system $C/constant $C/0
cp $HERE/inputs/blockMeshDict $HERE/inputs/controlDict $HERE/inputs/fvSchemes $HERE/inputs/fvSolution $C/system/
cp $HERE/inputs/transportProperties $HERE/inputs/turbulenceProperties $C/constant/
cp $HERE/inputs/U $HERE/inputs/p $C/0/
cd $C
$WITH blockMesh > log.blockMesh
mkdir -p $HERE/reference
rm -f $HERE/reference/ref_cavity_p.txt.gz
DUMP_FILE=$C/dump.txt DUMP_NITER=$NITER DUMP_SOLVERS="PCG:DIC,PBiCGStab:DILU,smoothSolver:GaussSeidel,GAMG:GaussSeidel" \
  $WITH /home/user/upstream-build/gh1363/bin/pimpleFoam > log.pimpleFoam 2> log.err || true
grep -E "^[A-Za-z_0-9]+ [0-9]+ " dump.txt | gzip -9n > $HERE/reference/ref_cavity_p.txt.gz
