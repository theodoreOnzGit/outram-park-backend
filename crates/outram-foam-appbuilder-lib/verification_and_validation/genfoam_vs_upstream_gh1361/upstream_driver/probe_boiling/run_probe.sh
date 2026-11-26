#!/bin/bash
# usage: run_probe.sh <tutorial rel path> <workname> <probe file> <maxstep> <endtime> [blockMesh region]
# Runs the unmodified GeN-Foam binary with the instrumented libFFSSrc2.so first on LD_LIBRARY_PATH.
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export LD_LIBRARY_PATH=/home/user/upstream-build/probe1361/libbin:$LD_LIBRARY_PATH
export PROBE1361_FILE=$3 PROBE1361_MAXSTEP=$4
T=/home/user/upstream-build/GeN-Foam-src/Tutorials/$1
W=/home/user/upstream-build/gh1361/runs/$2
rm -rf "$W"; mkdir -p "$W"; cp -r "$T/." "$W/"; cd "$W" || exit 1
[ -d 0.orig ] && cp -r 0.orig 0
export TIGHT=1 EVERY_STEP=1 END_TIME=$5
bash /home/user/outram-park-backend/.claude/worktrees/agent-a3e3d68ae403e63e3/crates/outram-foam-appbuilder-lib/verification_and_validation/genfoam_vs_upstream_gh1361/scripts/psbt_closed_protocol.sh
[ -n "$6" ] && blockMesh -region "$6" > log.blockMesh 2>&1
GeN-Foam > log.GeN-Foam 2>&1
echo "exit $?"
