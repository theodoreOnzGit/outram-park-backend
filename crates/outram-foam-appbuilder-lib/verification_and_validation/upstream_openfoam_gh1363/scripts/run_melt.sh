#!/bin/bash
# gh:#1363 row 6. Write a 1-D melting slab (a solid with ESI v2506's
# `solidificationMelting` fvOption), mesh it with the ESI blockMesh, build
# driver/meltDump.C against the ESI v2506 closure, run it and write the committed
# gzipped reference.
#   usage: run_melt.sh [deltaT] [nSteps]
#   env: NOREF=1 leaves the committed reference untouched
set -e
DT=${1:-0.5}
NSTEPS=${2:-200}
HERE=$(cd "$(dirname "$0")/.." && pwd)
W=/home/user/upstream-build/gh1363
WITH=/home/user/upstream-build/scripts/with-of.sh
C=$HERE/melt
mkdir -p $W/bin
rm -rf $W/src/meltDump; mkdir -p $W/src/meltDump/Make
cp $HERE/driver/meltDump.C $W/src/meltDump/
printf 'meltDump.C\n\nEXE = %s/bin/meltDump\n' $W > $W/src/meltDump/Make/files
cat > $W/src/meltDump/Make/options <<EOF
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
    -lregionModels
EOF
(cd $W/src/meltDump && WM_NCOMPPROCS=2 $WITH wmake)
rm -rf $W/src/meltDump/Make/linux*

rm -rf $C; mkdir -p $C/system $C/constant $C/0
HDR() { printf 'FoamFile { version 2.0; format ascii; class %s; object %s; }\n' "$1" "$2"; }
{ HDR dictionary controlDict
cat <<EOF
application meltDump;
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
EOF
} > $C/system/controlDict
{ HDR dictionary fvSchemes
cat <<'EOF'
ddtSchemes { default Euler; }
gradSchemes { default Gauss linear; }
divSchemes { default none; }
laplacianSchemes { default Gauss linear corrected; }
interpolationSchemes { default linear; }
snGradSchemes { default corrected; }
EOF
} > $C/system/fvSchemes
{ HDR dictionary fvSolution
cat <<'EOF'
solvers
{
    "h.*" { solver PCG; preconditioner DIC; tolerance 1e-15; relTol 0; maxIter 1000; }
}
EOF
} > $C/system/fvSolution
# gallium-like solid: rho 6093, Cp 381.5, kappa 32 W/m/K, L 80160 J/kg, Tmelt 302.78 K
{ HDR dictionary thermophysicalProperties
cat <<'EOF'
thermoType { type heSolidThermo; mixture pureMixture; transport constIso; thermo hConst; equationOfState rhoConst; specie specie; energy sensibleEnthalpy; }
mixture
{
    specie { molWeight 69.7; }
    transport { kappa 32; }
    thermodynamics { Hf 0; Cp 381.5; }
    equationOfState { rho 6093; }
}
EOF
} > $C/constant/thermophysicalProperties
{ HDR dictionary fvOptions
cat <<'EOF'
melt
{
    type            solidificationMeltingSource;
    active          yes;
    selectionMode   all;
    solidificationMeltingSourceCoeffs
    {
        selectionMode all;
        thermoMode  thermo;
        Tmelt       302.78;
        L           80160;
        relax       0.9;
        rhoRef      6093;
        beta        1.2e-4;
    }
}
EOF
} > $C/constant/fvOptions
# 40 cells over 20 mm, 1 mm x 1 mm cross-section, hot wall at x = 0
cat > $C/system/blockMeshDict <<EOF
FoamFile { version 2.0; format ascii; class dictionary; object blockMeshDict; }
convertToMeters 1;
vertices ( (0 0 0) (0.02 0 0) (0.02 0.001 0) (0 0.001 0) (0 0 0.001) (0.02 0 0.001) (0.02 0.001 0.001) (0 0.001 0.001) );
blocks ( hex (0 1 2 3 4 5 6 7) (40 1 1) simpleGrading (1 1 1) );
edges ();
boundary
(
    hot { type wall; faces ((0 4 7 3)); }
    far { type wall; faces ((1 2 6 5)); }
    sides { type empty; faces ((0 1 5 4) (3 7 6 2) (0 3 2 1) (4 5 6 7)); }
);
EOF
(cd $C && $WITH blockMesh > $W/blockMesh_melt.log 2>&1)
{ HDR volScalarField T
cat <<'EOF'
dimensions [0 0 0 1 0 0 0];
internalField uniform 301;
boundaryField
{
    hot { type fixedValue; value uniform 311; }
    far { type zeroGradient; }
    sides { type empty; }
}
EOF
} > $C/0/T
{ HDR volScalarField p
cat <<'EOF'
dimensions [1 -1 -2 0 0 0 0];
internalField uniform 1e5;
boundaryField
{
    "(hot|far)" { type calculated; value uniform 1e5; }
    sides { type empty; }
}
EOF
} > $C/0/p

(cd $C && $WITH $W/bin/meltDump > $W/melt.log 2>&1)
grep -E "^S |^C " $W/melt.log > $W/ref_melt.txt
[ -n "$NOREF" ] || { mkdir -p $HERE/melt/reference; gzip -9n -c $W/ref_melt.txt > $HERE/melt/reference/melt_v2506.txt.gz; }
tail -3 $W/melt.log
