#!/bin/bash
# gh:#1383. Run the real, unmodified ESI v2506 solidDisplacementFoam on the case
# (blockMesh first) and keep its full-precision (writePrecision 17) fields and log.
# Usage: run_solid_displacement.sh <thermal|mech> [work dir]
#   thermal : thermalStress yes (T solved, grad(threeKalpha*T) in DEqn)
#   mech    : thermalStress no  (displacement-driven only)
#   mechlong: as mech, 400 steps (the converged fixed point; second comparison row)
#   mech3d, mech3dlong: as mech / mechlong with the top patch fixedValue (0 3e-5 0)
# Needs the `stress` stage of scripts/upstream/build-esi-v2506-and-genfoam.sh.
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
V=${1:-thermal}
W=${2:-/home/user/upstream-build/gh1383/run_$V}
WITH=/home/user/upstream-build/scripts/with-of.sh
rm -rf "$W"
mkdir -p "$W"
cp -r "$HERE/../case/." "$W/"
case "$V" in mech*)
  sed -i 's/thermalStress   yes;/thermalStress   no;/' "$W/constant/thermalProperties" ;;
esac
case "$V" in *3d*)
  # genuinely three-dimensional field: the top patch is held at a transverse displacement
  sed -i 's/top    { type zeroGradient; }/top    { type fixedValue; value uniform (0 3e-5 0); }/' "$W/0/D" ;;
esac
case "$V" in *long)
  # 400 steps x 4 correctors, written once at the end: the fixed point of the segregated loop
  sed -i 's/^endTime .*/endTime         400000;/; s/^writeInterval .*/writeInterval   400;/' "$W/system/controlDict" ;;
esac
cd "$W"
$WITH blockMesh > log.blockMesh 2>&1
$WITH /home/user/upstream-build/gh1363/bin/solidDisplacementFoam > log.solidDisplacementFoam 2>&1
tail -3 log.solidDisplacementFoam
