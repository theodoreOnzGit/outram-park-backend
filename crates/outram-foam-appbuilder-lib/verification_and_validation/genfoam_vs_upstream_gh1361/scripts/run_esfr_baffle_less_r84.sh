#!/bin/bash
# gh:#1954 (README round 84): the real 3D_SmallESFR_old steady state AS SHIPPED (make_esfr_old_case.sh, loose, INFLATE=1) and its transient
# (Allrun's setTransient), serial, so that the baffle-less mesh of every directory the binary writes can be read:
#   ss/0/fluidRegion.baffleLess/polyMesh          (written by removeBaffles at the steady start, before addZones; sets/duplicateFaces)
#   ss/200/fluidRegion.baffleLess/polyMesh        (a LATER write: the 7 cell zones and 2 face zones copied by addZones)
#   tr/200/fluidRegion.baffleLess/polyMesh        (the transient's start, rewritten by -initializeMappedFields and the run's own removeBaffles)
#   tr/210/fluidRegion.baffleLess/polyMesh        (the transient's first later write)
# usage: run_esfr_baffle_less_r84.sh <scratch dir>
D=$1
S=$(dirname "$(readlink -f "$0")")
W=/home/user/upstream-build/scripts/with-of.sh
export LD_LIBRARY_PATH=/home/user/upstream-build/gh1781/lib:$LD_LIBRARY_PATH
mkdir -p "$D"
INFLATE=1 bash "$S/make_esfr_old_case.sh" steady "$D/ss"
T0=$(date +%s)
(cd "$D/ss" && $W GeN-Foam -case "$D/ss" > log.GeN-Foam 2>&1)
echo "steady: exit $? in $(( $(date +%s)-T0 )) s"
bash "$S/make_esfr_old_case.sh" transient "$D/tr" "$D/ss"
(cd "$D/tr" && $W GeN-Foam -initializeMappedFields -case "$D/tr" > log.init 2>&1)
T0=$(date +%s)
(cd "$D/tr" && $W GeN-Foam -case "$D/tr" > log.GeN-Foam 2>&1)
echo "transient: exit $? in $(( $(date +%s)-T0 )) s"
