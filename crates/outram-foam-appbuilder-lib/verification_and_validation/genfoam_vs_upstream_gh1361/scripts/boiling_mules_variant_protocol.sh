#!/bin/bash
# gh:#1799 / round 34 (run_tutorial.sh PRE_RUN hook): copy the overlay files of cases/boiling_mules_r34/$VARIANT over the
# shipped 1D_boiling work dir, then apply the round-7 protocol (boiling_mules_protocol.sh: writePrecision 17, EVERY_STEP, TIGHT, END_TIME).
# usage: VARIANT=c_vap_bump_sub EVERY_STEP=1 TIGHT=1 END_TIME=3 PRE_RUN=<this script> run_tutorial.sh featureCases/1D_boiling <work>
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cp -r "$HERE/cases/boiling_mules_r34/$VARIANT/." .
bash "$HERE/scripts/boiling_mules_protocol.sh"
grep -n "solvePhase\|alphaMaxCo" system/fluidRegion/fvSolution
