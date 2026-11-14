#!/bin/bash
# gh:#1541. The gh:#1363 two-region case with the ESI `viewFactor` radiation model ON in the fluid, run by
# the gh:#1531 driver (../../upstream_radiation_gh1531/driver/chtRadDump.C) on ESI v2506.
# Needs libchtBCs (../../upstream_openfoam_gh1363/scripts/build_cht_bcs.sh), libradiationModels and the
# `viewfactors` stage (viewFactorsGen) of scripts/upstream/build-esi-v2506-and-genfoam.sh.
# Only change to the gh:#1363 case: the fluid `frontAndBack` is a `wall` (not `empty`), so the fluid
# boundary is a closed 3-D box and viewFactorsGen takes its 3-D branch.
# Writes reference/cht_vf_v2506.txt.gz, the fluid polyMesh copy in fluid/polyMesh and the three
# viewFactorsGen files gzipped in reference/.   usage: run_cht_vf.sh [nSteps]   env: NOREF=1
set -e
NSTEPS=${1:-50}
HERE=$(cd "$(dirname "$0")/.." && pwd)
R31=$HERE/../upstream_radiation_gh1531
SRC13=$HERE/../upstream_openfoam_gh1363
W=/home/user/upstream-build/gh1541
WITH=/home/user/upstream-build/scripts/with-of.sh
C=$W/case
mkdir -p $W/bin $W/src
rm -rf $W/src/chtRadDump; mkdir -p $W/src/chtRadDump/Make
cp $R31/driver/chtRadDump.C $W/src/chtRadDump/
cp $R31/driver/Make/options $W/src/chtRadDump/Make/options
printf 'chtRadDump.C\n\nEXE = %s/bin/chtRadDump\n' $W > $W/src/chtRadDump/Make/files
(cd $W/src/chtRadDump && WM_NCOMPPROCS=2 $WITH wmake)
rm -rf $W/src/chtRadDump/Make/linux*

rm -rf $C; mkdir -p $C
cp -r $SRC13/cht/constant $SRC13/cht/system $SRC13/cht/0 $C/
rm -rf $C/constant/*/reference $C/reference
# fluid boundary: frontAndBack empty -> wall; every non-empty patch in the viewFactorWall group
B=$C/constant/fluid/polyMesh/boundary
python3 -I - "$B" <<'PY'
import sys
p = sys.argv[1]
s = open(p).read()
s = s.replace("        type            patch;\n        nFaces          5;\n        startFace       175;",
              "        type            patch;\n        inGroups        1(viewFactorWall);\n        nFaces          5;\n        startFace       175;")
s = s.replace("inGroups        2(wall mappedPatch);", "inGroups        3(wall mappedPatch viewFactorWall);")
s = s.replace("        type            wall;\n        inGroups        1(wall);\n        nFaces          40;",
              "        type            wall;\n        inGroups        2(wall viewFactorWall);\n        nFaces          40;")
s = s.replace("        type            empty;\n        inGroups        1(empty);", "        type            wall;\n        inGroups        2(wall viewFactorWall);")
assert s.count("viewFactorWall") == 4 and "empty" not in s
open(p, "w").write(s)
PY
cat > $C/constant/fluid/radiationProperties <<'EOF'
FoamFile { version 2.0; format ascii; class dictionary; object radiationProperties; }
radiation on;
radiationModel viewFactor;
viewFactorCoeffs { smoothing false; constantEmissivity false; useDirectSolver true; }
solverFreq 1;
absorptionEmissionModel none;
scatterModel none;
sootModel none;
EOF
cat > $C/constant/fluid/boundaryRadiationProperties <<'EOF'
FoamFile { version 2.0; format ascii; class dictionary; object boundaryRadiationProperties; }
left         { type lookup; emissivity 0.9; absorptivity 0.9; }
interface    { type lookup; emissivity 0.8; absorptivity 0.8; }
walls        { type lookup; emissivity 0.5; absorptivity 0.5; }
frontAndBack { type lookup; emissivity 0.6; absorptivity 0.6; }
EOF
cat > $C/constant/fluid/viewFactorsDict <<'EOF'
FoamFile { version 2.0; format ascii; class dictionary; object viewFactorsDict; }
writeViewFactorMatrix true; writeFacesAgglomeration false; dumpRays false; debug 0;
EOF
cat > $C/0/fluid/qr <<'EOF'
FoamFile { version 2.0; format ascii; class volScalarField; object qr; }
dimensions [1 0 -3 0 0 0 0];
internalField uniform 0;
boundaryField {
    "(left|interface|walls|frontAndBack)" { type greyDiffusiveRadiationViewFactor; qro uniform 0; value uniform 0; }
}
EOF
sed -i 's/    frontAndBack { type empty; }/    frontAndBack { type zeroGradient; }/' $C/0/fluid/T
sed -i 's/    frontAndBack { type empty; }/    frontAndBack { type noSlip; }/' $C/0/fluid/U
sed -i 's/    frontAndBack { type empty; }/    frontAndBack { type calculated; value uniform 1e5; }/' $C/0/fluid/p
# the interface conditions carry the radiative flux as in ESI's solarBeamWithTrees
sed -i 's/ qr none;/ qr qr;/' $C/0/fluid/T
sed -i 's/qrNbr none;/qrNbr qr;/' $C/0/solid/T
sed -i "s|^endTime .*|endTime $(awk -v d=0.1 -v n=$NSTEPS 'BEGIN{printf "%.17g", d*(n-0.25)}');|" $C/system/controlDict
(cd $C && $WITH viewFactorsGen -region fluid > $W/viewFactorsGen.log 2>&1) || { tail -30 $W/viewFactorsGen.log; exit 1; }
ls $C/constant/fluid
(cd $C && $WITH $W/bin/chtRadDump > $W/cht_vf.log 2>&1) || { tail -30 $W/cht_vf.log; exit 1; }
grep -E "^S |^C " $W/cht_vf.log > $W/ref_cht_vf.txt
if [ -z "$NOREF" ]; then
  mkdir -p $HERE/reference $HERE/fluid/polyMesh
  gzip -9n -c $W/ref_cht_vf.txt > $HERE/reference/cht_vf_v2506.txt.gz
  cp $C/constant/fluid/polyMesh/{points,faces,owner,neighbour,boundary} $HERE/fluid/polyMesh/
  for f in F globalFaceFaces mapDist; do gzip -9n -c $C/constant/fluid/$f > $HERE/reference/$f.gz; done
fi
tail -3 $W/cht_vf.log
