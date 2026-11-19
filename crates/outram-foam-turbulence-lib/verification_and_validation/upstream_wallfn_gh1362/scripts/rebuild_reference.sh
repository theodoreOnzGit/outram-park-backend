#!/bin/bash
# Rebuild the wall-function reference dump (gh:#1362 row 5, wall-function slice).
# Needs the partial OpenFOAM-v2506 build in /home/user/upstream-build (scripts/with-of.sh).
set -e
HERE=$(cd "$(dirname "$0")/.." && pwd)
W=/home/user/upstream-build/wallfn1362
WITH=/home/user/upstream-build/scripts/with-of.sh
mkdir -p $W/bin $W/src
rm -rf $W/src/wallFnDump
cp -r $HERE/driver $W/src/wallFnDump
(cd $W/src/wallFnDump && WM_NCOMPPROCS=1 $WITH wmake)
$WITH $W/bin/wallFnDump 2>/dev/null | grep -E "^(yplus_lam|nutk) " > $HERE/reference/wallfn_esi_v2506.txt
