#!/bin/bash
# gh:#1944 round 74: stage the SHIPPED Tutorials/reactorCases/3D_SmallESFR_NewSolverVerification/<legacySolver|newSolver> steady state
# (fluid onePhase faceCentered + neutro diffusionNeutronics eigenvalue + thermoMechanicalRegion legacyThermoMechanics), serial.
# usage: make_esfr_coupled_case.sh <out dir> [legacySolver]
# env: NEUTRO_GAMG=1 (round 82: the neutronics flux entry -> the shipped commented-out GAMG block; with NEUTRO_NCOARSEST=<n>, round 86, its
# nCellsInCoarsestLevel set to <n>) TIGHT=1 (default) END=<endTime> (default 200) WRITE=<writeInterval> (default 1) INFLATE=1 (gunzip the polyMesh files for the port)
# Edits to the shipped files (nothing else): controlDict `libs ("libThermoMechanicsSolvers.so")` (the built binary lists no legacyThermoMechanics,
# README round 54), endTime, writePrecision 17, writeControl timeStep / writeInterval; with TIGHT=1 every `tolerance` 1e-14 and `relTol` 0 in the
# three fvSolution (GAMG kept: the round 67 protocol).
# The shipped Allrun_parallel runs `decomposePar` + `mpirun -np 4` (parallel); this is the SERIAL run of the same case. No -initializeMappedFields
# (the shipped script has none).
SRC=${GENFOAM_UPSTREAM:-/home/user/upstream-build/GeN-Foam-src}/Tutorials/reactorCases/3D_SmallESFR_NewSolverVerification/${2:-legacySolver}
C=$1
rm -rf "$C"
mkdir -p "$C"
cp -r "$SRC/0" "$SRC/constant" "$SRC/system" "$C/"
printf 'libs ("libThermoMechanicsSolvers.so");\n' >> "$C/system/controlDict"
if [ "${NEUTRO_GAMG:-0}" = 1 ]; then
  # gh:#1956 round 82: the one-edit neutronics GAMG variant (before the TIGHT sed, so the tight protocol tightens the GAMG lines)
  python3 -I "$(dirname "$0")/make_esfr_neutro_gamg_variant_r82.py" "$C/system/neutroRegion/fvSolution"
  if [ -n "$NEUTRO_NCOARSEST" ]; then
    # gh:#1966 round 86: the ONE further edit of variant g<n>: `nCellsInCoarsestLevel <n>` in the neutronics flux GAMG block (the only
    # nCellsInCoarsestLevel line of that file; shipped 20)
    [ "$(grep -c 'nCellsInCoarsestLevel' "$C/system/neutroRegion/fvSolution")" = 1 ] || { echo "expected one nCellsInCoarsestLevel line"; exit 1; }
    sed -i -E "s/^( *)nCellsInCoarsestLevel +[0-9]+;/\1nCellsInCoarsestLevel ${NEUTRO_NCOARSEST};/" "$C/system/neutroRegion/fvSolution"
  fi
fi
sed -i "s/^endTime .*/endTime ${END:-200};/; s/^writePrecision .*/writePrecision  17;/; s/^writeControl .*/writeControl timeStep;/; s/^writeInterval .*/writeInterval ${WRITE:-1};/" "$C/system/controlDict"
if [ "${TIGHT:-1}" = 1 ]; then
  for r in fluidRegion neutroRegion thermoMechanicalRegion; do
    sed -i -E 's/^( *)tolerance +[0-9.e+-]+;/\1tolerance        1e-14;/; s/^( *)relTol +[0-9.e+-]+;/\1relTol           0;/' "$C/system/$r/fvSolution"
  done
fi
if [ -n "$INFLATE" ]; then
  for r in fluidRegion neutroRegion thermoMechanicalRegion; do
    (cd "$C/constant/$r/polyMesh" && gunzip -f ./*.gz)
  done
fi
