#!/bin/bash
# Run the real GeN-Foam binary on a CLOSED 1D_CHF variant (gh:#1361 round 10; gh:#1223, gh:#1224).
# usage: chf_closed_run.sh <variant dir under tests/data/genfoam_chf_closed_gh1361> <workname> [end time]
#   e.g. chf_closed_run.sh constant_chf chf_constant 0.3
# Copies the variant to /home/user/upstream-build/gh1361/runs/<workname>, applies the tight protocol
# (writePrecision 17, every step written, every linear-solver tolerance in `solvers` 1e-14 with relTol 0;
# the residualControl tolerances in PIMPLE are NOT touched), then runs the unmodified binary, serial.
# Never changes physics, mesh or boundary conditions. For `lookup_table_chf` the `chfTables` file is copied
# next to the case's phaseProperties (the case `#include`s it).
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
SRC="$HERE/tests/data/genfoam_chf_closed_gh1361/$1"
W=/home/user/upstream-build/gh1361/runs/$2
END=${3:-0.3}
rm -rf "$W"; mkdir -p "$W"; cp -r "$SRC/." "$W/"
cd "$W" || exit 1
sed -i 's/^writePrecision .*/writePrecision  17;/' system/controlDict
sed -i "s/^endTime .*/endTime         $END;/" system/controlDict
sed -i 's/^writeControl .*/writeControl    timeStep;/; s/^writeInterval .*/writeInterval   1;/' system/controlDict
f=system/fluidRegion/fvSolution
awk 'BEGIN{ins=0} /^solvers/{ins=1} /^PIMPLE/{ins=0}
     { if(ins && $1=="tolerance"){sub(/tolerance .*/,"tolerance       1e-14;")}
       if(ins && $1=="relTol"){sub(/relTol .*/,"relTol          0;")}
       print }' "$f" > "$f.new" && mv "$f.new" "$f"
S=$(date +%s)
/home/user/upstream-build/scripts/with-of.sh GeN-Foam > log.GeN-Foam 2>&1
echo "GeN-Foam exit $? in $(( $(date +%s)-S )) s"
tail -4 log.GeN-Foam
