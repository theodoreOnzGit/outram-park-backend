#!/bin/bash
# run-case.sh <name> <tutorial-dir-or-empty> <app> [setup-command]
# Copies the tutorial to runs/<name>, sets writePrecision 17, runs <app> then checkMesh,
# serially (OMP_NUM_THREADS=1), timing each with /usr/bin/time -v.
name=$1; src=$2; app=$3; setup=$4
. /home/user/upstream-build/scripts/of-env.sh
export PATH=$PATH:/home/user/upstream-build/cfmesh-bin
set -e
R=/home/user/upstream-build/runs/$name
rm -rf "$R"; mkdir -p "$R"
cp -r "$src"/. "$R"/
cd "$R"
rm -f Allrun Allclean Allrun-parallel
grep -q '^writePrecision' system/controlDict || sed -i 's/^\(writeFormat .*\)$/\1\nwritePrecision  17;/' system/controlDict
sed -i 's/^writeCompression.*$/writeCompression off;/' system/controlDict
if [ -n "$setup" ]; then eval "$setup"; fi
echo "case $name: app=$app OMP_NUM_THREADS=$OMP_NUM_THREADS $(date -Is)" > log.meta
/usr/bin/time -v $app > log.$app 2> log.$app.time || { echo "$app FAILED"; cat log.$app.time | tail -30; }
/usr/bin/time -v checkMesh > log.checkMesh 2> log.checkMesh.time || echo "checkMesh returned nonzero"
grep "Elapsed (wall" log.$app.time log.checkMesh.time
