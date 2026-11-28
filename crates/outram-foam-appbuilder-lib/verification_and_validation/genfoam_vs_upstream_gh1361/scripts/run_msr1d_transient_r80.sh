#!/bin/bash
# gh:#1905 / gh:#1361 round 80, step 4: the REAL GeN-Foam binary on the shipped `transient` stage of
# `Tutorials/reactorCases/1D_MSR_pointKinetics` (fluid onePhase + liquid-fuel pointKinetics, loss of flow from t = 100).
# usage: run_msr1d_transient_r80.sh <steadyState run dir (Allrun's stage, written 100)> <out run dir> <MODE: full|first> [N_FIRST=60]
#   Allrun's `setTransient` edits exactly (copy 0/neutroRegion and 0/uniform into 100/, endTime 400, adjustTimeStep true,
#   writeControl adjustableRunTime, writeInterval 10, regionSolvers/neutroRegion pointKinetics, removeBaffles/neutroRegion false,
#   multiRegionCouplingDict_Coupled, `GeN-Foam -initializeMappedFields`), then the protocol edits only: writePrecision 17;
#   MODE first: writeControl timeStep, writeInterval 1, endTime = the time of step N_FIRST of `<out dir>/../tr_full/log.GeN-Foam`.
set -e
SS=$1; OUT=$2; MODE=$3; NF=${4:-60}
W=/home/user/upstream-build/scripts/with-of.sh
rm -rf "$OUT"; mkdir -p "$OUT"
cp -r "$SS/0" "$SS/100" "$SS/constant" "$SS/system" "$OUT/"
cd "$OUT"
cp -r 0/neutroRegion 100/
cp -r 0/uniform 100/
$W foamDictionary system/controlDict -entry endTime -set 400 >/dev/null
$W foamDictionary system/controlDict -entry adjustTimeStep -set true >/dev/null
$W foamDictionary system/controlDict -entry writeControl -set adjustableRunTime >/dev/null
$W foamDictionary system/controlDict -entry writeInterval -set 10 >/dev/null
$W foamDictionary system/controlDict -entry regionSolvers/neutroRegion -set pointKinetics >/dev/null
$W foamDictionary system/controlDict -entry removeBaffles/neutroRegion -set false >/dev/null
cp constant/multiRegionCouplingDict_Coupled constant/multiRegionCouplingDict
$W foamDictionary system/controlDict -entry writePrecision -set 17 >/dev/null
if [ "$MODE" = first ]; then
  T=$(awk -v n="$NF" '/^Time = /{s++; if(s==n){print $3; exit}}' "$(dirname "$OUT")/tr_full/log.GeN-Foam")
  $W foamDictionary system/controlDict -entry writeControl -set timeStep >/dev/null
  $W foamDictionary system/controlDict -entry writeInterval -set 1 >/dev/null
  $W foamDictionary system/controlDict -entry endTime -set "$T" >/dev/null
fi
$W GeN-Foam -initializeMappedFields > log.init 2>&1
$W GeN-Foam > log.GeN-Foam 2>&1
echo "exit $? steps $(grep -c '^Time = ' log.GeN-Foam)"
