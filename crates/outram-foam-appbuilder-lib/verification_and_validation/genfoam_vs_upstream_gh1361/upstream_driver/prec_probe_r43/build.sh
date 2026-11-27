#!/bin/bash
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export FOAM_USER_LIBBIN=/home/user/upstream-build/gh1361/r43/lib
export WM_NCOMPPROCS=2
cd /home/user/upstream-build/gh1361/r43/src/classes/common && wmakeLnInclude .
cd ../neutronics && wmakeLnInclude . && wmake -j2 libso .
find Make -name '*.o' -delete
ls -l $FOAM_USER_LIBBIN
