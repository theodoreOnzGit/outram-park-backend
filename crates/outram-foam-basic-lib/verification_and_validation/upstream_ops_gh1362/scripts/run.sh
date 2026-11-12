#!/bin/bash
# Rebuild the two reference dumps. Needs the partial OpenFOAM-v2506 build of
# crates/outram-park-fork-cfmesh/verification_and_validation/upstream_cfmesh_gh1385/README.md
# (scripts of-env.sh, build-of.sh) in /home/user/upstream-build; builds nothing in the repo.
set -e
HERE=$(cd "$(dirname "$0")/.." && pwd)
W=/home/user/upstream-build/ops1362
WITH=/home/user/upstream-build/scripts/with-of.sh
mkdir -p $W/bin $W/src $W/case/system
cp $HERE/inputs/* $W/case/system/
rm -rf $W/src/opsDump; cp -r $HERE/driver $W/src/opsDump
(cd $W/src/opsDump && WM_NCOMPPROCS=2 $WITH wmake)
(cd $W/case && rm -rf constant && $WITH blockMesh >/dev/null && $WITH ../bin/opsDump | grep -E "^[A-Za-z_0-9]+ [0-9]+ |^#" > $HERE/reference/ref_hexblock.txt)
rm -rf $W/caseB; cp -r $W/case $W/caseB
(cd $W/caseB && $WITH polyDualMesh 45 -overwrite >/dev/null && $WITH ../bin/opsDump | grep -E "^[A-Za-z_0-9]+ [0-9]+ |^#" > $HERE/reference/ref_polydual.txt)
