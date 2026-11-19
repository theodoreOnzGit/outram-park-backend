#!/bin/bash
# gh:#1363 row 4. Write the two-region case (one frozen-flow fluid, one solid),
# mesh each region with the ESI blockMesh, build driver/chtDump.C against the
# ESI v2506 closure (+ libchtBCs from scripts/build_cht_bcs.sh), run it and
# write the committed gzipped reference.
#   usage: run_cht.sh [deltaT] [nSteps]
#   env: NOREF=1 leaves the committed reference untouched
set -e
DT=${1:-0.1}
NSTEPS=${2:-50}
HERE=$(cd "$(dirname "$0")/.." && pwd)
W=/home/user/upstream-build/gh1363
WITH=/home/user/upstream-build/scripts/with-of.sh
C=$HERE/cht
mkdir -p $W/bin
rm -rf $W/src/chtDump; mkdir -p $W/src/chtDump/Make
cp $HERE/driver/chtDump.C $W/src/chtDump/
printf 'chtDump.C\n\nEXE = %s/bin/chtDump\n' $W > $W/src/chtDump/Make/files
cat > $W/src/chtDump/Make/options <<EOF
EXE_INC = \\
    -I\$(LIB_SRC)/finiteVolume/lnInclude \\
    -I\$(LIB_SRC)/meshTools/lnInclude \\
    -I\$(LIB_SRC)/sampling/lnInclude \\
    -I\$(LIB_SRC)/transportModels/compressible/lnInclude \\
    -I\$(LIB_SRC)/thermophysicalModels/basic/lnInclude \\
    -I\$(LIB_SRC)/thermophysicalModels/specie/lnInclude \\
    -I\$(LIB_SRC)/thermophysicalModels/solidThermo/lnInclude \\
    -I\$(LIB_SRC)/thermophysicalModels/solidSpecie/lnInclude \\
    -I\$(LIB_SRC)/thermophysicalModels/reactionThermo/lnInclude \\
    -I\$(LIB_SRC)/thermophysicalModels/thermophysicalProperties/lnInclude \\
    -I\$(LIB_SRC)/thermophysicalModels/radiation/lnInclude \\
    -I\$(LIB_SRC)/TurbulenceModels/turbulenceModels/lnInclude \\
    -I\$(LIB_SRC)/TurbulenceModels/compressible/lnInclude \\
    -I\$(LIB_SRC)/regionModels/regionModel/lnInclude

EXE_LIBS = \\
    -lfiniteVolume \\
    -lfvOptions \\
    -lmeshTools \\
    -lsampling \\
    -lcompressibleTransportModels \\
    -lfluidThermophysicalModels \\
    -lspecie \\
    -lsolidThermo \\
    -lradiationModels \\
    -lturbulenceModels \\
    -lcompressibleTurbulenceModels \\
    -lregionModels
EOF
(cd $W/src/chtDump && WM_NCOMPPROCS=2 $WITH wmake)
rm -rf $W/src/chtDump/Make/linux*

rm -rf $C; mkdir -p $C/system/fluid $C/system/solid $C/constant/fluid $C/constant/solid $C/0/fluid $C/0/solid
HDR() { printf 'FoamFile { version 2.0; format ascii; class %s; object %s; }\n' "$1" "$2"; }
{ HDR dictionary controlDict
cat <<EOF
application chtDump;
libs ("$W/bin/libchtBCs.so");
startFrom startTime;
startTime 0;
stopAt endTime;
endTime $(awk -v d=$DT -v n=$NSTEPS 'BEGIN{printf "%.17g", d*(n-0.25)}');
deltaT $DT;
writeControl timeStep;
writeInterval 100000;
writeFormat ascii;
writePrecision 17;
timeFormat general;
timePrecision 8;
runTimeModifiable false;
adjustTimeStep no;
interfacePatch interface;
EOF
} > $C/system/controlDict
{ HDR dictionary fvSchemes
cat <<'EOF'
ddtSchemes { default Euler; }
gradSchemes { default Gauss linear; }
divSchemes { default none; div(phi,h) Gauss upwind; div(phi,K) Gauss linear; }
laplacianSchemes { default Gauss linear corrected; }
interpolationSchemes { default linear; }
snGradSchemes { default corrected; }
EOF
} > $C/system/fvSchemes
cp $C/system/fvSchemes $C/system/fluid/fvSchemes
cp $C/system/fvSchemes $C/system/solid/fvSchemes
{ HDR dictionary fvSolution
cat <<'EOF'
solvers { }
PIMPLE { nOuterCorrectors 1; }
EOF
} > $C/system/fvSolution
{ HDR dictionary fvSolution
cat <<'EOF'
solvers
{
    "h.*" { solver PBiCGStab; preconditioner DILU; tolerance 1e-15; relTol 0; maxIter 1000; }
}
PIMPLE { frozenFlow yes; nNonOrthogonalCorrectors 0; }
EOF
} > $C/system/fluid/fvSolution
{ HDR dictionary fvSolution
cat <<'EOF'
solvers
{
    "h.*" { solver PCG; preconditioner DIC; tolerance 1e-15; relTol 0; maxIter 1000; }
}
PIMPLE { nNonOrthogonalCorrectors 0; }
EOF
} > $C/system/solid/fvSolution
{ HDR dictionary regionProperties
cat <<'EOF'
regions ( fluid (fluid) solid (solid) );
EOF
} > $C/constant/regionProperties
{ HDR uniformDimensionedVectorField g
cat <<'EOF'
dimensions [0 1 -2 0 0 0 0];
value (0 -9.81 0);
EOF
} > $C/constant/g
for r in fluid solid; do
  { HDR dictionary radiationProperties
    echo "radiation off; radiationModel none;"; } > $C/constant/$r/radiationProperties
done
{ HDR dictionary momentumTransport
  echo "simulationType laminar;"; } > $C/constant/fluid/momentumTransport
cp $C/constant/fluid/momentumTransport $C/constant/fluid/turbulenceProperties
# fluid: the multiRegionHeater bottomWater properties; solid: its heater properties
{ HDR dictionary thermophysicalProperties
cat <<'EOF'
thermoType { type heRhoThermo; mixture pureMixture; transport const; thermo hConst; equationOfState rhoConst; specie specie; energy sensibleEnthalpy; }
mixture
{
    specie { molWeight 18; }
    equationOfState { rho 1000; }
    thermodynamics { Cp 4181; Hf 0; }
    transport { mu 959e-6; Pr 6.62; }
}
EOF
} > $C/constant/fluid/thermophysicalProperties
{ HDR dictionary thermophysicalProperties
cat <<'EOF'
thermoType { type heSolidThermo; mixture pureMixture; transport constIso; thermo hConst; equationOfState rhoConst; specie specie; energy sensibleEnthalpy; }
mixture
{
    specie { molWeight 50; }
    transport { kappa 80; }
    thermodynamics { Hf 0; Cp 450; }
    equationOfState { rho 8000; }
}
EOF
} > $C/constant/solid/thermophysicalProperties

# meshes: fluid x 0..0.02 (20 cells, graded x2 toward the interface),
# solid x 0.02..0.03 (10 cells, graded x0.5 away from the interface); both
# y 0..0.01 (5 cells), z 0..0.001 (1 cell, empty front and back)
mkblock() { # region x0 x1 nx grading leftName rightName rightType
cat <<EOF
FoamFile { version 2.0; format ascii; class dictionary; object blockMeshDict; }
convertToMeters 1;
vertices ( ($2 0 0) ($3 0 0) ($3 0.01 0) ($2 0.01 0) ($2 0 0.001) ($3 0 0.001) ($3 0.01 0.001) ($2 0.01 0.001) );
blocks ( hex (0 1 2 3 4 5 6 7) ($4 5 1) simpleGrading ($5 1 1) );
edges ();
boundary
(
    $6 { type $8; faces ((0 4 7 3)); ${10} }
    $7 { type $9; faces ((1 2 6 5)); ${11} }
    walls { type wall; faces ((0 1 5 4) (3 7 6 2)); }
    frontAndBack { type empty; faces ((0 3 2 1) (4 5 6 7)); }
);
EOF
}
IFACE_F="sampleMode nearestPatchFace; sampleRegion solid; samplePatch interface;"
IFACE_S="sampleMode nearestPatchFace; sampleRegion fluid; samplePatch interface;"
mkblock fluid 0 0.02 20 2 left interface patch mappedWall "" "$IFACE_F" > $C/system/fluid/blockMeshDict
mkblock solid 0.02 0.03 10 0.5 interface right mappedWall patch "$IFACE_S" "" > $C/system/solid/blockMeshDict
(cd $C && $WITH blockMesh -region fluid > $W/blockMesh_fluid.log 2>&1 && $WITH blockMesh -region solid > $W/blockMesh_solid.log 2>&1)

fld() { # region name class dims internal body
  { HDR $3 $2
    echo "dimensions $4;"; echo "internalField $5;"; echo "boundaryField {"; echo "$6"; echo "}"; } > $C/0/$1/$2
}
fld fluid T volScalarField "[0 0 0 1 0 0 0]" "uniform 300" "
    left { type fixedValue; value uniform 300; }
    interface { type compressible::turbulentTemperatureRadCoupledMixed; Tnbr T; kappaMethod fluidThermo; qrNbr none; qr none; value uniform 300; }
    walls { type zeroGradient; }
    frontAndBack { type empty; }"
fld solid T volScalarField "[0 0 0 1 0 0 0]" "uniform 300" "
    interface { type compressible::turbulentTemperatureRadCoupledMixed; Tnbr T; kappaMethod solidThermo; qrNbr none; qr none; value uniform 300; }
    right { type fixedValue; value uniform 360; }
    walls { type zeroGradient; }
    frontAndBack { type empty; }"
fld fluid p volScalarField "[1 -1 -2 0 0 0 0]" "uniform 1e5" "
    \"(left|interface|walls)\" { type calculated; value uniform 1e5; }
    frontAndBack { type empty; }"
fld solid p volScalarField "[1 -1 -2 0 0 0 0]" "uniform 1e5" "
    \"(interface|right|walls)\" { type calculated; value uniform 1e5; }
    frontAndBack { type empty; }"
fld fluid U volVectorField "[0 1 -1 0 0 0 0]" "uniform (0 0 0)" "
    \"(left|interface|walls)\" { type noSlip; }
    frontAndBack { type empty; }"

(cd $C && $WITH $W/bin/chtDump > $W/cht.log 2>&1)
grep -E "^S |^C " $W/cht.log > $W/ref_cht.txt
[ -n "$NOREF" ] || { mkdir -p $HERE/cht/reference; gzip -9n -c $W/ref_cht.txt > $HERE/cht/reference/cht_v2506.txt.gz; }
tail -3 $W/cht.log
