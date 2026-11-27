#!/bin/bash
# usage: run_probe.sh <name> [maxcorr]  : PSBT2 Ex3 first step, turbulence off, tight PCG/DIC (round 17 protocol); dump to probe_<name>.txt
NAME=$1
CRATE=/home/user/outram-park-backend/.claude/worktrees/agent-aa466fa7d1ce4e929/crates/outram-foam-appbuilder-lib
P=/home/user/upstream-build/probe1909
R=$P/runs/$NAME
rm -rf $R
mkdir -p $R
cp -r $CRATE/tests/data/genfoam_psbt2_ex3_gh1802/. $R/
cd $R || exit 1
sed -i 's/^writePrecision .*/writePrecision  17;/; s/^endTime .*/endTime         0.005;/; s/^writeControl .*/writeControl    timeStep;/; s/^writeInterval .*/writeInterval   1;/' system/controlDict
f=system/fluidRegion/fvSolution
awk 'BEGIN{ins=0} /^solvers/{ins=1} /^PIMPLE/{ins=0} { if(ins && $1=="tolerance"){sub(/tolerance .*/,"tolerance       1e-14;")} if(ins && $1=="relTol"){sub(/relTol .*/,"relTol          0;")} print }' $f > $f.new && mv $f.new $f
sed -i 's/^\( *\)solver  *GAMG;/\1solver          PCG;/; s/^\( *\)smoother  *DIC;/\1preconditioner  DIC;/' $f
sed -i 's/^\( *\)turbulence  *on;/\1turbulence      off;/' constant/fluidRegion/turbulenceProperties.liquid constant/fluidRegion/turbulenceProperties.vapour
export PROBE1833_FILE=$P/probe_$NAME.txt
export PROBE1833_MAXCORR=${2:-2}
rm -f $PROBE1833_FILE
/home/user/upstream-build/scripts/with-of.sh bash -c "export LD_LIBRARY_PATH=$P/libbin:\$LD_LIBRARY_PATH; GeN-Foam" > log.GeN-Foam 2>&1
echo "exit $?"
ls -l $PROBE1833_FILE
