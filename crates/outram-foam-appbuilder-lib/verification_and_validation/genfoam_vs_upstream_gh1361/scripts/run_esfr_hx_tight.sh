#!/bin/bash
# gh:#1903 round 52: run cases/esfr_hx (3-D ESFR mesh, MSFR steadyStateTH settings, two core halves as heat exchangers) with the REAL unmodified
# GeN-Foam binary, three steps, tight. usage: run_esfr_hx_tight.sh <name> [CASE_DIR] [RUN_ROOT]   (the case dir is staged as is: no edit beyond endTime)
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
HERE=$(cd "$(dirname "$0")/.." && pwd)
NAME=$1; CASE=${2:-$HERE/cases/esfr_hx}; R=${3:-/home/user/upstream-build/gh1903/runs}
mkdir -p "$R"; cd "$R" || exit 1
rm -rf "$NAME"; mkdir "$NAME"; cp -r "$CASE"/. "$NAME"/
C=$NAME
sed -i "s/^endTime .*/endTime ${END_TIME:-0.0037};/" $C/system/controlDict
S=$(date +%s)
GeN-Foam -case $C > $C/log.GeN-Foam 2>&1
echo "$NAME: GeN-Foam exit $? in $(( $(date +%s)-S )) s"
tail -3 $C/log.GeN-Foam
