#!/bin/bash
# gh:#1367 row 2. Rebuild reference/drag_v2506.txt.gz: compiles driver/dragDump.C
# against the partial OpenFOAM-v2506 build (libOpenFOAM only) and runs it.
# Nothing of OpenFOAM is committed.
set -e
HERE=$(cd "$(dirname "$0")/.." && pwd)
W=/home/user/upstream-build/gh1367
WITH=/home/user/upstream-build/scripts/with-of.sh
rm -rf $W/src/dragDump
mkdir -p $W/bin $W/src/dragDump/Make $HERE/reference
cp $HERE/driver/dragDump.C $W/src/dragDump/
printf 'EXE_INC =\n\nEXE_LIBS =\n' > $W/src/dragDump/Make/options
printf 'dragDump.C\n\nEXE = %s/bin/dragDump\n' $W > $W/src/dragDump/Make/files
(cd $W/src/dragDump && WM_NCOMPPROCS=1 $WITH wmake)
$WITH $W/bin/dragDump | gzip -9n > $HERE/reference/drag_v2506.txt.gz
