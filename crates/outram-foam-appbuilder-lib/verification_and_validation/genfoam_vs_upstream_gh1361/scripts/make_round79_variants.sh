#!/bin/bash
# gh:#1943, #1942 round 79: one-edit variants of `cases/fullcoupling_fluid` (cell-centred, tight, round 55 protocol), written and committed
# BEFORE their real runs:
#   f9_cc_tort     : the ONE edit `localTortuosity (0.7 1.3 0.01389);` in structureProperties.zone0 (the cell-centred tortuous UEqn of round 64,
#                    control on this case)
#   f10_fc_tort    : f9 + `momentumMode faceCentered;` (named pair: the tortuous UEqn in the faceCentered branch, gh:#1943)
#   s1_h_reltol    : "h.*" `relTol 0` -> `relTol 0.1` (one edit; smoothSolver relTol, gh:#1942)
#   s2_h_nsweeps   : "h.*" + `nSweeps 2;` (one edit)
#   s3_h_maxiter   : "h.*" + `maxIter 2;` (one edit; the 1e-14 tolerance is then reached by no solve: every h solve stops at maxIter)
#   s4_h_all       : "h.*" `relTol 0.05`, `nSweeps 3`, `maxIter 7` (named triple; 7 is not a multiple of 3: upstream stops at 9)
#   s5_h_fixed     : "h.*" + `nSweeps -2;` (one edit: two fixed sweeps per solve, no residual)
# usage: make_round79_variants.sh <base case dir> <out dir>
BASE=$1
OUT=$2
mkdir -p "$OUT"
ALL="f9_cc_tort f10_fc_tort s1_h_reltol s2_h_nsweeps s3_h_maxiter s4_h_all s5_h_fixed"
for v in $ALL; do
  rm -rf "$OUT/$v"
  mkdir -p "$OUT/$v"
  cp -r "$BASE/0" "$BASE/constant" "$BASE/system" "$OUT/$v/"
done
fv=system/fluidRegion/fvSolution
pp=constant/fluidRegion/phaseProperties
for v in f9_cc_tort f10_fc_tort; do
  perl -0pi -e 's/(volumeFraction\s+0\.5;)/$1\n        localTortuosity     (0.7 1.3 0.01389);/' "$OUT/$v/$pp"
done
perl -0pi -e 's/(PIMPLE\s*\{)/$1\n    momentumMode        faceCentered;/' "$OUT/f10_fc_tort/$fv"
hblk() { # hblk <variant> <perl substitution body applied inside the "h.*" block>
  perl -0pi -e "s/(\"h\\.\\*\"\\s*\\{[^}]*)\\}/\$1$2}/" "$OUT/$1/$fv"; }
perl -0pi -e 's/("h\.\*"\s*\{[^}]*?relTol\s+)0;/${1}0.1;/' "$OUT/s1_h_reltol/$fv"
hblk s2_h_nsweeps '    nSweeps         2;\n    '
hblk s3_h_maxiter '    maxIter         2;\n    '
perl -0pi -e 's/("h\.\*"\s*\{[^}]*?relTol\s+)0;/${1}0.05;/' "$OUT/s4_h_all/$fv"
hblk s4_h_all '    nSweeps         3;\n        maxIter         7;\n    '
hblk s5_h_fixed '    nSweeps         -2;\n    '
