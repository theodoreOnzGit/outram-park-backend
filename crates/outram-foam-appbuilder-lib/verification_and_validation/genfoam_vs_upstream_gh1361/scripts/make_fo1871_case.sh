#!/bin/bash
# gh:#1871 round 51: stage the function-object / boundary-condition case: a 4 x 3 x 2 hex box graded in all three
# directions, patches inlet (x-), outlet (x+), walls (y-, y+; type wall), caps (z-, z+); face sets fsA (the internal
# plane between x-cells 2 and 3) and fsB (internal faces of a box, mixed orientations) with face zones zA, zB and cell zone czA.
# usage: make_fo1871_case.sh <out case dir>      (needs the OpenFOAM environment: with-of.sh)
OUT=$1
rm -rf "$OUT"; mkdir -p "$OUT/constant" "$OUT/system" "$OUT/0"
hdr() { printf 'FoamFile\n{\n    version 2.0;\n    format ascii;\n    class %s;\n    object %s;\n}\n\n' "$1" "$2"; }
{ hdr dictionary controlDict; cat <<EOT
application     fo1871Dump;
startFrom       startTime;
startTime       0;
stopAt          endTime;
endTime         100;
deltaT          1;
writeControl    timeStep;
writeInterval   1000000;
writeFormat     ascii;
writePrecision  17;
timeFormat      general;
timePrecision   8;
runTimeModifiable no;
libs            ("libFFSSrc2.so");
EOT
} > "$OUT/system/controlDict"
{ hdr dictionary fvSchemes; cat <<EOT
ddtSchemes { default Euler; }
gradSchemes { default Gauss linear; }
divSchemes { default none; }
laplacianSchemes { default Gauss linear corrected; }
interpolationSchemes { default linear; }
snGradSchemes { default corrected; }
EOT
} > "$OUT/system/fvSchemes"
{ hdr dictionary fvSolution; echo 'solvers {}'; } > "$OUT/system/fvSolution"
{ hdr dictionary blockMeshDict; cat <<EOT
convertToMeters 1;
vertices ((0 0 0) (2 0 0) (2 1.5 0) (0 1.5 0) (0 0 1) (2 0 1) (2 1.5 1) (0 1.5 1));
blocks ( hex (0 1 2 3 4 5 6 7) (4 3 2) simpleGrading (2.5 1.8 0.6) );
edges ();
boundary
(
    inlet  { type patch; faces ((0 4 7 3)); }
    outlet { type patch; faces ((1 2 6 5)); }
    walls  { type wall;  faces ((0 1 5 4) (3 7 6 2)); }
    caps   { type patch; faces ((0 3 2 1) (4 5 6 7)); }
);
mergePatchPairs ();
EOT
} > "$OUT/system/blockMeshDict"
{ hdr dictionary topoSetDict; cat <<EOT
actions
(
    { name fsA; type faceSet; action new; source boxToFace; box (0.69 -1 -1) (0.72 5 5); }
    { name fsB; type faceSet; action new; source boxToFace; box (0.3 0.2 0.1) (1.5 1.1 0.9); }
    { name czA; type cellSet; action new; source boxToCell; box (0.3 -1 -1) (1.5 5 0.6); }
);
EOT
} > "$OUT/system/topoSetDict"
{ hdr dictionary topoSetDict; cat <<EOT
actions
(
    { name zA; type faceZoneSet; action new; source setToFaceZone; faceSet fsA; }
    { name zB; type faceZoneSet; action new; source setToFaceZone; faceSet fsB; }
    { name czA; type cellZoneSet; action new; source setToCellZone; set czA; }
);
EOT
} > "$OUT/system/topoSetDict.zones"
{ hdr dictionary thermophysicalProperties; cat <<EOT
thermoType { type heRhoThermo; mixture pureMixture; transport polynomial; thermo hPolynomial; equationOfState icoPolynomial; specie specie; energy sensibleEnthalpy; }
pRef 100000;
mixture
{
    specie { molWeight 33.0; }
    equationOfState { rhoCoeffs<8> (2413.03 -0.4884 0 0 0 0 0 0); }
    thermodynamics { CpCoeffs<8> (1500 1.2 0.0005 0 0 0 0 0); Hf 0; Sf 0; }
    transport { muCoeffs<8> (0.001 0 0 0 0 0 0 0); kappaCoeffs<8> (1.1 0 0 0 0 0 0 0); }
}
EOT
} > "$OUT/constant/thermophysicalProperties"
{ hdr volScalarField T; cat <<EOT
dimensions [0 0 0 1 0 0 0]; internalField uniform 400;
boundaryField { inlet { type fixedValue; value uniform 400; } outlet { type fixedValue; value uniform 400; } walls { type fixedValue; value uniform 400; } caps { type fixedValue; value uniform 400; } }
EOT
} > "$OUT/0/T"
{ hdr volScalarField p; cat <<EOT
dimensions [1 -1 -2 0 0 0 0]; internalField uniform 100000;
boundaryField { inlet { type fixedValue; value uniform 100000; } outlet { type fixedValue; value uniform 100000; } walls { type fixedValue; value uniform 100000; } caps { type fixedValue; value uniform 100000; } }
EOT
} > "$OUT/0/p"
# round 51, boundary conditions (bc1871Dump): blackBodyRadiation on outlet, timeFieldTable on inlet, velocityRundown on inlet
bcs() { # name class dims patchname patchdict fixedvalue
  { hdr "$2" "$1"; cat <<EOT
dimensions $3; internalField uniform $6;
boundaryField
{
    inlet  { type fixedValue; value uniform $6; }
    outlet { type fixedValue; value uniform $6; }
    walls  { type fixedValue; value uniform $6; }
    caps   { type fixedValue; value uniform $6; }
    $4 { $5 }
}
EOT
  } > "$OUT/0/$1"
}
bcs Tbb volScalarField "[0 0 0 1 0 0 0]" outlet "type blackBodyRadiation; emissivity 0.78; kappa 10; Ta 300; value uniform 500;" 450
bcs Ttab volScalarField "[0 0 0 1 0 0 0]" inlet "type timeFieldTable; table ( (2 (10 11 12 13 14 15)) (5 (20 18.5 17 15.5 14 12.5)) (9 (7.25 8.75 10.5 3.125 9 1.5)) (12 (30 29 28 27 26 25)) );" 0
bcs Urun volVectorField "[0 1 -1 0 0 0 0]" inlet "type velocityRundown; const 1; coeff -0.04; exp 1.5; startTime 3; value uniform (1 0.5 0.25);" "(0 0 0)"
bcs Urun2 volVectorField "[0 1 -1 0 0 0 0]" inlet "type velocityRundown; const 1; coeff -0.04; exp 1.5; startTime 3; value uniform (1 0.5 0.25);" "(0 0 0)"
cd "$OUT" && blockMesh > log.blockMesh 2>&1 && topoSet > log.topoSet 2>&1 && topoSet -dict system/topoSetDict.zones > log.topoSetZones 2>&1
echo "case staged in $OUT"
