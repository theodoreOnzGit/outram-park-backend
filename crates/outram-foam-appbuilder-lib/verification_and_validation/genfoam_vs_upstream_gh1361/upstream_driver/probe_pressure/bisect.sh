#!/bin/bash
# usage: bisect.sh <name> <END>
# The variant port-data dir tests/data/zz_<name> must exist (made by the caller). Runs the real binary on a copy with the
# round-16 PCG/tight/turbulence-off protocol and extracts refs/v_<name>.ref.txt.gz.
NAME=$1; END=$2
CRATE=/home/user/outram-park-backend/.claude/worktrees/agent-a87e6a256ab6ce699/crates/outram-foam-appbuilder-lib
R=/home/user/upstream-build/gh1361/runs/v_$NAME
rm -rf $R
mkdir -p $R
cp -r $CRATE/tests/data/zz_$NAME/. $R/
cd $R || exit 1
sed -i 's/^writePrecision .*/writePrecision  17;/; s/^endTime .*/endTime         '$END';/; s/^writeControl .*/writeControl    timeStep;/; s/^writeInterval .*/writeInterval   1;/' system/controlDict
f=system/fluidRegion/fvSolution
awk 'BEGIN{ins=0} /^solvers/{ins=1} /^PIMPLE/{ins=0} { if(ins && $1=="tolerance"){sub(/tolerance .*/,"tolerance       1e-14;")} if(ins && $1=="relTol"){sub(/relTol .*/,"relTol          0;")} print }' $f > $f.new && mv $f.new $f
sed -i 's/^\( *\)solver  *GAMG;/\1solver          PCG;/; s/^\( *\)smoother  *DIC;/\1preconditioner  DIC;/' $f
if [ -z "$TURB_ON" ]; then
sed -i 's/^\( *\)turbulence  *on;/\1turbulence      off;/' constant/fluidRegion/turbulenceProperties.liquid constant/fluidRegion/turbulenceProperties.vapour
fi
/home/user/upstream-build/scripts/with-of.sh GeN-Foam > log.GeN-Foam 2>&1
echo "exit $?"
tail -3 log.GeN-Foam
cd $CRATE/verification_and_validation/genfoam_vs_upstream_gh1361/scripts || exit 1
NC=${NC:-477} EXTRA_FIELDS="dgdt.liquid" bash extract_boiling_reference.sh v_$NAME v_$NAME . > /dev/null
rm -f /home/user/upstream-build/gh1361/v_$NAME.ref.txt
ls -l $CRATE/verification_and_validation/genfoam_vs_upstream_gh1361/refs/v_$NAME.ref.txt.gz
