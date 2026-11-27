#!/bin/bash
# gh:#1230 round 35: stage the `powerModelDump` case: an 8-cell 1-D mesh (x = 0..8, unit cross-section, `empty` sides) with
# cell zones zoneA (3 cells), zoneB (2), zoneC (2) and one cell (7) in no zone, plus one structureProperties file per power model.
# usage: make_power_model_case.sh <out case dir> [nSteps] [nOuter] [deltaT]
# Written by hand for the differential test; the material and geometry numbers are those of the shipped tutorials' powerModel
# dictionaries (FFTF / one-phase-and-PK for nuclearFuelPin, the lumpedNuclearStructure usage example), not tuned to anything.
OUT=$1
NSTEPS=${2:-12}
NOUTER=${3:-1}
DT=${4:-0.5}
rm -rf "$OUT"
mkdir -p "$OUT/constant/polyMesh" "$OUT/system" "$OUT/0"
hdr() { printf 'FoamFile\n{\n    version 2.0;\n    format ascii;\n    class %s;\n    object %s;\n}\n\n' "$1" "$2"; }
{ hdr dictionary controlDict; cat <<EOF
application     powerModelDump;
startFrom       startTime;
startTime       0;
stopAt          endTime;
endTime         100;
deltaT          $DT;
writeControl    timeStep;
writeInterval   1000000;
writeFormat     ascii;
writePrecision  17;
timeFormat      general;
timePrecision   8;
runTimeModifiable no;
EOF
} > "$OUT/system/controlDict"
{ hdr dictionary fvSchemes; cat <<EOF
ddtSchemes { default Euler; }
gradSchemes { default Gauss linear; }
divSchemes { default none; }
laplacianSchemes { default Gauss linear corrected; }
interpolationSchemes { default linear; }
snGradSchemes { default corrected; }
EOF
} > "$OUT/system/fvSchemes"
{ hdr dictionary fvSolution; echo 'solvers {}'; } > "$OUT/system/fvSolution"
{ hdr dictionary blockMeshDict; cat <<EOF
convertToMeters 1;
vertices
(
    (0 0 0) (0 1 0) (0 1 1) (0 0 1)
    (3 0 0) (3 1 0) (3 1 1) (3 0 1)
    (5 0 0) (5 1 0) (5 1 1) (5 0 1)
    (7 0 0) (7 1 0) (7 1 1) (7 0 1)
    (8 0 0) (8 1 0) (8 1 1) (8 0 1)
);
blocks
(
    hex (0 4 5 1 3 7 6 2) zoneA (3 1 1) simpleGrading (1 1 1)
    hex (4 8 9 5 7 11 10 6) zoneB (2 1 1) simpleGrading (1 1 1)
    hex (8 12 13 9 11 15 14 10) zoneC (2 1 1) simpleGrading (1 1 1)
    hex (12 16 17 13 15 19 18 14) (1 1 1) simpleGrading (1 1 1)
);
edges ();
boundary
(
    ends { type patch; faces ((0 3 2 1) (16 17 18 19)); }
);
defaultPatch { name sides; type empty; }
mergePatchPairs ();
EOF
} > "$OUT/system/blockMeshDict"
{ hdr dictionary couplingProperties; cat <<EOF
q0 ${Q0:-2.0e8};
H0 ${H0:-1.0e4};
Tf0 ${TF0:-600};
nSteps $NSTEPS;
nOuter $NOUTER;
EOF
} > "$OUT/constant/couplingProperties"
SP="$OUT/constant"
{ hdr dictionary sp; cat <<'EOF'
type byZone;
zoneA
{
    Dh 0.01; volumeFraction 0.4;
    powerModel
    {
        type lumpedNuclearStructure;
        volumetricArea 40;
        powerDensity 5e6;
        nodesNumber 3;
        nodeFuel 0; nodeClad 1; nodeMatrix 2;
        kappaMatrix 12;
        heatConductances (600000 240000 600000 1200000);
        rhoCp (53576.92 53576.92 53576.92);
        volumeFractions (0.75 0.125 0.125);
        powerFractions (1 0 0);
        T0 900;
    }
}
zoneB
{
    Dh 0.01; volumeFraction 0.3;
    powerModel
    {
        type lumpedNuclearStructure;
        volumetricArea 55;
        powerDensity 4e6;
        nodesNumber 1;
        nodeFuel 0; nodeClad 0; nodeMatrix 0;
        kappaMatrix 10;
        heatConductances (450000 225000);
        rhoCp (60000);
        volumeFractions (1);
        powerFractions (1);
        fractionOfPowerFromNeutronics 0.9;
        T0 850;
    }
}
zoneC
{
    Dh 0.01; volumeFraction 0.5;
    powerModel
    {
        type lumpedNuclearStructure;
        volumetricArea 30;
        powerDensity 3e6;
        nodesNumber 2;
        nodeFuel 0; nodeClad 1;
        heatConductances (500000 300000 700000);
        rhoCp (50000 70000);
        volumeFractions (0.6 0.4);
        powerFractions (0.8 0.2);
        T0 700;
    }
}
EOF
} > "$SP/structureProperties.lumpedNuclearStructure"
{ hdr dictionary sp; cat <<'EOF'
type byZone;
zoneA
{
    Dh 0.01; volumeFraction 0.4;
    powerModel
    {
        type heatedPin;
        powerDensity 3e8;
        innerRadius 0;
        outerRadius 0.0045;
        meshSize 12;
        rho 7800; Cp 500;
        k 20;
        T 600;
    }
}
zoneB
{
    Dh 0.01; volumeFraction 0.3;
    powerModel
    {
        type heatedPin;
        powerDensity 1e8;
        innerRadius 0.001;
        outerRadius 0.005;
        meshSize 9;
        rhoCp 3.9e6;
        k 15;
        T 650;
        volumeFraction 0.25;
    }
}
EOF
} > "$SP/structureProperties.heatedPin"
{ hdr dictionary sp; cat <<'EOF'
type byZone;
zoneA
{
    Dh 0.01; volumeFraction 0.5539729557;
    powerModel
    {
        type nuclearFuelPin;
        powerDensity 0;
        volumeFraction 0.5539729557;
        fuelMeshSize 10;
        cladMeshSize 5;
        fuelT 574.56; cladT 574.56;
        cladK 20; fuelK 3;
        cladRho 7870; fuelRho 10480;
        cladCp 500; fuelCp 250;
        cladOuterRadius 0.002921;
        cladInnerRadius 0.002540;
        fuelOuterRadius 0.002470;
        fuelInnerRadius 0.0;
        gapH 3000;
    }
}
zoneB
{
    Dh 0.01; volumeFraction 0.45;
    powerModel
    {
        type nuclearFuelPin;
        powerDensity 0;
        fuelInnerRadius 0.0012;
        fuelOuterRadius 0.004715;
        cladInnerRadius 0.004865;
        cladOuterRadius 0.005365;
        fuelMeshSize 16;
        cladMeshSize 4;
        fuelRhoCp 2.62e6; fuelK 3;
        cladRho 7500; cladCp 500; cladK 20;
        gapH 3000;
        fuelT 600; cladT 590;
        fractionOfPowerFromNeutronics 0.95;
    }
}
zoneC
{
    Dh 0.01; volumeFraction 0.5;
    powerModel
    {
        type nuclearFuelPin;
        powerDensity 0;
        volumeFraction 0.5539729557;
        fuelMeshSize 8;
        cladMeshSize 3;
        fuelT 574.56; cladT 574.56;
        cladK 20; fuelK 3;
        cladRho 7870; fuelRho 10480;
        cladCp 500; fuelCp 250;
        cladOuterRadius 0.002921;
        cladInnerRadius 0.002540;
        fuelOuterRadius 0.002470;
        fuelInnerRadius 0.0;
        gapHPowerDensity table
        (
            (   0               528.4782467867  )
            (   795443280.75916 3170.8694807204 )
        );
    }
}
EOF
} > "$SP/structureProperties.nuclearFuelPin"
{ hdr dictionary sp; cat <<'EOF'
type byZone;
zoneA
{
    Dh 0.01; volumeFraction 0.4;
    powerModel
    {
        type interpolatedNuclearFuelPin;
        fuelOuterRadius 0.0047;
        powerDensity 0;
        T 700;
    }
}
zoneB
{
    Dh 0.01; volumeFraction 0.3;
    powerModel
    {
        type interpolatedNuclearFuelPin;
        fuelOuterRadius 0.0052;
        powerDensity 0;
        T 650;
    }
}
EOF
} > "$SP/structureProperties.interpolatedNuclearFuelPin"
