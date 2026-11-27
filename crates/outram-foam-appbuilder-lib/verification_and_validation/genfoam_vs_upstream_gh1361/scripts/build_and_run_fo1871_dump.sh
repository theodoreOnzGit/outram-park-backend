#!/bin/bash
# gh:#1871 round 51: build fo1871Dump against the unmodified libFFSSrc2, stage the case, run it, and write the gzipped
# reference (driver inputs + the classes' own postProcessing files) into refs/fo1871.ref.txt.gz.
# usage: build_and_run_fo1871_dump.sh      (scratch under /home/user/upstream-build/gh1871)
HERE="$(cd "$(dirname "$0")/.." && pwd)"
W=/home/user/upstream-build/gh1871
export GENFOAM_SRC=/home/user/upstream-build/GeN-Foam-src
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export WM_NCOMPPROCS=2
rm -rf $W/driver; mkdir -p $W/driver
cp -r $HERE/upstream_driver/fo1871_dump/. $W/driver/
(cd $W/driver && wmake > $W/build-driver.log 2>&1; echo "driver exit $?"; find . -name '*.o' -delete)
bash $HERE/scripts/make_fo1871_case.sh $W/case
(cd $W/case && fo1871Dump -case $W/case $W/out.txt > $W/log.run 2>&1; echo "run exit $?")
{
  cat $W/out.txt
  for f in $(cd $W/case && find postProcessing -type f | sort); do
    echo "FILE $f"; cat $W/case/$f; echo
  done
} > $W/all.txt
gzip -9 -n -c $W/all.txt > $HERE/refs/fo1871.ref.txt.gz
wc -c $W/all.txt
