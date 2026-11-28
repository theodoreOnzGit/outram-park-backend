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
# ADDENDUM A (round 79, written after the first real run of s1-s5 showed every `h` solve as `diagonalPBiCGStab`, and BEFORE any port run of
# them): the case's `"h.*"` entry is shadowed by the later catch-all `".*"` (the last matching pattern wins), so the edits above changed
# nothing. The s-variants are now round 55's `v3_smooth` (explicit `h` and `hFinal` entries after the catch-all: exact keywords win) with
# the same edit in both entries: s0_h_smooth is v3_smooth itself (the base of the s-variants).
sm() { # sm <variant> <extra keys>: append explicit h and hFinal smoothSolver entries after the catch-all
  local e="        solver          smoothSolver;\n        smoother        symGaussSeidel;\n        tolerance       1e-14;\n$2"
  perl -0pi -e "s/(    \"\\.\\*\"\\s*\\{[^}]*\\}\\n)/\$1    h\\n    {\\n$e    }\\n    hFinal\\n    {\\n$e    }\\n/" "$OUT/$1/$fv"; }
rm -rf "$OUT/s0_h_smooth"; mkdir -p "$OUT/s0_h_smooth"; cp -r "$BASE/0" "$BASE/constant" "$BASE/system" "$OUT/s0_h_smooth/"
for v in s1_h_reltol s2_h_nsweeps s3_h_maxiter s4_h_all s5_h_fixed; do cp "$BASE/$fv" "$OUT/$v/$fv"; done
sm s0_h_smooth '        relTol          0;\n'
sm s1_h_reltol '        relTol          0.1;\n'
sm s2_h_nsweeps '        relTol          0;\n        nSweeps         2;\n'
sm s3_h_maxiter '        relTol          0;\n        maxIter         2;\n'
sm s4_h_all '        relTol          0.05;\n        nSweeps         3;\n        maxIter         7;\n'
sm s5_h_fixed '        relTol          0;\n        nSweeps         -2;\n'
# ADDENDUM B (round 79, written BEFORE its real run): f11_fc_tort_half = round 64's w7_tort (zone0 shrunk to its first 72 of 144 cells:
# an alpha interface, + localTortuosity) + momentumMode faceCentered. Built from make_fftf_piece_variants.py's own w7_tort.
TMPW=$(mktemp -d)
python3 -I "$(dirname "$0")/make_fftf_piece_variants.py" "$BASE" "$TMPW" >/dev/null
rm -rf "$OUT/f11_fc_tort_half"; cp -r "$TMPW/w7_tort" "$OUT/f11_fc_tort_half"; rm -rf "$TMPW"
perl -0pi -e 's/(PIMPLE\s*\{)/$1\n    momentumMode        faceCentered;/' "$OUT/f11_fc_tort_half/$fv"
