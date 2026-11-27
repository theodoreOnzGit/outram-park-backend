#!/bin/bash
# gh:#1871 round 51: build bc1871Dump against the unmodified libFFSSrc2, stage the case, run it, and write the gzipped
# reference (the BC patch values at 17 digits) into refs/bc1871.ref.txt.gz.
# usage: build_and_run_bc1871_dump.sh      (scratch under /home/user/upstream-build/gh1871)
HERE="$(cd "$(dirname "$0")/.." && pwd)"
W=/home/user/upstream-build/gh1871b
export GENFOAM_SRC=/home/user/upstream-build/GeN-Foam-src
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export WM_NCOMPPROCS=2
rm -rf $W/driver; mkdir -p $W/driver
cp -r $HERE/upstream_driver/bc1871_dump/. $W/driver/
(cd $W/driver && wmake > $W/build-driver.log 2>&1; echo "driver exit $?"; find . -name '*.o' -delete)
bash $HERE/scripts/make_fo1871_case.sh $W/case
(cd $W/case && bc1871Dump -case $W/case $W/out.txt > $W/log.run 2>&1; echo "run exit $?")
cp $W/out.txt $W/all.txt
gzip -9 -n -c $W/all.txt > $HERE/refs/bc1871.ref.txt.gz
wc -c $W/all.txt
