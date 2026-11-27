#!/bin/bash
# usage: run.sh <variant> <endTime>
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export LD_LIBRARY_PATH=/home/user/upstream-build/probe1799/libbin:$LD_LIBRARY_PATH
S=/home/user/outram-park-backend/.claude/worktrees/agent-ac78c8c695c5879e6/crates/outram-foam-appbuilder-lib/verification_and_validation/genfoam_vs_upstream_gh1361/scripts
W=/home/user/upstream-build/probe1799/run_$1
rm -rf "$W"; mkdir -p "$W"; cp -r /home/user/upstream-build/GeN-Foam-src/Tutorials/featureCases/1D_boiling/. "$W/"; cd "$W" || exit 1
export VARIANT=$1 TIGHT=1 EVERY_STEP=1 END_TIME=$2
rm -f /home/user/upstream-build/probe1799/dump.txt
bash $S/boiling_mules_variant_protocol.sh > /dev/null
GeN-Foam > log.GeN-Foam 2>&1
echo "exit $?"
mv /home/user/upstream-build/probe1799/dump.txt /home/user/upstream-build/probe1799/dump_$1.txt
