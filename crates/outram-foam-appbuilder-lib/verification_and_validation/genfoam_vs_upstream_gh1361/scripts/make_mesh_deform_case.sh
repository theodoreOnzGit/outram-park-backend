#!/bin/bash
# gh:#1784 round 26: stage the mesh-deformation case from upstream's 3D_SmallESFR_NewSolverVerification/legacySolver:
# the mechanics region (make_legacy_tm_case.sh, which carries its own `disp` with fixedValue 0 patches), plus the neutroRegion
# and fluidRegion polyMesh (points, faces, owner, neighbour, boundary only; zones and sets dropped), their `disp` fields, and a
# multiRegionCouplingDict whose `meshDeformation` lists the three regions (upstream's lists only neutroRegion).
# The ONLY edits to the tutorial: the zones are dropped from the two added meshes; fluidRegion/0/disp is new (fixedValue on `bottom`, to
# exercise a non-zeroGradient patch); `meshDeformation` names all three regions.
# usage: make_mesh_deform_case.sh <out case dir>
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
SRC=/home/user/upstream-build/GeN-Foam-src/Tutorials/reactorCases/3D_SmallESFR_NewSolverVerification/legacySolver
OUT=$1
bash $HERE/make_legacy_tm_case.sh "$OUT" >/dev/null
for r in neutroRegion fluidRegion; do
  mkdir -p "$OUT/constant/$r/polyMesh" "$OUT/0/$r"
  for f in points faces owner neighbour; do cp "$SRC/constant/$r/polyMesh/$f.gz" "$OUT/constant/$r/polyMesh/"; done
  cp "$SRC/constant/$r/polyMesh/boundary" "$OUT/constant/$r/polyMesh/"
done
for r in neutroRegion fluidRegion; do mkdir -p "$OUT/system/$r"; cp "$OUT/system/thermoMechanicalRegion/fvSchemes" "$OUT/system/thermoMechanicalRegion/fvSolution" "$OUT/system/$r/"; done   # fvMesh reads them; unused
cp "$SRC/0/neutroRegion/disp" "$OUT/0/neutroRegion/disp"
cat > "$OUT/0/fluidRegion/disp" <<'EOT'
FoamFile
{
    version     2.0;
    format      ascii;
    class       volVectorField;
    object      disp;
}
dimensions      [0 1 0 0 0 0 0];
internalField   uniform (0 0 0);
boundaryField
{
    top       { type zeroGradient; }
    bottom    { type fixedValue; value uniform (0.0001 -0.0002 0.0003); }
    walls     { type zeroGradient; }
    baffle0   { type zeroGradient; }
    baffle1   { type zeroGradient; }
}
EOT
cat > "$OUT/constant/multiRegionCouplingDict" <<'EOT'
FoamFile
{
    version     2.0;
    format      ascii;
    class       dictionary;
    object      multiRegionCouplingDict;
}
meshDeformation
{
    neutroRegion            { displacementField disp; }
    fluidRegion             { displacementField disp; }
    thermoMechanicalRegion  { displacementField disp; }
}
mappings
{
}
EOT
