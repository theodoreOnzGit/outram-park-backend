#!/bin/bash
# Rebuild the reference dumps of gh:#1379 (P1 radiation vs OpenFOAM-v2506, ESI).
# Needs the ESI build of scripts/upstream/build-esi-v2506-and-genfoam.sh
# (stage `radiation` builds libradiationModels and the radDump driver);
# builds nothing in the repo. Run: nice -n 10 scripts/run.sh   (2 jobs).
set -e
HERE=$(cd "$(dirname "$0")/.." && pwd)
W=/home/user/upstream-build/gh1379
WITH=/home/user/upstream-build/scripts/with-of.sh
REF=$HERE/reference
mkdir -p $W/bin $W/src "$REF"
rm -rf $W/src/radDump; cp -r $HERE/driver $W/src/radDump
(cd $W/src/radDump && WM_NCOMPPROCS=2 $WITH wmake)

# mkcase <dir> <mesh-kind: hex|slab> <A> <E> <EE> [ncells]
mkcase() {
  local d=$1 kind=$2 a=$3 e=$4 ee=$5 n=${6:-40}
  rm -rf $d; mkdir -p $d/system $d/constant $d/0
  cp $HERE/inputs/common/* $d/system/
  cp $HERE/inputs/$kind/system_radDumpDict $d/system/radDumpDict
  cp $HERE/inputs/$kind/0/G $d/0/G
  cp $HERE/inputs/$kind/constant/boundaryRadiationProperties $d/constant/
  sed "s/@A@/$a/;s/@E@/$e/;s/@EE@/$ee/" $HERE/inputs/radiationProperties.tmpl > $d/constant/radiationProperties
  sed "s/NCELLS/$n/" $HERE/inputs/$kind/blockMeshDict > $d/system/blockMeshDict
}

# run <dir> <out-name> [extra args]: dump + the PCG iteration counts of the log
run() {
  local d=$1 out=$2; shift 2
  (cd $d && $WITH blockMesh >/dev/null && $WITH ../bin/radDump "$@" > log.radDump 2>&1)
  { grep -E "^[A-Za-z_0-9]+ [0-9]+ |^#" $d/log.radDump
    grep "Solving for G" $d/log.radDump | sed 's/^/# solve /'; } > "$REF/$out"
}

mkcase $W/hexA hex 2.0 2.0 0
mkcase $W/hexB hex 1.5 1.2 500
mkcase $W/slabThick slab 2.0 2.0 0 40
mkcase $W/slabThin slab 0.5 0.5 0 40
run $W/hexA   ref_hexA.txt
run $W/hexB   ref_hexB.txt -noGeom
run $W/slabThick ref_slab_thick.txt
run $W/slabThin  ref_slab_thin.txt -noGeom
ls -l "$REF"
