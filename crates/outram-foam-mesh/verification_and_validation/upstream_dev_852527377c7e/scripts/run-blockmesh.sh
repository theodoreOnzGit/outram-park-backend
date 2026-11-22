#!/bin/bash
# Run OpenFOAM-dev blockMesh on each fixture dict (writePrecision 17).
# Output: $DEV_WORK/blockMesh/<case>/constant/polyMesh. FIX = the port's tests/fixtures dir.
. /home/user/upstream-build/scripts/of-dev-env.sh
W=${DEV_WORK:?set DEV_WORK to a scratch directory}/blockMesh
FIX=${FIX:?set FIX to crates/outram-foam-mesh/tests/fixtures}
CAV=${CAV:?set CAV to crates/outram-foam-mesh/tests/blockMeshDict.cavity}
rm -rf $W; mkdir -p $W
run() { # name dictfile
  d=$W/$1; mkdir -p $d/system $d/constant; cp "$2" $d/system/blockMeshDict
  cat > $d/system/controlDict <<'CD'
FoamFile { format ascii; class dictionary; object controlDict; }
application blockMesh; startFrom startTime; startTime 0; stopAt endTime; endTime 1;
deltaT 1; writeControl timeStep; writeInterval 1; writeFormat ascii;
writePrecision 17;
writeCompression off; timeFormat general; timePrecision 6; runTimeModifiable false;
CD
  ( cd $d; blockMesh > log.blockMesh 2>&1; echo "$1: exit $? $(grep -c . log.blockMesh) log lines; $(ls constant/polyMesh | tr '\n' ' ')" )
}
run cavity "$CAV"
run T3A $FIX/T3A_blockMeshDict
run pitzDailySteady $FIX/pitzDailySteady_blockMeshDict
for f in $FIX/proj_*.dict $FIX/sweep_*.dict; do n=$(basename $f .dict); run $n $f; done
