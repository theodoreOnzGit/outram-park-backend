#!/bin/bash
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export FOAM_USER_LIBBIN=/home/user/upstream-build/probe1833/libbin
export WM_NCOMPPROCS=2
cd /home/user/upstream-build/probe1833/classes
for d in common thermalHydraulics/src thermalHydraulics/solvers; do
  (cd $d && wmakeLnInclude . >> /home/user/upstream-build/probe1833/build.log 2>&1)
done
cd /home/user/upstream-build/probe1833/classes/thermalHydraulics/solvers
nice -n 10 wmake -j2 libso . >> /home/user/upstream-build/probe1833/build.log 2>&1
echo "exit $?" >> /home/user/upstream-build/probe1833/build.log
