#!/bin/bash
# Rebuild the five per-model reference dumps. Needs the ESI v2506 partial build in
# /home/user/upstream-build (scripts/with-of.sh); builds nothing in the repo.
set -e
HERE=$(cd "$(dirname "$0")/.." && pwd)
SOLV=$HERE/../../../outram-foam-basic-lib/verification_and_validation/upstream_solvers_gh1362/driver/dumpSolver
W=/home/user/upstream-build/turb1362
WITH=/home/user/upstream-build/scripts/with-of.sh
mkdir -p $W/bin $W/lib $W/src
rm -rf $W/lib/dumpSolver
cp -r $SOLV $W/lib/dumpSolver
(cd $W/lib/dumpSolver && WM_NCOMPPROCS=1 $WITH wmake libso >/dev/null)
rm -rf $W/src/turbDump
cp -r $HERE/driver $W/src/turbDump
(cd $W/src/turbDump && WM_NCOMPPROCS=1 $WITH wmake)
for M in ${MODELS:-kEpsilon kOmega kOmegaSST SpalartAllmaras Smagorinsky}; do
  C=$W/case_$M
  rm -rf $C
  mkdir -p $C/system $C/constant $C/0
  cp $HERE/inputs/controlDict $HERE/inputs/fvSchemes $HERE/inputs/fvSolution $HERE/inputs/blockMeshDict $C/system/
  cp $HERE/inputs/transportProperties $C/constant/
  if [ $M = Smagorinsky ]; then
    printf 'FoamFile { version 2.0; format ascii; class dictionary; object momentumTransport; }\nsimulationType LES;\nLES { model Smagorinsky; turbulence on; printCoeffs on; delta cubeRootVol; cubeRootVolCoeffs { deltaCoeff 1; } }\n' > $C/constant/momentumTransport
  else
    printf 'FoamFile { version 2.0; format ascii; class dictionary; object momentumTransport; }\nsimulationType RAS;\nRAS { model %s; turbulence on; printCoeffs on; }\n' $M > $C/constant/momentumTransport
  fi
  (cd $C && $WITH blockMesh >/dev/null && $WITH ../bin/turbDump -model $M > $C/log.out 2> $C/log.err; grep -E "^[A-Za-z_0-9]+ [0-9]+ |^#|^audit " $C/log.out | gzip -9n > $HERE/reference/ref_$M.txt.gz)
done
