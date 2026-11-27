#!/bin/bash
# Round 13 probe build: instrumented copy of GeN-Foam thermalHydraulics/src, into a SEPARATE libbin.
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export FOAM_USER_LIBBIN=/home/user/upstream-build/probe1361/libbin
export WM_NCOMPPROCS=2
cd /home/user/upstream-build/probe1361/classes/thermalHydraulics/src
wmakeLnInclude . > /home/user/upstream-build/probe1361/build.log 2>&1
nice -n 10 wmake -j2 libso . >> /home/user/upstream-build/probe1361/build.log 2>&1
echo "exit $?" >> /home/user/upstream-build/probe1361/build.log
