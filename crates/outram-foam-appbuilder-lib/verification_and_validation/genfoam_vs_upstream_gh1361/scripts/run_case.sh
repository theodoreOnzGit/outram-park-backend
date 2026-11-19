#!/bin/bash
# Usage: run_case.sh <case dir (absolute)> <workname>
# Like run_tutorial.sh, but for a case vendored in this repository (tests/data/...).
# PRE_RUN: optional protocol script run inside the work dir (end time, precision, tolerances).
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
W=/home/user/upstream-build/gh1361/runs/$2
rm -rf "$W"
mkdir -p "$W"
cp -r "$1/." "$W/"
cd "$W" || exit 1
[ -n "$PRE_RUN" ] && bash "$PRE_RUN"
S=$(date +%s)
GeN-Foam > log.GeN-Foam 2>&1
echo "GeN-Foam exit $? in $(( $(date +%s)-S )) s"
tail -4 log.GeN-Foam
