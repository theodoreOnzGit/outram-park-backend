#!/bin/bash
# Rebuild the thermophysics reference dump (gh:#1362 row 4). Needs the partial
# OpenFOAM-v2506 build in /home/user/upstream-build (scripts/with-of.sh).
set -e
HERE=$(cd "$(dirname "$0")/.." && pwd)
W=/home/user/upstream-build/thermo1362
WITH=/home/user/upstream-build/scripts/with-of.sh
mkdir -p $W/bin $W/src
rm -rf $W/src/thermoDump
cp -r $HERE/driver $W/src/thermoDump
(cd $W/src/thermoDump && WM_NCOMPPROCS=1 $WITH wmake)
$WITH $W/bin/thermoDump 2>/dev/null | grep -E "^[A-Za-z]|^#" | gzip -9n > $HERE/reference/thermo_esi_v2506.txt.gz
