#!/bin/bash
# collect-case.sh <run name>: copy a finished run's inputs, polyMesh (gzipped) and logs
# into the V&V folder runs/<name>/.
name=$1
R=/home/user/upstream-build/runs/$name
V=/home/user/outram-park-backend/crates/outram-park-fork-cfmesh/verification_and_validation/upstream_cfmesh_gh1385/runs/$name
rm -rf "$V"; mkdir -p "$V/constant/polyMesh" "$V/system"
cp "$R"/system/meshDict "$R"/system/controlDict "$V/system/"
for f in "$R"/constant/polyMesh/*; do
  [ -f "$f" ] || continue
  gzip -9 -n -c "$f" > "$V/constant/polyMesh/$(basename "$f").gz"
done
for f in "$R"/log.*; do cp "$f" "$V/"; done
du -sh "$V"
