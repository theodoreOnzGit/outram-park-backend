#!/bin/bash
# usage: run_up.sh <name> ; env ISO NOUTER NCORR END_TIME TOL
NAME=$1
CASE=${CASE:-/home/user/outram-park-backend/.claude/worktrees/agent-a4ad2e0194b2cf44d/crates/outram-foam-appbuilder-lib/tests/data/genfoam_1d_hx_onephase}
export PROBE1894_FILE=/home/user/upstream-build/probe1894/up_$NAME.txt
rm -f $PROBE1894_FILE
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export LD_LIBRARY_PATH=/home/user/upstream-build/probe1894/libbin:$LD_LIBRARY_PATH
W=/home/user/upstream-build/gh1361/runs/p1894_$NAME
rm -rf $W; mkdir -p $W; cp -r $CASE/. $W/; cd $W || exit 1
bash /home/user/upstream-build/probe1894/pre_run.sh
GeN-Foam > log.GeN-Foam 2>&1
echo "exit $?"
ls -l $PROBE1894_FILE
