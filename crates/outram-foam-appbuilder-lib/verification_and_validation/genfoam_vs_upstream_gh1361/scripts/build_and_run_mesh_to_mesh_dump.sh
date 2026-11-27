#!/bin/bash
# gh:#651 round 28: build the meshToMeshDump driver (needs only libfiniteVolume/libsampling/libmeshTools of OpenFOAM-ESI v2506) and run it
# on the 3D_SmallESFR meshes staged by make_mesh_deform_case.sh. usage: build_and_run_mesh_to_mesh_dump.sh <out.txt>
# Scratch: /home/user/upstream-build/gh651 (the driver objects are deleted after the link; delete the dump after comparing).
HERE="$(cd "$(dirname "$0")/.." && pwd)"
W=/home/user/upstream-build/gh651
export GENFOAM_SRC=/home/user/upstream-build/GeN-Foam-src
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export WM_NCOMPPROCS=2
mkdir -p $W
rm -rf $W/driver; mkdir $W/driver
cp -r $HERE/upstream_driver/mesh_to_mesh_dump/. $W/driver/
(cd $W/driver && wmake > $W/build-driver.log 2>&1; echo "driver exit $?"; find . -name '*.o' -delete)
bash $HERE/scripts/make_mesh_deform_case.sh $W/case >/dev/null
# a second copy of neutroRegion's mesh, moved by the driver (genuine partial overlap with the original)
mkdir -p $W/case/constant/neutroMovedRegion $W/case/system/neutroMovedRegion
cp -r $W/case/constant/neutroRegion/polyMesh $W/case/constant/neutroMovedRegion/
cp $W/case/system/neutroRegion/* $W/case/system/neutroMovedRegion/
S0=$(date +%s)
meshToMeshDump -case $W/case "$1" > $W/case/log.meshToMeshDump 2>&1
echo "meshToMeshDump exit $? in $(( $(date +%s)-S0 )) s"
tail -8 $W/case/log.meshToMeshDump
