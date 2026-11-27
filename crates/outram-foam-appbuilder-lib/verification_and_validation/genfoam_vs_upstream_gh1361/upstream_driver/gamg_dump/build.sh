#!/bin/bash
# gh:#1934 round 67: build libGAMGdump.so = a renamed COPY of ESI v2506 GAMGSolver (typeName GAMGdump) plus dump calls.
# The shipped OpenFOAM tree is only read. Output: /home/user/upstream-build/gh1934/lib/libGAMGdump.so
HERE="$(cd "$(dirname "$0")" && pwd)"
G=/home/user/upstream-build/OpenFOAM-v2506/src/OpenFOAM/matrices/lduMatrix/solvers/GAMG
W=/home/user/upstream-build/gh1934/lib
mkdir -p "$W/Make"; cd "$W" || exit 1
for f in GAMGSolver.H GAMGSolver.C GAMGSolverAgglomerateMatrix.C GAMGSolverInterpolate.C GAMGSolverScale.C GAMGSolverSolve.C; do
  n=$(echo $f | sed 's/GAMGSolver/GAMGdumpSolver/')
  sed 's/GAMGSolver/GAMGdumpSolver/g; s/GAMGPreconditioner/GAMGPreconditionerDumpNone/g; s/TypeName("GAMG")/TypeName("GAMGdump")/' $G/$f > $n
done
cp "$HERE/gamgDump.H" .; cp "$HERE/Make_files" Make/files; cp "$HERE/Make_options" Make/options
python3 "$HERE/instrument.py" GAMGdumpSolverSolve.C
. /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
WM_NCOMPPROCS=2 wmake libso .
