#!/bin/bash
# gh:#1363 row 5. Build the ESI v2506 twoPhaseEulerFoam stage
# (scripts/upstream/build-esi-v2506-and-genfoam.sh twophase), build the dump
# copy of the solver (driver/twoPhaseEulerDump.diff applied to the ESI sources:
# energy equations not solved, text dumps of the pressure-equation
# intermediates), write the reduced bubbleColumn case, mesh it with the ESI
# blockMesh, run it and keep the gzipped dump beside the case.
#   usage: run_bubble.sh [deltaT] [nSteps] [tag]
#   tag names the reference file (default: dt of the run), e.g. dt1e-3
set -e
DT=${1:-0.001}
NSTEPS=${2:-10}
TAG=${3:-dt1e-3}
HERE=$(cd "$(dirname "$0")/.." && pwd)
W=/home/user/upstream-build/gh1363
WITH=/home/user/upstream-build/scripts/with-of.sh
OF=/home/user/upstream-build/OpenFOAM-v2506
if [ ! -x $W/bin/twoPhaseEulerDump ]; then
  rm -rf $W/src/tpDump; mkdir -p $W/src
  cp -r $OF/applications/solvers/multiphase/twoPhaseEulerFoam $W/src/tpDump
  rm -rf $W/src/tpDump/Make/linux*
  cp $HERE/driver/twoPhaseEulerDump/dump.H $W/src/tpDump/dump.H
  (cd $W/src/tpDump && patch -p1 < $HERE/driver/twoPhaseEulerDump/twoPhaseEulerDump.diff)
  sed -i "s#\$(FOAM_APPBIN)#$W/bin#; s#/twoPhaseEulerFoam#/twoPhaseEulerDump#" $W/src/tpDump/Make/files
  # thermoTools is not built here and nothing in the solver includes it
  sed -i '/-lthermoTools/d' $W/src/tpDump/Make/options
  sed -i 's#-lphaseCompressibleTurbulenceModels \\#-lphaseCompressibleTurbulenceModels#' $W/src/tpDump/Make/options
  (cd $W/src/tpDump && nice -n 10 env WM_NCOMPPROCS=2 $WITH wmake)
fi
"$HERE/scripts/make_bubble_case.sh" $DT $NSTEPS
rm -rf $W/bubble; mkdir -p $W/bubble
cp -r $HERE/bubble/system $HERE/bubble/constant $HERE/bubble/0 $W/bubble/
rm -rf $W/bubble/constant/polyMesh
# CVM=1: the constant-Cvm virtual-mass variant (reference tag gets _vm)
if [ "${CVM:-0}" = 1 ]; then cp $HERE/bubble/constant/phaseProperties.virtualMass $W/bubble/constant/phaseProperties; TAG=${TAG}_vm; fi
(cd $W/bubble && $WITH blockMesh > blockMesh.log 2>&1 && $WITH twoPhaseEulerDump > log.run 2>&1)
mkdir -p $HERE/bubble/reference
gzip -9 -n -c < $W/bubble/dump.txt > $HERE/bubble/reference/bubble_v2506_$TAG.txt.gz
rm -rf $HERE/bubble/constant/polyMesh
cp -r $W/bubble/constant/polyMesh $HERE/bubble/constant/polyMesh
cp $W/bubble/log.run $HERE/bubble/reference/log.twoPhaseEulerDump_$TAG
