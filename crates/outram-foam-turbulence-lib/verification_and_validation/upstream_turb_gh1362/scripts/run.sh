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
for M in ${MODELS:-kEpsilon kOmega kOmegaSST SpalartAllmaras Smagorinsky RNGkEpsilon realizableKE WALE SpalartAllmarasDES SpalartAllmarasDDES SpalartAllmarasIDDES kOmegaSSTDES kEqn LaunderSharmaKE LaunderSharmaKE_lowRe LESfilters}; do
  C=$W/case_$M
  MODEL=${M%_lowRe}
  rm -rf $C
  mkdir -p $C/system $C/constant $C/0
  cp $HERE/inputs/controlDict $HERE/inputs/fvSchemes $HERE/inputs/fvSolution $HERE/inputs/blockMeshDict $C/system/
  cp $HERE/inputs/transportProperties $C/constant/
  # *_lowRe: nu = 1e-2 (1000x) so the low-Reynolds damping functions leave saturation (README slice 6)
  if [ "$M" != "$MODEL" ]; then sed -i "s/^nu .*/nu 1e-02;/" $C/constant/transportProperties; fi
  if [ $MODEL = SpalartAllmarasIDDES ]; then
    printf 'FoamFile { version 2.0; format ascii; class dictionary; object turbulenceProperties; }\nsimulationType LES;\nLES { model %s; turbulence on; printCoeffs on; delta IDDESDelta; }\n' $MODEL > $C/constant/turbulenceProperties
  elif [ $MODEL = Smagorinsky ] || [ $MODEL = WALE ] || [ $MODEL = SpalartAllmarasDES ] || [ $MODEL = SpalartAllmarasDDES ] || [ $MODEL = kOmegaSSTDES ] || [ $MODEL = kEqn ] || [ $MODEL = LESfilters ]; then
    printf 'FoamFile { version 2.0; format ascii; class dictionary; object turbulenceProperties; }\nsimulationType LES;\nLES { model %s; turbulence on; printCoeffs on; delta cubeRootVol; cubeRootVolCoeffs { deltaCoeff 1; } }\n' ${MODEL/LESfilters/kEqn} > $C/constant/turbulenceProperties
  else
    printf 'FoamFile { version 2.0; format ascii; class dictionary; object turbulenceProperties; }\nsimulationType RAS;\nRAS { model %s; turbulence on; printCoeffs on; }\n' $MODEL > $C/constant/turbulenceProperties
  fi
  (cd $C && $WITH blockMesh >/dev/null && $WITH ../bin/turbDump -model $MODEL > $C/log.out 2> $C/log.err; grep -E "^[A-Za-z_0-9]+ [0-9]+ |^#|^audit " $C/log.out | gzip -9n > $HERE/reference/ref_$M.txt.gz)
done
