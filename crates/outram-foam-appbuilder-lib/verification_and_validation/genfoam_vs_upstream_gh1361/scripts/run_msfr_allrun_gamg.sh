#!/bin/bash
# gh:#1951 round 77: the 2D_MSFR Allrun chain with GAMG KEPT AS SHIPPED for p_rgh (the protocol of `AllrunEdits::as_shipped_gamg()`),
# real unmodified GeN-Foam binary, serial.
# usage: run_msfr_allrun_gamg.sh <th|en|transient> [RUN_ROOT]        RUN_ROOT holds `rootCase` (writePrecision 17, plain polyMesh)
#   th        : scripts/run_msfr_th_tight.sh with PSOLVER=gamg (THE ONLY DIFFERENCE from the committed PCG references: the `GAMG -> PCG/DIC`
#               perl edit is SKIPPED, so `solver GAMG; smoother DIC;` stays). env: PROTOCOL=tight|shipped (TOLERANCE), END_TIME, WRITE=step|adjustable,
#               WRITE_INTERVAL, NAME (run dir, default steadyStateTH). tight: every `tolerance` 1e-14 / `relTol` 0 (round 67's tight-GAMG protocol:
#               GAMG then runs to maxIter 1000 at its round-off floor); shipped: solver entries as shipped (1e-7, relTol 0).
#   en, transient : scripts/run_msfr_chain.sh (it never edited GAMG: with solveFluidMechanics false p_rgh is not solved) with PROTOCOL=tight;
#               env TH_END, EN_END, EN_WRITE, END_TIME as in that script.
HERE="$(cd "$(dirname "$0")" && pwd)"
STAGE=$1; R=${2:-/home/user/upstream-build/gh1951/runs}
case "$STAGE" in
  th) PSOLVER=gamg TOLERANCE=${PROTOCOL:-tight} bash "$HERE/run_msfr_th_tight.sh" "${NAME:-steadyStateTH}" "$R" ;;
  en) PROTOCOL=tight bash "$HERE/run_msfr_chain.sh" steadyStateEN "$R" ;;
  transient) PROTOCOL=tight bash "$HERE/run_msfr_chain.sh" transient "$R" ;;
  *) echo "usage: $0 <th|en|transient> [RUN_ROOT]"; exit 2 ;;
esac
