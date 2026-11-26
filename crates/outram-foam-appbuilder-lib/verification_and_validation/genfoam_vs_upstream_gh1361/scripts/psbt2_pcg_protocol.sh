#!/bin/bash
# Round 16 (gh:#1802): psbt_shipped_protocol.sh PLUS the p_rgh linear solver `GAMG`/`DIC` replaced by `PCG`/`DIC`
# (a solver setting, like the tolerances: same matrix, same answer, but PCG reaches 1e-14).
# Why: on the 1170-cell PhaseII mesh GAMG at tolerance 1e-14 stops at its 1000-iteration cap with a final residual of
# 1e-10 (relative to an initial 4e-4: p known to ~1e-7), so the reference itself is unconverged and no port can match it
# closer than that (measured: the turbulence-off control differs from the port by p 7e-7 at step 1 with GAMG).
# Set TURB_OFF=1 to also switch `turbulence off` in both turbulenceProperties files (control run).
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bash "$HERE/psbt_shipped_protocol.sh"
f=system/fluidRegion/fvSolution
sed -i 's/^\( *\)solver  *GAMG;/\1solver          PCG;/; s/^\( *\)smoother  *DIC;/\1preconditioner  DIC;/' "$f"
if [ -n "$TURB_OFF" ]; then
  sed -i 's/^\( *\)turbulence  *on;/\1turbulence      off;/' constant/fluidRegion/turbulenceProperties.liquid constant/fluidRegion/turbulenceProperties.vapour
fi
grep -n "PCG\|GAMG" "$f"
