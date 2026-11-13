#!/bin/bash
# gh:#1363 row 5. Write the REDUCED bubbleColumn case (committed under
# bubble/) from the ESI v2506 tutorial multiphase/twoPhaseEulerFoam/laminar/
# bubbleColumn. Every simplification is listed in
# crates/outram-foam-appbuilder-lib/verification_and_validation/
# reacting_two_phase_euler_foam_vs_openfoam.md, "Reduced case".
#   usage: make_bubble_case.sh [deltaT] [nSteps]
set -e
DT=${1:-0.001}
NSTEPS=${2:-10}
HERE=$(cd "$(dirname "$0")/.." && pwd)
C=$HERE/bubble
rm -rf $C/system $C/constant $C/0; mkdir -p $C/system $C/constant $C/0   # reference/ is kept
HDR() { printf 'FoamFile { version 2.0; format ascii; class %s; object %s; }\n' "$1" "$2"; }

{ HDR dictionary controlDict
cat <<EOF
application twoPhaseEulerFoam;
startFrom startTime;
startTime 0;
stopAt endTime;
endTime $(awk -v d=$DT -v n=$NSTEPS 'BEGIN{printf "%.17g", d*(n-0.25)}');
deltaT $DT;
writeControl timeStep;
writeInterval 1;
writeFormat ascii;
writePrecision 17;
timeFormat general;
timePrecision 8;
runTimeModifiable false;
adjustTimeStep no;
EOF
} > $C/system/controlDict

# 6 x 12 x 1 hexes, 0.15 x 0.30 x 0.10 m box, all four sides walls, z empty.
{ HDR dictionary blockMeshDict
cat <<'EOF'
convertToMeters 1;
vertices ( (0 0 0) (0.15 0 0) (0.15 0.3 0) (0 0.3 0) (0 0 0.1) (0.15 0 0.1) (0.15 0.3 0.1) (0 0.3 0.1) );
blocks ( hex (0 1 2 3 4 5 6 7) (6 12 1) simpleGrading (1 1 1) );
edges ();
boundary
(
    walls { type wall; faces ((0 4 7 3) (1 2 6 5) (0 1 5 4) (3 7 6 2)); }
    frontAndBack { type empty; faces ((0 3 2 1) (4 5 6 7)); }
);
EOF
} > $C/system/blockMeshDict

# All-upwind, so the ESI scheme choice is the port's first-order one.
{ HDR dictionary fvSchemes
cat <<'EOF'
ddtSchemes { default Euler; }
gradSchemes { default Gauss linear; }
divSchemes
{
    default                         none;
    div(phi,alpha.air)              Gauss upwind;
    div(phir,alpha.air)             Gauss upwind;
    "div\(alphaRhoPhi.*,U.*\)"      Gauss upwind;
    "div\(phi.*,U.*\)"              Gauss upwind;
    "div\(alphaRhoPhi.*,(h|e).*\)"  Gauss upwind;
    "div\(alphaRhoPhi.*,K.*\)"      Gauss upwind;
    "div\(alphaPhi.*,p\)"           Gauss upwind;
    "div\(\(\(\(alpha.*\*thermo:rho.*\)*nuEff.*\)\*dev2\(T\(grad\(U.*\)\)\)\)\)" Gauss linear;
}
laplacianSchemes { default Gauss linear uncorrected; }
interpolationSchemes { default linear; }
snGradSchemes { default uncorrected; }
EOF
} > $C/system/fvSchemes

{ HDR dictionary fvSolution
cat <<'EOF'
solvers
{
    alpha.air { nAlphaCorr 1; nAlphaSubCycles 1; }
    p_rgh { solver PCG; preconditioner DIC; tolerance 1e-15; relTol 0; maxIter 10000; }
    p_rghFinal { $p_rgh; }
    "U.*" { solver smoothSolver; smoother symGaussSeidel; tolerance 1e-15; relTol 0; minIter 1; }
    "e.*" { solver smoothSolver; smoother symGaussSeidel; tolerance 1e-15; relTol 0; minIter 1; }
}
PIMPLE { nOuterCorrectors 1; nCorrectors 1; nNonOrthogonalCorrectors 0; pRefCell 0; pRefValue 1e5; }
relaxationFactors { equations { ".*" 1; } }
EOF
} > $C/system/fvSolution

{ HDR uniformDimensionedVectorField g
cat <<'EOF'
dimensions [0 1 -2 0 0 0 0];
value (0 -9.81 0);
EOF
} > $C/constant/g

# Constant-density phases (rhoConst), laminar, tutorial viscosities.
for ph in air water; do
  if [ $ph = air ]; then RHO=1.2; CV=1e7; MU=1.84e-05; PR=0.7; MW=28.9; else RHO=1000; CV=4195; MU=1e-3; PR=7; MW=18; fi
  { HDR dictionary thermophysicalProperties.$ph
cat <<EOF
thermoType { type heRhoThermo; mixture pureMixture; transport const; thermo eConst; equationOfState rhoConst; specie specie; energy sensibleInternalEnergy; }
mixture
{
    specie { molWeight $MW; }
    equationOfState { rho $RHO; }
    thermodynamics { Cv $CV; Hf 0; }
    transport { mu $MU; Pr $PR; }
}
EOF
  } > $C/constant/thermophysicalProperties.$ph
  { HDR dictionary turbulenceProperties.$ph; echo "simulationType laminar;"; } > $C/constant/turbulenceProperties.$ph
done

# Drag only (air in water, Schiller-Naumann, no swarm), no blending, constant diameter.
{ HDR dictionary phaseProperties
cat <<'EOF'
phases (air water);
air   { diameterModel constant; constantCoeffs { d 3e-3; } residualAlpha 1e-6; }
water { diameterModel constant; constantCoeffs { d 1e-4; } residualAlpha 1e-6; }
blending { default { type none; continuousPhase water; } }
sigma ( (air and water) 0.07 );
aspectRatio ( (air in water) { type constant; E0 1.0; } (water in air) { type constant; E0 1.0; } );
drag
(
    (air in water) { type SchillerNaumann; residualRe 1e-3; swarmCorrection { type none; } }
    (water in air) { type SchillerNaumann; residualRe 1e-3; swarmCorrection { type none; } }
);
virtualMass ( );
heatTransfer
(
    (air in water) { type RanzMarshall; residualAlpha 1e-4; }
    (water in air) { type RanzMarshall; residualAlpha 1e-4; }
);
lift ( );
wallLubrication ( );
turbulentDispersion ( );
pMin 10000;
EOF
} > $C/constant/phaseProperties

# alpha.air: a uniform 0.10 background plus a Gaussian blob of +0.15 at (0.075, 0.075),
# sigma 0.03 m. Everywhere 0.10 <= alpha.air <= 0.25 < maxFullyDispersedAlpha 0.3, so
# the blended tutorial drag would equal the dispersed model anyway.
NX=6; NY=12; DX=0.025; DY=0.025
CELLS=$(awk -v nx=$NX -v ny=$NY -v dx=$DX -v dy=$DY 'BEGIN{
  for(j=0;j<ny;j++) for(i=0;i<nx;i++){
    x=(i+0.5)*dx; y=(j+0.5)*dy;
    printf "%.17g\n", 0.10+0.15*exp(-((x-0.075)^2+(y-0.075)^2)/(2*0.03^2)) }}')
{ HDR volScalarField alpha.air
echo "dimensions [0 0 0 0 0 0 0];"
echo "internalField nonuniform List<scalar> $((NX*NY))"; echo "("; echo "$CELLS"; echo ")"; echo ";"
echo "boundaryField { walls { type zeroGradient; } frontAndBack { type empty; } }"
} > $C/0/alpha.air
for ph in air water; do
  { HDR volVectorField U.$ph
    echo "dimensions [0 1 -1 0 0 0 0]; internalField uniform (0 0 0);"
    echo "boundaryField { walls { type noSlip; } frontAndBack { type empty; } }"
  } > $C/0/U.$ph
  { HDR volScalarField T.$ph
    echo "dimensions [0 0 0 1 0 0 0]; internalField uniform 300;"
    echo "boundaryField { walls { type zeroGradient; } frontAndBack { type empty; } }"
  } > $C/0/T.$ph
done
{ HDR volScalarField p_rgh
  echo "dimensions [1 -1 -2 0 0 0 0]; internalField uniform 1e5;"
  echo "boundaryField { walls { type fixedFluxPressure; value uniform 1e5; } frontAndBack { type empty; } }"
} > $C/0/p_rgh
{ HDR volScalarField p
  echo "dimensions [1 -1 -2 0 0 0 0]; internalField uniform 1e5;"
  echo "boundaryField { walls { type calculated; value uniform 1e5; } frontAndBack { type empty; } }"
} > $C/0/p
echo "wrote $C (deltaT $DT, $NSTEPS steps)"
