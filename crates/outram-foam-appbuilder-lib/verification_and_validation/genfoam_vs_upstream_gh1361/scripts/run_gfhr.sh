#!/bin/bash
# gh:#1361 round 3: 3D_gFHR (steadyStateNSSP, nuclearSteadyStatePebble) on the real GeN-Foam binary,
# writePrecision 17, END_TIME seconds (default 0.3: three steps of the tutorial's 0.1 s), serial.
# Mirrors the tutorial's Allrun for the NSSP case: clone rootCase, `GeN-Foam -initializeMappedFields`, run.
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
T=/home/user/upstream-build/GeN-Foam-src/Tutorials/reactorCases/3D_gFHR/rootCase
W=/home/user/upstream-build/gh1361/runs/gfhr
rm -rf "$W"
mkdir -p "$W"
cp -r "$T/." "$W/"
cd "$W" || exit 1
HERE=$(dirname "$(readlink -f "$0")")
sed -i 's/^writePrecision .*/writePrecision  17;/' system/controlDict
sed -i "/^stopAt/a endTime         ${END_TIME:-0.3};" system/controlDict
sed -i 's/^stopAt .*/stopAt          endTime;/' system/controlDict
sed -i 's/^writeControl .*/writeControl    timeStep;/; s/^writeInterval .*/writeInterval   1;/' system/controlDict
S=$(date +%s)
GeN-Foam -initializeMappedFields > log.init 2>&1
echo "init exit $? in $(( $(date +%s)-S )) s"
S=$(date +%s)
/usr/bin/time -v GeN-Foam > log.GeN-Foam 2> log.time
echo "GeN-Foam exit $? in $(( $(date +%s)-S )) s"
grep -E "Maximum resident|Elapsed" log.time
tail -3 log.GeN-Foam | cut -c1-200
ls "$W" | tr '\n' ' '
