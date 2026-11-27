#!/bin/bash
# usage: run_probe_chf.sh <variant> <workname> <probe file> <maxstep> <endtime>
# The closed 1D_CHF variant with the tight protocol (as scripts/chf_closed_run.sh), unmodified binary, instrumented libFFSSrc2.
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export LD_LIBRARY_PATH=/home/user/upstream-build/probe1361/libbin:$LD_LIBRARY_PATH
export PROBE1361_FILE=$3 PROBE1361_MAXSTEP=$4
HERE=/home/user/outram-park-backend/.claude/worktrees/agent-a3e3d68ae403e63e3/crates/outram-foam-appbuilder-lib
SRC="$HERE/tests/data/genfoam_chf_closed_gh1361/$1"
W=/home/user/upstream-build/gh1361/runs/$2
rm -rf "$W"; mkdir -p "$W"; cp -r "$SRC/." "$W/"
cd "$W" || exit 1
sed -i 's/^writePrecision .*/writePrecision  17;/' system/controlDict
sed -i "s/^endTime .*/endTime         $5;/" system/controlDict
sed -i 's/^writeControl .*/writeControl    timeStep;/; s/^writeInterval .*/writeInterval   1;/' system/controlDict
f=system/fluidRegion/fvSolution
awk 'BEGIN{ins=0} /^solvers/{ins=1} /^PIMPLE/{ins=0}
     { if(ins && $1=="tolerance"){sub(/tolerance .*/,"tolerance       1e-14;")}
       if(ins && $1=="relTol"){sub(/relTol .*/,"relTol          0;")}
       print }' "$f" > "$f.new" && mv "$f.new" "$f"
GeN-Foam > log.GeN-Foam 2>&1
echo "exit $?"
