#!/bin/bash
# Usage: run_tutorial.sh <tutorial path relative to GeN-Foam/Tutorials> <workname> [blockMesh regions...]
# Copies the tutorial to /home/user/upstream-build/gh1361/runs/<workname>, restores 0.orig,
# runs blockMesh per region (those given), then the real GeN-Foam binary (serial), logs to log.GeN-Foam.
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
T=/home/user/upstream-build/GeN-Foam-src/Tutorials/$1
W=/home/user/upstream-build/gh1361/runs/$2
shift 2
rm -rf "$W"
mkdir -p "$W"
cp -r "$T/." "$W/"
cd "$W" || exit 1
[ -d 0.orig ] && cp -r 0.orig 0
# optional hook run inside the work dir (protocol edits: write precision, tolerances, end time)
[ -n "$PRE_RUN" ] && bash "$PRE_RUN"
for r in "$@"; do
  blockMesh -region "$r" > "log.blockMesh.$r" 2>&1
  echo "blockMesh $r exit $?"
done
S=$(date +%s)
GeN-Foam > log.GeN-Foam 2>&1
echo "GeN-Foam exit $? in $(( $(date +%s)-S )) s"
tail -4 log.GeN-Foam
