#!/bin/bash
# usage: run_probe_boiling.sh <workname> <probe file> <maxstep> <endtime>
# 1D_boiling AS SHIPPED (alpha MULES), tight protocol (scripts/boiling_mules_protocol.sh), instrumented libs first on LD_LIBRARY_PATH.
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export LD_LIBRARY_PATH=/home/user/upstream-build/probe1361/libbin:$LD_LIBRARY_PATH
export PROBE1361_FILE=$2 PROBE1361_MAXSTEP=$3
T=/home/user/upstream-build/GeN-Foam-src/Tutorials/featureCases/1D_boiling
W=/home/user/upstream-build/gh1361/runs/$1
rm -rf "$W"; mkdir -p "$W"; cp -r "$T/." "$W/"; cd "$W" || exit 1
[ -d 0.orig ] && cp -r 0.orig 0
export TIGHT=1 EVERY_STEP=1 END_TIME=$4
bash /home/user/outram-park-backend/.claude/worktrees/agent-a3e3d68ae403e63e3/crates/outram-foam-appbuilder-lib/verification_and_validation/genfoam_vs_upstream_gh1361/scripts/boiling_mules_protocol.sh
GeN-Foam > log.GeN-Foam 2>&1
echo "exit $?"
