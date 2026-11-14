#!/bin/bash
# Rebuild the reference dumps of gh:#1529 (viewFactor radiation vs OpenFOAM-v2506, ESI).
# Needs the ESI build of scripts/upstream/build-esi-v2506-and-genfoam.sh with stages
# `radiation` and `viewfactors` (faceAgglomerate, viewFactorsGen, libradiationModels).
# Run: nice -n 10 scripts/run.sh   (2 jobs).  Builds nothing in the repository.
set -e
HERE=$(cd "$(dirname "$0")/.." && pwd)
W=/home/user/upstream-build/gh1529
WITH=/home/user/upstream-build/scripts/with-of.sh
REF=$HERE/reference
mkdir -p $W/bin $W/src "$REF"
rm -rf $W/src/vfDump; cp -r $HERE/driver $W/src/vfDump
(cd $W/src/vfDump && WM_NCOMPPROCS=2 $WITH wmake)

PATCHES="xmin xmax ymin ymax zmin zmax"

# mkcase <dir> <LX LY LZ> <NX NY NZ> <agglom: none|N> <smooth> <consteps> <Tscale2> \
#        "<Tx0 Tx1 Ty0 Ty1 Tz0 Tz1>" "<eps x6>" "<qro x6>"
mkcase() {
  local d=$1 lx=$2 ly=$3 lz=$4 nx=$5 ny=$6 nz=$7 agg=$8 sm=$9 ce=${10} ts=${11}
  local -a T=(${12}) E=(${13}) Q=(${14})
  rm -rf $d; mkdir -p $d/system $d/constant $d/0
  cp $HERE/inputs/{controlDict,fvSchemes,fvSolution} $d/system/
  sed "s/@LX@/$lx/g;s/@LY@/$ly/g;s/@LZ@/$lz/g;s/@NX@/$nx/;s/@NY@/$ny/;s/@NZ@/$nz/" \
      $HERE/inputs/blockMeshDict > $d/system/blockMeshDict
  sed "s/@SMOOTH@/$sm/;s/@CONSTE@/$ce/" $HERE/inputs/radiationProperties.tmpl > $d/constant/radiationProperties
  {
    echo 'FoamFile { version 2.0; format ascii; class dictionary; object boundaryRadiationProperties; }'
    local i=0; for p in $PATCHES; do echo "$p { type lookup; emissivity ${E[$i]}; absorptivity ${E[$i]}; }"; i=$((i+1)); done
  } > $d/constant/boundaryRadiationProperties
  {
    echo 'FoamFile { version 2.0; format ascii; class volScalarField; object qr; }'
    echo 'dimensions [1 0 -3 0 0 0 0]; internalField uniform 0; boundaryField {'
    local i=0; for p in $PATCHES; do
      echo "$p { type greyDiffusiveRadiationViewFactor; qro uniform ${Q[$i]}; value uniform 0; }"; i=$((i+1)); done
    echo '}'
  } > $d/0/qr
  {
    echo 'FoamFile { version 2.0; format ascii; class dictionary; object vfDumpDict; }'
    echo "Tscale2 $ts; Twall {"
    local i=0; for p in $PATCHES; do echo "$p ${T[$i]};"; i=$((i+1)); done
    echo '}'
  } > $d/system/vfDumpDict
  (cd $d && $WITH blockMesh >log.blockMesh 2>&1)
  # put the walls in the viewFactorWall group (the model selects its patches by it)
  sed -i 's/inGroups  *1(wall);/inGroups 2(wall viewFactorWall);/' $d/constant/polyMesh/boundary
  {
    echo 'FoamFile { version 2.0; format ascii; class dictionary; object viewFactorsDict; }'
    echo 'writeViewFactorMatrix true; writeFacesAgglomeration false; dumpRays false; debug 0;'
    if [ "$agg" != none ]; then echo "\"(xmin|xmax|ymin|ymax|zmin|zmax)\" { nFacesInCoarsestLevel $agg; featureAngle 10; }"; fi
  } > $d/constant/viewFactorsDict
  if [ "$agg" != none ]; then (cd $d && $WITH faceAgglomerate >log.faceAgglomerate 2>&1); fi
  (cd $d && $WITH viewFactorsGen >log.viewFactorsGen 2>&1)
}

# dump <dir> <out>
dump() { (cd $1 && $WITH ../bin/vfDump > log.vfDump 2>&1); grep -E '^[A-Za-z_0-9]+ [0-9]+ |^#|^patch ' $1/log.vfDump > "$REF/$2"; gzip -n -9 -f "$REF/$2"; }

T6="1000 500 800 600 700 900"
E6="0.9 0.5 0.7 0.8 0.6 1.0"
Q0="0 0 0 0 0 0"
Q1="0 0 0 0 0 1000"

mkcase $W/cubeA 1 1 1 4 4 4 none false false 1.1 "$T6" "$E6" "$Q1"
mkcase $W/cubeB 1 1 1 4 4 4 4    true  true  1.1 "$T6" "$E6" "$Q1"
mkcase $W/cubeC 1 1 1 4 4 4 none false true  1.1 "$T6" "$E6" "$Q1"
mkcase $W/cubeD 1 1 1 4 4 4 none false false 1.1 "$T6" "$E6" "$Q0"
dump $W/cubeA ref_cubeA.txt
dump $W/cubeB ref_cubeB.txt
dump $W/cubeC ref_cubeC.txt
dump $W/cubeD ref_cubeD.txt
# parallel plates (gh:#1529 prediction 7): square plates of side 1 m at distance d inside a
# closed box whose four side walls are black at 0 K; zmin 1000 K eps 0.8, zmax 500 K eps 0.5
TP="0 0 0 0 1000 500"; EP="1 1 1 1 0.8 0.5"
mkcase $W/plate4  1 1 0.25   8 8 1 none false false 1.1 "$TP" "$EP" "$Q0"
mkcase $W/plate8  1 1 0.125  8 8 1 none false false 1.1 "$TP" "$EP" "$Q0"
mkcase $W/plate16 1 1 0.0625 8 8 1 none false false 1.1 "$TP" "$EP" "$Q0"
dump $W/plate4 ref_plate4.txt
dump $W/plate8 ref_plate8.txt
dump $W/plate16 ref_plate16.txt
echo done
