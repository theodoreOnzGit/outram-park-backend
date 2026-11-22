#!/bin/bash
# Partial OpenFOAM-dev (852527377c7e) build, dummy Pstream (no MPI): libraries for blockMesh,
# polyDualMesh, checkMesh.  Re-runnable (wmake is incremental).
# Recipe note: finiteVolume was built at -O0 (build-fv-O0.sh) to save time; it is linked only.
. /home/user/upstream-build/scripts/of-dev-env.sh
set -e
cd "$WM_PROJECT_DIR"
step() { echo "STEP $1 $(date -Is)"; }
start=$(date +%s)
echo "START $(date -Is)"
step wmake-tools;  (cd wmake/src && make)
cd src
step core
wmakeLnInclude -u OpenFOAM; wmakeLnInclude -u OSspecific/POSIX
Pstream/Allwmake libso
OSspecific/POSIX/Allwmake   # own preferred target: libo
wmake libso OpenFOAM
for l in fileFormats surfMesh triSurface meshTools; do step $l; wmake libso $l; done
step dummyThirdParty; dummyThirdParty/Allwmake libso
step finiteVolume; wmake libso finiteVolume
for l in tracking lagrangian/basic sampling meshCheck mesh/extrudeModel polyTopoChange mesh/blockMesh generic/genericPatches generic/genericFields generic/genericFvPatches generic/genericFvFields; do step $l; wmake libso $l; done
cd "$WM_PROJECT_DIR"/applications/utilities/mesh
for a in generation/blockMesh manipulation/checkMesh; do step app-$a; (cd $a && wmake); done
# polyDualMesh: scratch copy, Make/options without the unused decomposition/scotch/snappyHexMesh libs
step app-polyDualMesh
P=${POLYDUAL_APP:-/home/user/upstream-build/polyDualMesh-dev}
rm -rf $P; cp -r manipulation/polyDualMesh $P; rm -rf $P/Make/linux*
cp ${POLYDUAL_OPTIONS:?set POLYDUAL_OPTIONS} $P/Make/options
(cd $P && wmake)
echo "END $(date -Is) elapsed $(( $(date +%s) - start )) s"
