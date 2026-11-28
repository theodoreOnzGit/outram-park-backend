#!/bin/bash
# gh:#1905 / gh:#1361 round 80: the REAL GeN-Foam binary (652b3da on ESI v2506, serial) on the `steadyState` stage of the shipped
# `Tutorials/reactorCases/1D_MSR_pointKinetics` (fluid region only: `regionSolvers { fluidRegion onePhase; }`, cyclic top/bottom).
# usage: run_msr1d_fluid_r80.sh <out run dir> <PROTO: tight|loose> <SOLVER: gamg|pcg> <MODE: full|first> [N_FIRST=60]
#   Allrun's steadyState edits (endTime 100, adjustTimeStep true, writeControl adjustableRunTime, multiRegionCouplingDict_TH,
#   `GeN-Foam -initializeMappedFields`), then the protocol edits only:
#   writePrecision 17; MODE full: writeControl timeStep, writeInterval 100 (every 100th step); MODE first: writeControl timeStep,
#   writeInterval 1 and endTime = the time of step N_FIRST read from `<out run dir>/../full_<PROTO>_<SOLVER>/log.GeN-Foam` (run full first).
#   PROTO tight: every `tolerance` 1e-14, every `relTol` 0 (the protocol of rounds 18-79). PROTO loose: tolerances as shipped.
#   SOLVER pcg: p_rgh/p_rghFinal `GAMG` -> `PCG`/`DIC` (the named substitute). SOLVER gamg: as shipped.
# The case directory is case-for-case what the port reads (`cases/msr1d_root` = the shipped rootCase verbatim).
# Round 81 (gh:#1962): the tutorial is a parameter, `MSR_CASE_DIR=<tutorial dir>` (default the shipped 1D_MSR_pointKinetics; e.g.
#   `MSR_CASE_DIR=.../Tutorials/reactorCases/1D_thermalMSR_pointKinetics`, the port's copy `cases/msr1d_thermal_root`). An Allrun edit
#   that only some tutorials' Allrun performs (`startFrom startTime`, `startTime 0` in 1D_thermalMSR's setSteadyState) is applied when
#   THAT tutorial's Allrun contains it (for 1D_MSR both are no-ops: `startFrom latestTime` with only `0` written).
set -e
OUT=$1; PROTO=$2; SOLVER=$3; MODE=$4; NF=${5:-60}
HERE=$(cd "$(dirname "$0")" && pwd)
CASEDIR=${MSR_CASE_DIR:-${GENFOAM_UPSTREAM:-/home/user/upstream-build/GeN-Foam-src}/Tutorials/reactorCases/1D_MSR_pointKinetics}
SRC=$CASEDIR/rootCase
W=/home/user/upstream-build/scripts/with-of.sh
rm -rf "$OUT"; cp -r "$SRC" "$OUT"; cd "$OUT"
if [ -n "$MSR_VARIANT" ]; then
  # round 81: a `kappaMatrix` variant of 1D_thermalMSR (`make_msr1d_thermal_kappa_r81.py`: core|seam), NOT shipped
  python3 -I "$HERE/make_msr1d_thermal_kappa_r81.py" constant/fluidRegion/phaseProperties "$MSR_VARIANT"
fi
if grep -q 'startTime -set 0' "$CASEDIR/Allrun"; then
  $W foamDictionary system/controlDict -entry startFrom -set startTime >/dev/null
  $W foamDictionary system/controlDict -entry startTime -set 0 >/dev/null
fi
$W foamDictionary system/controlDict -entry endTime -set 100 >/dev/null
$W foamDictionary system/controlDict -entry adjustTimeStep -set true >/dev/null
$W foamDictionary system/controlDict -entry writeControl -set timeStep >/dev/null
$W foamDictionary system/controlDict -entry writePrecision -set 17 >/dev/null
if [ "$MODE" = full ]; then
  $W foamDictionary system/controlDict -entry writeInterval -set 100 >/dev/null
else
  T=$(awk -v n="$NF" '/^Time = /{s++; if(s==n){print $3; exit}}' "$(dirname "$OUT")/full_${PROTO}_${SOLVER}/log.GeN-Foam")
  $W foamDictionary system/controlDict -entry writeInterval -set 1 >/dev/null
  $W foamDictionary system/controlDict -entry endTime -set "$T" >/dev/null
fi
cp constant/multiRegionCouplingDict_TH constant/multiRegionCouplingDict
f=system/fluidRegion/fvSolution
if [ "$PROTO" = tight ]; then
  sed -i -E 's/^( *)tolerance +[0-9.e+-]+; *$/\1tolerance       1e-14;/; s/^( *)relTol +[0-9.e+-]+; *(\/\/.*)?$/\1relTol          0;/' $f
fi
if [ "$SOLVER" = pcg ]; then
  perl -0pi -e 's/solver\s+GAMG;\s*smoother\s+DIC;/solver          PCG;\n        preconditioner  DIC;/g' $f
fi
$W GeN-Foam -initializeMappedFields > log.init 2>&1
$W GeN-Foam > log.GeN-Foam 2>&1
echo "exit $? steps $(grep -c '^Time = ' log.GeN-Foam)"
