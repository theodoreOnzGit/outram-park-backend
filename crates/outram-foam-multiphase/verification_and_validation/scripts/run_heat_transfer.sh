#!/bin/bash
# gh:#1367 row 3. Rebuild reference/heat_transfer_v2506.txt.gz: compiles
# driver/heatTransferDump.C against the partial OpenFOAM-v2506 build
# (libOpenFOAM, libfiniteVolume) and runs it. Nothing of OpenFOAM is committed.
set -e
HERE=$(cd "$(dirname "$0")/.." && pwd)
W=/home/user/upstream-build/gh1367
WITH=/home/user/upstream-build/scripts/with-of.sh
rm -rf $W/src
mkdir -p $W/bin $W/src/heatTransferDump/Make $HERE/reference
cp $HERE/driver/heatTransferDump.C $W/src/heatTransferDump/
cp $HERE/driver/Make/options $W/src/heatTransferDump/Make/
printf 'heatTransferDump.C\n\nEXE = %s/bin/heatTransferDump\n' $W > $W/src/heatTransferDump/Make/files
(cd $W/src/heatTransferDump && WM_NCOMPPROCS=1 $WITH wmake)
$WITH $W/bin/heatTransferDump | gzip -9n > $HERE/reference/heat_transfer_v2506.txt.gz
