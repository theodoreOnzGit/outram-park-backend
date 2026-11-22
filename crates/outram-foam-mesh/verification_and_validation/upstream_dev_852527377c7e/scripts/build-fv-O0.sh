#!/bin/bash
# finiteVolume is only linked, never on the numeric path of blockMesh/polyDualMesh: build the
# remaining objects at -O0 (c++Opt temporarily set to -O0) to save time, then restore -O3.
. /home/user/upstream-build/scripts/of-dev-env.sh
cd $WM_PROJECT_DIR/src
wmake libso finiteVolume
rc=$?
git -C $WM_PROJECT_DIR checkout wmake/rules/linux64Gcc/c++Opt
echo "FV-DONE rc=$rc $(date -Is)"
