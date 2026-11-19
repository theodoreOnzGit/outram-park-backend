#!/bin/bash
# Rebuild the ODE reference dump (gh:#1362 row 6). Needs the partial
# OpenFOAM-v2506 build in /home/user/upstream-build (scripts/with-of.sh).
set -e
HERE=$(cd "$(dirname "$0")/.." && pwd)
W=/home/user/upstream-build/ode1362
WITH=/home/user/upstream-build/scripts/with-of.sh
mkdir -p $W/bin $W/src
rm -rf $W/src/odeDump
cp -r $HERE/driver $W/src/odeDump
(cd $W/src/odeDump && WM_NCOMPPROCS=1 $WITH wmake)
$WITH $W/bin/odeDump 2>/dev/null | grep -E "^(Euler|RKF45|Rosenbrock23) " > $HERE/reference/ode_esi_v2506.txt
