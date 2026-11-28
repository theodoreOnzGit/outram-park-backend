#!/bin/bash
# gh:#1961 / #1962 (README round 81): the REAL GeN-Foam binary (652b3da on ESI v2506, serial) on Allrun's `steadyState` stage EXACTLY, the
# start of `run_msr1d_transient_r80.sh` (round 80 ran the same edits by hand; this script records them).
# usage: run_msr1d_steady_allrun_r81.sh <out run dir>
#   MSR_CASE_DIR=<tutorial dir> (default the shipped 1D_MSR_pointKinetics; 1D_thermalMSR_pointKinetics is the other user).
#   setSteadyState: (startFrom startTime, startTime 0: only where that Allrun sets them), endTime 100, adjustTimeStep true,
#   writeControl adjustableRunTime, writeInterval 100, multiRegionCouplingDict_TH, `GeN-Foam -initializeMappedFields`; no protocol edit.
set -e
OUT=$1
CASEDIR=${MSR_CASE_DIR:-${GENFOAM_UPSTREAM:-/home/user/upstream-build/GeN-Foam-src}/Tutorials/reactorCases/1D_MSR_pointKinetics}
W=/home/user/upstream-build/scripts/with-of.sh
rm -rf "$OUT"; cp -r "$CASEDIR/rootCase" "$OUT"; cd "$OUT"
if grep -q 'startTime -set 0' "$CASEDIR/Allrun"; then
  $W foamDictionary system/controlDict -entry startFrom -set startTime >/dev/null
  $W foamDictionary system/controlDict -entry startTime -set 0 >/dev/null
fi
$W foamDictionary system/controlDict -entry endTime -set 100 >/dev/null
$W foamDictionary system/controlDict -entry adjustTimeStep -set true >/dev/null
$W foamDictionary system/controlDict -entry writeControl -set adjustableRunTime >/dev/null
$W foamDictionary system/controlDict -entry writeInterval -set 100 >/dev/null
cp constant/multiRegionCouplingDict_TH constant/multiRegionCouplingDict
$W GeN-Foam -initializeMappedFields > log.init 2>&1
$W GeN-Foam > log.GeN-Foam 2>&1
echo "exit $? steps $(grep -c '^Time = ' log.GeN-Foam)"
