#!/bin/bash
# gh:#1941 round 73: the real GeN-Foam binary on the shipped 3D_SmallESFR fluid region (cases/esfr_fluid, made by make_esfr_fluid_case.sh), fluid-only.
#   usage: run_esfr_fluid_real.sh <tight|loose> <case dir (a copy of cases/esfr_fluid with inflated polyMesh)> <steps> <out prefix>
#   tight: the shipped solvers (GAMG/DIC for p_rgh and p_rghFinal, smoothSolver/symGaussSeidel for h) with tolerance 1e-14, relTol 0 (round 67's tight-GAMG
#          protocol; GAMG sits at its round-off floor and runs maxIter = 1000 cycles per solve), writePrecision 17
#   loose: as shipped (tolerance 1e-7), writePrecision 17 (the only edit)
# Fields kept (internal field, `F <time> <name> v...`): FIELDS (default `U p_rgh T alphaPhi T.passiveStructure T.fuelAvForNeutronics`) at the times in KEEP
# (default: steps 1, 2, 10 and the last), and EVERY step's solve log lines (`PRGH`, `H`, `TSTAT`, `DTNEXT`) in <prefix>_log.ref.txt.
MODE=$1
CASE=$2
STEPS=$3
OUT=$4
HERE=$(dirname "$0")
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
sed -i "s/^endTime .*/endTime         ${STEPS};/; s/^writeInterval .*/writeInterval   1;/; s/^writeControl .*/writeControl    timeStep;/; s/^writePrecision .*/writePrecision  17;/" "$CASE/system/controlDict"
if [ "$MODE" = tight ]; then
  sed -i 's/tolerance       1e-07;/tolerance       1e-14;/' "$CASE/system/fluidRegion/fvSolution"
fi
START=$(date +%s)
(cd "$(dirname "$CASE")" && GeN-Foam -case "$(basename "$CASE")" > "$CASE/log.GeN-Foam" 2>&1; echo "exit $?")
echo "wall seconds: $(( $(date +%s) - START ))"
KEEP=${KEEP:-"1 2 10 $STEPS"}
bash "$HERE/extract_msfr_th_fields.sh" "$CASE" "$OUT.ref.txt" "$KEEP" "${FIELDS:-U p_rgh T alphaPhi T.passiveStructure T.fuelAvForNeutronics}"
ls "$CASE" | grep -E '^[0-9.]+$' | grep -v '^0$' | sort -g | awk '{n++; print "STEPTIME", n, $1}' > "$OUT.steps.txt"
bash "$HERE/extract_msfr_th_log.sh" "$CASE/log.GeN-Foam" "$OUT""_log.ref.txt"
grep "^deltaT = " "$CASE/log.GeN-Foam" | awk '{n++; print "DTNEXT", n, $3}' >> "$OUT""_log.ref.txt"
cat "$OUT.steps.txt" >> "$OUT""_log.ref.txt"; rm "$OUT.steps.txt"
