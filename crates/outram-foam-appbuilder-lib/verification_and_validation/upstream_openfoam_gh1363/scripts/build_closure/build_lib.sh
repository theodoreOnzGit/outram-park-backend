#!/bin/bash
# usage: build_lib.sh <src-relative-dir> ; builds in-tree, records time and sizes, deletes objects
d=$1; n=$(basename $d)
OF=/home/user/upstream-build/OpenFOAM-v2506
C=/home/user/upstream-build/gh1363/closure
cd $OF/src/$d || exit 1
s=$(date +%s)
WM_NCOMPPROCS=2 /home/user/upstream-build/scripts/with-of.sh wmake -j > $C/$n.log 2>&1
rc=$?
e=$(date +%s)
objs=$(du -sm $OF/build/linux64GccDPInt32Opt/src/$d 2>/dev/null | cut -f1)
lib=$(ls -l $OF/platforms/linux64GccDPInt32Opt/lib/lib$n.so 2>/dev/null | awk '{print $5}')
echo "$d rc=$rc seconds=$((e-s)) objects_MB=$objs lib_bytes=$lib" >> $C/timings.txt
[ $rc = 0 ] && rm -rf $OF/build/linux64GccDPInt32Opt/src/$d
exit $rc
