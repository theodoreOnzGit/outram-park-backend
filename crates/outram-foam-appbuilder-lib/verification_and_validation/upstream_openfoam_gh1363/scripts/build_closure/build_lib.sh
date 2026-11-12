#!/bin/bash
# usage: build_lib.sh <src-relative-dir> ; builds in-tree, records time and sizes, deletes objects
d=$1
OF=/home/user/upstream-build/OpenFOAM-v2506
C=/home/user/upstream-build/gh1363/closure
cd $OF/src/$d || exit 1
lib=$(sed -n 's#^LIB *= *\$(FOAM_LIBBIN)/\(lib[A-Za-z0-9_]*\).*#\1#p' Make/files | head -1)
n=$(basename $d)
s=$(date +%s)
WM_NCOMPPROCS=2 /home/user/upstream-build/scripts/with-of.sh wmake -j > $C/$n.log 2>&1
rc=$?
e=$(date +%s)
objs=$(du -sm $OF/build/linux64GccDPInt32Opt/src/$d 2>/dev/null | cut -f1)
sz=$(ls -l $OF/platforms/linux64GccDPInt32Opt/lib/$lib.so 2>/dev/null | awk '{print $5}')
nc=$(grep -c "\.C" Make/files)
echo "$d lib=$lib rc=$rc seconds=$((e-s)) objects_MB=$objs lib_bytes=$sz files_lines=$nc" >> $C/timings.txt
[ $rc = 0 ] && rm -rf $OF/build/linux64GccDPInt32Opt/src/$d
exit $rc
