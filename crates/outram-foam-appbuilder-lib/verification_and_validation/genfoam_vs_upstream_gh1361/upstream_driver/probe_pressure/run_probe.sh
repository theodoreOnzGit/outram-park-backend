#!/bin/bash
# usage: run_probe.sh <name> [maxcorr]   (variant dir tests/data/zz_<name>); dump to scratch r17/probe_<name>.txt
NAME=$1
CRATE=/home/user/outram-park-backend/.claude/worktrees/agent-a87e6a256ab6ce699/crates/outram-foam-appbuilder-lib
S=/tmp/claude-0/-home-user-outram-park-backend/7cef3b0b-c239-561b-8dd4-50ffa1885b0f/scratchpad/r17
R=/home/user/upstream-build/gh1361/runs/p_$NAME
rm -rf $R
mkdir -p $R
cp -r $CRATE/tests/data/zz_$NAME/. $R/
cd $R || exit 1
sed -i 's/^writePrecision .*/writePrecision  17;/; s/^endTime .*/endTime         0.005;/; s/^writeControl .*/writeControl    timeStep;/; s/^writeInterval .*/writeInterval   1;/' system/controlDict
f=system/fluidRegion/fvSolution
awk 'BEGIN{ins=0} /^solvers/{ins=1} /^PIMPLE/{ins=0} { if(ins && $1=="tolerance"){sub(/tolerance .*/,"tolerance       1e-14;")} if(ins && $1=="relTol"){sub(/relTol .*/,"relTol          0;")} print }' $f > $f.new && mv $f.new $f
sed -i 's/^\( *\)solver  *GAMG;/\1solver          PCG;/; s/^\( *\)smoother  *DIC;/\1preconditioner  DIC;/' $f
sed -i 's/^\( *\)turbulence  *on;/\1turbulence      off;/' constant/fluidRegion/turbulenceProperties.liquid constant/fluidRegion/turbulenceProperties.vapour
export PROBE1833_FILE=$S/probe_$NAME.txt
export PROBE1833_MAXCORR=${2:-2}
rm -f $PROBE1833_FILE
/home/user/upstream-build/scripts/with-of.sh bash -c 'export LD_LIBRARY_PATH=/home/user/upstream-build/probe1833/libbin:$LD_LIBRARY_PATH; GeN-Foam' > log.GeN-Foam 2>&1
echo "exit $?"
ls -l $PROBE1833_FILE
