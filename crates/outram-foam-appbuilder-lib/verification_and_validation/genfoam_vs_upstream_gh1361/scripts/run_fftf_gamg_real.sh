#!/bin/bash
# gh:#1934 round 67: the REAL GeN-Foam binary on the shipped 2D_FFTF steady state with its OWN GAMG solver for p_rgh, either
# through the instrumented COPY of GAMGSolver (`upstream_driver/gamg_dump`, solver name `GAMGdump`, same code plus dumps) or
# through the shipped `GAMG` itself.
#
# usage: run_fftf_gamg_real.sh <work dir> <mode> <n steps> [dump]
#   mode  tight   : every `tolerance` 1e-14 and `relTol` 0 in `solvers`, writePrecision 17, every step written (the round 62 tight
#                   protocol EXCEPT that p_rgh keeps `solver GAMG; smoother DIC;`, as shipped)
#         shipped : fvSolution exactly as shipped (loose: p_rgh 1e-7 / relTol 0.001), writePrecision 17, every step written
#   dump  (optional, any value) `solver GAMG` -> `solver GAMGdump` for p_rgh/p_rghFinal and `libs (".../libGAMGdump.so")`;
#         GAMGDUMP_DIR=<work dir>/gamgdump receives matrix.txt, agglom.txt, solution.txt (solve $GAMGDUMP_TARGET, default 0) and
#         cycles.txt (every solve up to $GAMGDUMP_MAXSOLVES, default 400: `SOLVE`, `VC <solve> <cycle> <residual>`, `END`).
# The Allrun `setSteadyState` edits are applied with the real `foamDictionary`, then `GeN-Foam -initializeMappedFields`, then the
# run is stopped (kill) once <n steps> time directories exist. Wall time: ~1.2-3 s per step on this machine.
W=$1; MODE=$2; NSTEP=$3; DUMP=$4
SRC=${GENFOAM_UPSTREAM:-/home/user/upstream-build/GeN-Foam-src}/Tutorials/reactorCases/2D_FFTF/rootCase
. /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
rm -rf "$W"; mkdir -p "$W"; cp -r "$SRC/." "$W/"
cd "$W" || exit 1
foamDictionary system/controlDict -entry endTime -set 900
foamDictionary system/controlDict -entry writeInterval -set 1
foamDictionary system/controlDict -entry writeControl -set timeStep
foamDictionary system/controlDict -entry writePrecision -set 17
foamDictionary system/controlDict -entry maxCo -set 50.0
foamDictionary system/controlDict -entry maxDeltaT -set 2.0
foamDictionary system/controlDict -entry regionSolvers.fluidRegion -set onePhase
foamDictionary system/controlDict -entry removeBaffles.fluidRegion -set true
cp constant/multiRegionCouplingDict_steadyState constant/multiRegionCouplingDict
GeN-Foam -initializeMappedFields -case . > log.initializeMappedFields 2>&1
f=system/fluidRegion/fvSolution
if [ "$MODE" = tight ]; then
  awk 'BEGIN{ins=0} /^solvers/{ins=1} /^PIMPLE/{ins=0}
       { if(ins && $1=="tolerance"){sub(/tolerance .*/,"tolerance       1e-14;")}
         if(ins && $1=="relTol"){sub(/relTol .*/,"relTol          0;")}
         print }' "$f" > "$f.new" && mv "$f.new" "$f"
fi
if [ -n "$DUMP" ]; then
  sed -i 's/solver *GAMG;/solver          GAMGdump;/' "$f"
  sed -i '/^application/i libs ("/home/user/upstream-build/gh1934/lib/libGAMGdump.so");' system/controlDict
  mkdir -p gamgdump; export GAMGDUMP_DIR=$PWD/gamgdump
fi
GeN-Foam -case . > log.GeN-Foam 2>&1 &
PID=$!
while kill -0 $PID 2>/dev/null; do
  n=$(ls | grep -E '^[0-9.]+$' | grep -vx 0 | wc -l)
  if [ "$n" -ge "$NSTEP" ]; then sleep 2; kill $PID; break; fi
  sleep 2
done
wait $PID 2>/dev/null
echo "done: $(ls | grep -E '^[0-9.]+$' | grep -vx 0 | wc -l) time directories"
