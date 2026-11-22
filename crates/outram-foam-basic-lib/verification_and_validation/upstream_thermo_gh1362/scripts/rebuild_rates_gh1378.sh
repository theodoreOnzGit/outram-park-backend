#!/bin/bash
# Rebuild the gh:#1378 family-1 thermophysics reference dump. Needs the partial
# OpenFOAM-v2506 build in /home/user/upstream-build (scripts/with-of.sh).
set -e
HERE=$(cd "$(dirname "$0")/.." && pwd)
W=/home/user/upstream-build/thermo1362
WITH=/home/user/upstream-build/scripts/with-of.sh
mkdir -p $W/bin $W/src
rm -rf $W/src/rateDump1378
cp -r $HERE/driver_rates_gh1378 $W/src/rateDump1378
(cd $W/src/rateDump1378 && WM_NCOMPPROCS=1 nice $WITH wmake)
$WITH $W/bin/rateDump1378 2>/dev/null | grep -E "^[A-Za-z]|^#" | gzip -9n > $HERE/reference/rates_esi_v2506_gh1378.txt.gz
