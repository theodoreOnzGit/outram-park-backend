#!/bin/bash
C=/home/user/upstream-build/gh1363/closure
OF=/home/user/upstream-build/OpenFOAM-v2506
while pgrep -f chain1.sh > /dev/null; do sleep 5; done
cd $C
./build_lib.sh TurbulenceModels/turbulenceModels || exit 1
# scratch copy of TurbulenceModels/compressible with pruned LIB_LIBS
cd $C/scratch/compressibleTurbulenceModels
s=$(date +%s)
WM_NCOMPPROCS=2 /home/user/upstream-build/scripts/with-of.sh wmake -j > $C/compressibleTurbulenceModels.log 2>&1
rc=$?
e=$(date +%s)
objs=$(du -sm Make 2>/dev/null | cut -f1)
lib=$(ls -l $OF/platforms/linux64GccDPInt32Opt/lib/libcompressibleTurbulenceModels.so 2>/dev/null | awk '{print $5}')
echo "TurbulenceModels/compressible(scratch,pruned) rc=$rc seconds=$((e-s)) objects_MB=$objs lib_bytes=$lib" >> $C/timings.txt
[ $rc = 0 ] && rm -rf Make/linux*
