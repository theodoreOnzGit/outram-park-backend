#!/bin/bash
# Run the ESI v2506 trackParticles probe on the committed scripts of gh:#1382 and write
# tests/fixtures/esi_particle_tracking/<mesh>.esi.out.
# Needs the build of scripts/upstream/build-esi-v2506-and-genfoam.sh (stage `lagrangian`).
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
FIX=$HERE/../../tests/fixtures
OUT=${OUT:-/home/user/upstream-build/gh1382/runs}
WO=/home/user/upstream-build/scripts/with-of.sh
for mesh in warped3d dual_warped3d; do
  c=$OUT/$mesh
  rm -rf "$c"
  mkdir -p "$c/constant/polyMesh" "$c/system"
  cp "$FIX/esi_checkmesh/$mesh/"* "$c/constant/polyMesh/"
  cp "$HERE/cases/controlDict" "$c/system/"
  (cd "$c" && "$WO" trackParticles -script "$FIX/esi_particle_tracking/$mesh.script" > "$c/out.txt" 2> "$c/log")
  grep -E "^(new|reset|track|trackToFace) " "$c/out.txt" > "$FIX/esi_particle_tracking/$mesh.esi.out"
  echo "ok $mesh: $(wc -l < "$FIX/esi_particle_tracking/$mesh.esi.out") lines"
done
