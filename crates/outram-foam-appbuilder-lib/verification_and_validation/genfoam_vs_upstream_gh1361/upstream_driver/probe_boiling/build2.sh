#!/bin/bash
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export FOAM_USER_LIBBIN=/home/user/upstream-build/probe1361/libbin
export WM_NCOMPPROCS=2
cd /home/user/upstream-build/probe1361/classes/thermalHydraulics/solvers
wmakeLnInclude . > /home/user/upstream-build/probe1361/build2.log 2>&1
nice -n 10 wmake -j2 libso . >> /home/user/upstream-build/probe1361/build2.log 2>&1
echo "exit $?" >> /home/user/upstream-build/probe1361/build2.log
