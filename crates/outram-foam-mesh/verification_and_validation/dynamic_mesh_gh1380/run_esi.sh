#!/bin/bash
# Build the dumpMeshMotion probe and run it for every (mesh, motion) pair of gh:#1380.
# Needs the ESI v2506 build of scripts/upstream/build-esi-v2506-and-genfoam.sh.
# Output: $OUT/<mesh>_<motion>/probe/<time>/{points,meshPhi,faceCentres,faceAreas,cellCentres,cellVolumes}
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
FIX=$HERE/../../tests/fixtures
OUT=${OUT:-/home/user/upstream-build/gh1380}
WO=/home/user/upstream-build/scripts/with-of.sh
mkdir -p "$OUT/probe-src"
cp -r "$HERE/dumpMeshMotion/." "$OUT/probe-src/"
(cd "$OUT/probe-src" && "$WO" wmake)
for mesh in warped3d dual_warped3d; do
  for motion in linear rotating axisRotation oscillatingLinear; do
    c=$OUT/${mesh}_$motion
    rm -rf "$c"
    mkdir -p "$c/constant/polyMesh" "$c/system" "$c/0"
    cp "$FIX/esi_checkmesh/$mesh/"* "$c/constant/polyMesh/"
    cp "$HERE/cases/$motion.dynamicMeshDict" "$c/constant/dynamicMeshDict"
    cp "$HERE/cases/controlDict" "$HERE/cases/fvSchemes" "$HERE/cases/fvSolution" "$c/system/"
    (cd "$c" && "$WO" dumpMeshMotion > log 2>&1) || { echo "FAILED $mesh $motion"; tail -20 "$c/log"; exit 1; }
    echo "ok $mesh $motion"
  done
done
