#!/bin/bash
# Partial OpenFOAM-v2506 build: only what cfMesh (DAFoam e944f6ba695f) and checkMesh need.

. /home/user/upstream-build/scripts/of-env.sh
set -e
cd "$WM_PROJECT_DIR"
start=$(date +%s)
echo "START $(date -Is)"
wmake/src/Allmake
cd src
wmakeLnInclude -u OpenFOAM
wmakeLnInclude -u OSspecific/POSIX
OpenFOAM/Alltouch -check 2>/dev/null || true
OSspecific/POSIX/Allwmake   # own preferred target: libo
Pstream/Allwmake
wmake libso OpenFOAM
wmake libso fileFormats
wmake libso surfMesh
wmake libso meshTools
wmake libso finiteVolume
wmake libso mesh/blockMesh
wmake libso mesh/extrudeModel
wmake libso dynamicMesh
cd "$WM_PROJECT_DIR"/applications/utilities/mesh/manipulation/checkMesh
wmake
echo "END $(date -Is) elapsed $(( $(date +%s) - start )) s"
