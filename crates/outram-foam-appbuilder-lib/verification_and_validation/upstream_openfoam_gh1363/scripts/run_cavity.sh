#!/bin/bash
# gh:#1363 row 1. Rebuild the upstream cavity reference with the real
# OpenFOAM-dev (Foundation, 852527377c7e) icoFoam (legacy/incompressible), the
# partial build described in
# crates/outram-park-fork-cfmesh/verification_and_validation/upstream_cfmesh_gh1385/README.md.
# Builds only icoFoam (needs finiteVolume + meshTools, already built); nothing
# of OpenFOAM is committed here. Run from anywhere.
set -e
HERE=$(cd "$(dirname "$0")/.." && pwd)
W=/home/user/upstream-build/gh1363
WITH=/home/user/upstream-build/scripts/with-of-dev.sh
DEV=/home/user/upstream-build/OpenFOAM-dev
mkdir -p $W/bin $W/src
if [ ! -x $W/bin/icoFoam ]; then
  rm -rf $W/src/icoFoam; cp -r $DEV/applications/legacy/incompressible/icoFoam $W/src/icoFoam
  sed -i "s#\$(FOAM_APPBIN)#$W/bin#" $W/src/icoFoam/Make/files
  (cd $W/src/icoFoam && WM_NCOMPPROCS=2 $WITH wmake)
fi
rm -rf $W/cavity; mkdir -p $W/cavity
cp -r $HERE/cavity/system $HERE/cavity/constant $HERE/cavity/0 $W/cavity/
rm -rf $W/cavity/constant/polyMesh
(cd $W/cavity && $WITH blockMesh > blockMesh.log 2>&1 && $WITH ../bin/icoFoam > log.icoFoam 2>&1)
"$HERE/scripts/extract_cavity.sh" $W/cavity
