#!/bin/bash
# Build cfMesh (DAFoam e944f6ba695f) against the partial OpenFOAM-v2506 build:
# meshLibrary + the four executables (cartesianMesh, cartesian2DMesh, tetMesh, pMesh).
# Utilities are skipped (preparePar needs libdecompose, which is not built).
. /home/user/upstream-build/scripts/of-env.sh
set -e
CF=${CF:-/home/user/outram-park-backend/crates/outram-park-fork-cfmesh/upstream_source/cfmesh_dafoam}
cd "$CF"
start=$(date +%s)
echo "START $(date -Is)"
wmake libso meshLibrary
for e in cartesianMesh cartesian2DMesh tetMesh pMesh; do wmake executables/$e; done
echo "END $(date -Is) elapsed $(( $(date +%s) - start )) s"
