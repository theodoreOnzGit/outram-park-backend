#!/bin/bash
# gh:#1781 round 23: stage the `legacyThermoMechanics` differential case from upstream's
# Tutorials/reactorCases/3D_SmallESFR_NewSolverVerification/legacySolver (thermoMechanicalRegion).
# usage: [SHIPPED=1] make_legacy_tm_case.sh <out case dir>   (SHIPPED=1: the tutorial's own fvSolution, i.e. the shipped tolerances and GAMG for TStruct)
# The ONLY edits to the tutorial's thermoMechanicalRegion (each one stated in README "Round 23"):
#   1. D boundary conditions: `GFTractionDisplacement` (gh:#1235, not ported) -> fixedValue 0 on zeroDispl,
#      bottom, bottomCR; zeroGradient on top, topCR, defaultFaces;
#   2. zone materials differ (the tutorial's eight cards are identical, so field-valued mu/lambda would be uniform);
#   3. tight protocol: every linear solver tolerance 1e-14, relTol 0, TStruct GAMG -> PBiCG/DILU (named deviation: the row-scaled TEqn is asymmetric, PCG is refused by OpenFOAM),
#      writePrecision 17, writeControl timeStep;
#   4. TStruct initial field 668 K (READ_IF_PRESENT), instead of 0 K.
# gh:#1235 round 24: TRACTION=t0|t1 uses the GFTractionDisplacement disp of make_traction_disp.sh (t0 = the tutorial's own file); PLANE_STRESS=1 sets planeStress yes; MAXITER=n sets maxIter of every linear solver (default 1000).
set -e
[ -n "$PLANE_STRESS" ] && PLANE_STRESS_WORD=yes
SRC=/home/user/upstream-build/GeN-Foam-src/Tutorials/reactorCases/3D_SmallESFR_NewSolverVerification/legacySolver
OUT=$1
rm -rf "$OUT"
mkdir -p "$OUT/constant/thermoMechanicalRegion" "$OUT/system/thermoMechanicalRegion" "$OUT/0/thermoMechanicalRegion"
cp -r "$SRC/constant/thermoMechanicalRegion/polyMesh" "$OUT/constant/thermoMechanicalRegion/"
rm -rf "$OUT/constant/thermoMechanicalRegion/polyMesh/sets"
cp "$SRC/system/thermoMechanicalRegion/fvSchemes" "$OUT/system/thermoMechanicalRegion/"
cp "$SRC/0/thermoMechanicalRegion/fuelDisp" "$SRC/0/thermoMechanicalRegion/CRDisp" "$OUT/0/thermoMechanicalRegion/"
HDR='FoamFile
{
    version     2.0;
    format      ascii;
    class       CLASS;
    object      OBJ;
}
'
hdr() { echo "$HDR" | sed "s/CLASS/$1/; s/OBJ/$2/"; }
{ hdr dictionary controlDict; cat <<'EOF'
application     GeN-Foam;
startFrom       startTime;
startTime       0;
stopAt          endTime;
endTime         3;
deltaT          1;
writeControl    timeStep;
writeInterval   1;
purgeWrite      0;
writeFormat     ascii;
writePrecision  17;
writeCompression off;
timeFormat      general;
timePrecision   8;
runTimeModifiable no;
EOF
} > "$OUT/system/controlDict"
if [ -n "$SHIPPED" ]; then cp "$SRC/system/thermoMechanicalRegion/fvSolution" "$OUT/system/thermoMechanicalRegion/fvSolution"; else
{ hdr dictionary fvSolution; cat <<EOF
solvers
{
    ".*"
    {
        solver          PCG;
        preconditioner  DIC;
        tolerance       1e-14;
        maxIter         ${MAXITER:-1000};
        relTol          0;
    }
    "fuelDisp"
    {
        solver          PBiCG;
        preconditioner  DILU;
        tolerance       1e-14;
        maxIter         ${MAXITER:-1000};
        relTol          0;
    }
    "CRDisp"
    {
        solver          PBiCG;
        preconditioner  DILU;
        tolerance       1e-14;
        maxIter         ${MAXITER:-1000};
        relTol          0;
    }
    "TStruct"
    {
        solver          PBiCG;
        preconditioner  DILU;
        tolerance       1e-14;
        maxIter         ${MAXITER:-1000};
        relTol          0;
    }
}
stressAnalysis
{
    compactNormalStress yes;
    nCorrectors     3;
    D               1e-06;
}
relaxationFactors
{
    fields
    {
        Disp 0.99;
    }
}
EOF
} > "$OUT/system/thermoMechanicalRegion/fvSolution"
fi
mat() { # name E nu alpha [extra lines]
  cat <<EOF
    $1
    {
        material        constant;
        rho         rho     [1 -3 0 0 0]    $2;
        Cp          Cp      [0 2 -2 -1 0]   $3;
        k           k       [1 1 3 -1 0]    $4;
        emissivity emissivity [0 0 0 0 0]   0.0;
        E           E       [1 -1 -2 0 0]   $5;
        nu          nu      [0 0 0 0 0]     $6;
        G           G       [1 -1 -2 0 0]   0.0;
        alpha       alpha   [0 0 0 0 0]     $7;
        Tref        Tref    [0 0 0 1 0]     668;
$8
    }
EOF
}
{ hdr dictionary solverDict; cat <<EOF
thermalSolver       on;
mechanicsSolver     on;
globalOptions
{
    linkedFuel      false;
    pinDirection    (0 0 1);
}
rheologyOption
{
    planeStress 	${PLANE_STRESS_WORD:-no};
}
materials
{
EOF
mat innerCore 1000 200 5 1e9 0.30 1.8e-5 "        alphaFuel   alphaFuel   [0 0 0 0 0]     1.1e-5;
        TFuelRef    TFuelRef    [0 0 0 1 0]     668;"
mat outerCore 1100 210 6 1.2e9 0.28 1.7e-5 "        alphaFuel   alphaFuel   [0 0 0 0 0]     1.0e-5;
        TFuelRef    TFuelRef    [0 0 0 1 0]     700;"
mat follower 900 190 4 0.8e9 0.33 1.9e-5 ""
mat controlRod 1200 220 7 1.5e9 0.25 2.0e-5 "        TCRRef      TCRRef  [0 0 0 1 0]     668;
        alphaCR     alphaCR   [0 0 0 0 0]   5.4e-5;"
mat diagrid 1500 250 9 2.0e9 0.27 1.6e-5 ""
mat radialReflector 1300 230 8 1.1e9 0.31 1.5e-5 ""
mat rest 1000 200 5 0.9e9 0.35 1.8e-5 ""
mat softStructure 800 180 3 0.5e9 0.40 2.1e-5 ""
echo "}"
} > "$OUT/constant/thermoMechanicalRegion/thermoMechanicalProperties"
{ hdr volScalarField TStruct; cat <<'EOF'
dimensions      [0 0 0 1 0 0 0];
internalField   uniform 668;
boundaryField
{
    ".*"
    {
        type            zeroGradient;
    }
}
EOF
} > "$OUT/0/thermoMechanicalRegion/TStruct"
{ hdr volVectorField disp; cat <<'EOF'
dimensions      [0 1 0 0 0 0 0];
internalField   uniform (0 0 0);
boundaryField
{
    zeroDispl { type fixedValue; value uniform (0 0 0); }
    bottom    { type fixedValue; value uniform (0 0 0); }
    bottomCR  { type fixedValue; value uniform (0 0 0); }
    top       { type zeroGradient; }
    topCR     { type zeroGradient; }
    defaultFaces { type zeroGradient; }
}
EOF
} > "$OUT/0/thermoMechanicalRegion/disp"
[ -n "$TRACTION" ] && bash "$(dirname "$0")/make_traction_disp.sh" "$TRACTION" "$OUT/0/thermoMechanicalRegion/disp"
echo "staged $OUT"
