#!/bin/bash
# gh:#1871 round 51: build nb1871Dump (a literal transcription of NusseltThermalBaffle1D's arithmetic on real OpenFOAM
# scalarFields; NOT the upstream class) and write refs/nb1871.ref.txt.gz.   usage: build_and_run_nb1871_dump.sh
HERE="$(cd "$(dirname "$0")/.." && pwd)"
W=/home/user/upstream-build/gh1871c
export GENFOAM_SRC=/home/user/upstream-build/GeN-Foam-src
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export WM_NCOMPPROCS=2
rm -rf $W; mkdir -p $W/driver
cp -r $HERE/upstream_driver/nb1871_dump/. $W/driver/
(cd $W/driver && wmake > $W/build-driver.log 2>&1; echo "driver exit $?"; find . -name '*.o' -delete)
nb1871Dump $W/out.txt > $W/log.run 2>&1; echo "run exit $?"
gzip -9 -n -c $W/out.txt > $HERE/refs/nb1871.ref.txt.gz
wc -c $W/out.txt
