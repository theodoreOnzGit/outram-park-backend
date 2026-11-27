#!/bin/bash
# gh:#1784 round 26: build the meshDeformDump driver against round 23's libThermoMechanicsSolvers (build_and_run_legacy_tm_dump.sh builds
# it first) and run it on the staged case. usage: build_and_run_mesh_deform_dump.sh <nSteps> <out.txt>   (objects deleted after the link)
HERE="$(cd "$(dirname "$0")/.." && pwd)"
W=/home/user/upstream-build/gh1784
export GENFOAM_SRC=/home/user/upstream-build/GeN-Foam-src
export TM_LIB_DIR=/home/user/upstream-build/gh1781/lib
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export WM_NCOMPPROCS=2
mkdir -p $W
rm -rf $W/driver; mkdir $W/driver
cp -r $HERE/upstream_driver/mesh_deform_dump/. $W/driver/
(cd $W/driver && wmake > $W/build-driver.log 2>&1; echo "driver exit $?"; find . -name '*.o' -delete)
bash $HERE/scripts/make_mesh_deform_case.sh $W/case
export LD_LIBRARY_PATH=$TM_LIB_DIR:$LD_LIBRARY_PATH
S0=$(date +%s)
meshDeformDump -case $W/case "$2" "$1" > $W/case/log.meshDeformDump 2>&1
echo "meshDeformDump exit $? in $(( $(date +%s)-S0 )) s"
tail -5 $W/case/log.meshDeformDump
