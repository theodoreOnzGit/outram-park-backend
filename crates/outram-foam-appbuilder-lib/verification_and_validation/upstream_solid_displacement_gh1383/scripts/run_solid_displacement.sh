#!/bin/bash
# gh:#1383. Run the real, unmodified ESI v2506 solidDisplacementFoam on the case
# (blockMesh first) and keep its full-precision (writePrecision 17) fields and log.
# Usage: run_solid_displacement.sh <thermal|mech> [work dir]
#   thermal : thermalStress yes (T solved, grad(threeKalpha*T) in DEqn)
#   mech    : thermalStress no  (displacement-driven only)
# Needs the `stress` stage of scripts/upstream/build-esi-v2506-and-genfoam.sh.
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
V=${1:-thermal}
W=${2:-/home/user/upstream-build/gh1383/run_$V}
WITH=/home/user/upstream-build/scripts/with-of.sh
rm -rf "$W"
mkdir -p "$W"
cp -r "$HERE/../case/." "$W/"
if [ "$V" = mech ]; then
  sed -i 's/thermalStress   yes;/thermalStress   no;/' "$W/constant/thermalProperties"
fi
cd "$W"
$WITH blockMesh > log.blockMesh 2>&1
$WITH /home/user/upstream-build/gh1363/bin/solidDisplacementFoam > log.solidDisplacementFoam 2>&1
tail -3 log.solidDisplacementFoam
